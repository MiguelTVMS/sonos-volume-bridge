"""Release dependency graph checks (no GitHub credentials or network required)."""
import re
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[2] / '.github/workflows/release-candidate.yml'


def jobs(text):
    return dict(re.findall(r'^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)', text, re.M | re.S))


def dependencies(body):
    match = re.search(r'^    needs:\s*(\[[^\]]*\]|[\w-]+)', body, re.M)
    return re.findall(r'[\w-]+', match[1]) if match else []


class ReleaseWorkflowTests(unittest.TestCase):
    def test_full_uninstall_cleans_native_toast_registration_but_updates_preserve_it(self):
        root = WORKFLOW.parents[2]
        template = (root / 'src-tauri/windows/installer.nsi').read_text(encoding='utf-8')
        uninstall = template.split('Section Uninstall', 1)[1]
        cleanup = uninstall.split('${If} $UpdateMode <> 1', 1)[1].split('${EndIf}', 1)[0]
        for key in ('AppUserModelId\\${BUNDLEID}', 'CLSID\\{a607018c-48b4-45c8-b0b2-46c243fde206}'):
            command = 'DeleteRegKey HKCU "Software\\Classes\\' + key + '"'
            self.assertIn(command, cleanup)
            self.assertEqual(template.count(command), 1)

    def test_all_platform_builds_gate_signing_and_store_submission(self):
        graph = jobs(WORKFLOW.read_text(encoding='utf-8'))
        builds = {'macos-app', 'linux-deb', 'windows-app', 'windows-nsis', 'windows-msix', 'windows-store-upload'}
        self.assertEqual(set(dependencies(graph['all-platform-builds'])), builds)
        self.assertIsNone(re.search(r'^    if:', graph['all-platform-builds'], re.M))
        for job in ('macos-direct', 'macos-app-store', 'publish-microsoft-store'):
            self.assertIn('all-platform-builds', dependencies(graph[job]), job)
            self.assertNotIn('always()', graph[job].split('    steps:')[0])

        def visit(job, pending):
            self.assertNotIn(job, pending, 'Release workflow has a dependency cycle')
            for dependency in dependencies(graph[job]):
                self.assertIn(dependency, graph)
                visit(dependency, pending | {job})
        for job in graph:
            visit(job, set())

    def test_downstream_checkouts_pin_a_commit_from_trusted_history(self):
        graph = jobs(WORKFLOW.read_text())
        for name, body in graph.items():
            if name == 'prepare-version':
                continue
            for checkout in body.split('uses: actions/checkout@')[1:]:
                settings, following = checkout.split('      - ', 1)
                self.assertIn('ref: develop', settings, name)
                self.assertIn('fetch-depth: 0', settings, name)
                self.assertIn('persist-credentials: false', settings, name)
                pin = following.split('      - ', 1)[0]
                self.assertIn('name: Pin the trusted release commit', pin, name)
                self.assertIn('RELEASE_COMMIT: ${{ needs.prepare-version.outputs.commit }}', pin)
                self.assertIn('[[ "$RELEASE_COMMIT" =~ ^[0-9a-f]{40}$ ]]', pin)
                ancestry = pin.index('git merge-base --is-ancestor "$RELEASE_COMMIT" origin/develop')
                detach = pin.index('git checkout --detach "$RELEASE_COMMIT"')
                self.assertLess(ancestry, detach)
                self.assertIn('test "$(git rev-parse HEAD)" = "$RELEASE_COMMIT"', pin)

    def test_release_pin_survives_branch_advance_and_rejects_untrusted_commits(self):
        body = jobs(WORKFLOW.read_text())['macos-app']
        step = body.split('name: Pin the trusted release commit', 1)[1].split('      - ', 1)[0]
        script = '\n'.join(line[10:] for line in step.split('        run: |\n', 1)[1].splitlines())
        with tempfile.TemporaryDirectory() as directory:
            def git(*args):
                return subprocess.check_output(['git', *args], cwd=directory, text=True, stderr=subprocess.DEVNULL).strip()
            git('init', '-b', 'develop')
            git('config', 'user.name', 'Test')
            git('config', 'user.email', 'test@example.invalid')
            git('commit', '--allow-empty', '-m', 'release')
            release = git('rev-parse', 'HEAD')
            git('commit', '--allow-empty', '-m', 'later approved change')
            tip = git('rev-parse', 'HEAD')
            git('update-ref', 'refs/remotes/origin/develop', tip)
            git('checkout', '--orphan', 'unrelated')
            git('commit', '--allow-empty', '-m', 'unrelated')
            unrelated = git('rev-parse', 'HEAD')
            for commit, accepted in ((release, True), (unrelated, False), ('--help', False)):
                git('checkout', 'develop')
                result = subprocess.run(['bash', '-e', '-o', 'pipefail', '-c', script], cwd=directory,
                                        env=dict(os.environ, RELEASE_COMMIT=commit), capture_output=True)
                self.assertEqual(result.returncode == 0, accepted, result.stderr)
                self.assertEqual(git('rev-parse', 'HEAD'), release if accepted else tip)

    def test_combined_store_upload_is_built_before_publication_and_reused(self):
        graph = jobs(WORKFLOW.read_text())
        self.assertIn('windows-store-upload', dependencies(graph['all-platform-builds']))
        package = graph['windows-store-upload']
        self.assertIn('windows-msix', dependencies(package))
        self.assertIn('./scripts/build-msix-upload.ps1', package)
        self.assertNotIn('environment:', package)
        self.assertNotIn('    if:', package)
        submission = graph['publish-microsoft-store']
        self.assertIn('uses: ./.github/workflows/microsoft-store-publish.yml', submission)
        self.assertIn('release_tag: ${{ needs.prepare-version.outputs.tag }}', submission)
        self.assertIn('actions: read', submission)
        self.assertIn('dry_run: false', submission)
        self.assertNotIn('steps:', submission)
        self.assertNotIn('MakeAppx', submission)
        self.assertNotIn('Compress-Archive', submission)
        ci = (WORKFLOW.parent / 'ci.yml').read_text()
        self.assertIn('./scripts/build-msix.ps1', ci)
        self.assertIn('./scripts/tests/test-msix-upload.ps1', ci)
        manual = (WORKFLOW.parent / 'microsoft-store-package.yml').read_text()
        self.assertIn('./scripts/build-msix-upload.ps1', manual)

    def test_publication_waits_for_every_requested_variant(self):
        body = jobs(WORKFLOW.read_text())['publish-release']
        required = {'prepare-version', 'all-platform-builds', 'macos-direct'}
        self.assertEqual(set(dependencies(body)), required | {'macos-app-store'})
        expression = re.search(r'    if: >-\n(.*?)    runs-on:', body, re.S)[1]
        expression = expression.strip().removeprefix('${{').removesuffix('}}').strip()

        def permitted(results, selected, cancelled=False):
            value = expression.replace('cancelled()', str(cancelled))
            value = value.replace('inputs.build_app_store', str(selected))
            value = re.sub(r'needs\.([\w-]+)\.result', lambda match: repr(results[match[1]]), value)
            value = value.replace('&&', ' and ').replace('||', ' or ').replace('!', ' not ')
            return eval(' '.join(value.split()), {'__builtins__': {}}, {})

        for selected in (False, True):
            for apple in ('success', 'failure', 'cancelled', 'skipped'):
                results = dict.fromkeys(required, 'success') | {'macos-app-store': apple}
                expected = apple == 'success' or (not selected and apple == 'skipped')
                self.assertEqual(permitted(results, selected), expected, (selected, apple))
                self.assertFalse(permitted(results, selected, cancelled=True))
                for job in required:
                    for failure in ('failure', 'cancelled', 'skipped'):
                        self.assertFalse(permitted(results | {job: failure}, selected))

    def test_macos_publishes_only_the_verified_dmg(self):
        body = jobs(WORKFLOW.read_text())['macos-direct']
        self.assertNotIn('macos.zip', body)
        self.assertNotIn('Archive the notarized app bundle', body)
        self.assertIn('sonos-volume-bridge-*-macos.dmg', body)
        self.assertIn('xcrun stapler validate "$image"', body)
        self.assertIn('hdiutil verify "$image"', body)

    def test_windows_artifacts_are_separate_for_both_native_architectures(self):
        graph = jobs(WORKFLOW.read_text(encoding='utf-8'))
        for job in ('windows-app', 'windows-nsis', 'windows-msix'):
            self.assertIn('os: windows-latest', graph[job])
            self.assertIn('arch: x64', graph[job])
            self.assertIn('os: windows-11-arm', graph[job])
            self.assertIn('arch: arm64', graph[job])
            uploads = graph[job].split('uses: actions/upload-artifact@')[1:]
            self.assertTrue(uploads)
            for upload in uploads:
                self.assertRegex(upload, r'name: [^\n]*\$\{\{ matrix.arch \}\}')


if __name__ == '__main__':
    unittest.main()

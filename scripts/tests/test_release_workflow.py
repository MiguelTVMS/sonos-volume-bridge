"""Release dependency graph checks (no GitHub credentials or network required)."""
import re
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[2] / '.github/workflows/release-candidate.yml'


def jobs(text):
    return dict(re.findall(r'^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)', text, re.M | re.S))


def dependencies(body):
    match = re.search(r'^    needs:\s*(\[[^\]]*\]|[\w-]+)', body, re.M)
    return re.findall(r'[\w-]+', match[1]) if match else []


class ReleaseWorkflowTests(unittest.TestCase):
    def test_all_platform_builds_gate_signing_and_store_submission(self):
        graph = jobs(WORKFLOW.read_text(encoding='utf-8'))
        builds = {'macos-app', 'linux-deb', 'windows-app', 'windows-nsis', 'windows-msix'}
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

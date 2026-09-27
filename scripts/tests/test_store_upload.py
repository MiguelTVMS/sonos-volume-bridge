"""Tag-based Store recovery without network access or Partner Center mutations."""
import importlib.util
import io
import re
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('store_upload', ROOT / 'scripts/resolve-store-upload.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def archive(entries):
    data = io.BytesIO()
    with zipfile.ZipFile(data, 'w') as z:
        for name, value in entries.items():
            z.writestr(name, value)
    return data.getvalue()


def upload(version='1.2.3.0', architectures=('x64', 'arm64'), child_version=None):
    entries = {}
    packages = []
    for arch in architectures:
        filename = f'package_{arch}.msix'
        entries[filename] = archive({'AppxManifest.xml': f'<Package xmlns="urn:package"><Identity Name="Example" Publisher="Example" Version="{child_version or version}" ProcessorArchitecture="{arch}"/></Package>'})
        packages.append(f'<Package Architecture="{arch}" Version="{version}" FileName="{filename}"/>')
    entries['AppxMetadata/AppxBundleManifest.xml'] = f'<Bundle xmlns="urn:bundle"><Identity Name="Example" Publisher="Example" Version="{version}"/><Packages>{"".join(packages)}</Packages></Bundle>'
    return archive({'Example.msixbundle': archive(entries)})


class StoreUploadTests(unittest.TestCase):
    def test_verify_exact_tag_and_both_embedded_packages(self):
        for data, accepted in ((upload(), True), (upload('1.2.4.0'), False),
                               (upload(architectures=('x64',)), False),
                               (upload(child_version='1.2.4.0'), False)):
            with tempfile.TemporaryDirectory() as directory:
                (Path(directory) / 'Example.msixupload').write_bytes(data)
                if accepted:
                    module.verify(directory, 'v1.2.3')
                else:
                    with self.assertRaises(ValueError):
                        module.verify(directory, 'v1.2.3')

    def test_failed_store_run_can_supply_successfully_published_artifact(self):
        replies = [
            {'isDraft': False, 'isPrerelease': False, 'tagName': 'v1.2.3'},
            {'workflow_runs': [{'id': 42, 'head_branch': 'develop', 'event': 'workflow_dispatch', 'conclusion': 'failure'}]},
            {'artifacts': [{'name': 'microsoft-store-upload-v1.2.3', 'expired': False}]},
            {'jobs': [{'name': name, 'conclusion': 'success'} for name in
                      ('Build and verify combined Microsoft Store upload', 'Publish GitHub Release')]},
        ]
        # Automatic release calls run before the parent workflow is complete;
        # retries must also accept a parent that failed only at Store submission.
        for conclusion in (None, 'failure', 'success'):
            replies[1]['workflow_runs'][0]['conclusion'] = conclusion
            with patch.object(module, 'gh', side_effect=replies):
                self.assertEqual(module.resolve('v1.2.3', 'submit', 'owner/repo', 'refs/heads/develop'), (42, 'microsoft-store-upload-v1.2.3'))


    def test_rejects_unpublished_expired_and_unverified_artifacts(self):
        for artifacts, jobs in (([], []), ([{'name': 'microsoft-store-upload-v1.2.3', 'expired': True}], []),
                                ([{'name': 'microsoft-store-upload-v1.2.3', 'expired': False}], [])):
            replies = [{'isDraft': False, 'isPrerelease': False, 'tagName': 'v1.2.3'},
                       {'workflow_runs': [{'id': 42, 'head_branch': 'develop', 'event': 'workflow_dispatch'}]},
                       {'artifacts': artifacts}, {'jobs': jobs}]
            with patch.object(module, 'gh', side_effect=replies), self.assertRaises(ValueError):
                module.resolve('v1.2.3', 'validate', 'owner/repo', 'refs/heads/develop')

    def test_submission_requires_ga_and_trusted_dispatch(self):
        with patch.object(module, 'gh') as mocked:
            for tag, ref in (('--help', 'refs/heads/develop'), ('develop', 'refs/heads/develop'), ('', 'refs/heads/develop'), ('v1.2.3', 'refs/heads/feature')):
                with self.assertRaises(ValueError):
                    module.resolve(tag, 'submit', 'owner/repo', ref)
            mocked.assert_not_called()
        with patch.object(module, 'gh', return_value={'isDraft': False, 'isPrerelease': True, 'tagName': 'v1.2.3'}), self.assertRaises(ValueError):
            module.resolve('v1.2.3', 'submit', 'owner/repo', 'refs/heads/develop')

    def test_manual_and_release_publish_share_tag_and_dry_run_workflow(self):
        text = (ROOT / '.github/workflows/microsoft-store-publish.yml').read_text()
        for event in ('workflow_dispatch', 'workflow_call'):
            block = text.split(f'  {event}:', 1)[1].split('\n\n', 1)[0].split('  workflow_call:', 1)[0]
            self.assertEqual(re.findall(r'^      (\w+):', block, re.M), ['release_tag', 'dry_run'])
            self.assertIn('required: true', block)
            self.assertIn('default: true', block)
            self.assertIn('type: boolean', block)
        self.assertIn('--mode submit', text)
        submission = text.split('  submit-release:', 1)[1]
        self.assertIn('if: ${{ !inputs.dry_run }}', submission)
        self.assertNotIn('secrets.', text.split('  submit-release:', 1)[0])
        self.assertNotIn('msstore reconfigure', text.split('  submit-release:', 1)[0])
        self.assertIn('environment: microsoft-store', text)
        self.assertIn('group: microsoft-store-submission', text)
        self.assertIn('needs: [resolve-release, validate-release]', text)
        self.assertNotIn('build-msix', text)
        package = (ROOT / '.github/workflows/microsoft-store-package.yml').read_text()
        self.assertNotIn('publish-msstore', package)
        self.assertNotIn('secrets.', package)

    def test_workflows_use_the_shared_tested_cli_command(self):
        for name in ('microsoft-store-publish.yml',):
            text = (ROOT / '.github/workflows' / name).read_text()
            self.assertNotIn('--inputFile', text)
            self.assertIn('./scripts/publish-msstore.ps1 -InputDirectory store-upload', text)
            self.assertIn('version: v0.4.3', text)
        ci = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('version: v0.4.3', ci)
        regression = (ROOT / 'scripts/tests/test-msix-upload.ps1').read_text()
        self.assertIn('publish-msstore.ps1', regression)
        self.assertIn('-ValidateOnly', regression)


if __name__ == '__main__':
    unittest.main()

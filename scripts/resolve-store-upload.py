"""Resolve an existing release upload, or verify its version and architectures."""
import argparse
import io
import json
import os
from pathlib import Path
import re
import subprocess
import xml.etree.ElementTree as ET
import zipfile


def gh(*args):
    return json.loads(subprocess.check_output(['gh', *args], text=True))


def resolve(tag, mode, repo, ref):
    if not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise ValueError('Use an existing release tag such as v1.2.3.')
    if mode == 'submit' and ref != 'refs/heads/develop':
        raise ValueError('Store submission must be dispatched from develop.')
    release = gh('release', 'view', tag, '--repo', repo, '--json', 'isDraft,isPrerelease,tagName')
    if release['isDraft'] or release['tagName'] != tag:
        raise ValueError('The selected release must already be published.')
    if mode == 'submit' and release['isPrerelease']:
        raise ValueError('Only GA releases can be submitted to the Store.')
    name = f'microsoft-store-upload-{tag}'
    # Paginate so a recent release is not hidden by other workflows or retries.
    for page in range(1, 11):
        runs = gh('api', f'repos/{repo}/actions/workflows/release-candidate.yml/runs?branch=develop&event=workflow_dispatch&per_page=100&page={page}')['workflow_runs']
        for run in runs:
            if run['head_branch'] != 'develop' or run['event'] != 'workflow_dispatch':
                continue
            artifacts = gh('api', f'repos/{repo}/actions/runs/{run["id"]}/artifacts?per_page=100')['artifacts']
            matches = [item for item in artifacts if item['name'] == name and not item['expired']]
            if len(matches) != 1:
                continue
            jobs = gh('api', f'repos/{repo}/actions/runs/{run["id"]}/jobs?per_page=100')['jobs']
            required = {'Build and verify combined Microsoft Store upload', 'Publish GitHub Release'}
            succeeded = {job['name'] for job in jobs if job['conclusion'] == 'success'}
            if required <= succeeded:
                return run['id'], name
        if len(runs) < 100:
            break
    raise ValueError('No retained verified upload found for this tag. Do not rebuild or bump the version automatically.')


def verify(directory, tag):
    if not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise ValueError('Invalid release tag.')
    files = list(Path(directory).iterdir())
    if len(files) != 1 or files[0].suffix != '.msixupload':
        raise ValueError('Expected exactly one Store upload.')
    version = tag[1:] + '.0'
    with zipfile.ZipFile(files[0]) as upload:
        if len(upload.namelist()) != 1 or not upload.namelist()[0].endswith('.msixbundle'):
            raise ValueError('Expected one bundle in the upload.')
        with zipfile.ZipFile(io.BytesIO(upload.read(upload.namelist()[0]))) as bundle:
            manifest = ET.fromstring(bundle.read('AppxMetadata/AppxBundleManifest.xml'))
            identity = manifest.find('{*}Identity')
            if identity is None or identity.get('Version') != version:
                raise ValueError('Bundle version does not match the selected tag.')
            packages = manifest.findall('{*}Packages/{*}Package')
            if len(packages) != 2 or {p.get('Architecture') for p in packages} != {'x64', 'arm64'}:
                raise ValueError('Expected exactly x64 and ARM64 packages.')
            for package in packages:
                if package.get('Version') != version:
                    raise ValueError('Package version does not match the selected tag.')
                with zipfile.ZipFile(io.BytesIO(bundle.read(package.get('FileName')))) as payload:
                    child = ET.fromstring(payload.read('AppxManifest.xml')).find('{*}Identity')
                    if child is None or any(child.get(key) != identity.get(key) for key in ('Name', 'Publisher', 'Version')):
                        raise ValueError('Embedded package identity differs from its bundle.')
                    if child.get('ProcessorArchitecture') != package.get('Architecture'):
                        raise ValueError('Embedded package architecture differs from its bundle.')


def verify_submission(data, tag, upload_directory=None):
    if not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise ValueError('Invalid release tag.')
    # Accept the CLI serializer's camelCase or PascalCase property casing.
    data = {key.lower(): value for key, value in data.items()}
    status = data.get('status', '')
    if status not in {'PreProcessing', 'Certification', 'Release', 'Published', 'ReadyForRelease'}:
        raise ValueError('Store submission is not in an accepted processing state.')
    packages = [{key.lower(): value for key, value in item.items()}
                for item in (data.get('applicationpackages') or [])]
    uploaded = [item for item in packages if item.get('version') == tag[1:] + '.0'
                and item.get('filestatus') == 'Uploaded']
    architectures = {(item.get('architecture') or '').lower() for item in uploaded}
    if {'x64', 'arm64'} <= architectures:
        return
    # Partner Center reports a multi-architecture upload as one Neutral package.
    # Match its exact filename/version and establish coverage from the verified
    # local payload, never from the Neutral label alone.
    if upload_directory is not None:
        verify(upload_directory, tag)
        filename = next(Path(upload_directory).iterdir()).name
        if any(item.get('filename') == filename
               and (item.get('architecture') or '').lower() == 'neutral'
               for item in uploaded):
            return
    raise ValueError('Store has not confirmed the selected version and verified architecture coverage.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--mode', choices=('validate', 'submit'), default='validate')
    parser.add_argument('--verify-directory')
    parser.add_argument('--verify-submission')
    args = parser.parse_args()
    if args.verify_submission:
        verify_submission(json.loads(Path(args.verify_submission).read_text(encoding='utf-8-sig')), args.tag, args.verify_directory)
        print('Store confirms the selected upload version; architecture coverage verified. Certification is separate.')
    elif args.verify_directory:
        verify(args.verify_directory, args.tag)
        print('Release version and both architecture packages verified.')
    else:
        run_id, artifact = resolve(args.tag, args.mode, os.environ['GITHUB_REPOSITORY'], os.environ['GITHUB_REF'])
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            output.write(f'run_id={run_id}\nartifact={artifact}\n')
        print('Resolved the retained upload from a verified published release.')


if __name__ == '__main__':
    main()

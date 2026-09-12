"""Open an approval-required main PR from the exact published stable release."""
import json
import os
import re
import subprocess


def gh(*args):
    return subprocess.check_output(['gh', *args], text=True).strip()


def promote(tag, commit):
    if not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise ValueError('Promotion requires a stable version tag')
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('Invalid release commit')
    release = json.loads(gh('release', 'view', tag, '--json', 'isDraft,isPrerelease'))
    if release['isDraft'] or release['isPrerelease']:
        raise ValueError('Release must be published and stable')
    resolved = subprocess.check_output(['git', 'rev-parse', f'{tag}^{{commit}}'], text=True).strip()
    if resolved != commit:
        raise ValueError('Release tag does not match the validated commit')
    repo = os.environ['GITHUB_REPOSITORY']
    comparison = json.loads(gh('api', f'repos/{repo}/compare/main...{commit}'))
    if comparison['status'] in ('identical', 'behind'):
        print('Release is already contained in main.')
        return
    if comparison['status'] != 'ahead':
        raise ValueError('main has diverged; resolve manually before promotion')
    branch = f'release/{tag[1:]}'
    refs = json.loads(gh('api', f'repos/{repo}/git/matching-refs/heads/{branch}'))
    exact = [ref for ref in refs if ref['ref'] == f'refs/heads/{branch}']
    if exact:
        if exact[0]['object']['sha'] != commit:
            raise ValueError('Existing release branch points elsewhere; refusing to overwrite')
    else:
        gh('api', '--method', 'POST', f'repos/{repo}/git/refs', '-f', f'ref=refs/heads/{branch}', '-f', f'sha={commit}')
    prs = json.loads(gh('pr', 'list', '--base', 'main', '--head', branch, '--state', 'all', '--json', 'state'))
    if prs:
        if any(pr['state'] == 'OPEN' for pr in prs):
            print('Promotion PR already exists.')
            return
        raise ValueError('Promotion PR was closed; review manually instead of creating a duplicate')
    gh('pr', 'create', '--base', 'main', '--head', branch,
       '--title', f'Release {tag}: synchronize main',
       '--body', 'Bring the published stable release into main. The source branch is fixed to the released code and excludes subsequent development.\n\nRelease publication has completed. Review and merge this PR using a merge commit to preserve release ancestry. Publication does not wait for this approval.')


if __name__ == '__main__':
    promote(os.environ['RELEASE_TAG'], os.environ['RELEASE_COMMIT'])

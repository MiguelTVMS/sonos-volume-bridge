#!/usr/bin/env python3
"""Remove only signing environment wait timers; preview unless --apply is supplied."""
import argparse
import json
import subprocess

ENVIRONMENTS = ('apple-signing', 'apple-app-store')


def update_payload(environment):
    rules = environment['protection_rules']
    reviewers = next((r for r in rules if r['type'] == 'required_reviewers'), None)
    if not reviewers or not reviewers.get('reviewers'):
        raise ValueError('Expected signing reviewers; refusing to change this environment')
    return {
        'wait_timer': 0,
        'reviewers': [{'type': r['type'], 'id': r['reviewer']['id']}
                      for r in reviewers['reviewers']],
        'prevent_self_review': reviewers['prevent_self_review'],
        'deployment_branch_policy': environment['deployment_branch_policy'],
    }


def api(endpoint, payload=None):
    args = ['gh', 'api', endpoint]
    if payload is not None:
        args += ['--method', 'PUT', '--input', '-']
    return json.loads(subprocess.check_output(
        args, input=json.dumps(payload) if payload is not None else None, text=True))


def run(repo, apply=False, request=api):
    # Validate both existing environments before any mutation. GET failure never creates one.
    plans = []
    for name in ENVIRONMENTS:
        endpoint = f'repos/{repo}/environments/{name}'
        before = request(endpoint)
        plans.append((name, endpoint, before, update_payload(before)))
    for name, endpoint, before, payload in plans:
        if not apply:
            print(f'{name}: preview only; remove wait timer and retain existing approvals and branch policy')
            continue
        # Refuse stale updates if an administrator edited the rules during this operation.
        if request(endpoint) != before:
            raise RuntimeError('Environment changed during review; rerun before applying')
        request(endpoint, payload)
        after = request(endpoint)
        timers = [r.get('wait_timer', 0) for r in after['protection_rules']
                  if r['type'] == 'wait_timer']
        if any(timers) or update_payload(after) != payload:
            raise RuntimeError('Environment verification failed; inspect settings before continuing')
        print(f'{name}: wait timer removed; retained rules verified')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', required=True, help='owner/repository')
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    run(args.repo, args.apply)

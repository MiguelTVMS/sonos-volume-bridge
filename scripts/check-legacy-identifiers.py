#!/usr/bin/env python3
"""Reject undocumented old product identifiers, including newly added files."""
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ALLOWLIST = ROOT / 'docs/legacy-identifiers.json'
PATTERN = re.compile(r'sonos[-_\s]?volume[-_\s]?bridge', re.IGNORECASE)


def check():
    allowed = json.loads(ALLOWLIST.read_text())
    paths = subprocess.check_output(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=ROOT
    ).decode().split('\0')
    failures = []
    for name in set(filter(None, paths)):
        path = ROOT / name
        if not path.is_file() or name == 'docs/legacy-identifiers.json':
            continue
        try:
            content = path.read_text()
        except UnicodeError:
            continue
        if PATTERN.search(content) and not allowed.get(name):
            failures.append(name)
    if failures:
        raise SystemExit('Undocumented legacy product references: ' + ', '.join(sorted(failures)))
    print('Legacy identifiers are documented.')


if __name__ == '__main__':
    check()

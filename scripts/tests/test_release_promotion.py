"""Exercise promotion decisions without writing to GitHub."""
import importlib.util
import json
import os
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('promotion', Path(__file__).parents[1] / 'open-release-promotion.py')
promotion = importlib.util.module_from_spec(spec)
spec.loader.exec_module(promotion)
SHA = 'a' * 40


class PromotionTests(unittest.TestCase):
    def run_case(self, status='ahead', refs=None, prs=None, prerelease=False):
        calls = []
        def fake(*args):
            calls.append(args)
            if args[:2] == ('release', 'view'):
                return json.dumps({'isDraft': False, 'isPrerelease': prerelease})
            if args[0] == 'api' and '/compare/' in args[1]:
                return json.dumps({'status': status})
            if args[0] == 'api' and '/matching-refs/' in args[1]:
                return json.dumps(refs or [])
            if args[:2] == ('pr', 'list'):
                return json.dumps(prs or [])
            return '{}'
        with patch.object(promotion, 'gh', side_effect=fake), patch.object(promotion.subprocess, 'check_output', return_value=SHA), patch.dict(os.environ, GITHUB_REPOSITORY='owner/project'):
            promotion.promote('v1.2.3', SHA)
        return calls

    def test_creates_pinned_branch_and_main_pr(self):
        calls = self.run_case()
        self.assertTrue(any('sha=' + SHA in call for call in calls))
        self.assertTrue(any(call[:2] == ('pr', 'create') and 'main' in call and 'release/1.2.3' in call for call in calls))

    def test_existing_pr_is_not_duplicated(self):
        calls = self.run_case(refs=[{'ref':'refs/heads/release/1.2.3','object':{'sha':SHA}}], prs=[{'state':'OPEN'}])
        self.assertFalse(any(call[:2] == ('pr', 'create') for call in calls))

    def test_contained_release_does_not_mutate(self):
        for status in ('identical', 'behind'):
            calls = self.run_case(status=status)
            self.assertFalse(any('--method' in call or call[:2] == ('pr', 'create') for call in calls))

    def test_unsafe_states_fail(self):
        for kwargs in ({'status':'diverged'}, {'prerelease':True}, {'refs':[{'ref':'refs/heads/release/1.2.3','object':{'sha':'b'*40}}]}, {'prs':[{'state':'CLOSED'}]}):
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                self.run_case(**kwargs)


if __name__ == '__main__':
    unittest.main()

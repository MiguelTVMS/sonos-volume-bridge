import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('signing_wait', Path(__file__).resolve().parents[1] / 'remove-signing-wait.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class SigningWaitTests(unittest.TestCase):
    def setUp(self):
        self.state = {'protection_rules': [
            {'type': 'wait_timer', 'wait_timer': 5},
            {'type': 'required_reviewers', 'prevent_self_review': True,
             'reviewers': [{'type': 'Team', 'reviewer': {'id': 42}}]}],
            'deployment_branch_policy': {'protected_branches': False, 'custom_branch_policies': True}}
        self.writes = []
        self.states = {}

    def request(self, endpoint, payload=None):
        state = self.states.setdefault(endpoint, copy.deepcopy(self.state))
        if payload is not None:
            self.writes.append(copy.deepcopy(payload))
            state['protection_rules'][0]['wait_timer'] = payload['wait_timer']
        return copy.deepcopy(state)

    def test_preview_never_writes(self):
        module.run('test/repo', request=self.request)
        self.assertEqual(self.writes, [])

    def test_apply_removes_only_timer_and_verifies(self):
        module.run('test/repo', apply=True, request=self.request)
        self.assertEqual(len(self.writes), 2)
        for payload in self.writes:
            self.assertEqual(payload['wait_timer'], 0)
            self.assertEqual(payload['reviewers'], [{'type': 'Team', 'id': 42}])
            self.assertTrue(payload['prevent_self_review'])
            self.assertEqual(payload['deployment_branch_policy'], self.state['deployment_branch_policy'])

    def test_missing_approvers_fails_before_any_write(self):
        self.state['protection_rules'].pop()
        with self.assertRaises(ValueError):
            module.run('test/repo', apply=True, request=self.request)
        self.assertEqual(self.writes, [])

    def test_failed_update_is_not_reported_as_success(self):
        def unchanged(endpoint, payload=None):
            return copy.deepcopy(self.state)
        with self.assertRaises(RuntimeError):
            module.run('test/repo', apply=True, request=unchanged)

    def test_concurrent_change_refuses_update(self):
        count = 0
        def changed(endpoint, payload=None):
            nonlocal count
            count += 1
            if count == 3:
                self.states[endpoint]['deployment_branch_policy'] = None
            return self.request(endpoint, payload)
        with self.assertRaises(RuntimeError):
            module.run('test/repo', apply=True, request=changed)
        self.assertEqual(self.writes, [])

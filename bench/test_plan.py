"""Tests for plan.py: python3 -m unittest discover -s bench"""

import unittest

from plan import plan

REGISTRY = {
    "kernels": [
        {"kernel": k, "adapter": {"commit": c * 40}}
        for k, c in (("springrep", "a"), ("beta", "b"), ("gamma", "c"))
    ]
}
INDEX = {
    "runs": [{"reports": {"springrep": {"seconds": 150}, "gamma": {"seconds": 176}}}],
    "leaderboard": {"2.0.0": {"run": "1-1"}},
}
SMOKE = "--family boxes\n"
MAIN = {"REF": "refs/heads/main", "SHA": "29e5bcb" + "0" * 33}


def run(**env):
    return plan({**MAIN, **env}, REGISTRY, INDEX, SMOKE, "2.0.0")


class Plan(unittest.TestCase):
    def test_the_form_unchanged_runs_every_kernel_and_test_to_prod(self):
        settings, lines, errors = run()
        self.assertEqual(errors, [])
        self.assertEqual(settings, {"kernels": "springrep beta gamma", "scope": "all",
                                    "suite_args": "", "nist": "true"})
        self.assertIn("Leaderboard: every kernel; proposed as suite 2.0.0's leaderboard", lines)

    def test_named_kernels_run_alone_in_the_registry_order_as_rows(self):
        settings, lines, _ = run(KERNELS="gamma, springrep")
        self.assertEqual(settings["kernels"], "springrep gamma")
        self.assertEqual(settings["scope"], "named")
        self.assertIn("Leaderboard: springrep, gamma: proposed as rows of suite 2.0.0's "
                      "leaderboard", lines)
        self.assertIn("Estimated: ~15 min", lines)

    def test_named_kernels_on_a_suite_with_no_leaderboard_are_only_listed(self):
        _, lines, _ = plan({**MAIN, "KERNELS": "gamma"}, REGISTRY, INDEX, SMOKE, "2.0.1")
        self.assertIn("Leaderboard: gamma: listed with their pages; suite 2.0.1 has no "
                      "leaderboard yet, and only a run of every kernel starts one", lines)

    def test_an_unknown_kernel_is_refused_with_the_registry_named(self):
        _, _, errors = run(KERNELS="gamaa,beta")
        self.assertEqual(errors, ["not in kernels.json: gamaa; it lists springrep, beta, gamma"])

    def test_publishing_from_a_branch_is_refused(self):
        _, _, errors = run(REF="refs/heads/feature")
        self.assertIn("publish is prod, which runs only from main or a v* tag", errors[0])
        self.assertEqual(run(REF="refs/heads/feature", PUBLISH="none")[2], [])
        self.assertEqual(run(REF="refs/tags/v2.0.0")[2], [])

    def test_smoke_and_custom_tests_are_not_eligible(self):
        settings, lines, _ = run(TESTS="smoke")
        self.assertEqual(settings["suite_args"], "--family boxes")
        self.assertIn("Leaderboard: none", lines)
        self.assertEqual(run(TESTS="custom")[2], ["tests is custom but custom_args is empty"])
        self.assertEqual(run(TESTS="custom", CUSTOM_ARGS="--family x")[0]["suite_args"],
                         "--family x")

    def test_without_nist_or_prod_a_run_is_not_eligible(self):
        self.assertIn("Leaderboard: none", run(NIST="false")[1])
        self.assertIn("Leaderboard: none", run(PUBLISH="dev")[1])


if __name__ == "__main__":
    unittest.main()

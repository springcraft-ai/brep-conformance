"""The Benchmark workflow's plan: checks the form's inputs and says what the run will do, before any
machine starts. Reads the inputs from the environment (KERNELS, TESTS, CUSTOM_ARGS, NIST, PUBLISH,
REF, SHA) and the repository at <root>.

  plan.py <root>

Writes kernels, scope, suite_args and nist to $GITHUB_OUTPUT and the plan to
$GITHUB_STEP_SUMMARY, where set; exits 1 with every problem found.
"""

import json
import os
import re
import sys

# The machine's start-up and shutdown, about 10 minutes in the sessions so far.
START_SECONDS = 600


def suite_version(root):
    for line in open(os.path.join(root, "CHANGELOG.md")):
        if line.startswith("## "):
            return line.split()[1]
    return "?"


def last_seconds(index):
    """Each kernel's pass time in the newest listed run that has one."""
    seconds = {}
    for run in index.get("runs", []):
        for kernel, report in run["reports"].items():
            if report.get("seconds") is not None:
                seconds[kernel] = report["seconds"]
    return seconds


def plan(env, registry, index, smoke, suite):
    """The run's settings and the summary's lines, or the problems with the inputs."""
    errors, notes = [], []
    names = [k["kernel"] for k in registry["kernels"]]
    asked = env.get("KERNELS", "all").strip() or "all"
    if asked == "all":
        kernels = list(names)
    else:
        kernels = [k for k in re.split(r"[,\s]+", asked) if k]
        unknown = [k for k in kernels if k not in names]
        if unknown:
            errors.append(f"not in kernels.json: {', '.join(unknown)}; "
                          f"it lists {', '.join(names)}")
    kernels = [k for k in names if k in kernels]
    scope = "all" if asked == "all" else "named"

    tests = env.get("TESTS", "all")
    custom = env.get("CUSTOM_ARGS", "").strip()
    if tests == "all":
        suite_args = ""
    elif tests == "smoke":
        suite_args = smoke.strip()
    elif tests == "custom":
        suite_args = custom
        if not custom:
            errors.append("tests is custom but custom_args is empty")
    else:
        errors.append(f"tests is {tests}, not all, smoke or custom")
        suite_args = ""
    if custom and tests != "custom":
        notes.append(f"custom_args is ignored: tests is {tests}")
    nist = env.get("NIST", "true") != "false"

    publish = env.get("PUBLISH", "prod")
    ref = env.get("REF", "")
    if publish != "none" and not (ref == "refs/heads/main" or ref.startswith("refs/tags/v")):
        errors.append(f"publish is {publish}, which runs only from main or a v* tag, not {ref}")

    complete = suite_args == "" and nist
    if not complete:
        notes.append("not eligible for a leaderboard: every test must run, with NIST's files")
    elif publish != "prod":
        notes.append(f"not eligible for a leaderboard: published to {publish}, not prod")
    if not (complete and publish == "prod"):
        board = "none"
    elif scope == "all":
        board = f"every kernel; proposed as suite {suite}'s leaderboard"
    elif suite in index.get("leaderboard", {}):
        board = f"{', '.join(kernels)}: proposed as rows of suite {suite}'s leaderboard"
    else:
        board = (f"{', '.join(kernels)}: listed with their pages; suite {suite} has no "
                 "leaderboard yet, and only a run of every kernel starts one")

    seconds = last_seconds(index)
    timed = [k for k in kernels if k in seconds]
    total = START_SECONDS + sum(seconds[k] for k in timed)
    estimate = f"~{round(total / 60)} min"
    if len(timed) < len(kernels):
        estimate += f" (no earlier time for {', '.join(k for k in kernels if k not in timed)})"
    if suite_args:
        estimate = f"under {estimate}: only some tests run"

    sha = env.get("SHA", "")[:7]
    lines = [
        f"**Suite {suite} at {sha}** · kernels: {scope} · tests: {tests}"
        + (f" (`{suite_args}`)" if suite_args else "")
        + f" · NIST {'on' if nist else 'off'} · publish: {publish}",
        "",
        "| Kernel | Adapter | Last pass |",
        "| --- | --- | --- |",
    ]
    commits = {k["kernel"]: k["adapter"]["commit"][:7] for k in registry["kernels"]}
    for k in kernels:
        last = f"{seconds[k]:.0f} s" if k in seconds else "–"
        lines.append(f"| {k} | {commits.get(k, '?')} | {last} |")
    lines += ["", f"Leaderboard: {board}", f"Estimated: {estimate}"]
    lines += [f"> {n}" for n in notes]
    settings = {
        "kernels": " ".join(kernels),
        "scope": scope,
        "suite_args": suite_args,
        "nist": "true" if nist else "false",
    }
    return settings, lines, errors


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    root = sys.argv[1]
    registry = json.load(open(os.path.join(root, "kernels.json")))
    index_file = os.path.join(root, "reports.json")
    index = json.load(open(index_file)) if os.path.exists(index_file) else {}
    smoke = open(os.path.join(root, "bench", "smoke.txt")).read()
    settings, lines, errors = plan(os.environ, registry, index, smoke, suite_version(root))
    for e in errors:
        print(f"::error::{e}")
    if errors:
        sys.exit(1)
    print("\n".join(lines))
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as out:
            for k, v in settings.items():
                out.write(f"{k}={v}\n")
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as out:
            out.write("## Plan\n\n" + "\n".join(lines) + "\n")


if __name__ == "__main__":
    main()

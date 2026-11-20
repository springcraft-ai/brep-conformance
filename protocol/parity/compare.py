#!/usr/bin/env python3
"""Send requests.jsonl to the Rust and the C++ echo adapters, and fail where their replies differ.

Usage: compare.py RUST_ADAPTER CPP_ADAPTER

Each line goes to both adapters as it is written. After every reply that issues a handle, both are
asked to `write_step` that solid, so the arguments each binding read are compared in full. Replies
are compared as parsed JSON, so the two JSON libraries may write a number differently. Every
function in doc/api.md's tables must be answered `ok` at least once, so a function that one binding
lacks, or that the corpus does not call, fails the test.
"""

import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
API = HERE.parent.parent / "doc" / "api.md"


def functions():
    names = []
    for line in API.read_text().splitlines():
        if line.startswith("| [`"):
            names.append(line.split("`")[1])
    return names


class Adapter:
    def __init__(self, path):
        self.proc = subprocess.Popen(
            [path], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding="utf-8")

    def ask(self, line):
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()
        reply = self.proc.stdout.readline()
        if not reply:
            sys.exit(f"{self.proc.args[0]} closed its output after {line!r}")
        return json.loads(reply)

    def close(self):
        self.proc.stdin.close()
        stderr = self.proc.stderr.read()
        self.proc.wait()
        return stderr


def handles(cmd, reply):
    ok = reply.get("ok")
    if cmd == "split" and isinstance(ok, list):
        return ok
    if isinstance(ok, int) and not isinstance(ok, bool) and cmd not in ("hello", "reset"):
        return [ok]
    return []


def readable(cmd, reply):
    """The reply as it is compared: a written solid parsed, so its numbers compare by value, and a
    binding's own reason for declining a request dropped, since each binding words it its own way.
    A reason the kernel gave (the echo kernel's start "echo") must reach the client unchanged."""
    if cmd == "write_step" and isinstance(reply.get("ok"), str):
        return {"ok": json.loads(reply["ok"])}
    if "err" in reply and not reply["err"].startswith("echo"):
        return {**reply, "err": None}
    return reply


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    rust, cpp = Adapter(sys.argv[1]), Adapter(sys.argv[2])
    answered = set()
    differences = 0
    sent = 0

    def both(line, cmd):
        nonlocal differences, sent
        sent += 1
        a, b = rust.ask(line), cpp.ask(line)
        if readable(cmd, a) != readable(cmd, b):
            differences += 1
            print(f"request: {line}\n  rust: {json.dumps(a)}\n  c++:  {json.dumps(b)}")
        return a, b

    for line in (HERE / "requests.jsonl").read_text(encoding="utf-8").splitlines():
        try:
            cmd = json.loads(line).get("cmd")
        except (ValueError, AttributeError):
            cmd = None
        a, b = both(line, cmd)
        if "ok" in a and "ok" in b:
            answered.add(cmd)
        for h in handles(cmd, a):
            both(json.dumps({"cmd": "write_step", "s": h}), "write_step")

    rust_err, cpp_err = rust.close(), cpp.close()
    if rust_err != cpp_err:
        differences += 1
        print(f"stderr differs:\n  rust: {rust_err!r}\n  c++:  {cpp_err!r}")
    missing = [f for f in functions() if f not in answered]
    if missing:
        print("functions no request had answered ok by both:", ", ".join(missing))
    print(f"{sent} requests, {differences} with different replies, {len(missing)} functions "
          "not covered")
    sys.exit(1 if differences or missing else 0)


if __name__ == "__main__":
    main()

"""Reads kernels.json for the session scripts; root is the repository's, adapters under adapters/.

  adapters.py list <root>                      each kernel, its adapter's repository and commit
  adapters.py build <root> <kernel>            the adapter's build commands, one per line
  adapters.py run <root> <kernel>              the command that starts it, with {dir} filled in
  adapters.py check <root> <kernel> <results>  fails unless the pass's adapter reported the kernel
                                               and release kernels.json pins
"""

import json
import os
import sys


def entry(root, kernel):
    for k in json.load(open(os.path.join(root, "kernels.json")))["kernels"]:
        if k["kernel"] == kernel:
            return k
    sys.exit(f"adapters.py: no kernel {kernel} in {root}/kernels.json")


def main():
    what = sys.argv[1] if len(sys.argv) > 1 else ""
    if (what, len(sys.argv)) not in (("list", 3), ("build", 4), ("run", 4), ("check", 5)):
        sys.exit(__doc__)
    if what == "list":
        for k in json.load(open(os.path.join(sys.argv[2], "kernels.json")))["kernels"]:
            print(k["kernel"], k["adapter"]["repo"], k["adapter"]["commit"])
        return
    root, kernel = sys.argv[2:4]
    pinned = entry(root, kernel)
    adapter = pinned["adapter"]
    if what == "check":
        got = json.load(open(sys.argv[4]))["kernel"]
        if (got.get("name"), got.get("version")) != (kernel, pinned["version"]):
            sys.exit(f"reported {got.get('name')} {got.get('version')}; "
                     f"kernels.json pins {kernel} {pinned['version']}")
    elif what == "build":
        print("\n".join(adapter["build"]))
    else:
        print(adapter["run"].replace("{dir}", os.path.join(root, "adapters", kernel)))


main()

"""The Adapter update workflow's steps: moves an adapter the suite maintains to a newer release of
its kernel, compares the suite's results before and after, and moves the kernel's registry entry.
Reads kernels.json at <root>.

  adapter_update.py plan <root> <kernel>      fails unless the workflow can update the kernel's
                                              adapter; prints its repository, build and binary
  adapter_update.py bump <adapter> [version]  pins each kernel crate to the release, or to its
                                              newest, with the version the adapter reports
  adapter_update.py compare <before> <after>  a Markdown table of what changed between two
                                              results files
  adapter_update.py registry <root> <kernel> <results> <commit>
                                              moves the kernel's entry to the release the results
                                              report and to the adapter commit

A kernel crate is a dependency of the adapter, other than brep-protocol, pinned to one release:
exactly, `=X.Y.Z`, from crates.io, or by `rev` from a GitHub repository, whose releases are then its
GitHub releases. An adapter that links its kernel from a Python package pins it in pyproject.toml,
`"name==X.Y.Z"`, and that package's releases on PyPI are the kernel's.
"""

import collections
import json
import os
import re
import sys
import urllib.request

OWNER = "https://github.com/springcraft-ai/"
PROTOCOL = "brep-protocol"
USER_AGENT = "brep-conformance adapter-update (https://github.com/springcraft-ai/brep-conformance)"

# A dependency line: `name = "=1.2.3"` or `name = { package = "Crate", version = "=1.2.3" }`, or
# `name = { git = "https://github.com/owner/repo", rev = "bb09c367" }`.
PLAIN = re.compile(r'^(?P<key>[A-Za-z0-9_-]+)\s*=\s*"=(?P<version>[^"]+)"\s*(#.*)?$')
TABLE = re.compile(r'^(?P<key>[A-Za-z0-9_-]+)\s*=\s*\{(?P<body>.*)\}\s*(#.*)?$')
FIELD = re.compile(r'\b(?P<name>package|version|git|rev|path)\s*=\s*"(?P<value>[^"]*)"')
GITHUB = re.compile(r"^https://github\.com/(?P<slug>[^/]+/[^/]+?)(\.git)?/?$")
REPORTED = re.compile(r'(fn version\(&self\) -> String \{\s*)"(?P<version>[^"]+)"')
STABLE = re.compile(r"^\d+\.\d+\.\d+$")
# A pyproject.toml requirement pinned to one release, `"name==1.2.3.4"`; a final release's number.
PYPI_PIN = re.compile(r'"(?P<name>[A-Za-z0-9][A-Za-z0-9._-]*)==(?P<version>[0-9][^"\s;,]*)"')
FINAL = re.compile(r"^\d+(\.\d+)*$")
BUILDS = (["cargo build --release --locked"], ["./build.sh"])


def read(path):
    with open(path) as f:
        return f.read()


def write(path, text):
    with open(path, "w") as f:
        f.write(text)


def load(path):
    return json.loads(read(path))


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as reply:
        return json.load(reply)


def github(path):
    return fetch(f"https://api.github.com/{path}")


def entry(root, kernel):
    for k in load(os.path.join(root, "kernels.json"))["kernels"]:
        if k["kernel"] == kernel:
            return k
    sys.exit(f"adapter_update.py: no kernel {kernel} in kernels.json")


def plan(root, kernel):
    """The adapter's repository, its build commands and the path of the binary they make inside
    its checkout, or exits when the workflow cannot update it."""
    adapter = entry(root, kernel)["adapter"]
    if not adapter["repo"].startswith(OWNER):
        sys.exit(f"{kernel}'s adapter is {adapter['repo']}, which the suite does not maintain; "
                 "its maintainers move it by pull request")
    if adapter["build"] not in BUILDS:
        sys.exit(f"{kernel}'s adapter builds with {adapter['build']}, which this workflow does "
                 "not run")
    if not re.fullmatch(r"\{dir\}/[A-Za-z0-9_./-]+", adapter["run"]) or ".." in adapter["run"]:
        sys.exit(f"{kernel}'s adapter runs as {adapter['run']}, not a binary in its checkout")
    return {"repo": adapter["repo"], "build": adapter["build"],
            "binary": adapter["run"].removeprefix("{dir}/")}


def kernel_crates(manifest):
    """Each kernel crate, in the order the manifest lists them: {"crate", "version"} for one from
    crates.io, {"crate", "slug", "rev"} for one from GitHub."""
    crates, section = [], None
    for line in manifest.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            section = stripped
            continue
        if section != "[dependencies]":
            continue
        if m := PLAIN.match(stripped):
            crate = {"crate": m["key"], "version": m["version"]}
        elif m := TABLE.match(stripped):
            fields = {f["name"]: f["value"] for f in FIELD.finditer(m["body"])}
            name = fields.get("package", m["key"])
            if "path" in fields:
                continue
            if "git" in fields:
                g = GITHUB.match(fields["git"])
                if not g or "rev" not in fields:
                    continue
                crate = {"crate": name, "slug": g["slug"], "rev": fields["rev"]}
            elif fields.get("version", "").startswith("="):
                crate = {"crate": name, "version": fields["version"][1:]}
            else:
                continue
        else:
            continue
        if crate["crate"] != PROTOCOL:
            crates.append(crate)
    return crates


def pin(manifest, moves):
    """The manifest with each kernel crate's pin moved. moves maps a crate to (from, to): versions
    for one from crates.io, revs for one from GitHub. Links to a moved GitHub release's tag, in a
    comment say, move with it: (from, to, from_tag, to_tag)."""
    out, section = [], None
    for line in manifest.splitlines(keepends=True):
        stripped = line.strip()
        if stripped.startswith("["):
            section = stripped
        elif section == "[dependencies]":
            for name, (old, new, *tags) in moves.items():
                key = re.escape(name)
                plain = re.match(rf'^\s*{key}\s*=\s*"={re.escape(old)}"', line)
                table = (re.search(rf'\bpackage\s*=\s*"{key}"', line)
                         or re.match(rf'^\s*{key}\s*=\s*\{{', line))
                if plain or (table and f'version = "={old}"' in line):
                    line = line.replace(f'"={old}"', f'"={new}"', 1)
                elif table and f'rev = "{old}"' in line:
                    line = line.replace(f'rev = "{old}"', f'rev = "{new}"', 1)
                if len(tags) == 2 and tags[0]:
                    line = line.replace(f"/releases/tag/{tags[0]}", f"/releases/tag/{tags[1]}")
        out.append(line)
    return "".join(out)


def report(source, old, new):
    """The source with the literal version `fn version` returns moved from old to new, and whether
    it held one. An adapter that reports its kernel's own constant needs no change."""
    m = REPORTED.search(source)
    if not m or m["version"] != old:
        return source, False
    return source[:m.start("version")] + new + source[m.end("version"):], True


def newest(crate):
    """The crate's newest release on crates.io that is not yanked and not a pre-release."""
    versions = fetch(f"https://crates.io/api/v1/crates/{crate}/versions")["versions"]
    stable = [v["num"] for v in versions if not v["yanked"] and STABLE.match(v["num"])]
    if not stable:
        sys.exit(f"crates.io has no stable release of {crate}")
    return max(stable, key=lambda v: tuple(int(p) for p in v.split(".")))


def kernel_packages(pyproject):
    """Each Python package pinned to one release in the project's dependencies: (name, version)."""
    m = re.search(r"^dependencies\s*=\s*\[(?P<list>[^\]]*)\]", pyproject, re.M)
    return [(p["name"], p["version"]) for p in PYPI_PIN.finditer(m["list"])] if m else []


def pin_packages(pyproject, moves):
    for name, (old, new) in moves.items():
        pyproject = pyproject.replace(f'"{name}=={old}"', f'"{name}=={new}"', 1)
    return pyproject


def newest_package(name):
    """The package's newest final release on PyPI that has files and is not yanked."""
    releases = fetch(f"https://pypi.org/pypi/{name}/json")["releases"]
    final = [v for v, files in releases.items()
             if FINAL.match(v) and files and not all(f.get("yanked") for f in files)]
    if not final:
        sys.exit(f"PyPI has no final release of {name}")
    return max(final, key=lambda v: tuple(int(p) for p in v.split(".")))


def has_package(name, version):
    files = fetch(f"https://pypi.org/pypi/{name}/json")["releases"].get(version) or []
    return FINAL.match(version) and files and not all(f.get("yanked") for f in files)


def release(slug, version=None):
    """The tag of one of the repository's published releases, never a branch: the one for the
    version, or its newest. Drafts and pre-releases do not count."""
    if version is None:
        return github(f"repos/{slug}/releases/latest")["tag_name"]
    tags = {r["tag_name"] for r in github(f"repos/{slug}/releases?per_page=100")
            if not r["draft"] and not r["prerelease"]}
    for tag in (f"v{version}", version):
        if tag in tags:
            return tag
    sys.exit(f"{slug} has no published release {version}")


def tag_of(slug, commit):
    """The repository's tag at the commit, or None."""
    for t in github(f"repos/{slug}/tags?per_page=100"):
        if t["commit"]["sha"] == commit:
            return t["name"]
    return None


def move_git(crate, version):
    slug, rev = crate["slug"], crate["rev"]
    old_commit = github(f"repos/{slug}/commits/{rev}")["sha"]
    old_tag = tag_of(slug, old_commit)
    new_tag = release(slug, version)
    new_commit = github(f"repos/{slug}/commits/{new_tag}")["sha"]
    return {"from": old_tag.removeprefix("v") if old_tag else rev,
            "to": new_tag.removeprefix("v"), "repository": f"https://github.com/{slug}",
            "from_commit": old_commit, "to_commit": new_commit,
            "pin": (rev, new_commit[:max(len(rev), 8)], old_tag, new_tag)}


def bump(adapter, version=None):
    """Moves the pins and the reported version; the first kernel crate names the kernel's
    release."""
    moved = {}
    path = os.path.join(adapter, "Cargo.toml")
    if os.path.exists(path):
        manifest = read(path)
        crates = {}
        for c in kernel_crates(manifest):
            if "slug" in c:
                crates[c["crate"]] = move_git(c, version)
            else:
                to = version or newest(c["crate"])
                crates[c["crate"]] = {"from": c["version"], "to": to, "pin": (c["version"], to)}
        write(path, pin(manifest, {n: m.pop("pin") for n, m in crates.items()}))
        moved.update(crates)
    path = os.path.join(adapter, "pyproject.toml")
    if os.path.exists(path):
        pyproject = read(path)
        packages = {}
        for name, old in kernel_packages(pyproject):
            if version and not has_package(name, version):
                sys.exit(f"PyPI has no final release {version} of {name}")
            packages[name] = {"from": old, "to": version or newest_package(name), "pypi": True}
        write(path, pin_packages(pyproject, {n: (p["from"], p["to"]) for n, p in packages.items()}))
        moved.update(packages)
    if not moved:
        sys.exit("the adapter pins no kernel crate or package to a release")
    first = next(iter(moved))
    old, new = moved[first]["from"], moved[first]["to"]
    reported = []
    for dirpath, _, files in os.walk(os.path.join(adapter, "src")):
        for f in sorted(files):
            if f.endswith(".rs"):
                p = os.path.join(dirpath, f)
                text, changed = report(read(p), old, new)
                if changed:
                    write(p, text)
                    reported.append(os.path.relpath(p, adapter))
    return {"crates": moved, "kernel_crate": first, "from": old, "to": new,
            "reported_in": reported}


def outcomes(path):
    data = load(path)
    return data["kernel"], {c["id"]: (c["test"], c["result"]) for c in data["cases"]}


def compare(before, after):
    """A Markdown summary of the two runs: the tallies, then every test whose result changed."""
    (k0, a), (k1, b) = outcomes(before), outcomes(after)
    results = sorted(set(r for _, r in a.values()) | set(r for _, r in b.values()))
    count0 = collections.Counter(r for _, r in a.values())
    count1 = collections.Counter(r for _, r in b.values())
    lines = [f"| Result | {k0['version']} | {k1['version']} |", "| --- | --- | --- |"]
    lines += [f"| {r} | {count0[r]} | {count1[r]} |" for r in results]
    lines += [f"| cases | {len(a)} | {len(b)} |", ""]
    moved = collections.Counter()
    for case in sorted(set(a) | set(b)):
        was = a.get(case, (None, "absent"))
        now = b.get(case, (None, "absent"))
        if was[1] != now[1]:
            moved[(was[0] or now[0], was[1], now[1])] += 1
    if not moved:
        lines.append("No case changed its result.")
        return "\n".join(lines) + "\n"
    lines += ["| Test | Was | Now | Cases |", "| --- | --- | --- | --- |"]
    lines += [f"| `{t}` | {w} | {n} | {c} |" for (t, w, n), c in sorted(moved.items())]
    return "\n".join(lines) + "\n"


def registry(root, kernel, results, commit):
    """Moves the kernel's entry in kernels.json, keeping the file's own formatting."""
    reported = load(results)["kernel"]
    if reported.get("name") != kernel:
        sys.exit(f"the adapter reports {reported.get('name')}, not {kernel}")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        sys.exit(f"{commit} is not a full commit hash")
    path = os.path.join(root, "kernels.json")
    text = read(path)
    pinned = entry(root, kernel)
    start = text.index(f'"kernel": "{kernel}"')
    end = text.index(pinned["adapter"]["commit"], start) + len(pinned["adapter"]["commit"])
    old = text[start:end]
    new = old.replace(f'"version": "{pinned["version"]}"',
                      f'"version": "{reported["version"]}"', 1)
    new = new.replace(pinned["adapter"]["commit"], commit)
    write(path, text[:start] + new + text[end:])
    return reported["version"]


def main():
    args = sys.argv[1:]
    if args[:1] == ["plan"] and len(args) == 3:
        print(json.dumps(plan(args[1], args[2])))
    elif args[:1] == ["bump"] and len(args) in (2, 3):
        print(json.dumps(bump(args[1], args[2] if len(args) == 3 and args[2] else None)))
    elif args[:1] == ["compare"] and len(args) == 3:
        sys.stdout.write(compare(args[1], args[2]))
    elif args[:1] == ["registry"] and len(args) == 5:
        print(registry(*args[1:]))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()

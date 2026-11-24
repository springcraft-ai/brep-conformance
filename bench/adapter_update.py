"""The Adapter update workflow's steps: finds the release an adapter the suite maintains can move
to, checks that the adapter now pins it, sums up the suite's run on it, and moves the kernel's
registry entry. Reads kernels.json at <root>. Changes no adapter: Claude moves the pins.

  adapter_update.py kernels <root> <kernels>  the kernels to run, as a JSON list: every one whose
                                              adapter the workflows can update for `all` or
                                              nothing, or those named, separated by commas
  adapter_update.py plan <root> <kernel>      fails unless the workflows can update the kernel's
                                              adapter; prints its repository, build and binary
  adapter_update.py target <adapter> [version]
                                              the release the adapter's checkout pins and the one
                                              to move to, the given one or the newest, with each
                                              pin's move and each release's licence
  adapter_update.py check <adapter> <target>  fails unless the checkout pins what the target says
  adapter_update.py licences <old> <new>      each way the kernel licences two checkouts of an
                                              adapter pin differ, one line each; none if alike
  adapter_update.py summary <results>         a Markdown summary of a results file
  adapter_update.py pinned <adapter>          the kernel release an adapter's checkout pins
  adapter_update.py registry <root> <kernel> <version> <commit>
                                              moves the kernel's entry to the release and the
                                              adapter commit; prints whether it changed

A kernel pin is one of the adapter's dependencies, other than brep-protocol, pinned to one release:
in Cargo.toml exactly, `=X.Y.Z`, from crates.io, or by `rev` from a GitHub repository, whose
releases are then its GitHub releases; or in build.sh, as a line `package=X.Y.Z`, from conda-forge.
The first pin names the kernel's release.
"""

import collections
import json
import os
import re
import sys
import urllib.error
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
# A build.sh line that pins a conda-forge package: `kernelpkg=1.2.3`.
CONDA = re.compile(r"^(?P<package>[a-z0-9][a-z0-9_-]*)=(?P<version>\d+\.\d+\.\d+)\s*(#.*)?$")
STABLE = re.compile(r"^\d+\.\d+\.\d+$")
BUILDS = (["cargo build --release --locked"], ["./build.sh"])
# The results that pass a case: the tiers, from the suite's own tolerance to its loosest. A case
# beyond them is `fail`.
ANSWERED = ("1e-7", "1e-6", "1e-5", "1e-3")


def read(path):
    with open(path) as f:
        return f.read()


def write(path, text):
    with open(path, "w") as f:
        f.write(text)


def load(path):
    return json.loads(read(path))


def fetch(url, headers=None):
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, **(headers or {})})
    with urllib.request.urlopen(request, timeout=30) as reply:
        return json.load(reply)


def github(path):
    """GitHub's API, with GITHUB_TOKEN where the environment has one: a runner shares its address,
    and with it the limit GitHub sets on calls that carry no token."""
    token = os.environ.get("GITHUB_TOKEN")
    return fetch(f"https://api.github.com/{path}",
                 {"Authorization": f"Bearer {token}"} if token else None)


def entry(root, kernel):
    for k in load(os.path.join(root, "kernels.json"))["kernels"]:
        if k["kernel"] == kernel:
            return k
    sys.exit(f"adapter_update.py: no kernel {kernel} in kernels.json")


def refusal(adapter):
    """Why the workflows cannot update this adapter, or None."""
    if not adapter["repo"].startswith(OWNER):
        return (f"its adapter is {adapter['repo']}, which the suite does not maintain; its "
                "maintainers move it by pull request")
    if adapter["build"] not in BUILDS:
        return f"its adapter builds with {adapter['build']}, which the workflows do not run"
    if not re.fullmatch(r"\{dir\}/[A-Za-z0-9_./-]+", adapter["run"]) or ".." in adapter["run"]:
        return f"its adapter runs as {adapter['run']}, not a binary in its checkout"
    return None


def plan(root, kernel):
    """The adapter's repository, its build commands and the path of the binary they make inside
    its checkout, or exits when the workflows cannot update it."""
    adapter = entry(root, kernel)["adapter"]
    if why := refusal(adapter):
        sys.exit(f"{kernel}: {why}")
    return {"repo": adapter["repo"], "build": adapter["build"],
            "binary": adapter["run"].removeprefix("{dir}/")}


def kernels(root, asked):
    """The kernels to run: every one the workflows can update, for `all` or nothing, or those
    named, in the registry's order. Exits naming each one asked for that cannot be run."""
    registry = load(os.path.join(root, "kernels.json"))["kernels"]
    asked = asked.strip()
    if asked in ("", "all"):
        return [k["kernel"] for k in registry if refusal(k["adapter"]) is None]
    names = [n.strip() for n in asked.split(",") if n.strip()]
    known = {k["kernel"]: k for k in registry}
    problems = [f"{n}: not in kernels.json" for n in names if n not in known]
    problems += [f"{n}: {why}" for n in names if n in known
                 if (why := refusal(known[n]["adapter"]))]
    if problems:
        sys.exit("\n".join(problems))
    return [k["kernel"] for k in registry if k["kernel"] in names]


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


def conda_packages(script):
    """Each conda-forge package a build script pins, `package=X.Y.Z` on a line of its own, in the
    order it pins them: (package, version). A script that installs nothing from conda-forge pins
    none."""
    if "conda-forge" not in script:
        return []
    return [(m["package"], m["version"]) for line in script.splitlines()
            if (m := CONDA.match(line.strip()))]


def pins(adapter):
    """The kernel's pins in the adapter's checkout, the first naming the kernel's release: each
    {"name", "source"} with "version", or with "slug" and "rev" for one from git."""
    found = []
    path = os.path.join(adapter, "Cargo.toml")
    if os.path.exists(path):
        for c in kernel_crates(read(path)):
            if "slug" in c:
                found.append({"name": c["crate"], "source": "git", "slug": c["slug"],
                              "rev": c["rev"]})
            else:
                found.append({"name": c["crate"], "source": "crates.io",
                              "version": c["version"]})
    path = os.path.join(adapter, "build.sh")
    if os.path.exists(path):
        found += [{"name": p, "source": "conda-forge", "version": v}
                  for p, v in conda_packages(read(path))]
    if not found:
        sys.exit("the adapter pins no kernel crate or package to a release")
    return found


def newest(versions):
    stable = [v for v in versions if STABLE.match(v)]
    return max(stable, key=lambda v: tuple(int(p) for p in v.split("."))) if stable else None


def crate_versions(crate):
    """The crate's releases on crates.io that are not yanked."""
    return [v["num"] for v in fetch(f"https://crates.io/api/v1/crates/{crate}/versions")["versions"]
            if not v["yanked"]]


def crate_licence(crate, version):
    return fetch(f"https://crates.io/api/v1/crates/{crate}/{version}")["version"]["license"]


def conda_files(package):
    """The package's files on conda-forge for linux-64, the platform the workflows build on."""
    return [f for f in fetch(f"https://api.anaconda.org/package/conda-forge/{package}")["files"]
            if f["attrs"].get("subdir") == "linux-64"]


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


def git_licence(slug, commit):
    """The licence GitHub reads in the repository at the commit, or None where it finds no licence
    file."""
    try:
        return github(f"repos/{slug}/license?ref={commit}")["license"]["spdx_id"]
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise


def version_key(version):
    return tuple(int(p) for p in version.split(".")) if STABLE.match(version) else None


def forward(name, old, new, asked):
    """The release to move to: the newest, or the one asked for, never one older than the pin. A
    pin newer than the newest release stays where it is; a release asked for that is older than
    the pin is refused."""
    a, b = version_key(old), version_key(new)
    if a is None or b is None or b >= a:
        return new
    if asked:
        sys.exit(f"{name} pins {old}, newer than {new}: the workflows move a kernel forward only")
    return old


def move(pin, version):
    """Where one pin moves: to the given release, which must be a stable one, or to its newest;
    with the licence each release declares where its source says."""
    name = pin["name"]
    if pin["source"] == "git":
        slug, rev = pin["slug"], pin["rev"]
        old_commit = github(f"repos/{slug}/commits/{rev}")["sha"]
        old_tag = tag_of(slug, old_commit)
        new_tag = release(slug, version)
        old_version = old_tag.removeprefix("v") if old_tag else rev
        if forward(name, old_version, new_tag.removeprefix("v"), version) == old_version:
            new_tag = old_tag
        new_commit = github(f"repos/{slug}/commits/{new_tag}")["sha"]
        return {"source": "git", "repository": f"https://github.com/{slug}",
                "from": old_version,
                "to": new_tag.removeprefix("v"), "from_commit": old_commit,
                "to_commit": new_commit, "licence": {"from": git_licence(slug, old_commit),
                                                     "to": git_licence(slug, new_commit)}}
    if pin["source"] == "crates.io":
        published = crate_versions(name)
        to = version or newest(published)
        if to is None or to not in published or not STABLE.match(to):
            sys.exit(f"crates.io has no stable release {version or ''} of {name}".replace("  ", " "))
        to = forward(name, pin["version"], to, version)
        return {"source": "crates.io", "from": pin["version"], "to": to,
                "licence": {"from": crate_licence(name, pin["version"]),
                            "to": crate_licence(name, to)}}
    files = conda_files(name)
    by_version = {f["version"]: f["attrs"].get("license") for f in files}
    to = version or newest(by_version)
    if to is None or to not in by_version or not STABLE.match(to):
        sys.exit(f"conda-forge has no stable release {version or ''} of {name}".replace("  ", " "))
    to = forward(name, pin["version"], to, version)
    return {"source": "conda-forge", "from": pin["version"], "to": to,
            "licence": {"from": by_version.get(pin["version"]), "to": by_version.get(to)}}


def target(adapter, version=None):
    """The release the checkout pins, the one to move to, each pin's move, and each pin whose
    licence differs between the two releases. Changes nothing."""
    moves = {p["name"]: move(p, version) for p in pins(adapter)}
    first = next(iter(moves.values()))
    changed = [{"name": n, **m["licence"]} for n, m in moves.items()
               if m["licence"]["from"] != m["licence"]["to"]]
    return {"from": first["from"], "to": first["to"], "pins": moves, "licence_changed": changed}


def licence_of(pin):
    """The licence the pinned release declares, where its source says."""
    if pin["source"] == "git":
        return git_licence(pin["slug"], github(f"repos/{pin['slug']}/commits/{pin['rev']}")["sha"])
    if pin["source"] == "crates.io":
        return crate_licence(pin["name"], pin["version"])
    return {f["version"]: f["attrs"].get("license") for f in conda_files(pin["name"])}.get(
        pin["version"])


def licences(old, new):
    """What differs between the kernel licences two checkouts of an adapter pin, one line for
    each; none when each pin's release declares the licence the other's does. A checkout whose
    pins cannot be read, or a pin one has and the other does not, is a difference: a person reads
    those."""
    def read_pins(adapter):
        try:
            return {p["name"]: p for p in pins(adapter)}
        except SystemExit:
            return None

    was, now = read_pins(old), read_pins(new)
    if was is None or now is None:
        return [f"the {'old' if was is None else 'new'} adapter pins no release this can read"]
    lines = [f"{n}: pinned in only one of the two" for n in sorted(set(was) ^ set(now))]
    for name in sorted(set(was) & set(now)):
        a, b = licence_of(was[name]), licence_of(now[name])
        if a is None or b is None:
            lines.append(f"{name}: no licence for {'the old' if a is None else 'the new'} release")
        elif a != b:
            lines.append(f"{name}: {a} → {b}")
    return lines


def check(adapter, wanted):
    """What the checkout pins that the target does not say: one line for each, none when the
    checkout pins each release the target moves to."""
    problems = []
    have = {p["name"]: p for p in pins(adapter)}
    for name, m in wanted["pins"].items():
        p = have.get(name)
        if p is None:
            problems.append(f"{name}: no longer pinned")
        elif m["source"] == "git":
            if p.get("rev") is None or not m["to_commit"].startswith(p["rev"]) \
                    or len(p["rev"]) < 8:
                problems.append(f"{name}: rev {p.get('rev')}, not {m['to_commit'][:8]} or longer")
        elif p.get("version") != m["to"]:
            problems.append(f"{name}: {p.get('version')}, not {m['to']}")
    problems += [f"{name}: pinned now, and not before" for name in have if name not in
                 wanted["pins"]]
    return problems


def summary(path):
    """A Markdown summary of one run: the cases at each result, then each test with a case that
    did not answer, other than one the kernel does not support."""
    data = load(path)
    cases = data["cases"]
    counts = collections.Counter(c["result"] for c in cases)
    lines = [f"{data['kernel']['name']} {data['kernel']['version']}, {len(cases)} cases:", "",
             "| Result | Cases |", "| --- | --- |"]
    lines += [f"| {r} | {counts[r]} |" for r in sorted(counts, key=lambda r: (
        r not in ANSWERED, ANSWERED.index(r) if r in ANSWERED else 0, r))]
    lines.append("")
    short = collections.Counter((c["test"], c["result"]) for c in cases
                                if c["result"] not in ANSWERED and c["result"] != "unsupported")
    if not short:
        lines.append("Every case it supports answered.")
        return "\n".join(lines) + "\n"
    lines += ["| Test | Result | Cases |", "| --- | --- | --- |"]
    lines += [f"| `{t}` | {r} | {n} |" for (t, r), n in sorted(short.items())]
    return "\n".join(lines) + "\n"


def pinned(adapter):
    """The kernel release the adapter's checkout pins, read without building it: the first pin's
    version, or the tag of the GitHub release its rev is."""
    first = pins(adapter)[0]
    if first["source"] != "git":
        return first["version"]
    slug, rev = first["slug"], first["rev"]
    tag = tag_of(slug, github(f"repos/{slug}/commits/{rev}")["sha"])
    if tag is None:
        sys.exit(f"{slug} has no release at {rev}")
    return tag.removeprefix("v")


def registry(root, kernel, version, commit):
    """Moves the kernel's entry in kernels.json to the release and the adapter commit, keeping the
    file's own formatting; returns whether it changed."""
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        sys.exit(f"{commit} is not a full commit hash")
    path = os.path.join(root, "kernels.json")
    text = read(path)
    pinned = entry(root, kernel)
    start = text.index(f'"kernel": "{kernel}"')
    end = text.index(pinned["adapter"]["commit"], start) + len(pinned["adapter"]["commit"])
    old = text[start:end]
    new = old.replace(f'"version": "{pinned["version"]}"', f'"version": "{version}"', 1)
    new = new.replace(pinned["adapter"]["commit"], commit)
    write(path, text[:start] + new + text[end:])
    return new != old


def main():
    args = sys.argv[1:]
    if args[:1] == ["kernels"] and len(args) in (2, 3):
        print(json.dumps(kernels(args[1], args[2] if len(args) == 3 else "")))
    elif args[:1] == ["plan"] and len(args) == 3:
        print(json.dumps(plan(args[1], args[2])))
    elif args[:1] == ["target"] and len(args) in (2, 3):
        print(json.dumps(target(args[1], args[2] if len(args) == 3 and args[2] else None)))
    elif args[:1] == ["check"] and len(args) == 3:
        if problems := check(args[1], load(args[2])):
            sys.exit("\n".join(problems))
    elif args[:1] == ["licences"] and len(args) == 3:
        for line in licences(args[1], args[2]):
            print(line)
    elif args[:1] == ["summary"] and len(args) == 2:
        sys.stdout.write(summary(args[1]))
    elif args[:1] == ["pinned"] and len(args) == 2:
        print(pinned(args[1]))
    elif args[:1] == ["registry"] and len(args) == 5:
        print("moved" if registry(*args[1:]) else "same")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()

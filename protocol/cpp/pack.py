# SPDX-License-Identifier: MIT
"""Packs the C++ binding into brep_protocol-<version>.tar.xz, the release asset adapters fetch, and
prints its path and SHA-256. The same files give the same bytes on one Python and xz library.

usage: python3 pack.py <output folder>"""

import hashlib
import io
import lzma
import pathlib
import re
import sys
import tarfile

HERE = pathlib.Path(__file__).resolve().parent
# What an adapter needs, and the example. The tests read doc/api.md, so they stay in the repository.
FILES = {
    "CMakeLists.txt": HERE / "CMakeLists.txt",
    "README.md": HERE / "README.md",
    "LICENSE": HERE.parent.parent / "LICENSE",
    "include/brep_protocol/brep_protocol.h": HERE / "include/brep_protocol/brep_protocol.h",
    "examples/box_kernel.cpp": HERE / "examples/box_kernel.cpp",
}


# A release of the protocol, brep-protocol@<version>, carries every binding at one version: the Rust
# crate's, which the C++ binding's must be.
def version():
    project = re.search(r"project\(brep_protocol VERSION (\d+\.\d+\.\d+)",
                        (HERE / "CMakeLists.txt").read_text())
    crate = re.search(r'^version = "(\d+\.\d+\.\d+)"',
                      (HERE.parent / "Cargo.toml").read_text(), re.MULTILINE)
    if project is None or crate is None:
        sys.exit("pack.py: no version in CMakeLists.txt or in ../Cargo.toml")
    if project.group(1) != crate.group(1):
        sys.exit(f"pack.py: the C++ binding is at {project.group(1)}, but the protocol's release, "
                 f"the brep-protocol crate, is at {crate.group(1)}")
    return project.group(1)


def main():
    out = pathlib.Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    v = version()
    root = f"brep_protocol-{v}"
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.PAX_FORMAT) as tar:
        folders = sorted({str(f) for n in FILES for f in pathlib.PurePosixPath(n).parents} - {"."})
        for name in [root] + [f"{root}/{f}" for f in folders]:
            info = tarfile.TarInfo(name)
            info.type, info.mode = tarfile.DIRTYPE, 0o755
            tar.addfile(info)
        for name in sorted(FILES):
            data = FILES[name].read_bytes()
            info = tarfile.TarInfo(f"{root}/{name}")
            info.size, info.mode = len(data), 0o644
            tar.addfile(info, io.BytesIO(data))
    path = out / f"{root}.tar.xz"
    path.write_bytes(lzma.compress(raw.getvalue(), preset=9))
    print(path)
    print(hashlib.sha256(path.read_bytes()).hexdigest())


if __name__ == "__main__":
    main()

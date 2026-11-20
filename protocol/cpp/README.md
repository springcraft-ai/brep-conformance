# brep_protocol for C++

The C++ binding of the [brep-conformance](https://github.com/springcraft-ai/brep-conformance)
protocol: the B-rep operations the suite calls, as C++ types, a `Kernel` class, and `serve`, which
answers the suite's line protocol with any implementation of it. It is the C++ counterpart of the
Rust crate [`brep-protocol`](../README.md), for an adapter whose kernel has a C++ API.

## Overview

The suite starts an adapter as a separate process and sends it one JSON request per line on stdin.
The adapter writes one JSON reply per line on stdout.
[doc/api.md](https://github.com/springcraft-ai/brep-conformance/blob/main/doc/api.md) defines every
function, value and message.

[`brep_protocol.h`](include/brep_protocol/brep_protocol.h) is the whole binding, in one header:

- `Primitive`, `Profile`, `Frame` and the other types hold the values a request carries.
- `Kernel<Solid>` is the class an adapter derives from, with one virtual method per function of the
  protocol, and `supports`, which lists the functions the kernel offers. A method that is not pure
  virtual refuses the call as `unsupported` unless the adapter overrides it, and a function is
  offered only when `supports` lists it. `Solid` must be movable: `serve` moves each solid a method
  returns into its store. The methods that describe the kernel, such as `name` and `tolerance`, are
  `const`; the operations are not, so a kernel may keep state of its own between calls.
- `serve` answers the protocol on stdin and stdout with a `Kernel`. It holds the solids by handle. A
  kernel that prints to the standard output corrupts the replies: its adapter sends that output
  elsewhere, or passes `serve` a stream of its own for the replies.

A method declines a request by throwing a `Refusal`: `Refusal::unsupported` where the kernel has no
call for the function or for these arguments, `Refusal::invalid` where the request is at fault, and
`Refusal::failed` where the request is valid and the kernel could not carry it out. Any other
exception is a crash, and the worker carries on. An adapter whose kernel throws its own error class
overrides `refusal_for` to read it as a refusal.

The binding's one dependency is [nlohmann/json](https://github.com/nlohmann/json), under the MIT
licence. The binding itself is under the MIT licence, which covers it and not a kernel that an
adapter drives.

## Usage

Each release of the protocol, `brep-protocol@<version>`, carries the C++ binding at that version as
an archive: the header, its CMake file, the licence and the example, and nothing else from the
repository. An adapter's CMake build fetches it by URL and pins it by the hash the release's notes
give:

```cmake
include(FetchContent)
FetchContent_Declare(
    brep_protocol
    URL https://github.com/springcraft-ai/brep-conformance/releases/download/brep-protocol-v1.0.0/brep_protocol-1.0.0.tar.xz
    URL_HASH SHA256=<the release's hash>
)
FetchContent_MakeAvailable(brep_protocol)
target_link_libraries(my-adapter PRIVATE brep_protocol::brep_protocol)
```

A build with another tool downloads the same archive.

It uses an installed nlohmann/json 3.11 or later where CMake finds one, and otherwise fetches the
release by its hash. A build without CMake needs the archive's `include` folder and nlohmann/json
3.11 or later on the include path, and C++17.

An adapter derives from `Kernel` with one call of its kernel per method, and calls `serve` from
`main`:

```cpp
#include <brep_protocol/brep_protocol.h>

using namespace brep_protocol;

class Adapter : public Kernel<my_kernel::Solid> {
  public:
    std::string name() const override { return "my-kernel"; }
    std::string version() const override { return my_kernel::VERSION; }
    double tolerance() const override { return 1e-7; }
    Solid make(const Primitive& p) override;
    Solid unite(const Solid& a, const Solid& b) override;
    Solid intersection(const Solid& a, const Solid& b) override;
    Solid difference(const Solid& a, const Solid& b) override;
    double volume(const Solid& s) override;
    double area(const Solid& s) override;
    std::vector<std::string> check(const Solid& s) override;
};

int main() {
    Adapter adapter;
    return serve(adapter);
}
```

`union` is a keyword in C++, so the protocol's `union` is the method `unite`.

[`examples/box_kernel.cpp`](examples/box_kernel.cpp) is a complete adapter for a kernel that makes
boxes and nothing else, as the Rust crate's example is. To build it and its tests, and run the suite
on it, from this folder:

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
ctest --test-dir build
cd ../.. && cargo run --release -p brep-conformance -- --adapter protocol/cpp/build/box_kernel
```

The suite gives it the same outcome on every case as the Rust example.

## Releases

There is one release per version of the protocol's bindings, `brep-protocol@<version>`: the Rust
crate at that version on crates.io, and the C++ binding at that version attached to the release. The
C++ binding's version is the `VERSION` in [`CMakeLists.txt`](CMakeLists.txt), and
[`pack.py`](pack.py) refuses to pack it unless it is the crate's version in
[`../Cargo.toml`](../Cargo.toml).

To attach the C++ binding to a release, from the release's commit:

```bash
python3 pack.py dist                     # prints the archive's path and its SHA-256
./check-package.sh dist/brep_protocol-<version>.tar.xz <sha-256>
gh release upload brep-protocol-v<version> dist/brep_protocol-<version>.tar.xz
```

Then add the hash to the release's notes, with the `FetchContent_Declare` above. Every pull request
packs and checks the archive the same way, in the `cpp-binding` job.

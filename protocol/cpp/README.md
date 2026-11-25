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
licence. The binding itself is under the MIT licence or the Apache License, Version 2.0, at your
option, which covers it and not a kernel that an adapter drives.

## Usage

Each release of the C++ binding, `brep-protocol-cpp-v<version>`, carries the binding as an archive:
the header, its CMake file, the licence and the example, and nothing else from the repository. An
adapter's CMake build fetches it by URL and pins it by the hash the release's notes give:

```cmake
include(FetchContent)
FetchContent_Declare(
    brep_protocol
    URL https://github.com/springcraft-ai/brep-conformance/releases/download/brep-protocol-cpp-v2.0.0/brep_protocol-2.0.0.tar.xz
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
cd ../.. && cargo run --release -p brep-conformance -- --adapter protocol/cpp/build/box_kernel --skip-nist
```

The suite gives it the same outcome on every case as the Rust example.

The two bindings must read every request and write every reply alike.
[`../parity/compare.py`](../parity/compare.py) checks this: it sends each request of
[`../parity/requests.jsonl`](../parity/requests.jsonl) to an echo kernel in each language, which
answers with the arguments it was handed, and fails on any reply that differs, or on a function
of doc/api.md that no request has both answer. A protocol change updates both bindings and the
corpus together. From the repository's root, after the build above:

```bash
cargo build --release -p brep-protocol --example echo_kernel
python3 protocol/parity/compare.py target/release/examples/echo_kernel \
    protocol/cpp/build/echo_kernel
```

## Releases

The C++ binding is released on its own, apart from the Rust crate and apart from the protocol. Its
version is the `VERSION` in [`CMakeLists.txt`](CMakeLists.txt), and each release is tagged
`brep-protocol-cpp-v<version>`. Version 1.0.0 is the one exception: its archive is attached to the
Rust crate's release, `brep-protocol-v1.0.0`.

The version follows the C++ API of [`brep_protocol.h`](include/brep_protocol/brep_protocol.h):

- **Major:** a change that stops an adapter from compiling, or changes what it does: a member
  removed or renamed, a signature changed, or a pure virtual function added. A new major version of
  the protocol also raises it.
- **Minor:** an addition that compiles with every adapter, such as a virtual function with a default
  body. A new minor version of the protocol also raises it.
- **Patch:** a fix that changes no API and no message.

The protocol version that the binding speaks is `PROTOCOL_VERSION` in the header, a separate
number. A release's notes give both numbers. No tool checks the C++ API, so a reviewer checks each
change to the header against the table above.

To release the C++ binding, from the release's commit:

```bash
python3 pack.py dist                     # prints the archive's path and its SHA-256
./check-package.sh dist/brep_protocol-<version>.tar.xz <sha-256>
gh release create brep-protocol-cpp-v<version> dist/brep_protocol-<version>.tar.xz
```

Then add the hash to the release's notes, with the `FetchContent_Declare` above. Every pull request
packs and checks the archive the same way, in the `cpp-binding` job.

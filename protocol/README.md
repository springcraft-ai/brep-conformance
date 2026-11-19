# brep-protocol

The B-rep operations that the [brep-conformance](https://github.com/springcraft-ai/brep-conformance)
suite calls, as Rust types, a `Kernel` trait, and a binding to the suite's line protocol.

## Overview

The suite does not link a kernel. It starts the kernel's adapter as a separate process and sends it
one JSON request per line on stdin. The adapter writes one JSON reply per line on stdout.
[doc/api.md](https://github.com/springcraft-ai/brep-conformance/blob/main/doc/api.md) defines every
function, value and message of the protocol.

This crate is the Rust binding of that protocol:

- `model`, `profile` and `path` hold the values a request carries: frames, primitives,
  profiles and paths.
- `Kernel` is the trait an adapter implements, with one method per function of the protocol. Ten
  methods are required. Every other method has a default that refuses the call as `unsupported`.
- `serve` answers the protocol on stdin and stdout with any implementation of `Kernel`. It holds the
  solids by handle, and it replies `crash` when a method panics.
- `Remote` is an implementation of `Kernel` that sends each call to worker processes that speak the
  protocol. The suite's runner uses it to drive an adapter.

An adapter in another language does not need this crate. The protocol is lines of JSON. An adapter
in C++ can use the C++ binding in [`cpp/`](cpp/README.md), which mirrors this crate.

The crate depends on no kernel. Its dependencies are `serde_json`, and `libc` on Unix.

## Versions

`PROTOCOL_VERSION` is the version of the protocol that this crate speaks, and `hello` reports it.
A client and an adapter work together when their major versions agree; `speaks` makes that test.
The crate's own version is separate from the protocol's version.

## Usage

```toml
[dependencies]
brep-protocol = "1"
```

To add a kernel to the suite, follow
[Add your kernel](https://github.com/springcraft-ai/brep-conformance/blob/main/doc/overview.md#add-your-kernel).
It is built around
[`examples/box_kernel.rs`](https://github.com/springcraft-ai/brep-conformance/blob/main/protocol/examples/box_kernel.rs),
a complete adapter for a kernel that makes only boxes, which the suite runs and scores end to end.

An adapter implements `Kernel` with one call of its kernel per method, and calls `serve` from
`main`:

```rust,ignore
use brep_protocol::model::Primitive;
use brep_protocol::{serve, Kernel, Refusal};

struct Adapter;

impl Kernel for Adapter {
    type Solid = my_kernel::Solid;
    fn name(&self) -> String { "my-kernel".into() }
    fn version(&self) -> String { my_kernel::VERSION.into() }
    fn tolerance(&self) -> f64 { 1e-7 }
    fn make(&self, p: &Primitive) -> Result<Self::Solid, Refusal> { todo!() }
    fn union(&self, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, Refusal> { todo!() }
    fn intersection(&self, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, Refusal> { todo!() }
    fn difference(&self, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, Refusal> { todo!() }
    fn volume(&self, s: &Self::Solid) -> Result<f64, Refusal> { todo!() }
    fn area(&self, s: &Self::Solid) -> Result<f64, Refusal> { todo!() }
    fn check(&self, s: &Self::Solid) -> Result<Vec<String>, Refusal> { todo!() }
}

fn main() -> std::io::Result<()> {
    serve(&Adapter)
}
```

A method returns `Refusal::unsupported` when the kernel has no call for the function or for these
arguments. It returns `Refusal::invalid` when the request is at fault, for example when it names an
edge that is not there. It returns `Refusal::failed` when the request is valid and the kernel could
not carry it out.
[What an adapter may do](https://github.com/springcraft-ai/brep-conformance/blob/main/doc/contract.md#what-an-adapter-may-do)
gives the rule that an adapter in an official run must follow.

A client drives an adapter through `Remote`:

```rust,ignore
use std::time::Duration;
use brep_protocol::Remote;

let kernel = Remote::new(vec!["path/to/adapter".into()], 4, Duration::from_secs(60))?;
```

## Licence

MIT. The licence covers this crate and not a kernel that an adapter drives.

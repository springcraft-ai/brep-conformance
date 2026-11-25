# Introduction

## What this is

brep-conformance is a test suite and a benchmark for exact B-rep kernels: the geometry engines under
CAD tools, which build solids from exact curves and surfaces and compute on them.

It defines a set of functions a kernel offers, such as making a box, cutting one solid from another,
rounding an edge or measuring a volume. It defines each function's result as a set of points, and it
checks a kernel's answers against that definition. It scores every kernel the same way and publishes
the results.

It is for people who build kernels, and for people who choose one.

## How it works

- **The suite draws cases.** A case is a few calls: make two solids, combine them, measure the
  result. Each case belongs to a test, such as "booleans of crossing cylinders".
- **An adapter passes each call to the kernel.** An adapter is a small program that answers the
  suite's calls with the kernel's own functions. It adds no geometry of its own.
- **The suite checks the answers.** No expected value comes from a kernel. Volumes, areas and
  moments come from closed forms. Where there is none, the suite tests which points lie inside the
  exact solid itself and compares.
- **Each case gets a result.** A case passes at the tightest tolerance at which all its checks hold:
  1e-7, 1e-6, 1e-5 or 1e-3 mm. Otherwise it fails, or the kernel refused it, does not support it,
  timed out or crashed.

A kernel's headline number is the share of all cases it passes at 1e-7 mm.

## What it covers

| Area             | Functions                                                          |
| ---------------- | ------------------------------------------------------------------ |
| Constructors     | Boxes, cylinders, extrusions, revolutions, lofts and sweeps        |
| Booleans         | Union, intersection and difference, of two solids or many          |
| Blends           | Fillets and chamfers, of one size or several, and full rounds      |
| Changing a solid | Offsets, shells, drafts, and moving, deleting and thickening faces |
| Exchange         | Reading and writing STEP, and sewing loose faces                   |
| Queries          | Volume, area, moments, distance, sections, meshes, rays and more   |

It covers kernels that compute exact geometry. Mesh kernels are a different class of tool, and this
suite does not test them.

## Add your kernel

An adapter is one program that reads a call on a line of input and writes the reply on a line of
output. [Calling a kernel](api.md#calling-a-kernel) is the whole of what it must do.

1. **Run the example.**
   [`box_kernel.rs`](https://github.com/springcraft-ai/brep-conformance/blob/main/protocol/examples/box_kernel.rs)
   is a complete adapter for a kernel that makes boxes and nothing else, in about a hundred lines of
   Rust. From the repository's root:

   ```bash
   cargo build --release --example box_kernel -p brep-protocol
   cargo run --release -p brep-conformance -- --adapter target/release/examples/box_kernel --skip-nist
   ```

   The summary says how many cases call only functions the adapter offers, and how many of those it
   passes: every one, for these boxes. The suite runs only those cases, and scores the rest
   unsupported.

2. **Put your kernel's calls in.** Each method of the `Kernel` trait in
   [`brep-protocol`](https://github.com/springcraft-ai/brep-conformance/tree/main/protocol) is one
   function of [the API](api.md#constructors), and its body is your kernel's own call for it. A
   kernel with a C++ API can use the
   [C++ binding](https://github.com/springcraft-ai/brep-conformance/tree/main/protocol/cpp), whose
   `Kernel` class has the same methods and whose example is the same box kernel. An adapter in
   another language answers the same lines of JSON without either.

3. **List what your kernel offers.** `supports` lists the functions your kernel has one call for;
   `hello` reports them, and the suite runs no case that needs another. A function you do not list
   is not offered, whatever its method does. Never build a function out of others:
   [the adapter rule](contract.md#what-an-adapter-may-do) says why. [Refusals](api.md#refusals)
   says what to answer for an input you cannot build.

4. **Run the suite, and read the report.** `--html report.html` writes a page with every case, its
   checks and the shapes it drew. `--family` runs one area of the suite while you work on it.

5. **Ask to be scored.** An official run uses an adapter at a commit the suite's maintainers have
   reviewed against the adapter rule, listed in [the registry](process.md#the-registry). Open an
   issue with your adapter's repository; once it is reviewed, a pull request adds it.

## Where to look

| To find                                        | Read                                        |
| ---------------------------------------------- | ------------------------------------------- |
| What a function must return                    | [API](api.md#calling-a-kernel), by function |
| How a value is written                         | [Types](api.md#types)                       |
| How an answer is judged, and at what tolerance | [The contract](contract.md)                 |
| What a results file holds                      | [Result files](formats.md)                  |
| How a function enters the suite                | [Process](process.md)                       |

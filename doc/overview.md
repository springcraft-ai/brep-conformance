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

1. Write an adapter: one program that reads a call on a line of input and writes the reply on a line
   of output. [Calling a kernel](api.md#calling-a-kernel) is the whole of what it must do.
2. Pass each call to your kernel's own function, and list the functions it does not have as
   unsupported. [Refusals](api.md#refusals) says what to do with an input you cannot build.
3. Run the suite against it, and read the report.

## Where to look

| To find                                        | Read                                        |
| ---------------------------------------------- | ------------------------------------------- |
| What a function must return                    | [API](api.md#calling-a-kernel), by function |
| How a value is written                         | [Types](api.md#types)                       |
| How an answer is judged, and at what tolerance | [The contract](contract.md)                 |
| What a results file holds                      | [Result files](formats.md)                  |
| How a function enters the suite                | [Process](process.md)                       |

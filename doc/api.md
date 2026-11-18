# API

This document defines the functions a B-rep kernel offers the suite, and says how a client calls
them. Each function has a signature, a statement of what it requires, and a definition of the solid
or value it returns, written as a set of points in our own words. A kernel is judged against these
definitions alone, never against another kernel or an outside standard. [contract.md](contract.md)
says how results are checked.

Each group of functions opens with a table of its commands. The definitions follow the table.

## Calling a kernel

A kernel takes part by shipping an **adapter**, a program that answers these calls with the kernel's
own functions. The suite runs the adapter as one or more **worker** processes, so the client never
links the kernel. This is version 1.0.0 of the protocol.

### Lifecycle

1. The client starts the worker from a command line, with stdin and stdout as pipes. The worker
   keeps its stderr for its own logs, and the client does not read it.
2. The client's first request is `hello`. The client compares the reply's `protocol` with the
   version the client speaks. It stops when the two versions do not agree in the way
   [Versions](#versions) defines.
3. The client sends one request, waits for its reply, then sends the next. Each worker has one
   request in flight at a time, and replies come in request order.
4. When stdin closes, the worker exits with status 0.

A client may run several workers at once, typically one per thread. Each worker holds its own
solids, and a handle means something only to the worker that issued it.

### Requests and replies

Every message is one JSON object on one line, encoded as UTF-8 and ended by a newline.

A request calls one function. It has the form `{"cmd": name, …}`, with each of the function's
parameters as a field of the same name. A value is written in the format that the Format column of
[Types](#types) gives for its type: a `Point3` is `[x, y, z]`, a `Frame` is
`{"origin": …, "x": …, "y": …, "z": …}`, and a `Solid` is its handle.

A reply is exactly one of:

| Reply                        | Meaning                                                                 |
| ---------------------------- | ----------------------------------------------------------------------- |
| `{"ok": value}`              | The call completed; `value` is what the function returns                |
| `{"err": reason, "kind": k}` | A **refusal**: the kernel declines with a reason; the worker carries on |
| `{"crash": message}`         | A **crash**: the kernel failed unexpectedly, and the worker recovered   |

- **Refusals.** [Refusals](#refusals) says when a function must refuse and how a refusal is scored.
- **Crashes.** A worker that writes no reply, or exits, has crashed. The client starts a fresh
  worker for its next request, and every handle of the old worker is gone.
- **Time limit.** Every call has a time limit, which is 120 s unless the run sets another with
  `--timeout`. A worker that has not replied within the limit has timed out. The client kills that
  worker and starts a fresh one.
- **Handles.** A `Solid` travels as a handle. A handle is a positive integer that the worker issues
  when a function returns a solid. The solid itself stays in the worker.
- **Repeated handles.** A handle may appear more than once in one request. `union`, `intersection`
  and `difference` may take the same handle as `a` and `b`, and the adapter then passes that solid
  to the kernel as both operands. The adapter never copies the solid to get round a kernel that
  refuses the same solid as both operands. Such a kernel refuses, and the case scores refused.
- **Numbers.** A writer gives each double in a form that reads back exactly. NaN and infinities are
  not JSON numbers and never appear in a request.
- **No answer.** In a reply, `null` where a value belongs means there is no value. `normal` gives
  `null` at a crease, and `centroid` gives `null` for an empty solid. A function the kernel lacks is
  refused as `unsupported` and is never answered with `null`.

### The session

Two commands are not functions of a kernel:

| Command | Fields | Returns                                                         |
| ------- | ------ | --------------------------------------------------------------- |
| `hello` |        | `{name, version, tolerance, configuration, protocol, supports}` |
| `reset` |        | `null`                                                          |

- `hello` gives the kernel's `name` and `version`. `tolerance` is the linear tolerance declared for
  the kernel, in mm. `configuration` is a sentence on anything the adapter set differently from the
  kernel's defaults, and it is empty when the adapter set nothing differently. `protocol` is the
  protocol version the adapter speaks. `supports` lists the names of the functions the kernel
  offers, out of the functions of that protocol version. A case that needs a function that
  `supports` leaves out is not run. A function added in a later version is not in the list of any
  older adapter, so an adapter is never taken to offer a function that was added after the adapter
  was written. [contract.md](contract.md#tolerance) says what tolerance an adapter sets and
  declares.
- `reset` releases every solid. No handle issued before a `reset` is valid after it.

The session below makes a box and a cylinder, intersects them and measures the result. `>` marks a
request and `<` marks its reply.

```text
> {"cmd": "hello"}
< {"ok": {"name": "example-kernel", "version": "1.2.0", "tolerance": 1e-7,
   "configuration": "", "protocol": "1.0.0",
   "supports": ["box", "cylinder", "intersection", "volume", "check"]}}
> {"cmd": "box", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "size": [10, 10, 10]}
< {"ok": 1}
> {"cmd": "cylinder", "frame": {"origin": [5, 5, -1], "x": [1, 0, 0], "y": [0, 1, 0],
   "z": [0, 0, 1]}, "radius": 2, "height": 12}
< {"ok": 2}
> {"cmd": "intersection", "a": 1, "b": 2}
< {"ok": 3}
> {"cmd": "volume", "s": 3}
< {"ok": 125.66370614359172}
> {"cmd": "check", "s": 3}
< {"ok": []}
```

Requests are wrapped here to fit the page. On the wire each request is one line.

### Versions

A version is three numbers, `major.minor.patch`. Adding a function or field without changing what
the existing ones mean raises the minor number. A correction to this text that changes no message
raises the patch number. Any other change raises the major number.

A client and an adapter work together when their major versions agree. A client stops when an
adapter's major version differs from the client's own.

An adapter ignores fields it does not know. A command it does not know is refused as `unsupported`.

### Implementations

The crate [`brep-protocol`](../protocol/) is the Rust binding. It holds four things: the values as
types; a `Kernel` trait with a method per function; `serve`, which answers the protocol with any
implementation of the trait; and `Remote`, an implementation of the trait that forwards to workers.
An adapter need not use the crate, because the protocol is lines of JSON, which a program in any
language can answer.

## Types

| Type        | Format                                                       | Meaning                                                     |
| ----------- | ------------------------------------------------------------ | ----------------------------------------------------------- |
| `Number`    | `double`                                                     | Unitless                                                    |
| `Length`    | `double`                                                     | mm                                                          |
| `Angle`     | `double`                                                     | rad                                                         |
| `Area`      | `double`                                                     | mm²                                                         |
| `Volume`    | `double`                                                     | mm³                                                         |
| `Count`     | `integer`                                                    | A number of things                                          |
| `Index`     | `integer`                                                    | A position in a list, from 0                                |
| `Point2`    | `double[2]`                                                  | `[x, y]`, mm                                                |
| `Point3`    | `double[3]`                                                  | `[x, y, z]`, mm                                             |
| `Vector3`   | `double[3]`                                                  | A direction `[x, y, z]`. Nonzero; any length unless stated  |
| `Frame`     | `{origin: Point3, x: Vector3, y: Vector3, z: Vector3}`       | Right-handed orthonormal axes at `origin`, `z = x × y`      |
| `Placement` | `double[3][4]`                                               | The rigid motion `p ↦ R·p + t`, rows `[R \| t]`, `t` in mm  |
| `Loop`      | `list<Segment>`                                              | Segments end to end, closed; or one `spline`                |
| `Profile`   | `list<Loop>`                                                 | The outer loop, counter-clockwise, then holes, clockwise    |
| `Path`      | `list<PathPiece>`                                            | Runs and bends end to end, each leaving as the last arrived |
| `Spline3`   | `{degree: integer, knots: list<Number>, ctrl: list<Point3>}` | A clamped B-spline curve in space                           |
| `Solid`     | `integer`                                                    | A handle, 1 or more, to a solid set the kernel holds        |

A `Segment` is one of:

| Segment  | Format                                                                  | Meaning                                              |
| -------- | ----------------------------------------------------------------------- | ---------------------------------------------------- |
| `line`   | `{line: Point2[2]}`                                                     | The straight line from the first point to the second |
| `arc`    | `{arc: {center: Point2, radius: Length, start: Angle, sweep: Angle}}`   | A circular arc from angle `start` through `sweep`    |
| `conic`  | `{conic: {a: Point2, apex: Point2, b: Point2, w: Number}}`              | A conic arc from `a` to `b`, defined below           |
| `gn`     | `{gn: {a: Point2, apex: Point2, b: Point2, order: integer, w: Number}}` | A blend section from `a` to `b`, defined below       |
| `spline` | `{spline: {degree: integer, knots: list<Number>, ctrl: list<Point2>}}`  | A periodic B-spline, the whole of its loop           |

A `PathPiece` is one of:

| Piece  | Format                                  | Meaning                       |
| ------ | --------------------------------------- | ----------------------------- |
| `line` | `{line: Length}`                        | A straight run of that length |
| `arc`  | `{arc: {radius: Length, angle: Angle}}` | A bend of `angle` at `radius` |

A `spline` segment's `ctrl` lists the `n` points of one period, followed by the first `degree` of
those points again. Its `knots` hold `n + 2·degree + 1` values, which run `degree` values past each
end of the period. One period is the parameter range `knots[degree] ..= knots[n + degree]`.

A `double` is a finite IEEE 754 binary64 value, and an `integer` is a whole number from 0 to 2⁵³.
`5` and `5.0` are the same value. A `string` is UTF-8 text. `T[n]` is exactly `n` values, `T[n][m]`
is `n` rows of `m` values, and `list<T>` is any number of values. A value of the wrong format, such
as a `Point3` of two numbers, is a malformed request and not an input a kernel refuses.

A matrix is a list of its rows, and each row is a list of numbers. The matrix multiplies a point
written as a column: `p ↦ M·p + t`. A `Number[3][4]` holds `[M | t]`. Each row holds three numbers
of `M` and then that row's part of `t`:

```text
[[m11, m12, m13, tx],        x' = m11·x + m12·y + m13·z + tx
 [m21, m22, m23, ty],        y' = m21·x + m22·y + m23·z + ty
 [m31, m32, m33, tz]]        z' = m31·x + m32·y + m33·z + tz
```

A **solid set** is a bounded set of points that is equal to the closure of its interior. It may be
empty, or it may be several separate pieces. Pieces may touch at points or along curves, but they
never share an area of boundary, because sharing an area would make them one.

A frame's **local coordinates** `(u, v, w)` name the point `o + u·x + v·y + w·z`. Every constructor
below defines its solid in local coordinates, and the constructor's frame places the solid.

A **conic arc** runs from `a` to `b`. At `a` and at `b` it is tangent to the line from that point to
the arc's corner, `apex`. The arc is the rational quadratic
`(t̄²a + 2w·t·t̄·apex + t²b) / (t̄² + 2w·t·t̄ + t²)`, where `t̄ = 1 − t`, over `t` from 0 to 1. Its
shoulder is the point at `t = ½`. The shoulder lies `rho = w/(1 + w)` of the way from the chord's
middle to the corner. The conic is an ellipse for `w < 1`, a parabola when `w` is 1 and a hyperbola
when `w` is above 1. A circular arc turning `θ` is the conic arc with `w = cos(θ/2)`.

A **blend section** of order `n` and thumbweight `w` runs from `a` to `b` between its two legs,
which are the lines from `apex` to `a` and from `apex` to `b`. The section is the curve
`apex + α(t)(a − apex) + β(t)(b − apex)`, with `α = f·(1 − p₁)`, `β = (1 − f)·p₂`,
`f = t̄ⁿ⁺¹/(t̄ⁿ⁺¹ + tⁿ⁺¹)`, `p₁ = t/(1 − w + wt)` and `p₂ = t(1 − w)/(1 − wt)`. The section meets each
leg with contact of order `n`. At `n = 2` the section starts as straight as the leg, so the section
is curvature-continuous with the leg. Its **equal-area thumbweight** is the `w` at which the section
removes from the triangle on its legs the same area that the circle tangent to both legs at `a` and
`b` removes. "Thumbweight" is the name this shape parameter has in parametric Gⁿ blending (E.
Hartmann, "Parametric Gⁿ blending of curves and surfaces", The Visual Computer 17, 2001, pp. 1–13).

A profile's **region** `Ω` is the set of points about which the winding numbers of the profile's
loops sum to a nonzero value, together with the boundary of that set. No loop may cross another loop
or itself. `A` is the area of `Ω`, `M` is the first moment `∫∫ u dA` of `Ω`, and `L` is the length
of the boundary of `Ω`.

Every function may **refuse**. To refuse is to decline with a reason instead of returning a result,
for input that the function does not support or cannot build correctly. A function never returns a
result it knows to be wrong.

## Constructors

| Function                        | Fields                                           | Returns |
| ------------------------------- | ------------------------------------------------ | ------- |
| [`box`](#box)                   | `frame`, `size`                                  | `Solid` |
| [`cylinder`](#cylinder)         | `frame`, `radius`, `height`                      | `Solid` |
| [`sphere`](#sphere)             | `frame`, `radius`                                | `Solid` |
| [`torus`](#torus)               | `frame`, `major`, `minor`                        | `Solid` |
| [`cone`](#cone)                 | `frame`, `bottom`, `top`, `height`               | `Solid` |
| [`extrude`](#extrude)           | `frame`, `profile`, `height`                     | `Solid` |
| [`revolve`](#revolve)           | `frame`, `profile`, `angle`                      | `Solid` |
| [`screw`](#screw)               | `frame`, `profile`, `rise`, `angle`              | `Solid` |
| [`loft`](#loft)                 | `frame`, `profile`, `scale`, `height`, `top`     | `Solid` |
| [`loft_through`](#loft_through) | `frame`, `profiles`, `heights`                   | `Solid` |
| [`sweep`](#sweep)               | `frame`, `profile`, `path`                       | `Solid` |
| [`sweep_path`](#sweep_path)     | `frame`, `profile`, `path`, `twist`              | `Solid` |
| [`slab`](#slab)                 | `frame`, `size`, `knots_u`, `knots_v`, `heights` | `Solid` |

### box

`box(frame: Frame, size: Length[3]) -> Solid`

Makes a rectangular block with one corner at the frame's origin and its edges along the frame's
axes, not centred on the origin.

Each of `size = (a, b, c)` must be positive. The result is the set of points where `0 ≤ u ≤ a`,
`0 ≤ v ≤ b` and `0 ≤ w ≤ c`.

**Example.**

```text
> {"cmd": "box", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "size": [10, 20, 30]}
< {"ok": 1}
```

### cylinder

`cylinder(frame: Frame, radius: Length, height: Length) -> Solid`

Makes a round bar standing on the frame's xy plane. Its axis runs up z from the origin.

`radius` and `height` must both be positive. The result is the set of points where
`u² + v² ≤ radius²` and `0 ≤ w ≤ height`.

**Example.**

```text
> {"cmd": "cylinder", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0],
   "z": [0, 0, 1]}, "radius": 5, "height": 20}
< {"ok": 1}
```

### sphere

`sphere(frame: Frame, radius: Length) -> Solid`

Makes a ball centred on the frame's origin.

`radius` must be positive. The result is the set of points where `u² + v² + w² ≤ radius²`.

**Example.**

```text
> {"cmd": "sphere", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "radius": 5}
< {"ok": 1}
```

### torus

`torus(frame: Frame, major: Length, minor: Length) -> Solid`

Makes a ring lying round the frame's z axis: a tube of radius `minor` whose centre runs round a
circle of radius `major` in the xy plane.

`major > minor > 0` must hold. The result is the set of points where
`(√(u² + v²) − major)² + w² ≤ minor²`.

**Example.**

```text
> {"cmd": "torus", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "major": 10, "minor": 2}
< {"ok": 1}
```

### cone

`cone(frame: Frame, bottom: Length, top: Length, height: Length) -> Solid`

Makes a cone or truncated cone that stands on the frame's xy plane round the frame's z axis. Its
radius goes from `bottom` at the origin to `top` at `height`.

`height` must be positive. `bottom` and `top` must each be at least zero, and must not both be zero.
A zero radius is an apex. The result is the set of points where
`√(u² + v²) ≤ bottom + (top − bottom)·w/height` and `0 ≤ w ≤ height`.

**Example.**

```text
> {"cmd": "cone", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "bottom": 5, "top": 2, "height": 10}
< {"ok": 1}
```

### extrude

`extrude(frame: Frame, profile: Profile, height: Length) -> Solid`

Pushes a flat profile straight out of its plane, like a part cut from plate. The profile lies in the
frame's xy plane and is swept `height` along z.

`height` must be positive. The result is the set of points where `(u, v) ∈ Ω` and `0 ≤ w ≤ height`:
the profile drawn in the frame's xy plane, moved along z. The result's volume is `A·height` and its
area is `2A + L·height`.

**Example.**

```text
> {"cmd": "extrude", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "profile": [[{"line": [[0, 0], [10, 0]]}, {"line": [[10, 0], [10, 10]]}, {"line": [[10, 10],
   [0, 10]]}, {"line": [[0, 10], [0, 0]]}]], "height": 5}
< {"ok": 1}
```

### revolve

`revolve(frame: Frame, profile: Profile, angle: Angle) -> Solid`

Spins a profile about an axis, as on a lathe. The profile lies beside the frame's z axis and is
turned about that axis by `angle`. A whole turn makes a closed body of revolution.

The profile is drawn in the plane of the frame's x and z, with its first coordinate measured out
from the z axis and its second coordinate measured along the z axis. `Ω` must lie in the half-plane
where the first coordinate is at least zero, and `0 < angle ≤ 2π` must hold. The result is the set
of points `(ρ cos φ, ρ sin φ, w)` for `(ρ, w) ∈ Ω` and `0 ≤ φ ≤ angle`: the profile turned
right-handed about z. The result's volume is `M·angle` (Pappus). Its area is `angle·∮ u ds`, plus
`2A` for the end faces when `angle < 2π`.

**Example.**

```text
> {"cmd": "revolve", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "profile": [[{"line": [[5, 0], [8, 0]]}, {"line": [[8, 0], [8, 4]]}, {"line": [[8, 4], [5, 4]]},
   {"line": [[5, 4], [5, 0]]}]], "angle": 6.283185307}
< {"ok": 1}
```

### screw

`screw(frame: Frame, profile: Profile, rise: Length, angle: Angle) -> Solid`

Spins a profile about an axis while moving the profile along the axis, as a thread is formed. Each
whole turn rises `2π·rise`.

The profile is drawn as for `revolve`. `Ω` must lie where the first coordinate is at least zero, and
`angle` must be positive. The turns must stay apart: `2π·|rise|` must be at least the profile's
extent along z. The result is the set of points `(ρ cos φ, ρ sin φ, w + rise·φ)` for `(ρ, w) ∈ Ω`
and `0 ≤ φ ≤ angle`: the profile turned about z while rising `rise` per radian, as a thread. The
result's volume is `M·angle`, because the rise moves each section within its own plane. Its area is
`angle·∮ √(u² + rise²·(du/ds)²) ds + 2A`.

**Example.**

```text
> {"cmd": "screw", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "profile": [[{"line": [[5, 0], [6, 0]]}, {"line": [[6, 0], [6, 1]]}, {"line": [[6, 1], [5, 1]]},
   {"line": [[5, 1], [5, 0]]}]], "rise": 0.5, "angle": 12.566370614}
< {"ok": 1}
```

### loft

`loft(frame: Frame, profile: Profile, scale: Number, height: Length, top: Profile) -> Solid`

Joins a profile by straight lines to a scaled copy of the profile above it, like a tapered post. A
scale of 1 gives an extrusion, and a smaller scale narrows the solid toward the top.

`top` is `profile` scaled by `scale` about the origin. The client sends `top`, so that an adapter
whose kernel lofts between two sections passes both profiles on and scales nothing itself.

The profile must have one loop, and `scale` and `height` must be positive. The result is the set of
points `(σu, σv, w)` for `(u, v) ∈ Ω` and `0 ≤ w ≤ height`, with `σ = 1 + (scale − 1)·w/height`. The
result is ruled from the profile in the xy plane to the profile's copy scaled about the origin,
`height` up z. The result's volume is `A·height·(1 + s + s²)/3` with `s = scale`, and its area is
`A·(1 + s²) + (1 + s)/2 · ∮ ‖(height·q′, (s − 1)·q × q′)‖` round the profile `q`.

**Example.**

```text
> {"cmd": "loft", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "profile": [[{"line": [[-5, -5], [5, -5]]}, {"line": [[5, -5], [5, 5]]}, {"line": [[5, 5],
   [-5, 5]]}, {"line": [[-5, 5], [-5, -5]]}]], "scale": 0.5, "height": 10, "top": [[{"line": [[-2.5,
   -2.5], [2.5, -2.5]]}, {"line": [[2.5, -2.5], [2.5, 2.5]]}, {"line": [[2.5, 2.5], [-2.5, 2.5]]},
   {"line": [[-2.5, 2.5], [-2.5, -2.5]]}]]}
< {"ok": 1}
```

### loft_through

`loft_through(frame: Frame, profiles: list<Profile>, heights: list<Length>) -> Solid`

Makes the smooth solid through two or more sections. Section `k` is profile `k` drawn in the frame's
xy plane and moved `heights[k]` up the frame's z, and the heights rise. The solid's section at each
of those heights is that profile, the solid's ends are the first and last profiles, and its side
passes smoothly from one section to the next. How the side runs between sections is open: tools
interpolate differently, and no tool's interpolation is the answer.

There must be two or more profiles and one height for each profile, and every height must be above
the one before. Each profile is one loop; sections with holes are not yet specified.

A section means the profile's region, not the way the profile's loop is written. The result does not
depend on which segment a loop starts with, nor on how a curve is cut into segments, so the side has
no twist that the regions do not ask for. When every section is the same profile, the result is that
profile's extrusion from the first height to the last, wherever each loop starts.

Smoothly means that the side has a tangent plane at every point between the first section and the
last, including the points at the heights of the inner sections. The side has no crease along a
section and no crease across one.

**Example.**

```text
> {"cmd": "loft_through", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0],
   "z": [0, 0, 1]}, "profiles": [[[{"line": [[-5, -5], [5, -5]]}, {"line": [[5, -5], [5, 5]]},
   {"line": [[5, 5], [-5, 5]]}, {"line": [[-5, 5], [-5, -5]]}]], [[{"line": [[-3, -3], [3, -3]]},
   {"line": [[3, -3], [3, 3]]}, {"line": [[3, 3], [-3, 3]]}, {"line": [[-3, 3], [-3, -3]]}]]],
   "heights": [0, 10]}
< {"ok": 1}
```

### sweep

`sweep(frame: Frame, profile: Profile, path: Path) -> Solid`

Carries a profile along a path of straight runs and bends, like a pipe or a bent bar.

The path starts at the frame's origin heading along z, with the profile in the xy plane square to
it. A line runs straight on by `length`. An arc bends toward the current x by `angle`, about an axis
that lies along the current y at a distance `radius` from the path, and the arc turns the frame with
it. The result is the union of the pieces: each line gives the extrusion of the profile along the
line, and each arc gives the revolution of the profile about the arc's axis. Every profile point
must be closer to the path than any bend's radius, and all lengths, radii and angles must be
positive. The result's volume, area and moments are those of its pieces added together, less the end
caps that meet inside the result.

**Example.**

```text
> {"cmd": "sweep", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "profile": [[{"line": [[-1, -1], [1, -1]]}, {"line": [[1, -1], [1, 1]]}, {"line": [[1, 1],
   [-1, 1]]}, {"line": [[-1, 1], [-1, -1]]}]], "path": [{"line": 10},
   {"arc": {"radius": 5, "angle": 1.570796327}}, {"line": 10}]}
< {"ok": 1}
```

### sweep_path

`sweep_path(frame: Frame, profile: Profile, path: Spline3, twist: Angle) -> Solid`

Carries a profile along a smooth space curve, like a bent pipe or a twisted rib.

`path` is a clamped B-spline in the frame's coordinates, given by its `degree`, `knots` and control
points. The path starts at the origin heading along z, with the profile in the xy plane square to
it. `twist` may be any angle, either way round. The profile's plane stays square to the path. The
profile's axes follow the path's rotation-minimizing frame, which never turns about the tangent. The
axes also turn by `twist` more by the end of the path, evenly in arc length. Every profile point
must be closer to the path than the path's least radius of curvature, and the sweep must never meet
itself.

Let `T` be the tangent, and let `K₁`, `K₂` be the curvature along the profile's turned axes. A
profile point `(X, Y)` then sweeps a volume element `(1 − K₁X − K₂Y) dA ds`. The volume and moments
are therefore integrals along the path of the profile's moments up to the third. The side's element
is `√(a²|b′|² + φ′²(b·b′)²)` along the boundary `b`, where `a = 1 − K₁X − K₂Y` and `φ′` is the twist
per length. A profile centred on the path gives volume `A·ℓ` (Pappus), where `ℓ` is the path's
length. If that profile is also untwisted, the side is `L·ℓ`.

**Example.**

```text
> {"cmd": "sweep_path", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0],
   "z": [0, 0, 1]}, "profile": [[{"line": [[-1, -1], [1, -1]]}, {"line": [[1, -1], [1, 1]]},
   {"line": [[1, 1], [-1, 1]]}, {"line": [[-1, 1], [-1, -1]]}]],
   "path": {"degree": 3, "knots": [0, 0, 0, 0, 1, 1, 1, 1], "ctrl": [[0, 0, 0], [0, 0, 5],
   [3, 0, 10], [8, 0, 12]]}, "twist": 0}
< {"ok": 1}
```

### slab

`slab(frame: Frame, size: Length[2], knots_u, knots_v: list<Number>, heights: list<list<Length>>) -> Solid`

Makes a block with a freeform top. The block is flat underneath and on its four sides, and its top
is a smooth surface.

The result is the set of points over the rectangle `0 ≤ u ≤ size[0]`, `0 ≤ v ≤ size[1]` from `w = 0`
up to the height field `w = f(u/size[0], v/size[1])`. `f` is the bicubic B-spline over `[0, 1]²`
with clamped knots `knots_u`, `knots_v` (each running four times 0 first and four times 1 last) and
coefficients `heights[i][j]`. Every height must be positive, which keeps `f` positive. As a surface,
the top is the B-spline whose control points sit at the knots' Greville abscissae times the size, so
the top's x and y run linearly with its parameters. The result's volume and moments are integrals of
polynomials in `f`, and its area is the integral of `√(1 + (f_u/size[0])² + (f_v/size[1])²)`.

**Example.**

```text
> {"cmd": "slab", "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
   "size": [10, 10], "knots_u": [0, 0, 0, 1, 1, 1], "knots_v": [0, 0, 0, 1, 1, 1],
   "heights": [[5, 5, 5], [5, 7, 5], [5, 5, 5]]}
< {"ok": 1}
```

## Booleans

Every boolean returns the regularized set operation, which is the closure of the interior of the set
result. The result may be empty. It may also be several pieces, touching as [Types](#types) allows.
Pieces that share an area of boundary are returned as one. The operands are left as they were.

| Function                            | Fields            | Returns |
| ----------------------------------- | ----------------- | ------- |
| [`union`](#union)                   | `a`, `b`          | `Solid` |
| [`intersection`](#intersection)     | `a`, `b`          | `Solid` |
| [`difference`](#difference)         | `a`, `b`          | `Solid` |
| [`union_all`](#union_all)           | `solids`          | `Solid` |
| [`difference_all`](#difference_all) | `target`, `tools` | `Solid` |

### union

`union(a: Solid, b: Solid) -> Solid`

Joins two solids. Returns the regularized `a ∪ b`, where `a ∪ b` is what either solid holds.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. Solid 2 is a cylinder
of radius 5 and height 20 on the z axis.

```text
> {"cmd": "union", "a": 1, "b": 2}
< {"ok": 3}
```

### intersection

`intersection(a: Solid, b: Solid) -> Solid`

Keeps what two solids share. Returns the regularized `a ∩ b`, where `a ∩ b` is what both solids
hold.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. Solid 2 is a cylinder
of radius 5 and height 20 on the z axis.

```text
> {"cmd": "intersection", "a": 1, "b": 2}
< {"ok": 3}
```

### difference

`difference(a: Solid, b: Solid) -> Solid`

Cuts one solid from another. Returns the regularized `a ∖ b`, where `a ∖ b` is what `a` holds and
`b` does not.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. Solid 2 is a cylinder
of radius 5 and height 20 on the z axis.

```text
> {"cmd": "difference", "a": 1, "b": 2}
< {"ok": 3}
```

### union_all

`union_all(solids: list<Solid>) -> Solid`

Joins two or more solids in one call. `solids` must list at least two solids. Returns their
regularized union, which is the same as the result of `union` applied to the solids one at a time,
in any order. A pattern is a `union_all` over copies of a solid moved by [`transform`](#transform).

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. Solid 2 is a cylinder
of radius 5 and height 20 on the z axis. Solid 3 is a ball 30 mm away.

```text
> {"cmd": "union_all", "solids": [1, 2, 3]}
< {"ok": 4}
```

### difference_all

`difference_all(target: Solid, tools: list<Solid>) -> Solid`

Cuts one or more tools from `target` in one call. `tools` must list at least one tool. Returns the
regularized difference, which is the same as the result of `difference` applied to `target` one tool
at a time, in any order.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. Solid 2 is a cylinder
of radius 5 and height 20 on the z axis. Solid 3 is a ball 30 mm away.

```text
> {"cmd": "difference_all", "target": 1, "tools": [2, 3]}
< {"ok": 4}
```

## Blends

An edge is named by a point on it. Each point names the edge of `s` that passes within `10·ε` of the
point, where ε is the suite's tolerance of 1e-7 mm. A call with a point that names no edge refuses.
Naming an edge by a point does not depend on how any kernel numbers or labels its topology, and a
test can compute such a point from the solid's definition.

A blend runs along the whole tangent chain of its edge. The chain is the named edge and every edge
reached from it through vertices where the next edge leaves in the direction in which the last edge
arrived. The top loop of a prism on a rounded rectangle is one chain, so a point on any edge of that
loop blends the whole loop. Following the chain is the work of the function, and so of the kernel.
An adapter passes on the edge the point names and no other edge.

| Function                                  | Fields                                | Returns |
| ----------------------------------------- | ------------------------------------- | ------- |
| [`fillet`](#fillet)                       | `s`, `edges`, `radius`                | `Solid` |
| [`fillet_radii`](#fillet_radii)           | `s`, `edges`, `radii`                 | `Solid` |
| [`fillet_variable`](#fillet_variable)     | `s`, `edge`, `from`, `r_from`, `r_to` | `Solid` |
| [`fillet_conic`](#fillet_conic)           | `s`, `edges`, `setback`, `rho`        | `Solid` |
| [`fillet_g2`](#fillet_g2)                 | `s`, `edges`, `offset`, `center`      | `Solid` |
| [`fillet_setback`](#fillet_setback)       | `s`, `edges`, `radius`, `setback`     | `Solid` |
| [`fillet_setback_g2`](#fillet_setback_g2) | `s`, `edges`, `radius`, `setback`     | `Solid` |
| [`fillet_faces`](#fillet_faces)           | `s`, `face_a`, `face_b`, `radius`     | `Solid` |
| [`full_round`](#full_round)               | `s`, `side_a`, `middle`, `side_b`     | `Solid` |
| [`chamfer`](#chamfer)                     | `s`, `edges`, `distance`              | `Solid` |
| [`chamfer_two`](#chamfer_two)             | `s`, `edge`, `face`, `d1`, `d2`       | `Solid` |
| [`chamfer_distances`](#chamfer_distances) | `s`, `edges`, `distances`             | `Solid` |

### fillet

`fillet(s: Solid, edges: list<Point3>, radius: Length) -> Solid`

Rounds edges. Each named edge is replaced by the surface that a ball of `radius` sweeps as the ball
rolls along the edge, touching both faces that meet there.

`edges` must name one edge or more, `radius` must be positive, and the ball must fit. The ball fits
when its contact with each face stays on that face, except where the fillet swallows a face as
described below. At a convex edge, the result is `s` less the material between the ball's sweep and
the edge. At a concave edge, the result is `s` with the space between the ball's sweep and the edge
filled. Where the two faces are planes, or surfaces of revolution meeting round a circle, the
section across the edge is the circle of `radius` tangent to both faces' traces. `s` is left as it
was.

A fillet swallows a face too narrow for its ball. Take a narrow face `N` between two faces `A` and
`B`, with both of `N`'s edges named: the edge `N` shares with `A` and the edge `N` shares with `B`.
A small enough ball rolls along each edge in turn, touching `A` and `N` along the first edge and `N`
and `B` along the second, and leaves a strip of `N` between two fillets. A ball too big for that
cannot touch `N` at all. Resting against `A` and `B` at once, the ball bridges over `N`. The ball
then rolls touching `A` and `B` alone, and makes one fillet from `A` to `B`. For this, `A` and `B`
are each continued past the edge shared with `N`, as if `N` were not there. `N` lies wholly in the
material the ball's sweep removes, so none of `N` is left. The result is the same as the result of
`fillet` on the solid in which `A` and `B` run on to meet in a single edge, with that edge named.
For example, a corner chamfered by a hair, with both edges of the chamfer filleted, gives the same
result as the sharp corner filleted.

Where named convex edges meet at a corner, the corner is what the balls touching the edges' faces
leave. Filleting every edge of a convex solid, or every convex edge of a prism whose concave edges
all run along its axis, gives the union of the balls of `radius` inside `s`. A box with all twelve
edges filleted by `r` is the box made `r` smaller on every side and then swelled by `r`. Other
corners are not yet specified. Whatever closes such a corner, a fillet leaves no crease. Where the
boundary of the result is not the boundary of `s`, the boundary of the result has a tangent plane,
and shares that tangent plane with each face it meets. A fillet changes nothing farther than twice
the radius from the named edges. Where a motion leaves `s` and the named edges as they were round a
blend, the motion also leaves the result as it was there, within a thousandth of the radius.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. The point names its
vertical edge at the origin.

```text
> {"cmd": "fillet", "s": 1, "edges": [[0, 0, 15]], "radius": 2}
< {"ok": 2}
```

### fillet_radii

`fillet_radii(s: Solid, edges: list<Point3>, radii: list<Length>) -> Solid`

Rounds edges as `fillet` does, in one operation, with each point's chain at its own radius, so that
fillets of unequal radii meet at shared corners. `radii` must give one radius for each point, and
each radius must be as `fillet` requires. Away from those corners, each edge is as `fillet` makes it
at that edge's radius. The corner itself is open within limits. The result lies, point for point,
between the solid filleted at the largest of the corner's radii and the solid filleted at the
smallest: the result holds every point of the first solid and no point outside the second. The
surface of the result is closed and has no crease: the corner joins every fillet and face it meets
with their tangent planes. The pieces, tunnels and voids of the result are those of `s`.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "fillet_radii", "s": 1, "edges": [[0, 0, 15], [10, 0, 15]], "radii": [1, 2]}
< {"ok": 2}
```

### fillet_variable

`fillet_variable(s: Solid, edge: Point3, from: Point3, r_from: Length, r_to: Length) -> Solid`

Rounds one edge as `fillet` does, but by a ball whose radius changes evenly with length along the
edge, from `r_from` at the end nearest `from` to `r_to` at the other end. Both radii must be
positive, the ball must fit all along the edge, and `from` must be nearer one end of the edge than
the other. The ball touches each face along the line where the foot of the ball's centre falls, and
the blend meets each face there tangentially. The surface of the blend between those two lines is
open within limits. The blend has no crease. A ball reaches no farther along the edge than its
radius. So at each point along the edge, the result lies between the edge filleted at the least
radius and the edge filleted at the greatest radius that the ball has within a distance of the
larger of `r_from` and `r_to` from that point.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "fillet_variable", "s": 1, "edge": [0, 0, 15], "from": [0, 0, 0], "r_from": 1, "r_to": 2}
< {"ok": 2}
```

### fillet_conic

`fillet_conic(s: Solid, edges: list<Point3>, setback: Length, rho: Number) -> Solid`

Rounds edges by a conic instead of a circle. Each named edge is replaced by a blend that meets both
faces `setback` back from the edge and is tangent to them there. Between planes, the section of the
blend across the edge is the conic arc from one contact to the other, with its corner on the edge
and shoulder `rho`. `rho = 0.5` gives a parabola. At a square edge, `rho = √2 − 1` gives a quarter
circle.

`setback` must be positive, both contacts must be on their faces, and `rho` must satisfy
`0 < rho < 1`.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "fillet_conic", "s": 1, "edges": [[0, 0, 15]], "setback": 2, "rho": 0.5}
< {"ok": 2}
```

### fillet_g2

`fillet_g2(s: Solid, edges: list<Point3>, offset: Length, center: Length) -> Solid`

Rounds edges by a curvature-continuous blend sized by two lengths. The first is its **tangent
offset**, the radius of the ball whose contacts the blend shares: the blend meets each face where a
ball of `offset` rolling along the edge touches that face. The second is its **center radius**, the
blend's radius of curvature at its middle. Between planes, the section of the blend across the edge
is the blend section of order 2 between the two contacts, with its apex on the edge, at the
thumbweight that gives the middle of the section the center radius. The blend shares each face's
tangent plane and the face's curvature too, so the blend starts as straight as the face. A circular
fillet bends at once by `1/offset`, and so fails to share the face's curvature. The section at the
equal-area thumbweight takes off what the ball's round takes off. At a square edge, the center
radius of that section is about 1.389 times the offset.

Both lengths must be positive, a ball of `offset` must fit as `fillet` requires, and `center` must
be a radius that the section takes at some thumbweight between 0 and 1.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "fillet_g2", "s": 1, "edges": [[0, 0, 15]], "offset": 2, "center": 1.5}
< {"ok": 2}
```

### fillet_setback

`fillet_setback(s: Solid, edges: list<Point3>, radius: Length, setback: Length) -> Solid`

Rounds edges as `fillet` of `radius` does, except at every corner where three named edges meet.
There, each fillet stops `setback` short of the corner, and one smooth patch closes the corner. The
patch meets the three fillets and the three faces tangentially. `setback` must be greater than
`radius`. Farther than `setback` from such a corner, each edge is as `fillet` makes it.

The patch is open within limits. The result lies, point for point, between two solids: `s` filleted
with the corner's cube of side `setback` taken away, and `s` filleted with the corner left sharp.
Neither of those two solids is itself a result, because the patch leaves no crease. On each face,
the flat stops short of the point where the contact lines of the face's two fillets would cross. The
rolling-ball corner reaches that point. A square corner whose three edges are filleted and set back
alike is closed alike on all three edges: the result is unchanged by any swap of the three edges,
within a thousandth of the radius.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. The three edges meet at
the origin.

```text
> {"cmd": "fillet_setback", "s": 1, "edges": [[5, 0, 0], [0, 10, 0], [0, 0, 15]],
   "radius": 1, "setback": 2}
< {"ok": 2}
```

### fillet_setback_g2

`fillet_setback_g2(s: Solid, edges: list<Point3>, radius: Length, setback: Length) -> Solid`

Rounds edges as `fillet_setback` does, and requires what `fillet_setback` requires, but with the
fillets and the patch curvature-continuous throughout. Each fillet is the `fillet_g2` blend of
tangent offset `radius` at the equal-area thumbweight, so the fillet meets its faces with their
curvature and takes off what the round of `radius` does. Each fillet holds that section until
`setback` from the corner. The patch meets the three fillets and the three faces with their
curvature as well as their tangent planes. Where fillet i ends, on its section square to its edge
`setback` from the corner, the boundary bends by the section's center radius round the edge and not
along it, on both sides of the seam. Across the whole corner, which is its faces, fillets and patch,
the boundary's curvature changes without a jump. Circular fillets cannot be closed this way. They
meet their faces with a jump in curvature, and no patch matches both the fillet and the face where a
fillet's end section meets a face.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "fillet_setback_g2", "s": 1, "edges": [[5, 0, 0], [0, 10, 0], [0, 0, 15]],
   "radius": 1, "setback": 2}
< {"ok": 2}
```

### fillet_faces

`fillet_faces(s: Solid, face_a: Point3, face_b: Point3, radius: Length) -> Solid`

Rounds between two faces that need not share an edge. A ball of `radius` rolls touching the face
that each point names. What lies outside the ball's path between the two contacts is removed,
including any faces between the contacts. A chamfer narrower than the round is consumed: two planes
that meet at a chamfered edge give the sharp edge filleted by `radius`. `radius` must be positive,
the two points must name different faces, and the ball must fit between the two faces.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "fillet_faces", "s": 1, "face_a": [5, 10, 0], "face_b": [0, 10, 15], "radius": 2}
< {"ok": 2}
```

### full_round

`full_round(s: Solid, side_a: Point3, middle: Point3, side_b: Point3) -> Solid`

Replaces the face that `middle` names by a round. The round is tangent to that face and to the faces
that `side_a` and `side_b` name, and those two side faces stay where they are. A rib between
parallel sides `w` apart gets a half cylinder of radius `w/2`. A rib between sides that close toward
the middle face gets the arc of the circle touching all three faces.

The three points must name three different faces, and the face that `middle` names must share an
edge with each of the other two.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "full_round", "s": 1, "side_a": [0, 10, 15], "middle": [5, 0, 15], "side_b": [10, 10, 15]}
< {"ok": 2}
```

### chamfer

`chamfer(s: Solid, edges: list<Point3>, distance: Length) -> Solid`

Bevels edges. Each named edge is cut by the surface through the two lines that lie `distance` back
from the edge, one along each face.

`edges` must name one edge or more, `distance` must be positive, and each cut must stay on its
faces. Where the faces are planes, the cut is the plane through those two lines. About a circular
edge of a solid of revolution, the cut is the cone through the two circles. Corners where chamfered
edges meet are not yet specified. `s` is left as it was.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "chamfer", "s": 1, "edges": [[0, 0, 15]], "distance": 2}
< {"ok": 2}
```

### chamfer_two

`chamfer_two(s: Solid, edge: Point3, face: Point3, d1: Length, d2: Length) -> Solid`

Bevels one edge unevenly. The cut runs through two lines: the line `d1` back from the edge along the
face that `face` names, and the line `d2` back from the edge along the other face that meets there.
Between planes, the section the cut takes off is the triangle on those two lines. A chamfer given by
a distance and an angle from the named face is a `chamfer_two` with `d2 = d1·tan(angle)`.

Both distances must be positive, each cut must be on its face, and `face` must name one of the two
faces that meet at the edge.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "chamfer_two", "s": 1, "edge": [0, 0, 15], "face": [0, 10, 15], "d1": 1, "d2": 2}
< {"ok": 2}
```

### chamfer_distances

`chamfer_distances(s: Solid, edges: list<Point3>, distances: list<Length>) -> Solid`

Bevels edges as `chamfer` does, in one operation, with each point's chain at its own distance, so
that chamfers of unequal distances meet at shared corners. `distances` must give one distance for
each point, and each distance must be as `chamfer` requires. Away from those corners, each edge is
as `chamfer` cuts it at that edge's distance. The corner is open within limits. The result lies,
point for point, within the solid chamfered at the smallest of the corner's distances. The volume of
the result is no less than the volume of the solid chamfered at the largest of the corner's
distances. The result holds every point of that second solid farther than the largest distance from
the corner along any edge. Nearer the corner, the three chamfers' planes meet at a point, and a
result may run out to that point or be cut flat across short of it.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "chamfer_distances", "s": 1, "edges": [[0, 0, 15], [10, 0, 15]], "distances": [1, 2]}
< {"ok": 2}
```

## Changing a solid

A face is named in the same way as an edge: each point names the face of `s` that the point lies on,
within `10·ε`.

| Function                        | Fields                                  | Returns |
| ------------------------------- | --------------------------------------- | ------- |
| [`offset`](#offset)             | `s`, `distance`                         | `Solid` |
| [`offset_faces`](#offset_faces) | `s`, `faces`, `distance`                | `Solid` |
| [`delete_faces`](#delete_faces) | `s`, `faces`                            | `Solid` |
| [`move_faces`](#move_faces)     | `s`, `faces`, `matrix`                  | `Solid` |
| [`thicken`](#thicken)           | `s`, `faces`, `thickness`               | `Solid` |
| [`shell`](#shell)               | `s`, `open`, `thickness`                | `Solid` |
| [`draft`](#draft)               | `s`, `faces`, `origin`, `pull`, `angle` | `Solid` |

### offset

`offset(s: Solid, distance: Length) -> Solid`

Grows the solid by a constant distance all round, rounding its convex edges and corners.

`distance` must be positive. The result is the set of points within `distance` of `s`. For a convex
solid, the volume of the result is given by Steiner's formula `V + S·d + M·d² + (4π/3)·d³`, where
`M` is the integral of mean curvature over the boundary of `s`.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "offset", "s": 1, "distance": 1}
< {"ok": 2}
```

### offset_faces

`offset_faces(s: Solid, faces: list<Point3>, distance: Length) -> Solid`

Pushes or pulls faces, as a direct edit. Each named face moves `distance` along its outward normal
onto its offset surface: a plane is moved, and a cylinder's radius is changed. The faces round the
named face are extended or trimmed to meet it. A negative distance pulls the face in.

Where a moved face comes to lie on another face's plane, the two faces merge into one face, and a
face between them that shrinks to nothing is removed. A step pushed level becomes one top, and a
slot's wall pushed across to the other wall closes the slot. `faces` must name one face or more,
`distance` must not be zero, and no face may pass beyond the face it meets.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. The point names its top
face.

```text
> {"cmd": "offset_faces", "s": 1, "faces": [[5, 10, 30]], "distance": 2}
< {"ok": 2}
```

### delete_faces

`delete_faces(s: Solid, faces: list<Point3>) -> Solid`

Takes the faces that the points lie on out of the solid, and closes the gap by extending or trimming
the faces round them, as defeaturing does. Taking out a hole's face leaves the plate whole. Taking
out a notch's or groove's faces leaves the block or shaft that the notch or groove was cut from.
Taking out a chamfer restores the corner. The faces round the gap must be able to close the gap.

**Example.** Solid 2 is the box with one edge chamfered. The point names the chamfer's face.

```text
> {"cmd": "delete_faces", "s": 2, "faces": [[1, 1, 15]]}
< {"ok": 3}
```

### move_faces

`move_faces(s: Solid, faces: list<Point3>, matrix: Placement) -> Solid`

Moves the faces that the points lie on rigidly by `p ↦ R·p + t`. The rows of `matrix` are `[R | t]`,
where `R` is a rotation. The faces round the moved faces are extended or trimmed to meet them, as a
direct edit. Moving a hole's face across a plate moves the hole. Moving a notch's faces along a side
moves the notch. Turning a side about its bottom edge leans the side. A plane moved along itself is
the same plane.

`faces` must name one face or more, `R` must be a rotation, and the faces round the moved faces must
reach them.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "move_faces", "s": 1, "faces": [[5, 10, 30]], "matrix": [[1, 0, 0, 0], [0, 1, 0, 0],
   [0, 0, 1, 3]]}
< {"ok": 2}
```

### thicken

`thicken(s: Solid, faces: list<Point3>, thickness: Length) -> Solid`

Makes the solid that each face the points lie on sweeps as the face moves from where it is to a
distance of `thickness` along its outward normal. The face moves inward where `thickness` is
negative. The walls of the swept solid are swept along the normals from the face's edges. The input
solid is not part of the result. A box's face gives a slab. A cylinder's side gives a tube. A ball's
face gives a spherical shell, and a ring's face gives a hollow ring.

`faces` must name one face or more, and `thickness` must not be zero. When the face moves inward,
the thickness must be less than the face's least radius of curvature on that side, so that the swept
face does not fold.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "thicken", "s": 1, "faces": [[5, 10, 30]], "thickness": 2}
< {"ok": 2}
```

### shell

`shell(s: Solid, open: list<Point3>, thickness: Length) -> Solid`

Hollows the solid to walls of one thickness, open through the named faces.

The result is the set of points of `s` within `thickness` of the boundary of `s`, with the named
faces left out of that boundary. With no faces named, the walls close round a void. `thickness` must
be positive, and the walls must not meet.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "shell", "s": 1, "open": [[5, 10, 30]], "thickness": 1}
< {"ok": 2}
```

### draft

`draft(s: Solid, faces: list<Point3>, origin: Point3, pull: Vector3, angle: Angle) -> Solid`

Tilts faces so that a part draws out of a mould. The neutral plane is the plane through `origin`
perpendicular to `pull`. Each named face turns by `angle` about the line where the face meets the
neutral plane, which narrows the part along `pull`.

`faces` must name one face or more, each face must be a plane that meets the neutral plane, and
`angle` must be between 0 and π/2, with neither included. The result is `s` with those faces moved
onto their tilted planes, and with the faces round them extended or trimmed to meet them. Where
tilted faces meet each other or another face before the part ends, the part closes there, along a
ridge or at an apex, and what lay beyond is removed. The result is still the part cut by the tilted
planes.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "draft", "s": 1, "faces": [[0, 10, 15]], "origin": [0, 0, 0], "pull": [0, 0, 1],
   "angle": 0.05}
< {"ok": 2}
```

## Refusals

A function **refuses** when it declines an input instead of returning a result. Any function may
refuse an input it cannot build correctly. Every function must refuse the inputs in the table at the
end of this section. The suite sends no other input outside what a definition requires. A kernel may
give such an input a meaning of its own: for example, a height below zero may extrude the other way.

How a refusal is made:

- The adapter passes each call to the kernel as it is. It does not check the input itself.
- When the kernel declines, by an error, an exception or no result at all, the adapter replies
  `{"err": reason, "kind": kind}` with the kernel's own reason, and goes on to the next request. In
  a Rust adapter, the `Kernel` method returns `Err(Refusal)`.
  [Requests and replies](#requests-and-replies) gives the reply.
- `kind` says why the kernel declined, as the kernel's own error reports it:

  | `kind`          | Meaning                                                                                   |
  | --------------- | ----------------------------------------------------------------------------------------- |
  | `"unsupported"` | The kernel has no way to do what is asked: not this function, or not with these arguments |
  | `"invalid"`     | The request is at fault: it describes no solid, or names an edge or a face not there      |
  | `"failed"`      | The kernel took the request as a fair one and could not carry it out                      |

  A reply with no `kind`, or with a `kind` that a client does not know, is read as `"failed"`. A
  kernel whose errors do not distinguish these kinds answers `"failed"` every time.

- A refusal is not a crash. A panic, or a process that dies, is a crash. An empty solid is not a
  refusal either: two solids that do not overlap intersect in the empty solid, which is a result.

How a refusal is scored, under clause B18 of [contract.md](contract.md):

| The input       | The kernel                       | The case's outcome                |
| --------------- | -------------------------------- | --------------------------------- |
| Must be refused | Refuses as `invalid` or `failed` | Pass                              |
| Must be refused | Returns a solid                  | Fail, however plausible the solid |
| Is allowed      | Refuses as `invalid` or `failed` | Refused                           |
| Either          | Refuses as `unsupported`         | Unsupported: no judgement         |
| Either          | Crashes or hangs                 | Crash or timeout                  |

`unsupported` means that the kernel has not built the function, or has not built it for arguments of
this sort. The case is scored in the same way as a case for a function that the kernel's `hello`
leaves out of `supports`: the case is left out of what the kernel is judged on. So a kernel is not
marked down for saying plainly what it cannot do, and is not given a pass for an input it never
judged. An input that has no answer, such as a fillet too big for its faces, is `invalid`, not
`unsupported`.

The inputs every kernel must refuse:

| Input                                                                             | Functions                                              | Tested |
| --------------------------------------------------------------------------------- | ------------------------------------------------------ | ------ |
| A profile whose loops cross themselves or each other                              | Every constructor that takes a profile                 | Yes    |
| A profile whose hole lies outside it                                              | Every constructor that takes a profile                 | Yes    |
| A profile that crosses the axis                                                   | `revolve`, `screw`                                     | Yes    |
| Turns that meet                                                                   | `screw`                                                | Yes    |
| A tube wider than its ring                                                        | `torus`                                                | Yes    |
| Heights that do not rise                                                          | `loft_through`                                         | Yes    |
| A radius or distance too large for the faces it must fit on                       | Every fillet and chamfer                               | Yes    |
| One radius or distance of several, or a variable fillet's ball, that is too large | `fillet_radii`, `chamfer_distances`, `fillet_variable` | Yes    |
| A shell as thick as the part                                                      | `shell`                                                | Yes    |
| A face pushed through the part, or past the face it meets                         | `offset_faces`                                         | Yes    |
| A side, radius or height of zero, or a side or radius below it                    | `box`, `cylinder`, `sphere`, `extrude`                 | Yes    |
| A radius or distance of zero or less                                              | `fillet`, `chamfer`                                    | Yes    |
| A cone with no radius at either end                                               | `cone`                                                 | Yes    |
| A point that names no edge or face                                                | `fillet`, `chamfer`, `offset_faces`                    | Yes    |

A point names nothing when no edge, or no face, of the solid passes within `10·ε` of the point.
Where a kernel takes its edges and faces by its own handles, the kernel's adapter finds the handle
that a point names, and the adapter, not the kernel, refuses a point that names no handle.

## Exchange

A STEP file travels as a `string` holding the file's contents: all of its text, from `ISO-10303-21;`
to `END-ISO-10303-21;`, in the clear-text encoding of ISO 10303-21. The `string` is never a path or
a file name, because the kernel and the suite need not share a disk. The adapter hands the kernel
that text as UTF-8 bytes. The files the suite writes use only the standard's basic alphabet, which
is plain ASCII.

| Function                    | Fields              | Returns  |
| --------------------------- | ------------------- | -------- |
| [`write_step`](#write_step) | `s`                 | `string` |
| [`sew`](#sew)               | `text`, `tolerance` | `Solid`  |
| [`read_step`](#read_step)   | `text`              | `Solid`  |

### write_step

`write_step(s: Solid) -> string`

Writes the solid as a STEP file. Returns the file's contents, as a string.

The kernel chooses the schema, which is AP203, AP214 or AP242, and the file's header names the
schema. Another reader must be able to read the file, not only the kernel that wrote it. When the
suite is run with a second kernel as `--peer`, the suite has the second kernel read the file, and
reports whether the second kernel reads the same solid.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "write_step", "s": 1}
< {"ok": "ISO-10303-21;\nHEADER;\n
   FILE_DESCRIPTION(('springrep solid'),'2;1');… (10,474 characters)"}
```

### sew

`sew(text: string, tolerance: Length) -> Solid`

Sews the faces of a STEP file into one closed solid. `text` is the contents of a STEP file that
holds loose faces and no solid.

In a solid, two faces that meet share one edge, and that sharing is what makes the solid's boundary
closed. A file of loose faces has lost that sharing. Each face is written as a surface of its own,
with its own copy of every edge and corner, so nothing in the file says which faces are neighbours.
Files like this come from tools that export surfaces, or that drop a solid's connections on the way
out. The faces in such files may not quite meet, and some faces may be written inside out.

Sewing joins the faces wherever their edges lie within `tolerance` of each other, and turns any face
written inside out to face outward. `tolerance` must be positive, and the faces must close into a
solid once joined. A prism whose faces are written loose and shifted apart by less than the
tolerance sews back into the prism, to within the shifts.

**Example.** The file holds the six faces of a 10 × 20 × 30 mm box, each written as its own surface
with its own edges and corners. Three are shifted off the box by less than 0.0001 mm, and one is
written inside out. Sewn, they close into the box: `volume` of the result gives 6000.04 mm³, the
box's 6000 to within the shifts.

```text
> {"cmd": "sew", "text": "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n
   FILE_NAME('case','',(''),(''),'','','');\n
   FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214… (15,069 characters)", "tolerance": 0.002}
< {"ok": 1}
```

### read_step

`read_step(text: string) -> Solid`

Reads the solids that a STEP file holds, together as one solid set. `text` is the contents of a STEP
file, in AP203, AP214 or AP242. A file written by `write_step` reads back as the same solid, which
means the same set of points within the tolerance.

**Example.** The text is the file `write_step` returned for the box.

```text
> {"cmd": "read_step", "text": "ISO-10303-21;\nHEADER;\n
   FILE_DESCRIPTION(('springrep solid'),'2;1');\n
   FILE_NAME('conformance','',(''),(''),'','springrep','');\nFILE_SCHEMA(('AU… (10,474 characters)"}
< {"ok": 1}
```

## Queries

| Function                        | Fields                                     | Returns                                                     |
| ------------------------------- | ------------------------------------------ | ----------------------------------------------------------- |
| [`volume`](#volume)             | `s`                                        | `Volume`                                                    |
| [`area`](#area)                 | `s`                                        | `Area`                                                      |
| [`classify`](#classify)         | `s`, `points`                              | `list<"in" or "on" or "out">`                               |
| [`closest`](#closest)           | `s`, `points`                              | `list<Point3>`                                              |
| [`normal`](#normal)             | `s`, `points`                              | `list<Vector3 or null>`                                     |
| [`curvature`](#curvature)       | `s`, `points`                              | `list<Number[2] or null>`                                   |
| [`check`](#check)               | `s`                                        | `list<string>`                                              |
| [`centroid`](#centroid)         | `s`                                        | `Point3 or null`                                            |
| [`inertia`](#inertia)           | `s`                                        | `Number[3][3]`                                              |
| [`distance`](#distance)         | `a`, `b`                                   | `Length`                                                    |
| [`interference`](#interference) | `a`, `b`                                   | `{volume: Volume, gap: Length}`                             |
| [`betti`](#betti)               | `s`                                        | `Count[3]`                                                  |
| [`section_area`](#section_area) | `s`, `origin`, `normal`                    | `Area`                                                      |
| [`section`](#section)           | `s`, `origin`, `normal`, `tolerance`       | `list<list<Point3>>`                                        |
| [`project`](#project)           | `s`, `direction`, `tolerance`              | `{visible: list<list<Point2>>, hidden: list<list<Point2>>}` |
| [`bounding_box`](#bounding_box) | `s`                                        | `Point3[2]`                                                 |
| [`split`](#split)               | `s`, `origin`, `normal`                    | `Solid[2]`                                                  |
| [`mesh`](#mesh)                 | `s`, `tolerance`                           | `{positions: list<Point3>, triangles: list<Index[3]>}`      |
| [`display`](#display)           | `s`, `tolerance`                           | `{faces: list<Face>, edges: list<list<Point3>>}`            |
| [`raycast`](#raycast)           | `s`, `origin`, `direction`; or `s`, `rays` | `list<Length>`; or `list<list<Length>>`                     |

### volume

`volume(s: Solid) -> Volume`

Measures how much space the solid fills.

Returns the measure of `s` in mm³. It is zero for an empty solid. A kernel that cannot measure the
solid answers `err`, never a number that is not one.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "volume", "s": 1}
< {"ok": 6000.0}
```

### area

`area(s: Solid) -> Area`

Measures the total area of the solid's surface.

Returns the measure of the boundary of `s` in mm². It is zero for an empty solid. A kernel that
cannot measure the boundary answers `err`.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "area", "s": 1}
< {"ok": 2200.0}
```

### classify

`classify(s: Solid, points: list<Point3>) -> list<"in" or "on" or "out">`

Asks where each of many points lies against the solid.

Returns one answer for each point, in order. The answer is `"in"` where `s` holds the point clear of
the boundary of `s`, `"on"` where the point lies on the boundary, and `"out"` otherwise. The
boundary is every point within `ε` of a face, an edge or a vertex. A point further than `10·ε` from
the boundary is `"in"` or `"out"`. A point off the boundary but within `10·ε` of it may be given as
either or as `"on"`. A kernel that cannot place one of the points refuses the whole call. It does
not answer for the other points alone.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. The second point lies
on its top face.

```text
> {"cmd": "classify", "s": 1, "points": [[5, 10, 15], [5, 10, 30], [20, 0, 0]]}
< {"ok": ["in", "on", "out"]}
```

### closest

`closest(s: Solid, points: list<Point3>) -> list<Point3>`

Finds the point of the solid nearest each point.

Returns, for each point in order, a point of `s` such that no other point of `s` is nearer to the
given point. Here `s` is the solid with its inside counted in, as for [`distance`](#distance), so a
point that `s` holds is its own nearest point. For a point outside `s`, the nearest point lies on
the boundary of `s`: on a face, an edge or a vertex. Where several points of `s` are equally near,
as the points round a ring are from a point on a torus's axis, any one of them is a correct answer.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. The first point is
inside it.

```text
> {"cmd": "closest", "s": 1, "points": [[5, 10, 15], [20, 10, 15]]}
< {"ok": [[5.0, 10.0, 15.0], [10.0, 10.0, 15.0]]}
```

### normal

`normal(s: Solid, points: list<Point3>) -> list<Vector3 or null>`

Gives the outward unit normal at each point of the solid's boundary. Where faces meet smoothly, as a
fillet meets the faces it rounds, the boundary still has a tangent plane, and the normal there is
the normal of both faces. At a crease, where faces meet at an angle, the boundary has no tangent
plane, and the answer is null. The answer is also null for a point that is off the boundary by more
than `10·ε`.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. The second point lies
on an edge, a crease.

```text
> {"cmd": "normal", "s": 1, "points": [[5, 10, 30], [0, 0, 15]]}
< {"ok": [[0.0, 0.0, 1.0], null]}
```

### curvature

`curvature(s: Solid, points: list<Point3>) -> list<Number[2] or null>`

Gives the principal curvatures `[k1, k2]` of the boundary, with `k1 ≥ k2`, in 1/mm. For each given
point, the curvatures are taken at the point of the boundary nearest to it, so the given point need
not lie on the boundary. A curvature is positive where the boundary bends away from its outward
normal, as on a ball, and negative where the boundary bends toward its outward normal, as round a
hole. Where the nearest point lies on a crease, the answer is null.

A plane has `[0, 0]`. A cylinder of radius `r` has `[1/r, 0]`, a ball has `[1/R, 1/R]`, and a hole
has `[0, −1/r]`. A cone has `[cos α/ρ, 0]` at radius `ρ`, where its side leans `α` from its axis. A
ring of radii `R` and `r` has `1/r` and `cos φ/(R + r cos φ)` at angle `φ` round its tube.

**Example.** Solid 1 is a cylinder of radius 5 on the z axis.

```text
> {"cmd": "curvature", "s": 1, "points": [[5, 0, 10]]}
< {"ok": [[0.2, 0.0]]}
```

### check

`check(s: Solid) -> list<string>`

Asks the kernel whether the solid is well formed.

Returns the problems that the kernel's own validity checker finds in `s`. The list is empty when the
checker finds `s` to be a valid solid set as [Types](#types) defines it.

A checker that declines the solid refuses, as any function does. The checker has not found the solid
valid, so a client reads a refusal as a problem found. A kernel with no checker leaves `check` out
of its `supports`, and that kernel's results are then not judged on validity.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "check", "s": 1}
< {"ok": []}
```

### centroid

`centroid(s: Solid) -> Point3 or null`

Finds the solid's balance point, as if it were of uniform density.

Returns `∭ r dV / volume(s)` over `s`, or `null` when `s` is empty.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "centroid", "s": 1}
< {"ok": [5.0, 10.0, 15.0]}
```

### inertia

`inertia(s: Solid) -> Number[3][3]`

Measures how the solid's material is spread about its centroid, as if it were of uniform density 1.

Returns `∭ (|d|²·I − d dᵀ) dV` over `s`, in mm⁵, where `d` is the offset of each point from the
centroid. The diagonal holds the moments about axes through the centroid, and the other entries hold
the negated products. The matrix is symmetric, so its rows and its columns are the same. Every entry
is zero when `s` is empty.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "inertia", "s": 1}
< {"ok": [[650000.0, -0.0, 0.0], [-0.0, 500000.0, 0.0], [0.0, 0.0, 250000.0]]}
```

### distance

`distance(a: Solid, b: Solid) -> Length`

Measures how far apart two solids are: the clearance between them.

Returns the least `|p − q|` over points `p` of `a` and `q` of `b`, insides included. It is zero when
`a` and `b` touch or overlap, or when one holds the other. `a` and `b` must both be non-empty.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. Solid 2 is a ball of
radius 5 at x = 30.

```text
> {"cmd": "distance", "a": 1, "b": 2}
< {"ok": 15.0}
```

### interference

`interference(a: Solid, b: Solid) -> {volume: Volume, gap: Length}`

Says how two solids meet, as they stand. It returns the volume they share and the gap between them,
which is the least distance from a point of one to a point of the other. Two solids that overlap
have a `volume` above 0 and a `gap` of 0. Two that only touch have 0 for both. Two that are apart
have a `volume` of 0 and the `gap` between them. A solid held whole inside the other shares its own
volume.

The function answers one pair. A client checking an assembly makes or moves each part to where it
stands and asks for the pairs it cannot rule out itself.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. Solid 2 is a cylinder
of radius 5 and height 20 on the z axis.

```text
> {"cmd": "interference", "a": 1, "b": 2}
< {"ok": {"gap": 0.0, "volume": 392.699081699}}
```

### betti

`betti(s: Solid) -> Count[3]`

Counts the solid's separate pieces, the tunnels that run through it and the voids it encloses.

Returns the solid set's Betti numbers `b0`, `b1`, `b2`. A box is `[1, 0, 0]`, a ring is `[1, 1, 0]`,
a plate with `n` holes is `[1, n, 0]`, and a box with a ball removed from inside it is `[1, 0, 1]`.
Pieces that only touch count as one piece here, because the point set is connected.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "betti", "s": 1}
< {"ok": [1, 0, 0]}
```

### section_area

`section_area(s: Solid, origin: Point3, normal: Vector3) -> Area`

Measures the area, in mm², of the cut that the plane through `origin` square to `normal` makes
through the solid. The area is zero where the plane misses the solid.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "section_area", "s": 1, "origin": [0, 0, 15], "normal": [0, 0, 1]}
< {"ok": 200.0}
```

### section

`section(s: Solid, origin: Point3, normal: Vector3, tolerance: Length) -> list<list<Point3>>`

Gives the curves that the plane through `origin` square to `normal` cuts from the solid's boundary.
The curves come in pieces, in any order. Each piece is a stretch of one curve, given as two or more
points along the curve from one end of the stretch to the other. Every point lies on the plane and
on the boundary, and every chord between consecutive points lies within `tolerance` of the curve.
`tolerance` must be positive.

The pieces join end to end into closed curves. Each end of a piece lies within `10·ε` of an end of
another piece, and a piece that is a whole closed curve ends on its first point. A kernel returns
the pieces as it has them: one for each edge of the section, or one for each curve. The client joins
the pieces.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "section", "s": 1, "origin": [0, 0, 15], "normal": [0, 0, 1], "tolerance": 0.01}
< {"ok": [[[10.0, -0.0, 15.0], [9.375, -0.0, 15.0], [8.75, -0.0, 15.0], [8.125, -0.0, 15.0],
   [7.5, -0.0, 15.0], [6.875, -0.0, 15.0], … 59 more]]}
```

### project

`project(s: Solid, direction: Vector3, tolerance: Length) -> {visible: list<list<Point2>>, hidden: list<list<Point2>>}`

Makes a hidden-line drawing of the solid seen looking along `direction`. The drawing shows the
solid's edges, and its outlines where the line of sight grazes a curved face. Each edge and each
outline is cut into the stretches the viewer sees and the stretches the solid hides. A point is
hidden when the ray from that point toward the viewer passes through the solid. An edge between
faces that meet smoothly is not drawn, and neither is a face's seam.

Each stretch is returned as a polyline in the view plane. Every vertex of the polyline lies on the
stretch, and every chord lies within `tolerance` of the stretch. `tolerance` must be positive. The
plane has two axes. The axis across is `u = d × z`, made unit, or `d × x` when `d` runs along z. The
axis up is `v = −d × u`. A point `p` draws at `(p·u, p·v)`.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "project", "s": 1, "direction": [0, 0, -1], "tolerance": 0.01}
< {"ok": {"hidden": [], "visible": [[[0.0, 0.0], [0.0, 0.625], [0.0, 1.25], [0.0, 1.875],
   [0.0, 2.5], [0.0, 3.125], … 11 more], [[0.0, 0.0], [0.0, 0.625], [0.0, 1.25], [0.0, 1.875],
   [0.0, 2.5], [0.0, 3.125], … 11 more], [[0.0, 10.0], [-1.25, 10.0], [-2.5, 10.0], [-3.75, 10.0],
   [-5.0, 10.0], [-6.25, 10.0], … 11 more], [[0.0, 10.0], [-1.25, 10.0], [-2.5, 10.0],
   [-3.75, 10.0], [-5.0, 10.0], [-6.25, 10.0], … 11 more], [[-20.0, 10.0], [-20.0, 9.375],
   [-20.0, 8.75], [-20.0, 8.125], [-20.0, 7.5], [-20.0, 6.875], … 11 more], [[-20.0, 10.0],
   [-20.0, 9.375], [-20.0, 8.75], [-20.0, 8.125], [-20.0, 7.5], [-20.0, 6.875], … 11 more],
   … 6 more]}}
```

### bounding_box

`bounding_box(s: Solid) -> Point3[2]`

Finds the least box with sides square to the axes that holds the solid, and returns the box as its
low and high corners. The box is not padded by the tolerance, and it is not a box round the control
points.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "bounding_box", "s": 1}
< {"ok": [[0.0, 0.0, 0.0], [10.0, 20.0, 30.0]]}
```

### split

`split(s: Solid, origin: Point3, normal: Vector3) -> Solid[2]`

Cuts the solid by the plane through `origin` square to `normal`. It returns the regularized part on
the normal's side first, then the part on the other side. Either part may be empty, and either may
be several pieces.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "split", "s": 1, "origin": [0, 0, 15], "normal": [0, 0, 1]}
< {"ok": [2, 3]}
```

### mesh

`mesh(s: Solid, tolerance: Length) -> {positions: list<Point3>, triangles: list<Index[3]>}`

Covers the solid's boundary with triangles. No triangle lies farther than `tolerance` from the
boundary. `tolerance` must be positive.

The mesh is closed. When positions within `10·ε` of each other are read as one vertex, every edge
between two triangles is used once each way round. The triangles, which are counter-clockwise seen
from outside, therefore all face outward. Because positions within `10·ε` of each other are read as
one vertex, a kernel that meshes each face on its own may repeat the vertices along an edge, once
for each face. The volume that the mesh bounds differs from the solid's volume by at most
`tolerance` times the solid's area.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "mesh", "s": 1, "tolerance": 0.5}
< {"ok": {"positions": [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 20.0, 0.0], [0.0, 20.0, 0.0],
   [0.0, 0.0, 30.0], [10.0, 0.0, 30.0], [10.0, 20.0, 30.0], [0.0, 20.0, 30.0]],
   "triangles": [[2, 1, 0], [3, 2, 0], [6, 4, 5], [4, 6, 7], [5, 0, 1], [0, 5, 4], … 6 more]}}
```

### display

`display(s: Solid, tolerance: Length) -> {faces: list<{positions: list<Point3>, normals: list<Vector3>, triangles: list<Index[3]>}>, edges: list<list<Point3>>}`

Draws the solid as a viewer shows it: each face meshed on its own, and its edges as polylines.

Each face has its own vertices, so shading is smooth within a face and sharp where two faces meet.
Every vertex lies on the face's surface, and `normals` gives the surface's outward unit normal at
each vertex. The triangles are counter-clockwise seen from outside, and no triangle lies farther
than `tolerance` from the face. Together the faces close the solid, as [`mesh`](#mesh) does.

`edges` holds each edge between two faces, whether they meet at an angle or smoothly, as where a
fillet meets the face it rounds into. Each edge is given as a polyline whose every vertex lies on
the edge and whose every chord lies within `tolerance` of the edge. A seam, where a face meets
itself, is not drawn, and neither is an edge between two faces on one surface: neither marks a
change in the shape.

The suite's runner draws the files it exports with `display` ([formats.md](formats.md#assets)).

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin. One face and one edge
of each are shown.

```text
> {"cmd": "display", "s": 1, "tolerance": 0.1}
< {"ok": {"faces": [{"positions": [[10.0, 20.0, 0.0], [10.0, 0.0, 0.0], [0.0, 0.0, 0.0],
   [0.0, 20.0, 0.0]], "normals": [[-0.0, -0.0, -1.0], [-0.0, -0.0, -1.0], [-0.0, -0.0, -1.0],
   [-0.0, -0.0, -1.0]], "triangles": [[0, 1, 2], [3, 0, 2]]}, … 5 more], "edges": [[[0.0, 0.0, 0.0],
   [10.0, 0.0, 0.0]], … 11 more]}}
```

### raycast

`raycast(s: Solid, origin: Point3, direction: Vector3) -> list<Length>`

`raycast(s: Solid, rays: list<[Point3, Vector3]>) -> list<list<Length>>`

Finds where a ray meets the solid's faces. It returns the distance from `origin` to each meeting, in
mm, in any order. `direction` need not be a unit vector. A ray through an edge or a vertex meets
every face there, and a kernel may list that point once for each face. A client reads meetings
within `10·ε` of each other as one crossing. A request may carry many rays at once by giving `rays`,
a list of `[origin, direction]` pairs, in place of `origin` and `direction`. The reply is then one
list of distances for each ray, in the rays' order. An adapter that reads only one ray per request
refuses the many-ray form, and the client falls back to one request per ray.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "raycast", "s": 1, "origin": [-5, 10, 15], "direction": [1, 0, 0]}
< {"ok": [5.0, 15.0]}
```

## Moving a solid

| Function                  | Fields        | Returns |
| ------------------------- | ------------- | ------- |
| [`transform`](#transform) | `s`, `matrix` | `Solid` |
| [`affine`](#affine)       | `s`, `matrix` | `Solid` |

### transform

`transform(s: Solid, matrix: Number[3][4]) -> Solid`

Moves the solid by `p ↦ M·p + t`, where the rows of `matrix` are `[M | t]`.

`M` must be a similarity: an orthogonal matrix (a rotation or a mirror) times a nonzero scale `c`.
The result's volume is `|c|³` times the solid's volume, and its area is `c²` times the solid's area.
A mirror keeps the result a valid solid, with its faces still facing outward. A part is joined to
its own mirror image by a `transform` and a [`union`](#booleans). Where the part has a face on the
mirror plane, the part and its image meet face to face.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "transform", "s": 1, "matrix": [[1, 0, 0, 5], [0, 1, 0, 0], [0, 0, 1, 0]]}
< {"ok": 2}
```

### affine

`affine(s: Solid, matrix: Number[3][4]) -> Solid`

Maps the solid by `p ↦ M·p + t`, where the rows of `matrix` are `[M | t]`. `M` may be any matrix
with a nonzero determinant: a stretch along any axes, a shear, a mirror, or a similarity.

The result's volume is `|det M|` times the solid's, and its centroid is `M·c + t`. A point lies
inside the result exactly when its preimage lies inside the solid. A surface element grows by the
factor `|det M|·|M⁻ᵀ n|`. For singular values σ₁ ≥ σ₂ ≥ σ₃, that factor lies between σ₂σ₃ and σ₁σ₂,
so the result's area lies between σ₂σ₃ times and σ₁σ₂ times the solid's area. A cylinder, sphere or
torus that is stretched unevenly is no longer a cylinder, sphere or torus, and a kernel may carry
such a face as a spline.

**Example.** Solid 1 is a 10 × 20 × 30 mm box with one corner at the origin.

```text
> {"cmd": "affine", "s": 1, "matrix": [[2, 0, 0, 0], [0, 1, 0, 0], [0, 0, 0.5, 0]]}
< {"ok": 2}
```

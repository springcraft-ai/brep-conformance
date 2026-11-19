// SPDX-License-Identifier: MIT
// The operations of doc/api.md as a C++ API and as a protocol: the shapes a kernel is asked to
// make, the `Kernel` class a kernel implements, and `serve`, which answers the protocol over stdin
// and stdout with any implementation. The C++ counterpart of the Rust crate `brep-protocol`.
#pragma once

#include <nlohmann/json.hpp>

#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <exception>
#include <iostream>
#include <map>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <variant>
#include <vector>

namespace brep_protocol {

using Json = nlohmann::json;

// The protocol's version, which `hello` reports.
inline constexpr const char* PROTOCOL_VERSION = "1.0.0";

// Every function of this version of the protocol, in the order of doc/api.md. `hello` lists the
// ones a kernel offers out of these.
inline const std::array<const char*, 62> FUNCTIONS = {
    "box",
    "cylinder",
    "sphere",
    "torus",
    "cone",
    "extrude",
    "revolve",
    "screw",
    "loft",
    "loft_through",
    "sweep",
    "sweep_path",
    "slab",
    "union",
    "intersection",
    "difference",
    "union_all",
    "difference_all",
    "fillet",
    "fillet_radii",
    "fillet_variable",
    "fillet_conic",
    "fillet_g2",
    "fillet_setback",
    "fillet_setback_g2",
    "fillet_faces",
    "full_round",
    "chamfer",
    "chamfer_two",
    "chamfer_distances",
    "offset",
    "offset_faces",
    "delete_faces",
    "move_faces",
    "thicken",
    "shell",
    "draft",
    "write_step",
    "sew",
    "read_step",
    "volume",
    "area",
    "classify",
    "closest",
    "normal",
    "curvature",
    "check",
    "centroid",
    "inertia",
    "distance",
    "interference",
    "betti",
    "section_area",
    "section",
    "project",
    "bounding_box",
    "split",
    "mesh",
    "display",
    "raycast",
    "transform",
    "affine",
};

using Vec2 = std::array<double, 2>;
using Vec3 = std::array<double, 3>;
// Rows `[M | t]` of the map `p ↦ M·p + t`.
using Matrix = std::array<std::array<double, 4>, 3>;

// Right-handed orthonormal axes at `origin`.
struct Frame {
    Vec3 origin;
    Vec3 x;
    Vec3 y;
    Vec3 z;
};

// A B-spline in a plane over the parameters `knots[degree] ..= knots[ctrl.size()]`: in a profile,
// periodic, with its knots running on past both ends and its first `degree` points repeated last.
struct Spline {
    std::size_t degree;
    std::vector<double> knots;
    std::vector<Vec2> ctrl;
};

// The segments of a profile's loop, in sketch coordinates.
struct Line {
    Vec2 a;
    Vec2 b;
};
// Counter-clockwise from `start` when `sweep` is positive, clockwise when negative; a sweep of
// ±2π is a whole circle.
struct Arc {
    Vec2 center;
    double radius;
    double start;
    double sweep;
};
// The rational quadratic Bézier from `a` to `b` with control points `a`, `apex`, `b` and weights
// 1, `w`, 1.
struct Conic {
    Vec2 a;
    Vec2 apex;
    Vec2 b;
    double w;
};
// The blend section of order `order` and thumbweight `w` from `a` to `b`, between the lines from
// `apex` to each.
struct Gn {
    Vec2 a;
    Vec2 apex;
    Vec2 b;
    unsigned order;
    double w;
};
using Segment = std::variant<Line, Arc, Conic, Gn, Spline>;
using Loop = std::vector<Segment>;

// Closed loops, the first the outer one counter-clockwise and any further ones holes, clockwise.
struct Profile {
    std::vector<Loop> loops;
};

// One piece of a sweep's path: a straight run, or a bend of `angle` at `radius`.
struct PathLine {
    double length;
};
struct PathArc {
    double radius;
    double angle;
};
using PathPiece = std::variant<PathLine, PathArc>;

// A clamped B-spline curve in space over `knots[degree] ..= knots[ctrl.size()]`.
struct Spline3 {
    std::size_t degree;
    std::vector<double> knots;
    std::vector<Vec3> ctrl;
};

// The primitive solids, each placed by its frame. doc/api.md defines each.
struct Box {
    Frame frame;
    Vec3 size;
};
struct Cylinder {
    Frame frame;
    double radius;
    double height;
};
struct Sphere {
    Frame frame;
    double radius;
};
struct Torus {
    Frame frame;
    double major;
    double minor;
};
struct Cone {
    Frame frame;
    double bottom;
    double top;
    double height;
};
struct Extrude {
    Frame frame;
    Profile profile;
    double height;
};
struct Revolve {
    Frame frame;
    Profile profile;
    double angle;
};
struct Screw {
    Frame frame;
    Profile profile;
    double rise;
    double angle;
};
// `top` is `profile` scaled by `scale` about the origin, which the client sends.
struct Loft {
    Frame frame;
    Profile profile;
    double scale;
    double height;
    Profile top;
};
struct Sweep {
    Frame frame;
    Profile profile;
    std::vector<PathPiece> path;
};
struct SweepPath {
    Frame frame;
    Profile profile;
    Spline3 path;
    double twist;
};
struct Slab {
    Frame frame;
    Vec2 size;
    std::vector<double> knots_u;
    std::vector<double> knots_v;
    std::vector<std::vector<double>> heights;
};
using Primitive = std::variant<Box,
                               Cylinder,
                               Sphere,
                               Torus,
                               Cone,
                               Extrude,
                               Revolve,
                               Screw,
                               Loft,
                               Sweep,
                               SweepPath,
                               Slab>;

// Where a point lies against a solid: inside it, on its boundary, or outside it.
enum class Side { In, On, Out };

inline const char* word(Side side) {
    switch (side) {
    case Side::In:
        return "in";
    case Side::On:
        return "on";
    default:
        return "out";
    }
}

// A triangle mesh: positions, and triangles as three indices each.
struct Mesh {
    std::vector<Vec3> positions;
    std::vector<std::array<std::uint32_t, 3>> triangles;
};

// One face of a `Display`: its own vertices, the exact surface's outward normal at each, and its
// triangles over them.
struct DisplayFace {
    std::vector<Vec3> positions;
    std::vector<Vec3> normals;
    std::vector<std::array<std::uint32_t, 3>> triangles;
};

// A solid as a viewer draws it: each face meshed on its own, and the edges that are creases as
// polylines.
struct Display {
    std::vector<DisplayFace> faces;
    std::vector<std::vector<Vec3>> edges;
};

// A solid's edges and outlines seen along a direction, as polylines in the view plane.
struct Projection {
    std::vector<std::vector<Vec2>> visible;
    std::vector<std::vector<Vec2>> hidden;
};

// How two solids meet: the volume they share, and the gap between them.
struct Meeting {
    double volume;
    double gap;
};

// The three booleans of two solids.
enum class Op { Union, Intersection, Difference };

inline const char* word(Op op) {
    switch (op) {
    case Op::Union:
        return "union";
    case Op::Intersection:
        return "intersection";
    default:
        return "difference";
    }
}

// A solid's volume, area and validity problems, gathered by `Kernel::measure`.
struct Measure {
    double volume;
    double area;
    // Validity problems its own checker finds; empty when valid.
    std::vector<std::string> issues;
};

// Why a kernel declined a request.
enum class Kind { Unsupported, Invalid, Failed };

inline const char* word(Kind kind) {
    switch (kind) {
    case Kind::Unsupported:
        return "unsupported";
    case Kind::Invalid:
        return "invalid";
    default:
        return "failed";
    }
}

// A request declined: of which kind, and the kernel's reason in its own words. A method throws it.
struct Refusal : std::exception {
    Kind kind;
    std::string reason;

    Refusal(Kind kind, std::string reason) : kind(kind), reason(std::move(reason)) {}
    static Refusal unsupported(std::string reason) {
        return Refusal(Kind::Unsupported, std::move(reason));
    }
    static Refusal invalid(std::string reason) {
        return Refusal(Kind::Invalid, std::move(reason));
    }
    static Refusal failed(std::string reason) {
        return Refusal(Kind::Failed, std::move(reason));
    }
    const char* what() const noexcept override {
        return reason.c_str();
    }
};

// One kernel: the functions of doc/api.md. A method that declines throws a `Refusal`; any other
// exception is a crash, unless `refusal_for` reads it as a refusal.
template <class S>
class Kernel {
  public:
    using Solid = S;
    using Solids = std::vector<const Solid*>;

    virtual ~Kernel() = default;

    virtual std::string name() const = 0;
    virtual std::string version() const = 0;
    // The linear tolerance the kernel declares, mm: the loosest still governing its results.
    virtual double tolerance() const = 0;
    // What the adapter set differently from the kernel's defaults; empty when nothing.
    virtual std::string configuration() const {
        return "";
    }
    // The functions of `FUNCTIONS` this kernel does not offer, by name: `hello` lists the rest as
    // supported, and the suite does not run a case that needs one of these.
    virtual std::vector<std::string> unsupported() const {
        return {};
    }
    // The refusal an exception other than `Refusal` stands for, such as a kernel's own error
    // class; none makes it a crash.
    virtual std::optional<Refusal> refusal_for(std::exception_ptr) const {
        return std::nullopt;
    }

    // Every constructor, each an alternative of `Primitive`.
    virtual Solid make(const Primitive& p) const = 0;
    virtual Solid unite(const Solid& a, const Solid& b) const = 0;
    virtual Solid intersection(const Solid& a, const Solid& b) const = 0;
    virtual Solid difference(const Solid& a, const Solid& b) const = 0;
    virtual double volume(const Solid& s) const = 0;
    virtual double area(const Solid& s) const = 0;
    // Validity problems the kernel's own checker finds; empty when valid.
    virtual std::vector<std::string> check(const Solid& s) const = 0;

    virtual std::vector<Side> classify(const Solid&, const std::vector<Vec3>&) const {
        throw Refusal::unsupported("classify is not supported");
    }
    virtual std::vector<Vec3> closest(const Solid&, const std::vector<Vec3>&) const {
        throw Refusal::unsupported("closest is not supported");
    }
    // `nullopt` at a crease, or off the boundary.
    virtual std::vector<std::optional<Vec3>> normal(const Solid&, const std::vector<Vec3>&) const {
        throw Refusal::unsupported("normal is not supported");
    }
    virtual Solid fillet(const Solid&, const std::vector<Vec3>&, double) const {
        throw Refusal::unsupported("fillet is not supported");
    }
    virtual Solid
    fillet_radii(const Solid&, const std::vector<Vec3>&, const std::vector<double>&) const {
        throw Refusal::unsupported("fillet_radii is not supported");
    }
    virtual Solid
    chamfer_distances(const Solid&, const std::vector<Vec3>&, const std::vector<double>&) const {
        throw Refusal::unsupported("chamfer_distances is not supported");
    }
    virtual Solid chamfer_two(const Solid&,
                              const Vec3& /*edge*/,
                              const Vec3& /*face*/,
                              double /*d1*/,
                              double /*d2*/) const {
        throw Refusal::unsupported("chamfer_two is not supported");
    }
    virtual Solid fillet_variable(const Solid&,
                                  const Vec3& /*edge*/,
                                  const Vec3& /*from*/,
                                  double /*r_from*/,
                                  double /*r_to*/) const {
        throw Refusal::unsupported("fillet_variable is not supported");
    }
    virtual Solid
    fillet_g2(const Solid&, const std::vector<Vec3>&, double /*offset*/, double /*center*/) const {
        throw Refusal::unsupported("fillet_g2 is not supported");
    }
    virtual Solid
    fillet_conic(const Solid&, const std::vector<Vec3>&, double /*setback*/, double /*rho*/) const {
        throw Refusal::unsupported("fillet_conic is not supported");
    }
    virtual Solid fillet_setback(const Solid&,
                                 const std::vector<Vec3>&,
                                 double /*radius*/,
                                 double /*setback*/) const {
        throw Refusal::unsupported("fillet_setback is not supported");
    }
    virtual Solid fillet_setback_g2(const Solid&,
                                    const std::vector<Vec3>&,
                                    double /*radius*/,
                                    double /*setback*/) const {
        throw Refusal::unsupported("fillet_setback_g2 is not supported");
    }
    virtual Solid fillet_faces(const Solid&,
                               const Vec3& /*face_a*/,
                               const Vec3& /*face_b*/,
                               double /*radius*/) const {
        throw Refusal::unsupported("fillet_faces is not supported");
    }
    virtual Solid full_round(const Solid&,
                             const Vec3& /*side_a*/,
                             const Vec3& /*middle*/,
                             const Vec3& /*side_b*/) const {
        throw Refusal::unsupported("full_round is not supported");
    }
    // `[k1, k2]`, `k1 ≥ k2`; `nullopt` where the nearest point lies on a crease.
    virtual std::vector<std::optional<Vec2>> curvature(const Solid&,
                                                       const std::vector<Vec3>&) const {
        throw Refusal::unsupported("curvature is not supported");
    }
    virtual Projection
    project(const Solid&, const Vec3& /*direction*/, double /*tolerance*/) const {
        throw Refusal::unsupported("project is not supported");
    }
    virtual Solid chamfer(const Solid&, const std::vector<Vec3>&, double) const {
        throw Refusal::unsupported("chamfer is not supported");
    }
    virtual std::string write_step(const Solid&) const {
        throw Refusal::unsupported("write_step is not supported");
    }
    virtual Solid read_step(const std::string&) const {
        throw Refusal::unsupported("read_step is not supported");
    }
    // `nullopt` for an empty solid.
    virtual std::optional<Vec3> centroid(const Solid&) const {
        throw Refusal::unsupported("centroid is not supported");
    }
    virtual std::array<Vec3, 3> inertia(const Solid&) const {
        throw Refusal::unsupported("inertia is not supported");
    }
    virtual double distance(const Solid&, const Solid&) const {
        throw Refusal::unsupported("distance is not supported");
    }
    virtual std::array<std::uint64_t, 3> betti(const Solid&) const {
        throw Refusal::unsupported("betti is not supported");
    }
    virtual double
    section_area(const Solid&, const Vec3& /*origin*/, const Vec3& /*normal*/) const {
        throw Refusal::unsupported("section_area is not supported");
    }
    virtual Mesh mesh(const Solid&, double /*tolerance*/) const {
        throw Refusal::unsupported("mesh is not supported");
    }
    virtual Display display(const Solid&, double /*tolerance*/) const {
        throw Refusal::unsupported("display is not supported");
    }
    virtual Solid offset(const Solid&, double /*distance*/) const {
        throw Refusal::unsupported("offset is not supported");
    }
    virtual Solid
    shell(const Solid&, const std::vector<Vec3>& /*open*/, double /*thickness*/) const {
        throw Refusal::unsupported("shell is not supported");
    }
    virtual Solid draft(const Solid&,
                        const std::vector<Vec3>& /*faces*/,
                        const Vec3& /*origin*/,
                        const Vec3& /*pull*/,
                        double /*angle*/) const {
        throw Refusal::unsupported("draft is not supported");
    }
    virtual Solid transform(const Solid&, const Matrix&) const {
        throw Refusal::unsupported("transform is not supported");
    }
    virtual Solid affine(const Solid&, const Matrix&) const {
        throw Refusal::unsupported("affine is not supported");
    }
    virtual std::vector<double>
    raycast(const Solid&, const Vec3& /*origin*/, const Vec3& /*direction*/) const {
        throw Refusal::unsupported("raycast is not supported");
    }
    // `raycast` for many rays, each an origin and a direction. A kernel that prepares a solid for
    // casting overrides it to prepare once.
    virtual std::vector<std::vector<double>>
    raycasts(const Solid& s, const std::vector<std::pair<Vec3, Vec3>>& rays) const {
        std::vector<std::vector<double>> out;
        out.reserve(rays.size());
        for (const auto& [origin, direction] : rays) {
            out.push_back(raycast(s, origin, direction));
        }
        return out;
    }
    virtual Meeting interference(const Solid&, const Solid&) const {
        throw Refusal::unsupported("interference is not supported");
    }
    // The kernel's own union of many solids in one call; never a run of pairwise unions.
    virtual Solid union_all(const Solids&) const {
        throw Refusal::unsupported("union_all is not supported");
    }
    virtual Solid difference_all(const Solid& /*target*/, const Solids& /*tools*/) const {
        throw Refusal::unsupported("difference_all is not supported");
    }
    virtual Solid
    offset_faces(const Solid&, const std::vector<Vec3>& /*faces*/, double /*distance*/) const {
        throw Refusal::unsupported("offset_faces is not supported");
    }
    virtual Solid loft_through(const Frame&,
                               const std::vector<Profile>&,
                               const std::vector<double>& /*heights*/) const {
        throw Refusal::unsupported("loft_through is not supported");
    }
    virtual Solid delete_faces(const Solid&, const std::vector<Vec3>& /*faces*/) const {
        throw Refusal::unsupported("delete_faces is not supported");
    }
    virtual Solid
    move_faces(const Solid&, const std::vector<Vec3>& /*faces*/, const Matrix&) const {
        throw Refusal::unsupported("move_faces is not supported");
    }
    virtual Solid
    thicken(const Solid&, const std::vector<Vec3>& /*faces*/, double /*thickness*/) const {
        throw Refusal::unsupported("thicken is not supported");
    }
    // The part on the normal's side, then the part on the other.
    virtual std::pair<Solid, Solid>
    split(const Solid&, const Vec3& /*origin*/, const Vec3& /*normal*/) const {
        throw Refusal::unsupported("split is not supported");
    }
    // The low and high corners.
    virtual std::array<Vec3, 2> bounding_box(const Solid&) const {
        throw Refusal::unsupported("bounding_box is not supported");
    }
    virtual std::vector<std::vector<Vec3>> section(const Solid&,
                                                   const Vec3& /*origin*/,
                                                   const Vec3& /*normal*/,
                                                   double /*tolerance*/) const {
        throw Refusal::unsupported("section is not supported");
    }
    virtual Solid sew(const std::string& /*text*/, double /*tolerance*/) const {
        throw Refusal::unsupported("sew is not supported");
    }
    // Releases every solid this caller made, once it is done with them.
    virtual void reset() const {}

    // Whether the solid holds each point, its boundary counted in: `classify` read as yes or no.
    virtual std::vector<bool> holds(const Solid& s, const std::vector<Vec3>& points) const {
        std::vector<bool> out;
        for (Side side : classify(s, points)) {
            out.push_back(side != Side::Out);
        }
        return out;
    }

    virtual Solid boolean(Op op, const Solid& a, const Solid& b) const {
        switch (op) {
        case Op::Union:
            return unite(a, b);
        case Op::Intersection:
            return intersection(a, b);
        default:
            return difference(a, b);
        }
    }

    // A checker that declines a solid has not found it valid: its reason is an issue, unless the
    // kernel has no checker to ask. A measure the kernel could not take is no number, and its
    // reason an issue.
    virtual Measure measure(const Solid& s) const {
        Measure out{std::nan(""), std::nan(""), {}};
        try {
            out.issues = check(s);
        } catch (const Refusal& e) {
            if (e.kind != Kind::Unsupported) {
                out.issues.push_back(std::string("the checker declined the solid, ") +
                                     word(e.kind) + ": " + e.reason);
            }
        }
        const auto taken = [&](const char* name, double (Kernel::*method)(const Solid&) const) {
            try {
                return (this->*method)(s);
            } catch (const Refusal& e) {
                out.issues.push_back(std::string(name) + ": " + word(e.kind) + ": " + e.reason);
                return std::nan("");
            }
        };
        out.volume = taken("volume", &Kernel::volume);
        out.area = taken("area", &Kernel::area);
        return out;
    }
};

// What serve is made of; an adapter has no need of it.
namespace detail {

namespace wire {

// Reading the values of a request. A value of the wrong format is a failed request, as in the Rust
// binding.

inline const Json& at(const Json& v, const char* key) {
    static const Json NONE;
    if (!v.is_object()) {
        return NONE;
    }
    const auto it = v.find(key);
    return it == v.end() ? NONE : *it;
}

inline double number(const Json& v, const char* what) {
    if (!v.is_number()) {
        throw Refusal::failed(std::string("`") + what + "` is not a number");
    }
    return v.get<double>();
}

inline double num(const Json& v, const char* key) {
    return number(at(v, key), key);
}

// A whole number from 0 to 2⁵³; `5.0` is `5`.
inline std::uint64_t count(const Json& v, const char* what) {
    if (v.is_number_unsigned()) {
        return v.get<std::uint64_t>();
    }
    if (v.is_number()) {
        const double d = v.get<double>();
        if (d >= 0 && d <= 9007199254740992.0 && std::floor(d) == d) {
            return static_cast<std::uint64_t>(d);
        }
    }
    throw Refusal::failed(std::string("`") + what + "` is not a whole number");
}

inline const Json::array_t& list(const Json& v, const char* what) {
    if (!v.is_array()) {
        throw Refusal::failed(std::string("`") + what + "` is not a list");
    }
    return v.get_ref<const Json::array_t&>();
}

inline std::vector<double> numbers(const Json& v, const char* what) {
    std::vector<double> out;
    for (const Json& x : list(v, what)) {
        out.push_back(number(x, what));
    }
    return out;
}

template <std::size_t N>
std::array<double, N> fixed(const Json& v, const char* what) {
    const auto& items = list(v, what);
    if (items.size() != N) {
        throw Refusal::failed(std::string("`") + what + "` is not " + std::to_string(N) +
                              " numbers");
    }
    std::array<double, N> out{};
    for (std::size_t i = 0; i < N; i++) {
        out[i] = number(items[i], what);
    }
    return out;
}

inline Vec2 vec2(const Json& v, const char* what) {
    return fixed<2>(v, what);
}
inline Vec3 vec3(const Json& v, const char* what) {
    return fixed<3>(v, what);
}
inline Vec3 vec3_at(const Json& v, const char* key) {
    return vec3(at(v, key), key);
}

inline std::vector<Vec3> points(const Json& v, const char* what) {
    std::vector<Vec3> out;
    for (const Json& p : list(v, what)) {
        out.push_back(vec3(p, what));
    }
    return out;
}

inline std::vector<Vec3> points_at(const Json& v, const char* key) {
    return points(at(v, key), key);
}

inline Matrix matrix(const Json& v) {
    const auto& rows = list(v, "matrix");
    if (rows.size() != 3) {
        throw Refusal::failed("`matrix` is not three rows");
    }
    Matrix m{};
    for (std::size_t i = 0; i < 3; i++) {
        m[i] = fixed<4>(rows[i], "matrix");
    }
    return m;
}

inline std::string text(const Json& v, const char* key) {
    const Json& t = at(v, key);
    if (!t.is_string()) {
        throw Refusal::failed(std::string("`") + key + "` is not a string");
    }
    return t.get<std::string>();
}

inline Frame frame(const Json& v) {
    return Frame{vec3_at(v, "origin"), vec3_at(v, "x"), vec3_at(v, "y"), vec3_at(v, "z")};
}

inline Spline spline(const Json& s) {
    Spline out{static_cast<std::size_t>(count(at(s, "degree"), "degree")),
               numbers(at(s, "knots"), "knots"),
               {}};
    for (const Json& p : list(at(s, "ctrl"), "ctrl")) {
        out.ctrl.push_back(vec2(p, "ctrl"));
    }
    return out;
}

inline Segment segment(const Json& v) {
    if (v.contains("line")) {
        const auto& ends = list(v["line"], "line");
        if (ends.size() != 2) {
            throw Refusal::failed("a line is two points");
        }
        return Line{vec2(ends[0], "line"), vec2(ends[1], "line")};
    }
    if (v.contains("spline")) {
        return spline(v["spline"]);
    }
    if (v.contains("gn")) {
        const Json& c = v["gn"];
        const std::uint64_t order = count(at(c, "order"), "order");
        if (order > 0xFFFFFFFFu) {
            throw Refusal::failed("`order` is too large");
        }
        return Gn{vec2(at(c, "a"), "a"),
                  vec2(at(c, "apex"), "apex"),
                  vec2(at(c, "b"), "b"),
                  static_cast<unsigned>(order),
                  num(c, "w")};
    }
    if (v.contains("conic")) {
        const Json& c = v["conic"];
        return Conic{
            vec2(at(c, "a"), "a"), vec2(at(c, "apex"), "apex"), vec2(at(c, "b"), "b"), num(c, "w")};
    }
    if (!v.is_object() || !v.contains("arc")) {
        throw Refusal::failed("a segment is a `line`, an `arc`, a `conic`, a `gn` or a `spline`");
    }
    const Json& a = v["arc"];
    return Arc{vec2(at(a, "center"), "center"), num(a, "radius"), num(a, "start"), num(a, "sweep")};
}

inline Profile profile(const Json& v) {
    Profile out;
    for (const Json& loop : list(v, "profile")) {
        Loop segments;
        for (const Json& s : list(loop, "loop")) {
            segments.push_back(segment(s));
        }
        out.loops.push_back(std::move(segments));
    }
    return out;
}

inline Vec2 scaled(const Vec2& p, double s) {
    return {p[0] * s, p[1] * s};
}

// The profile scaled by `s` about the origin: a loft's `top` where a request leaves it out.
inline Profile scaled(const Profile& p, double s) {
    Profile out = p;
    for (Loop& loop : out.loops) {
        for (Segment& seg : loop) {
            std::visit(
                [s](auto& g) {
                    using T = std::decay_t<decltype(g)>;
                    if constexpr (std::is_same_v<T, Line>) {
                        g.a = scaled(g.a, s);
                        g.b = scaled(g.b, s);
                    } else if constexpr (std::is_same_v<T, Arc>) {
                        g.center = scaled(g.center, s);
                        g.radius *= s;
                    } else if constexpr (std::is_same_v<T, Spline>) {
                        for (Vec2& c : g.ctrl) {
                            c = scaled(c, s);
                        }
                    } else {
                        g.a = scaled(g.a, s);
                        g.apex = scaled(g.apex, s);
                        g.b = scaled(g.b, s);
                    }
                },
                seg);
        }
    }
    return out;
}

inline std::vector<PathPiece> path(const Json& v) {
    std::vector<PathPiece> out;
    for (const Json& piece : list(v, "path")) {
        if (piece.is_object() && piece.contains("line")) {
            out.push_back(PathLine{number(piece["line"], "line")});
        } else if (piece.is_object() && piece.contains("arc")) {
            out.push_back(PathArc{num(piece["arc"], "radius"), num(piece["arc"], "angle")});
        } else {
            throw Refusal::failed("a path piece is a `line` or an `arc`");
        }
    }
    return out;
}

inline Spline3 spline3(const Json& v) {
    return Spline3{static_cast<std::size_t>(count(at(v, "degree"), "degree")),
                   numbers(at(v, "knots"), "knots"),
                   points_at(v, "ctrl")};
}

// The primitive a shape's JSON or a constructor call describes.
inline Primitive primitive(const Json& v) {
    // Its kind is `kind`, as a shape's JSON names it, or `cmd`, as a constructor call does.
    const Json& named = at(v, "kind").is_string() ? at(v, "kind") : at(v, "cmd");
    const std::string kind = named.is_string() ? named.get<std::string>() : "";
    const Frame f = frame(at(v, "frame"));
    const auto prof = [&] { return profile(at(v, "profile")); };
    if (kind == "box") {
        return Box{f, vec3_at(v, "size")};
    }
    if (kind == "cylinder") {
        return Cylinder{f, num(v, "radius"), num(v, "height")};
    }
    if (kind == "sphere") {
        return Sphere{f, num(v, "radius")};
    }
    if (kind == "torus") {
        return Torus{f, num(v, "major"), num(v, "minor")};
    }
    if (kind == "cone") {
        return Cone{f, num(v, "bottom"), num(v, "top"), num(v, "height")};
    }
    if (kind == "extrude") {
        return Extrude{f, prof(), num(v, "height")};
    }
    if (kind == "revolve") {
        return Revolve{f, prof(), num(v, "angle")};
    }
    if (kind == "screw") {
        return Screw{f, prof(), num(v, "rise"), num(v, "angle")};
    }
    if (kind == "loft") {
        const Profile p = prof();
        const double scale = num(v, "scale");
        const Json& top = at(v, "top");
        return Loft{f, p, scale, num(v, "height"), top.is_null() ? scaled(p, scale) : profile(top)};
    }
    if (kind == "sweep") {
        return Sweep{f, prof(), path(at(v, "path"))};
    }
    if (kind == "sweep_path") {
        return SweepPath{f, prof(), spline3(at(v, "path")), num(v, "twist")};
    }
    if (kind == "slab") {
        std::vector<std::vector<double>> heights;
        for (const Json& row : list(at(v, "heights"), "heights")) {
            heights.push_back(numbers(row, "heights"));
        }
        return Slab{f,
                    vec2(at(v, "size"), "size"),
                    numbers(at(v, "knots_u"), "knots_u"),
                    numbers(at(v, "knots_v"), "knots_v"),
                    std::move(heights)};
    }
    throw Refusal::unsupported("unknown constructor `" + kind + "`");
}

inline Json point(const Vec3& p) {
    return Json::array({p[0], p[1], p[2]});
}
inline Json point(const Vec2& p) {
    return Json::array({p[0], p[1]});
}

template <class P>
Json polylines(const std::vector<std::vector<P>>& lines) {
    Json out = Json::array();
    for (const auto& line : lines) {
        Json l = Json::array();
        for (const P& p : line) {
            l.push_back(point(p));
        }
        out.push_back(std::move(l));
    }
    return out;
}

inline Json triangles(const std::vector<std::array<std::uint32_t, 3>>& ts) {
    Json out = Json::array();
    for (const auto& t : ts) {
        out.push_back(Json::array({t[0], t[1], t[2]}));
    }
    return out;
}

inline Json positions(const std::vector<Vec3>& ps) {
    Json out = Json::array();
    for (const Vec3& p : ps) {
        out.push_back(point(p));
    }
    return out;
}

} // namespace wire

// The commands that construct a primitive.
inline bool is_constructor(const std::string& c) {
    for (const char* k : {"box",
                          "cylinder",
                          "sphere",
                          "torus",
                          "cone",
                          "extrude",
                          "revolve",
                          "screw",
                          "loft",
                          "sweep",
                          "slab",
                          "sweep_path"}) {
        if (c == k) {
            return true;
        }
    }
    return false;
}

// One worker's solids, under the handles it issued.
template <class Solid>
class Store {
  public:
    std::uint64_t put(Solid s) {
        const std::uint64_t h = next_++;
        solids_.emplace(h, std::move(s));
        return h;
    }
    const Solid& get(const Json& v) const {
        if (!v.is_number_unsigned()) {
            throw Refusal::failed(v.dump() + " is not a handle");
        }
        const auto h = v.get<std::uint64_t>();
        const auto it = solids_.find(h);
        if (it == solids_.end()) {
            throw Refusal::failed("no solid " + std::to_string(h));
        }
        return it->second;
    }
    void clear() {
        solids_.clear();
    }

  private:
    std::map<std::uint64_t, Solid> solids_;
    std::uint64_t next_ = 1;
};

// The value one request returns, or the refusal it throws.
template <class S>
Json answer(const Kernel<S>& k, const Json& req, Store<S>& solids) {
    using wire::at;
    const auto solid = [&](const char* key) -> const S& { return solids.get(at(req, key)); };
    const auto num = [&](const char* key) { return wire::num(req, key); };
    const auto vec = [&](const char* key) { return wire::vec3_at(req, key); };
    const auto pts = [&](const char* key) { return wire::points_at(req, key); };
    const auto made = [&](S s) { return Json(solids.put(std::move(s))); };
    const std::string c = at(req, "cmd").is_string() ? req["cmd"].get<std::string>() : "";

    if (c == "hello") {
        const std::vector<std::string> without = k.unsupported();
        Json supports = Json::array();
        for (const char* f : FUNCTIONS) {
            bool missing = false;
            for (const std::string& u : without) {
                missing = missing || u == f;
            }
            if (!missing) {
                supports.push_back(f);
            }
        }
        return Json{{"name", k.name()},
                    {"version", k.version()},
                    {"tolerance", k.tolerance()},
                    {"protocol", PROTOCOL_VERSION},
                    {"supports", supports},
                    {"configuration", k.configuration()}};
    }
    if (c == "reset") {
        solids.clear();
        return nullptr;
    }
    if (is_constructor(c)) {
        return made(k.make(wire::primitive(req)));
    }
    if (c == "union") {
        return made(k.unite(solid("a"), solid("b")));
    }
    if (c == "intersection") {
        return made(k.intersection(solid("a"), solid("b")));
    }
    if (c == "difference") {
        return made(k.difference(solid("a"), solid("b")));
    }
    if (c == "union_all" || c == "difference_all") {
        const auto handles = [&](const char* key) {
            typename Kernel<S>::Solids out;
            for (const Json& h : wire::list(at(req, key), key)) {
                out.push_back(&solids.get(h));
            }
            return out;
        };
        if (c == "union_all") {
            return made(k.union_all(handles("solids")));
        }
        return made(k.difference_all(solid("target"), handles("tools")));
    }
    if (c == "volume") {
        return k.volume(solid("s"));
    }
    if (c == "area") {
        return k.area(solid("s"));
    }
    if (c == "check") {
        return k.check(solid("s"));
    }
    if (c == "classify") {
        Json out = Json::array();
        for (Side side : k.classify(solid("s"), pts("points"))) {
            out.push_back(word(side));
        }
        return out;
    }
    if (c == "closest") {
        return wire::positions(k.closest(solid("s"), pts("points")));
    }
    if (c == "normal") {
        Json out = Json::array();
        for (const auto& n : k.normal(solid("s"), pts("points"))) {
            out.push_back(n ? wire::point(*n) : Json(nullptr));
        }
        return out;
    }
    if (c == "curvature") {
        Json out = Json::array();
        for (const auto& kk : k.curvature(solid("s"), pts("points"))) {
            out.push_back(kk ? wire::point(*kk) : Json(nullptr));
        }
        return out;
    }
    if (c == "fillet") {
        return made(k.fillet(solid("s"), pts("edges"), num("radius")));
    }
    if (c == "chamfer") {
        return made(k.chamfer(solid("s"), pts("edges"), num("distance")));
    }
    if (c == "fillet_radii" || c == "chamfer_distances") {
        const char* key = c == "fillet_radii" ? "radii" : "distances";
        const std::vector<Vec3> edges = pts("edges");
        const std::vector<double> sizes = wire::numbers(at(req, key), key);
        if (sizes.size() != edges.size()) {
            throw Refusal::failed(std::to_string(edges.size()) + " edges but " +
                                  std::to_string(sizes.size()) + " " + key);
        }
        if (c == "fillet_radii") {
            return made(k.fillet_radii(solid("s"), edges, sizes));
        }
        return made(k.chamfer_distances(solid("s"), edges, sizes));
    }
    if (c == "chamfer_two") {
        return made(k.chamfer_two(solid("s"), vec("edge"), vec("face"), num("d1"), num("d2")));
    }
    if (c == "fillet_variable") {
        return made(
            k.fillet_variable(solid("s"), vec("edge"), vec("from"), num("r_from"), num("r_to")));
    }
    if (c == "fillet_g2") {
        return made(k.fillet_g2(solid("s"), pts("edges"), num("offset"), num("center")));
    }
    if (c == "fillet_conic") {
        return made(k.fillet_conic(solid("s"), pts("edges"), num("setback"), num("rho")));
    }
    if (c == "fillet_setback") {
        return made(k.fillet_setback(solid("s"), pts("edges"), num("radius"), num("setback")));
    }
    if (c == "fillet_setback_g2") {
        return made(k.fillet_setback_g2(solid("s"), pts("edges"), num("radius"), num("setback")));
    }
    if (c == "fillet_faces") {
        return made(k.fillet_faces(solid("s"), vec("face_a"), vec("face_b"), num("radius")));
    }
    if (c == "full_round") {
        return made(k.full_round(solid("s"), vec("side_a"), vec("middle"), vec("side_b")));
    }
    if (c == "offset") {
        return made(k.offset(solid("s"), num("distance")));
    }
    if (c == "offset_faces") {
        return made(k.offset_faces(solid("s"), pts("faces"), num("distance")));
    }
    if (c == "delete_faces") {
        return made(k.delete_faces(solid("s"), pts("faces")));
    }
    if (c == "move_faces") {
        return made(k.move_faces(solid("s"), pts("faces"), wire::matrix(at(req, "matrix"))));
    }
    if (c == "thicken") {
        return made(k.thicken(solid("s"), pts("faces"), num("thickness")));
    }
    if (c == "shell") {
        return made(k.shell(solid("s"), pts("open"), num("thickness")));
    }
    if (c == "draft") {
        return made(k.draft(solid("s"), pts("faces"), vec("origin"), vec("pull"), num("angle")));
    }
    if (c == "transform") {
        return made(k.transform(solid("s"), wire::matrix(at(req, "matrix"))));
    }
    if (c == "affine") {
        return made(k.affine(solid("s"), wire::matrix(at(req, "matrix"))));
    }
    if (c == "loft_through") {
        const Frame f = wire::frame(at(req, "frame"));
        std::vector<Profile> profiles;
        for (const Json& p : wire::list(at(req, "profiles"), "profiles")) {
            profiles.push_back(wire::profile(p));
        }
        const std::vector<double> heights = wire::numbers(at(req, "heights"), "heights");
        if (heights.size() != profiles.size()) {
            throw Refusal::invalid(std::to_string(profiles.size()) + " profiles but " +
                                   std::to_string(heights.size()) + " heights");
        }
        return made(k.loft_through(f, profiles, heights));
    }
    if (c == "write_step") {
        return k.write_step(solid("s"));
    }
    if (c == "read_step") {
        return made(k.read_step(wire::text(req, "text")));
    }
    if (c == "sew") {
        const std::string t = wire::text(req, "text");
        return made(k.sew(t, num("tolerance")));
    }
    if (c == "centroid") {
        const auto p = k.centroid(solid("s"));
        return p ? wire::point(*p) : Json(nullptr);
    }
    if (c == "inertia") {
        const auto m = k.inertia(solid("s"));
        return Json::array({wire::point(m[0]), wire::point(m[1]), wire::point(m[2])});
    }
    if (c == "distance") {
        return k.distance(solid("a"), solid("b"));
    }
    if (c == "interference") {
        const Meeting m = k.interference(solid("a"), solid("b"));
        return Json{{"volume", m.volume}, {"gap", m.gap}};
    }
    if (c == "betti") {
        const auto b = k.betti(solid("s"));
        return Json::array({b[0], b[1], b[2]});
    }
    if (c == "section_area") {
        return k.section_area(solid("s"), vec("origin"), vec("normal"));
    }
    if (c == "section") {
        const S& s = solid("s");
        const Vec3 o = vec("origin");
        const Vec3 n = vec("normal");
        return wire::polylines(k.section(s, o, n, num("tolerance")));
    }
    if (c == "split") {
        auto parts = k.split(solid("s"), vec("origin"), vec("normal"));
        const auto above = solids.put(std::move(parts.first));
        const auto below = solids.put(std::move(parts.second));
        return Json::array({above, below});
    }
    if (c == "bounding_box") {
        const auto b = k.bounding_box(solid("s"));
        return Json::array({wire::point(b[0]), wire::point(b[1])});
    }
    if (c == "project") {
        const S& s = solid("s");
        const Vec3 d = vec("direction");
        const Projection p = k.project(s, d, num("tolerance"));
        return Json{{"visible", wire::polylines(p.visible)}, {"hidden", wire::polylines(p.hidden)}};
    }
    if (c == "mesh") {
        const Mesh m = k.mesh(solid("s"), num("tolerance"));
        return Json{{"positions", wire::positions(m.positions)},
                    {"triangles", wire::triangles(m.triangles)}};
    }
    if (c == "display") {
        const Display d = k.display(solid("s"), num("tolerance"));
        Json faces = Json::array();
        for (const DisplayFace& f : d.faces) {
            faces.push_back(Json{{"positions", wire::positions(f.positions)},
                                 {"normals", wire::positions(f.normals)},
                                 {"triangles", wire::triangles(f.triangles)}});
        }
        return Json{{"faces", std::move(faces)}, {"edges", wire::polylines(d.edges)}};
    }
    if (c == "raycast") {
        const S& s = solid("s");
        if (!at(req, "rays").is_null()) {
            std::vector<std::pair<Vec3, Vec3>> rays;
            for (const Json& r : wire::list(req["rays"], "rays")) {
                const auto& ends = wire::list(r, "rays");
                if (ends.size() != 2) {
                    throw Refusal::failed("a ray is an origin and a direction");
                }
                rays.emplace_back(wire::vec3(ends[0], "rays"), wire::vec3(ends[1], "rays"));
            }
            return k.raycasts(s, rays);
        }
        return k.raycast(s, vec("origin"), vec("direction"));
    }
    throw Refusal::unsupported("unknown command `" + c + "`");
}

// One request line's reply: `ok`, `err` for a refusal, or `crash` for any other exception, after
// which the worker carries on.
template <class S>
Json reply(const Kernel<S>& k, const std::string& line, Store<S>& solids) {
    Json req;
    try {
        req = Json::parse(line);
    } catch (const Json::exception& e) {
        return Json{{"err", std::string("not JSON: ") + e.what()}};
    }
    try {
        try {
            return Json{{"ok", answer(k, req, solids)}};
        } catch (const Refusal&) {
            throw;
        } catch (...) {
            if (auto r = k.refusal_for(std::current_exception())) {
                throw *r;
            }
            throw;
        }
    } catch (const Refusal& r) {
        return Json{{"err", r.reason}, {"kind", word(r.kind)}};
    } catch (const std::exception& e) {
        return Json{{"crash", e.what()}};
    } catch (...) {
        return Json{{"crash", "an exception that is not a std::exception"}};
    }
}

} // namespace detail

// Answers requests from `in` until it closes, each reply one line on `out`.
template <class S>
void serve(const Kernel<S>& k, std::istream& in, std::FILE* out) {
    detail::Store<S> solids;
    std::string line;
    while (std::getline(in, line)) {
        const std::string text =
            detail::reply(k, line, solids).dump(-1, ' ', false, Json::error_handler_t::replace) +
            "\n";
        std::fwrite(text.data(), 1, text.size(), out);
        std::fflush(out);
    }
}

// Answers the protocol on stdin and stdout until stdin closes.
template <class S>
int serve(const Kernel<S>& k) {
    std::ios::sync_with_stdio(false);
    serve(k, std::cin, stdout);
    return 0;
}

} // namespace brep_protocol

// SPDX-License-Identifier: MIT OR Apache-2.0
// A "kernel" that answers every function with the arguments it was handed, for the wire parity test
// in protocol/parity. Its Rust twin, protocol/examples/echo_kernel.rs, computes the same answers, so
// any difference in their replies is a difference in how the two bindings read a request or write a
// reply.
//
// A solid is the JSON of the call that made it, with the solids it was made from nested in it;
// `write_step` returns that JSON as text. A query answers with numbers taken from its arguments:
// `fold` adds up every number of a solid, and `count` counts them.
#include <brep_protocol/brep_protocol.h>

#include <cstdint>
#include <limits>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <vector>

using namespace brep_protocol;

namespace {

double fold(const Json& v) {
    if (v.is_number()) {
        return v.get<double>();
    }
    double sum = 0;
    if (v.is_structured()) {
        for (const Json& item : v) {
            sum += fold(item);
        }
    }
    return sum;
}

std::uint64_t count(const Json& v) {
    if (v.is_number()) {
        return 1;
    }
    std::uint64_t n = 0;
    if (v.is_structured()) {
        for (const Json& item : v) {
            n += count(item);
        }
    }
    return n;
}

Json frame(const Frame& f) {
    return Json{{"origin", f.origin}, {"x", f.x}, {"y", f.y}, {"z", f.z}};
}

Json segment(const Segment& s) {
    return std::visit(
        [](const auto& seg) -> Json {
            using T = std::decay_t<decltype(seg)>;
            if constexpr (std::is_same_v<T, Line>) {
                return Json{{"line", Json::array({seg.a, seg.b})}};
            } else if constexpr (std::is_same_v<T, Arc>) {
                return Json{{"arc",
                             {{"center", seg.center},
                              {"radius", seg.radius},
                              {"start", seg.start},
                              {"sweep", seg.sweep}}}};
            } else if constexpr (std::is_same_v<T, Conic>) {
                return Json{
                    {"conic", {{"a", seg.a}, {"apex", seg.apex}, {"b", seg.b}, {"w", seg.w}}}};
            } else if constexpr (std::is_same_v<T, Gn>) {
                return Json{{"gn",
                             {{"a", seg.a},
                              {"apex", seg.apex},
                              {"b", seg.b},
                              {"order", seg.order},
                              {"w", seg.w}}}};
            } else {
                return Json{
                    {"spline", {{"degree", seg.degree}, {"knots", seg.knots}, {"ctrl", seg.ctrl}}}};
            }
        },
        s);
}

Json profile(const Profile& p) {
    Json loops = Json::array();
    for (const Loop& loop : p.loops) {
        Json segments = Json::array();
        for (const Segment& s : loop) {
            segments.push_back(segment(s));
        }
        loops.push_back(std::move(segments));
    }
    return loops;
}

Json path_piece(const PathPiece& piece) {
    if (const auto* line = std::get_if<PathLine>(&piece)) {
        return Json{{"line", line->length}};
    }
    const auto& arc = std::get<PathArc>(piece);
    return Json{{"arc", {{"radius", arc.radius}, {"angle", arc.angle}}}};
}

Json primitive(const Primitive& p) {
    return std::visit(
        [](const auto& q) -> Json {
            using T = std::decay_t<decltype(q)>;
            Json out{{"frame", frame(q.frame)}};
            if constexpr (std::is_same_v<T, Box>) {
                out["kind"] = "box";
                out["size"] = q.size;
            } else if constexpr (std::is_same_v<T, Cylinder>) {
                out["kind"] = "cylinder";
                out["radius"] = q.radius;
                out["height"] = q.height;
            } else if constexpr (std::is_same_v<T, Sphere>) {
                out["kind"] = "sphere";
                out["radius"] = q.radius;
            } else if constexpr (std::is_same_v<T, Torus>) {
                out["kind"] = "torus";
                out["major"] = q.major;
                out["minor"] = q.minor;
            } else if constexpr (std::is_same_v<T, Cone>) {
                out["kind"] = "cone";
                out["bottom"] = q.bottom;
                out["top"] = q.top;
                out["height"] = q.height;
            } else if constexpr (std::is_same_v<T, Extrude>) {
                out["kind"] = "extrude";
                out["profile"] = profile(q.profile);
                out["height"] = q.height;
            } else if constexpr (std::is_same_v<T, Revolve>) {
                out["kind"] = "revolve";
                out["profile"] = profile(q.profile);
                out["angle"] = q.angle;
            } else if constexpr (std::is_same_v<T, Screw>) {
                out["kind"] = "screw";
                out["profile"] = profile(q.profile);
                out["rise"] = q.rise;
                out["angle"] = q.angle;
            } else if constexpr (std::is_same_v<T, Loft>) {
                out["kind"] = "loft";
                out["profile"] = profile(q.profile);
                out["scale"] = q.scale;
                out["height"] = q.height;
                out["top"] = profile(q.top);
            } else if constexpr (std::is_same_v<T, Sweep>) {
                out["kind"] = "sweep";
                out["profile"] = profile(q.profile);
                Json path = Json::array();
                for (const PathPiece& piece : q.path) {
                    path.push_back(path_piece(piece));
                }
                out["path"] = std::move(path);
            } else if constexpr (std::is_same_v<T, SweepPath>) {
                out["kind"] = "sweep_path";
                out["profile"] = profile(q.profile);
                out["path"] = Json{
                    {"degree", q.path.degree}, {"knots", q.path.knots}, {"ctrl", q.path.ctrl}};
                out["twist"] = q.twist;
            } else {
                static_assert(std::is_same_v<T, Slab>);
                out["kind"] = "slab";
                out["size"] = q.size;
                out["knots_u"] = q.knots_u;
                out["knots_v"] = q.knots_v;
                out["heights"] = q.heights;
            }
            return out;
        },
        p);
}

Json solids(const std::vector<const Json*>& list) {
    Json out = Json::array();
    for (const Json* s : list) {
        out.push_back(*s);
    }
    return out;
}

// Points with a negative x stand for points off the boundary, so both arms of an optional reply are
// written.
bool present(const Vec3& p) {
    return p[0] >= 0;
}

class Echo : public Kernel<Json> {
  public:
    std::string name() const override {
        return "echo";
    }
    std::string version() const override {
        return "1";
    }
    double tolerance() const override {
        return 1e-7;
    }
    std::string configuration() const override {
        return "echoes its arguments";
    }
    std::vector<std::string> supports() const override {
        std::vector<std::string> all(FUNCTIONS.begin(), FUNCTIONS.end());
        all.push_back("not_a_function");
        return all;
    }

    Json make(const Primitive& p) override {
        return primitive(p);
    }
    Json unite(const Json& a, const Json& b) override {
        return Json{{"op", "union"}, {"a", a}, {"b", b}};
    }
    Json intersection(const Json& a, const Json& b) override {
        return Json{{"op", "intersection"}, {"a", a}, {"b", b}};
    }
    Json difference(const Json& a, const Json& b) override {
        return Json{{"op", "difference"}, {"a", a}, {"b", b}};
    }
    double volume(const Json& s) override {
        return fold(s);
    }
    double area(const Json& s) override {
        return static_cast<double>(count(s));
    }
    std::vector<std::string> check(const Json& s) override {
        return {std::to_string(count(s)) + " numbers", "checked"};
    }
    std::vector<Side> classify(const Json&, const std::vector<Vec3>& points) override {
        std::vector<Side> out;
        for (const Vec3& p : points) {
            out.push_back(p[0] < 0 ? Side::Out : p[0] == 0 ? Side::On : Side::In);
        }
        return out;
    }
    std::vector<Vec3> closest(const Json& s, const std::vector<Vec3>& points) override {
        std::vector<Vec3> out;
        for (const Vec3& p : points) {
            out.push_back({p[0], p[1], fold(s)});
        }
        return out;
    }
    std::vector<std::optional<Vec3>> normal(const Json&, const std::vector<Vec3>& points) override {
        std::vector<std::optional<Vec3>> out;
        for (const Vec3& p : points) {
            out.push_back(present(p) ? std::optional<Vec3>(p) : std::nullopt);
        }
        return out;
    }
    std::vector<std::optional<Vec2>> curvature(const Json&,
                                               const std::vector<Vec3>& points) override {
        std::vector<std::optional<Vec2>> out;
        for (const Vec3& p : points) {
            out.push_back(present(p) ? std::optional<Vec2>(Vec2{p[1], p[2]}) : std::nullopt);
        }
        return out;
    }
    Json fillet(const Json& s, const std::vector<Vec3>& edges, double radius) override {
        return Json{{"op", "fillet"}, {"s", s}, {"edges", edges}, {"radius", radius}};
    }
    Json fillet_radii(const Json& s,
                      const std::vector<Vec3>& edges,
                      const std::vector<double>& radii) override {
        return Json{{"op", "fillet_radii"}, {"s", s}, {"edges", edges}, {"radii", radii}};
    }
    Json chamfer_distances(const Json& s,
                           const std::vector<Vec3>& edges,
                           const std::vector<double>& distances) override {
        return Json{
            {"op", "chamfer_distances"}, {"s", s}, {"edges", edges}, {"distances", distances}};
    }
    Json chamfer_two(
        const Json& s, const Vec3& edge, const Vec3& face, double d1, double d2) override {
        return Json{{"op", "chamfer_two"},
                    {"s", s},
                    {"edge", edge},
                    {"face", face},
                    {"d1", d1},
                    {"d2", d2}};
    }
    Json fillet_variable(
        const Json& s, const Vec3& edge, const Vec3& from, double r_from, double r_to) override {
        return Json{{"op", "fillet_variable"},
                    {"s", s},
                    {"edge", edge},
                    {"from", from},
                    {"r_from", r_from},
                    {"r_to", r_to}};
    }
    Json fillet_g2(const Json& s,
                   const std::vector<Vec3>& edges,
                   double offset,
                   double center) override {
        return Json{
            {"op", "fillet_g2"}, {"s", s}, {"edges", edges}, {"offset", offset}, {"center", center}};
    }
    Json fillet_conic(const Json& s,
                      const std::vector<Vec3>& edges,
                      double setback,
                      double rho) override {
        return Json{
            {"op", "fillet_conic"}, {"s", s}, {"edges", edges}, {"setback", setback}, {"rho", rho}};
    }
    Json fillet_setback(const Json& s,
                        const std::vector<Vec3>& edges,
                        double radius,
                        double setback) override {
        return Json{{"op", "fillet_setback"},
                    {"s", s},
                    {"edges", edges},
                    {"radius", radius},
                    {"setback", setback}};
    }
    Json fillet_setback_g2(const Json& s,
                           const std::vector<Vec3>& edges,
                           double radius,
                           double setback) override {
        return Json{{"op", "fillet_setback_g2"},
                    {"s", s},
                    {"edges", edges},
                    {"radius", radius},
                    {"setback", setback}};
    }
    Json fillet_faces(const Json& s, const Vec3& face_a, const Vec3& face_b, double radius) override {
        return Json{{"op", "fillet_faces"},
                    {"s", s},
                    {"face_a", face_a},
                    {"face_b", face_b},
                    {"radius", radius}};
    }
    Json full_round(const Json& s,
                    const Vec3& side_a,
                    const Vec3& middle,
                    const Vec3& side_b) override {
        return Json{{"op", "full_round"},
                    {"s", s},
                    {"side_a", side_a},
                    {"middle", middle},
                    {"side_b", side_b}};
    }
    Projection project(const Json& s, const Vec3& direction, double tolerance) override {
        return Projection{{{Vec2{direction[0], direction[1]}, Vec2{direction[2], tolerance}}},
                          {{Vec2{fold(s), static_cast<double>(count(s))}}, {}}};
    }
    Json chamfer(const Json& s, const std::vector<Vec3>& edges, double distance) override {
        return Json{{"op", "chamfer"}, {"s", s}, {"edges", edges}, {"distance", distance}};
    }
    std::string write_step(const Json& s) override {
        return s.dump();
    }
    // Text `refuse <kind>` is refused with that kind, and `crash` throws, so each reply that is not
    // `ok` is written too.
    Json read_step(const std::string& text) override {
        const std::string refuse = "refuse ";
        if (text.compare(0, refuse.size(), refuse) == 0) {
            const std::string kind = text.substr(refuse.size());
            throw Refusal(kind == "unsupported" ? Kind::Unsupported
                          : kind == "invalid"   ? Kind::Invalid
                                                : Kind::Failed,
                          "echo refuses \"" + text + "\"");
        }
        if (text == "crash") {
            throw std::runtime_error("echo crashes");
        }
        return Json{{"op", "read_step"}, {"text", text}};
    }
    std::optional<Vec3> centroid(const Json& s) override {
        const std::uint64_t n = count(s);
        if (n == 0) {
            return std::nullopt;
        }
        return Vec3{fold(s), static_cast<double>(n), 0};
    }
    std::array<Vec3, 3> inertia(const Json& s) override {
        return {Vec3{fold(s), 0, 0}, Vec3{0, static_cast<double>(count(s)), 0}, Vec3{0, 0, 1}};
    }
    double distance(const Json& a, const Json& b) override {
        return fold(a) - fold(b);
    }
    std::array<std::uint64_t, 3> betti(const Json& s) override {
        return {count(s), 0, 1};
    }
    double section_area(const Json& s, const Vec3& origin, const Vec3& normal) override {
        return fold(s) + fold(Json::array({origin, normal})) * 1000.0;
    }
    Mesh mesh(const Json& s, double tolerance) override {
        return Mesh{{Vec3{tolerance, fold(s), 0}, Vec3{0, 0, static_cast<double>(count(s))}},
                    {{0, 1, 0}, {1, 0, std::numeric_limits<std::uint32_t>::max()}}};
    }
    Display display(const Json& s, double tolerance) override {
        DisplayFace face{{Vec3{tolerance, 0, 0}, Vec3{0, fold(s), 0}},
                         {Vec3{0, 0, 1}, Vec3{0, 1, 0}},
                         {{0, 1, 1}}};
        DisplayEdge edge{{Vec3{static_cast<double>(count(s)), 0, 0}, Vec3{0, 0, 0}},
                         {0, 1},
                         std::array<double, 2>{-tolerance, 1.5},
                         false};
        return Display{{face, DisplayFace{}},
                       {edge, DisplayEdge{{}, {0, 1, 1}, std::nullopt, true}, DisplayEdge{}},
                       {{Vec3{0, fold(s), 0}, Vec3{0, 0, 1}}}};
    }
    Json offset(const Json& s, double distance) override {
        return Json{{"op", "offset"}, {"s", s}, {"distance", distance}};
    }
    Json shell(const Json& s, const std::vector<Vec3>& open, double thickness) override {
        return Json{{"op", "shell"}, {"s", s}, {"open", open}, {"thickness", thickness}};
    }
    Json draft(const Json& s,
               const std::vector<Vec3>& faces,
               const Vec3& origin,
               const Vec3& pull,
               double angle) override {
        return Json{{"op", "draft"},
                    {"s", s},
                    {"faces", faces},
                    {"origin", origin},
                    {"pull", pull},
                    {"angle", angle}};
    }
    Json transform(const Json& s, const Matrix& matrix) override {
        return Json{{"op", "transform"}, {"s", s}, {"matrix", matrix}};
    }
    Json affine(const Json& s, const Matrix& matrix) override {
        return Json{{"op", "affine"}, {"s", s}, {"matrix", matrix}};
    }
    std::vector<double> raycast(const Json& s, const Vec3& origin, const Vec3& direction) override {
        return {fold(Json(origin)), fold(Json(direction)), fold(s)};
    }
    Meeting interference(const Json& a, const Json& b) override {
        return Meeting{fold(a), fold(b)};
    }
    Json union_all(const Solids& list) override {
        return Json{{"op", "union_all"}, {"solids", solids(list)}};
    }
    Json difference_all(const Json& target, const Solids& tools) override {
        return Json{{"op", "difference_all"}, {"target", target}, {"tools", solids(tools)}};
    }
    Json offset_faces(const Json& s, const std::vector<Vec3>& faces, double distance) override {
        return Json{{"op", "offset_faces"}, {"s", s}, {"faces", faces}, {"distance", distance}};
    }
    Json loft_through(const Frame& f,
                      const std::vector<Profile>& profiles,
                      const std::vector<double>& heights) override {
        Json written = Json::array();
        for (const Profile& p : profiles) {
            written.push_back(profile(p));
        }
        return Json{{"op", "loft_through"},
                    {"frame", frame(f)},
                    {"profiles", std::move(written)},
                    {"heights", heights}};
    }
    Json delete_faces(const Json& s, const std::vector<Vec3>& faces) override {
        return Json{{"op", "delete_faces"}, {"s", s}, {"faces", faces}};
    }
    Json move_faces(const Json& s, const std::vector<Vec3>& faces, const Matrix& matrix) override {
        return Json{{"op", "move_faces"}, {"s", s}, {"faces", faces}, {"matrix", matrix}};
    }
    Json thicken(const Json& s, const std::vector<Vec3>& faces, double thickness) override {
        return Json{{"op", "thicken"}, {"s", s}, {"faces", faces}, {"thickness", thickness}};
    }
    std::pair<Json, Json> split(const Json& s, const Vec3& origin, const Vec3& normal) override {
        const auto part = [&](const char* side) {
            return Json{{"op", "split"},
                        {"side", side},
                        {"s", s},
                        {"origin", origin},
                        {"normal", normal}};
        };
        return {part("above"), part("below")};
    }
    std::array<Vec3, 2> bounding_box(const Json& s) override {
        return {Vec3{0, 0, 0}, Vec3{fold(s), static_cast<double>(count(s)), -1}};
    }
    std::vector<std::vector<Vec3>>
    section(const Json& s, const Vec3& origin, const Vec3& normal, double tolerance) override {
        return {{origin, normal, Vec3{tolerance, fold(s), 0}}, {}};
    }
    Json sew(const std::string& text, double tolerance) override {
        return Json{{"op", "sew"}, {"text", text}, {"tolerance", tolerance}};
    }
};

} // namespace

int main() {
    Echo echo;
    return serve(echo);
}

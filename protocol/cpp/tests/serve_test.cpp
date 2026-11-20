// SPDX-License-Identifier: MIT
// The binding's own tests: what `answer` and `reply` make of requests, with kernels that do
// nothing.
#include <brep_protocol/brep_protocol.h>

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <stdexcept>
#include <string>
#include <vector>

using namespace brep_protocol;

namespace {

int failures = 0;

void expect(bool ok, const std::string& what) {
    if (!ok) {
        std::fprintf(stderr, "FAILED: %s\n", what.c_str());
        failures++;
    }
}

// A kernel that offers nothing beyond what the class makes it answer, and whose checker declines
// in the way given.
class Bare : public Kernel<int> {
  public:
    explicit Bare(Kind declines = Kind::Failed) : declines_(declines) {}
    std::string name() const override {
        return "bare";
    }
    std::string version() const override {
        return "0";
    }
    double tolerance() const override {
        return 1e-7;
    }
    std::vector<std::string> unsupported() const override {
        return {"slab", "sew"};
    }
    int make(const Primitive& p) override {
        return static_cast<int>(p.index());
    }
    int unite(const int&, const int&) override {
        return 0;
    }
    int intersection(const int&, const int&) override {
        return 0;
    }
    int difference(const int&, const int&) override {
        return 0;
    }
    double volume(const int&) override {
        throw std::runtime_error("lost");
    }
    double area(const int&) override {
        throw std::logic_error("mine");
    }
    std::vector<std::string> check(const int&) override {
        throw Refusal(declines_, "no verdict");
    }
    std::optional<Refusal> refusal_for(std::exception_ptr e) const override {
        try {
            std::rethrow_exception(e);
        } catch (const std::logic_error& l) {
            return Refusal::failed(std::string("logic: ") + l.what());
        } catch (...) {
            return std::nullopt;
        }
    }

  private:
    Kind declines_;
};

Json ask(Kernel<int>& k, detail::Store<int>& solids, const std::string& line) {
    return detail::reply(k, line, solids);
}

const std::string FRAME =
    R"("frame": {"origin": [1, 2, 3], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]})";
const std::string SQUARE =
    R"([[{"line": [[0, 0], [2, 0]]}, {"line": [[2, 0], [2, 2]]}, {"line": [[2, 2], [0, 2]]},)"
    R"( {"line": [[0, 2], [0, 0]]}]])";

void every_listed_function_is_a_command_serve_knows() {
    Bare k;
    for (const char* f : FUNCTIONS) {
        detail::Store<int> solids;
        const Json r = ask(k, solids, std::string(R"({"cmd": ")") + f + "\"}");
        const bool unknown =
            r.contains("err") && r["err"].get<std::string>().rfind("unknown command", 0) == 0;
        expect(!unknown, std::string("serve knows ") + f);
    }
    detail::Store<int> solids;
    const Json r = ask(k, solids, R"({"cmd": "later"})");
    expect(r.value("kind", "") == "unsupported", "an unknown command is unsupported");
}

void hello_lists_what_the_kernel_offers_and_classify_refuses_where_it_has_none() {
    Bare k;
    detail::Store<int> solids;
    const Json hello = ask(k, solids, R"({"cmd": "hello"})")["ok"];
    const auto supports = hello["supports"].get<std::vector<std::string>>();
    expect(supports.size() == FUNCTIONS.size() - 2, "hello leaves out the unsupported");
    expect(std::find(supports.begin(), supports.end(), "slab") == supports.end(), "no slab");
    expect(hello["protocol"] == PROTOCOL_VERSION, "hello gives the protocol version");
    expect(hello["name"] == "bare" && hello["configuration"] == "", "hello names the kernel");
    const Json box = ask(k, solids, R"({"cmd": "box", )" + FRAME + R"(, "size": [1, 2, 3]})");
    expect(box == Json{{"ok", 1}}, "the first handle is 1");
    const Json c = ask(k, solids, R"({"cmd": "classify", "s": 1, "points": [[0, 0, 0]]})");
    expect(c.value("kind", "") == "unsupported", "classify is unsupported by default");
}

void refusals_crashes_and_handles() {
    Bare k(Kind::Invalid);
    detail::Store<int> solids;
    ask(k, solids, R"({"cmd": "sphere", )" + FRAME + R"(, "radius": 1})");
    expect(ask(k, solids, R"({"cmd": "check", "s": 1})") ==
               Json{{"err", "no verdict"}, {"kind", "invalid"}},
           "a refusal keeps its kind and reason");
    expect(ask(k, solids, R"({"cmd": "volume", "s": 1})") == Json{{"crash", "lost"}},
           "another exception is a crash");
    expect(ask(k, solids, R"({"cmd": "area", "s": 1})") ==
               Json{{"err", "logic: mine"}, {"kind", "failed"}},
           "refusal_for turns an exception into a refusal");
    expect(ask(k, solids, R"({"cmd": "volume", "s": 7})") ==
               Json{{"err", "no solid 7"}, {"kind", "failed"}},
           "a handle never issued is refused");
    expect(ask(k, solids, R"({"cmd": "volume", "s": "one"})").value("kind", "") == "failed",
           "a handle that is no number is refused");
    expect(ask(k, solids, "{not json").contains("err"), "a line that is not JSON is refused");
    expect(ask(k, solids, R"({"cmd": "reset"})") == Json{{"ok", nullptr}}, "reset answers null");
    expect(ask(k, solids, R"({"cmd": "volume", "s": 1})").value("err", "") == "no solid 1",
           "reset releases every solid");
    expect(
        ask(k, solids, R"({"cmd": "box", )" + FRAME + R"(, "size": [1, 2]})").value("kind", "") ==
            "failed",
        "a Point3 of two numbers is a malformed request");
}

void every_constructor_reads_its_fields() {
    const auto read = [](const std::string& fields) {
        return detail::wire::primitive(Json::parse("{" + FRAME + ", " + fields + "}"));
    };
    const Primitive box = read(R"("cmd": "box", "size": [1, 2.5, 3])");
    expect(std::get<Box>(box).size == Vec3{1, 2.5, 3}, "box");
    expect(std::get<Box>(box).frame.origin == Vec3{1, 2, 3}, "frame");
    const Primitive cone = read(R"("cmd": "cone", "bottom": 2, "top": 0, "height": 5)");
    expect(std::get<Cone>(cone).height == 5, "cone");
    const Primitive loft =
        read(R"("cmd": "loft", "profile": )" + SQUARE + R"(, "scale": 0.5, "height": 4)");
    const auto& top = std::get<Loft>(loft).top.loops.at(0);
    expect(std::get<Line>(top.at(1)).a == Vec2{1, 0}, "a loft's top is its profile scaled");
    const Primitive sweep = read(R"("cmd": "sweep", "profile": )" + SQUARE +
                                 R"(, "path": [{"line": 3}, {"arc": {"radius": 2, "angle": 1}}])");
    expect(std::get<PathArc>(std::get<Sweep>(sweep).path.at(1)).radius == 2, "sweep");
    const Primitive extrude = read(
        R"("cmd": "extrude", "height": 2, "profile": [[{"arc": {"center": [0, 0], "radius": 1,)"
        R"( "start": 0, "sweep": 6.283185307179586}}], [{"conic": {"a": [0, 0], "apex": [1, 1],)"
        R"( "b": [2, 0], "w": 0.5}}, {"gn": {"a": [2, 0], "apex": [1, -1], "b": [0, 0],)"
        R"( "order": 2, "w": 0.4}}], [{"spline": {"degree": 2, "knots": [0, 1, 2, 3, 4, 5, 6],)"
        R"( "ctrl": [[0, 0], [1, 0], [1, 1], [0, 0]]}}]])");
    const auto& loops = std::get<Extrude>(extrude).profile.loops;
    expect(std::get<Arc>(loops.at(0).at(0)).radius == 1, "arc");
    expect(std::get<Gn>(loops.at(1).at(1)).order == 2, "gn");
    expect(std::get<Spline>(loops.at(2).at(0)).ctrl.size() == 4, "spline");
    const Primitive path = read(R"("cmd": "sweep_path", "profile": )" + SQUARE +
                                R"(, "twist": 0.5, "path": {"degree": 1, "knots": [0, 0, 1, 1],)"
                                R"( "ctrl": [[0, 0, 0], [0, 0, 5]]})");
    expect(std::get<SweepPath>(path).path.ctrl.at(1) == Vec3{0, 0, 5}, "sweep_path");
    const Primitive slab =
        read(R"("cmd": "slab", "size": [4, 5], "knots_u": [0, 0, 0, 0, 1, 1, 1, 1],)"
             R"( "knots_v": [0, 0, 0, 0, 1, 1, 1, 1], "heights": [[1, 2], [3, 4]])");
    expect(std::get<Slab>(slab).heights.at(1).at(0) == 3, "slab");
    for (const char* kind : {"cylinder", "sphere", "torus", "revolve", "screw"}) {
        bool refused = false;
        try {
            read(std::string(R"("cmd": ")") + kind + "\"");
        } catch (const Refusal& r) {
            refused = r.kind == Kind::Failed;
        }
        expect(refused, std::string(kind) + " without its fields is malformed");
    }
}

// A kernel whose measures are numbers and whose checker declines every solid, in the way given.
class Unchecked : public Kernel<int> {
  public:
    explicit Unchecked(Kind declines) : declines_(declines) {}
    std::string name() const override {
        return "unchecked";
    }
    std::string version() const override {
        return "0";
    }
    double tolerance() const override {
        return 1e-7;
    }
    int make(const Primitive&) override {
        return 0;
    }
    int unite(const int&, const int&) override {
        return 1;
    }
    int intersection(const int&, const int&) override {
        return 2;
    }
    int difference(const int&, const int&) override {
        return 3;
    }
    double volume(const int&) override {
        return 1.0;
    }
    double area(const int&) override {
        throw Refusal::failed("no area");
    }
    std::vector<std::string> check(const int&) override {
        throw Refusal(declines_, "no verdict");
    }
    std::vector<Side> classify(const int&, const std::vector<Vec3>& points) override {
        std::vector<Side> out;
        for (std::size_t i = 0; i < points.size(); i++) {
            out.push_back(i == 0 ? Side::In : i == 1 ? Side::On : Side::Out);
        }
        return out;
    }

  private:
    Kind declines_;
};

void a_checker_that_declines_is_an_issue_and_a_kernel_without_one_is_not_judged() {
    const Measure declined = Unchecked(Kind::Failed).measure(0);
    expect(declined.volume == 1.0 && std::isnan(declined.area), "measure takes what it can");
    expect(declined.issues.size() == 2 &&
               declined.issues[0] == "the checker declined the solid, failed: no verdict" &&
               declined.issues[1] == "area: failed: no area",
           "a declined check and a refused measure are issues");
    expect(Unchecked(Kind::Unsupported).measure(0).issues.size() == 1,
           "a kernel with no checker is not judged");
    Unchecked k(Kind::Failed);
    expect(k.boolean(Op::Union, 0, 0) == 1 && k.boolean(Op::Intersection, 0, 0) == 2 &&
               k.boolean(Op::Difference, 0, 0) == 3,
           "boolean calls the boolean it names");
    expect(k.holds(0, {{0, 0, 0}, {0, 0, 0}, {0, 0, 0}}) == std::vector<bool>{true, true, false},
           "holds counts the boundary in");
    const Primitive shape = detail::wire::primitive(
        Json::parse(R"({"kind": "sphere", )" + FRAME + R"(, "radius": 2})"));
    expect(std::get<Sphere>(shape).radius == 2, "a shape's JSON names its kind as kind");
}

void the_function_list_is_the_reference_s_tables() {
    std::ifstream reference(API_MD);
    std::vector<std::string> documented;
    std::string line;
    bool versioned = false;
    while (std::getline(reference, line)) {
        if (line.rfind("| [`", 0) == 0) {
            documented.push_back(line.substr(4, line.find('`', 4) - 4));
        }
        versioned = versioned || line.find(std::string("version ") + PROTOCOL_VERSION +
                                           " of the protocol") != std::string::npos;
    }
    expect(documented == std::vector<std::string>(FUNCTIONS.begin(), FUNCTIONS.end()),
           "FUNCTIONS is the list of functions in doc/api.md");
    expect(versioned, "doc/api.md names PROTOCOL_VERSION");
}

} // namespace

int main() {
    the_function_list_is_the_reference_s_tables();
    a_checker_that_declines_is_an_issue_and_a_kernel_without_one_is_not_judged();
    every_listed_function_is_a_command_serve_knows();
    hello_lists_what_the_kernel_offers_and_classify_refuses_where_it_has_none();
    refusals_crashes_and_handles();
    every_constructor_reads_its_fields();
    if (failures > 0) {
        std::fprintf(stderr, "%d failed\n", failures);
        return 1;
    }
    std::puts("every test passed");
    return 0;
}

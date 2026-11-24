// SPDX-License-Identifier: MIT OR Apache-2.0
// The smallest adapter in C++: a "kernel" that makes boxes and nothing else, answering the protocol
// through brep_protocol::serve. A real adapter has the same shape, with each method one call of its
// kernel. protocol/cpp/README.md says how to build it and run the suite on it.
#include <brep_protocol/brep_protocol.h>

#include <algorithm>
#include <limits>
#include <string>
#include <vector>

using namespace brep_protocol;

namespace {

constexpr double TOLERANCE = 1e-7;
const std::vector<std::string> SUPPORTED = {"box", "volume", "area", "check", "classify"};

// A box: one corner at the frame's origin, its edges along the frame's axes.
struct Block {
    Frame frame;
    Vec3 size;
};

double dot(const Vec3& a, const Vec3& b) {
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

class BoxKernel : public Kernel<Block> {
  public:
    std::string name() const override {
        return "box-kernel";
    }
    std::string version() const override {
        return "1.0.0";
    }
    double tolerance() const override {
        return TOLERANCE;
    }

    // The functions `hello` lists; the suite runs no case that needs another.
    std::vector<std::string> supports() const override {
        return {SUPPORTED.begin(), SUPPORTED.end()};
    }

    Block make(const Primitive& p) override {
        const auto* box = std::get_if<Box>(&p);
        if (box == nullptr) {
            throw Refusal::unsupported("this kernel makes only boxes");
        }
        if (!(box->size[0] > 0 && box->size[1] > 0 && box->size[2] > 0)) {
            throw Refusal::invalid("a box needs every size positive");
        }
        return Block{box->frame, box->size};
    }

    Block unite(const Block&, const Block&) override {
        throw Refusal::unsupported("no booleans");
    }
    Block intersection(const Block&, const Block&) override {
        throw Refusal::unsupported("no booleans");
    }
    Block difference(const Block&, const Block&) override {
        throw Refusal::unsupported("no booleans");
    }

    double volume(const Block& s) override {
        return s.size[0] * s.size[1] * s.size[2];
    }

    double area(const Block& s) override {
        const auto [a, b, c] = s.size;
        return 2.0 * (a * b + b * c + c * a);
    }

    std::vector<std::string> check(const Block&) override {
        return {};
    }

    // A point is placed by its coordinates along the box's axes: inside each span by more than the
    // tolerance is in, outside any span by more than it is out, and anything else is on.
    std::vector<Side> classify(const Block& s, const std::vector<Vec3>& points) override {
        std::vector<Side> out;
        const Frame& f = s.frame;
        for (const Vec3& p : points) {
            const Vec3 d = {p[0] - f.origin[0], p[1] - f.origin[1], p[2] - f.origin[2]};
            const Vec3 along = {dot(d, f.x), dot(d, f.y), dot(d, f.z)};
            double nearest = std::numeric_limits<double>::infinity();
            for (int i = 0; i < 3; i++) {
                nearest = std::min(nearest, std::min(along[i], s.size[i] - along[i]));
            }
            out.push_back(nearest > TOLERANCE    ? Side::In
                          : nearest < -TOLERANCE ? Side::Out
                                                 : Side::On);
        }
        return out;
    }
};

} // namespace

int main() {
    BoxKernel kernel;
    return serve(kernel);
}

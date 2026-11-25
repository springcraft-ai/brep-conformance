//! The smallest adapter: a "kernel" that makes boxes and nothing else, answering the protocol through
//! [`brep_protocol::serve`]. A real adapter has the same shape, with each method one call of its
//! kernel. Run the suite on it from the repository's root:
//!
//! ```text
//! cargo build --release --example box_kernel -p brep-protocol
//! cargo run --release -p brep-conformance -- --adapter target/release/examples/box_kernel --skip-nist
//! ```

use brep_protocol::model::{Frame, Primitive, Side, Vec3};
use brep_protocol::{Kernel, Refusal};

const TOLERANCE: f64 = 1e-7;
const SUPPORTED: [&str; 5] = ["box", "volume", "area", "check", "classify"];

struct BoxKernel;

/// A box: one corner at the frame's origin, its edges along the frame's axes.
struct Block {
    frame: Frame,
    size: Vec3,
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Kernel for BoxKernel {
    type Solid = Block;

    fn name(&self) -> String {
        "box-kernel".into()
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn tolerance(&self) -> f64 {
        TOLERANCE
    }

    /// The functions `hello` lists; the suite runs no case that needs another.
    fn supports(&self) -> Vec<String> {
        SUPPORTED.iter().map(|f| f.to_string()).collect()
    }

    fn make(&mut self, p: &Primitive) -> Result<Block, Refusal> {
        match p {
            Primitive::Box { frame, size } if size.iter().all(|&s| s > 0.0) => Ok(Block {
                frame: *frame,
                size: *size,
            }),
            Primitive::Box { .. } => Err(Refusal::invalid("a box needs every size positive")),
            _ => Err(Refusal::unsupported("this kernel makes only boxes")),
        }
    }

    fn union(&mut self, _: &Block, _: &Block) -> Result<Block, Refusal> {
        Err(Refusal::unsupported("no booleans"))
    }

    fn intersection(&mut self, _: &Block, _: &Block) -> Result<Block, Refusal> {
        Err(Refusal::unsupported("no booleans"))
    }

    fn difference(&mut self, _: &Block, _: &Block) -> Result<Block, Refusal> {
        Err(Refusal::unsupported("no booleans"))
    }

    fn volume(&mut self, s: &Block) -> Result<f64, Refusal> {
        let [a, b, c] = s.size;
        Ok(a * b * c)
    }

    fn area(&mut self, s: &Block) -> Result<f64, Refusal> {
        let [a, b, c] = s.size;
        Ok(2.0 * (a * b + b * c + c * a))
    }

    fn check(&mut self, _: &Block) -> Result<Vec<String>, Refusal> {
        Ok(Vec::new())
    }

    /// A point is placed by its coordinates along the box's axes: inside each span by more than the
    /// tolerance is in, outside any span by more than it is out, and anything else is on.
    fn classify(&mut self, s: &Block, points: &[Vec3]) -> Result<Vec<Side>, Refusal> {
        let f = &s.frame;
        Ok(points
            .iter()
            .map(|p| {
                let d = [p[0] - f.origin[0], p[1] - f.origin[1], p[2] - f.origin[2]];
                let along = [dot(d, f.x), dot(d, f.y), dot(d, f.z)];
                let clear = (0..3).map(|i| along[i].min(s.size[i] - along[i]));
                let nearest = clear.fold(f64::INFINITY, f64::min);
                if nearest > TOLERANCE {
                    Side::In
                } else if nearest < -TOLERANCE {
                    Side::Out
                } else {
                    Side::On
                }
            })
            .collect())
    }
}

fn main() -> std::io::Result<()> {
    brep_protocol::serve(&mut BoxKernel)
}

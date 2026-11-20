//! Each case's test: a path naming what it checks, under the feature it exercises. A family that
//! draws several situations is split into one test per situation, from what the case holds; the
//! family's own name stays the case's seed and id prefix, so the cases themselves never change.

use super::*;

/// The top-level directories, in the order the report lists them.
pub const DIRS: [&str; 8] = [
    "booleans",
    "constructions",
    "fillets",
    "chamfers",
    "modify",
    "queries",
    "step",
    "parts",
];

fn kind(s: &Shape) -> &'static str {
    match s {
        Shape::Prim(p) => match p {
            Primitive::Box { .. } => "box",
            Primitive::Cylinder { .. } => "cylinder",
            Primitive::Sphere { .. } => "sphere",
            Primitive::Torus { .. } => "torus",
            Primitive::Cone { .. } => "cone",
            Primitive::Extrude { .. } => "extrude",
            Primitive::Revolve { .. } => "revolve",
            Primitive::Screw { .. } => "screw",
            _ => "other",
        },
        Shape::Bool(Op::Union, ..) => "union",
        Shape::Bool(Op::Difference, ..) => "difference",
        Shape::Bool(Op::Intersection, ..) => "intersection",
    }
}

fn act(c: &Case) -> Option<Act> {
    c.feature.as_ref().map(|f| f.act)
}

/// The test a case belongs to.
pub fn test_path(c: &Case) -> &'static str {
    if let Some(v) = c.exact.variant {
        return v;
    }
    match c.family {
        // Booleans of two solids sharing volume.
        "chain-after-union-l" => "fillets/after-a-union",
        "chain-rim-after-cut" => "fillets/hole-rims-after-a-cut",
        "chain-rims-of-a-pattern" => "fillets/rims-of-a-hole-pattern",
        "chain-top-loop-after-corners" => "fillets/top-loop-after-corner-fillets",
        "chain-edges-of-a-chamfer" => "fillets/edges-of-a-chamfer",
        "chain-chamfers-beside-fillets" => "chamfers/next-to-fillets",
        "chain-two-calls-as-one" => "fillets/two-calls-same-as-one",
        "chain-across-a-mirror-join" => "fillets/across-a-mirror-join",
        "chain-shell-after-fillet" => "modify/shell-after-fillet",
        "chain-offset-after-cut" => "modify/offset-after-a-cut",
        "chain-offset-twice" => "modify/offset-twice-same-as-once",
        "chain-move-and-back" => "modify/move-and-back",
        "chain-cut-through-fillets" => "booleans/sequences/cut-through-fillets",
        "chain-round-trip-after-chain" => "step/round-trip-after-a-chain",
        "part-gear-spur-pinion" | "part-gear-spur-wheel" => "parts/gears/spur",
        "part-gear-spur-lightened" => "parts/gears/spur-with-lightening-holes",
        "part-gear-helical-right-15" | "part-gear-helical-left-30" => "parts/gears/helical",
        "part-gear-internal" => "parts/gears/internal",
        "part-gear-rack" => "parts/gears/rack",
        "part-gears-timing-pulley" => "parts/gears/timing-pulley",
        "part-gears-sprocket" => "parts/gears/sprocket",
        "part-shafts-splined" => "parts/shafts/splined",
        "part-nut-m3" | "part-nut-m6" | "part-nut-m12" => "parts/fasteners/hex-nut",
        "part-nut-thin-m10-fine" => "parts/fasteners/thin-nut-on-a-fine-pitch",
        "part-socket-screw-m3x8" => "parts/fasteners/socket-screw-threaded-to-the-head",
        "part-socket-screw-m8x40" | "part-socket-screw-m12x60" => {
            "parts/fasteners/socket-screw-with-a-plain-shank"
        }
        "part-hex-screw-m5x10" | "part-hex-screw-m6x16" | "part-hex-screw-m10x30" => {
            "parts/fasteners/hex-head-screw"
        }
        "part-washer-m3" | "part-washer-m12" => "parts/fasteners/plain-washer",
        "part-fasteners-lock-washer" => "parts/fasteners/lock-washer",
        "part-insert-m3" | "part-insert-m5" => "parts/fasteners/heat-set-insert",
        "part-fasteners-compression-spring" => "parts/fasteners/compression-spring",
        "part-shafts-stepped" => "parts/shafts/stepped",
        "part-shafts-pipe-flange" => "parts/shafts/pipe-flange",
        "part-shafts-v-belt-pulley" => "parts/shafts/v-belt-pulley",
        "part-shafts-windowed-wheel" => "parts/shafts/windowed-wheel",
        "part-shafts-turbine-disc" => "parts/shafts/turbine-disc",
        "part-brackets-l-bracket" => "parts/brackets/l-bracket",
        "part-brackets-mounting-plate" => "parts/brackets/mounting-plate",
        "part-brackets-gusset" => "parts/brackets/gusset",
        "part-brackets-bent-z" => "parts/brackets/bent-z",
        "part-brackets-clevis" => "parts/brackets/clevis",
        "part-housings-enclosure" => "parts/housings/enclosure",
        "part-housings-gearbox" => "parts/housings/gearbox",
        "part-housings-pillow-block" => "parts/housings/pillow-block",
        "part-housings-motor-mount" => "parts/housings/motor-mount",
        "part-housings-valve-body" => "parts/housings/valve-body",
        "part-molded-drafted-boss-and-rib" => "parts/molded/drafted-boss-and-rib",
        "part-molded-snap-fit" => "parts/molded/snap-fit",
        "part-molded-bottle-cap" => "parts/molded/bottle-cap",
        "part-machined-pocketed-block" => "parts/machined/pocketed-block",
        "part-machined-t-slot-plate" => "parts/machined/t-slot-plate",
        "part-machined-dovetail-slide" => "parts/machined/dovetail-slide",
        "part-machined-plate-cam" => "parts/machined/plate-cam",
        "part-structural-i-beam" => "parts/structural/i-beam",
        "part-structural-channel" => "parts/structural/channel",
        "part-structural-angle" => "parts/structural/angle",
        "part-structural-flanged-tube" => "parts/structural/flanged-tube",
        "part-gears-meshing-pinion-and-wheel" | "part-gears-meshing-equal" => {
            "parts/gears/meshing-pair"
        }
        "self-booleans" => "booleans/a-solid-with-itself",
        "box-box" => "booleans/overlap/boxes",
        "coaxial-cylinders" => "booleans/overlap/cylinders-coaxial",
        "crossing-cylinders" => "booleans/overlap/cylinders-crossing",
        "sphere-sphere" => "booleans/overlap/spheres",
        "sphere-halfspace" => "booleans/overlap/sphere-and-half-space",
        "torus-halfspace" => "booleans/overlap/torus-and-half-space",
        "cone-cylinder-coaxial" => "booleans/overlap/cone-and-cylinder-coaxial",
        "cone-box" => "booleans/overlap/cone-and-box",
        "inscribed" => "booleans/overlap/solid-inside-solid",
        "separated" => "booleans/overlap/solids-apart",
        "voids" => "booleans/overlap/solid-with-a-void",
        "mirror-joins" => "booleans/overlap/part-and-its-mirror-image",
        // Meeting without sharing volume.
        "boxes-touching" => "booleans/touching/boxes-face-to-face",
        "spheres-touching" => "booleans/touching/spheres-at-a-point",
        "cones-touching" => "booleans/touching/cone-at-its-apex-or-along-a-line",
        "line-contact" => "booleans/touching/along-a-line",
        "flush-bores" => "booleans/touching/bore-flush-with-a-face",
        "flush-pockets" => "booleans/touching/pocket-flush-with-a-face",
        "circle-contact" => match (kind(&c.a), kind(&c.b)) {
            ("cylinder", _) => "booleans/touching/sphere-in-a-cylinder",
            ("cone", _) => "booleans/touching/sphere-in-a-cone",
            (_, "cylinder") => "booleans/touching/torus-around-a-cylinder",
            _ => "booleans/touching/sphere-in-a-torus-hole",
        },
        // Decided by a hair.
        "spheres-nearly-touching" => "booleans/tolerance/spheres-overlapping-by-a-hair",
        "sphere-shaved" => "booleans/tolerance/sphere-shaved-by-a-hair",
        "parallel-cylinders" => "booleans/tolerance/parallel-cylinders-overlapping-by-a-hair",
        "coaxial-nearly-equal" => "booleans/tolerance/coaxial-cylinders-a-hair-apart",
        "near-coincident-boxes" => "booleans/tolerance/box-faces-a-hair-apart",
        "crossing-near-tangent" => "booleans/tolerance/crossing-cylinders-overlapping-by-a-hair",
        "thin-solids" => "booleans/tolerance/thin-walls",
        // Where parametrizations degenerate.
        "steinmetz" => "booleans/degenerate/equal-cylinders-crossing",
        "near-steinmetz" => "booleans/degenerate/equal-cylinders-just-missing",
        // Many tools at once, or one after another.
        "many-holes" => "booleans/patterns/hole-grid-one-at-a-time",
        "dense-holes" => "booleans/patterns/holes-packed-tight",
        "patterns" => match c.exact.many.as_ref() {
            Some(m) if m.target.is_some() => "booleans/patterns/holes-in-one-operation",
            Some(m) if matches!(m.tools.first(), Some(Primitive::Box { .. })) => {
                "booleans/patterns/boxes-in-one-operation"
            }
            _ => "booleans/patterns/ring-of-pins-in-one-operation",
        },
        "drilled-plates" => "booleans/sequences/drilled-plates",
        "stacked-boxes" => "booleans/sequences/stacked-boxes",
        "sphere-clusters" => "booleans/sequences/sphere-clusters",
        "random-chains" => "booleans/sequences/random-chains",
        "islands" => "booleans/sequences/separate-islands",
        "long-chains" => "booleans/sequences/long-chains",
        // Constructions.
        "boxes" => "constructions/box",
        "cylinders" => "constructions/cylinder",
        "spheres" => "constructions/sphere",
        "tori" => "constructions/torus",
        "cones" => "constructions/cone",
        "extrusions" => "constructions/extrude",
        "revolutions" => "constructions/revolve",
        "helices" => "constructions/helical-sweep",
        "thread-runout-m8" | "thread-runout-m12" => {
            "booleans/coincident/thread-crest-on-a-shank-from-where-it-runs-out"
        }
        "thread-runout-m8-on-the-shank" => {
            "booleans/coincident/thread-crest-on-a-shank-across-where-it-runs-out"
        }
        "thread-runout-m12-on-the-cone" => "booleans/coincident/thread-starting-on-a-runout-cone",
        "lofts" => "constructions/ruled-loft",
        "lofts-smooth" => "constructions/smooth-loft-through-sections",
        "lofts-equal-sections" => "constructions/smooth-loft-through-equal-sections",
        "sweeps" => "constructions/sweep-along-lines-and-arcs",
        "sweeps-spline" => "constructions/sweep-along-a-spline",
        "slabs" => "constructions/height-field-slab",
        "spline-prisms" => "constructions/spline-prism",
        "invalid-constructions" => match &c.a {
            Shape::Prim(Primitive::Extrude { profile, .. }) if profile.loops.len() > 1 => {
                "constructions/rejects-hole-outside-profile"
            }
            Shape::Prim(Primitive::Extrude { .. }) => "constructions/rejects-self-crossing-profile",
            Shape::Prim(Primitive::Revolve { .. }) => {
                "constructions/rejects-revolve-across-its-axis"
            }
            Shape::Prim(Primitive::Screw { .. }) => {
                "constructions/rejects-screw-whose-turns-collide"
            }
            _ => "constructions/rejects-torus-tube-across-its-axis",
        },
        "invalid-calls" => "constructions/rejects-loft-heights-out-of-order",
        // Fillets.
        "rounded-boxes" => "fillets/box-all-edges",
        "fillet-prisms" => "fillets/prism-side-edges",
        "fillet-rims" => "fillets/cylinder-and-cone-rims",
        "fillet-chains" => "fillets/top-loop-of-a-rounded-prism",
        "fillet-chains-tight" => "fillets/top-loop-bigger-than-corner-rounds",
        "rounded-prisms" => "fillets/prism-all-edges",
        "opened-l-prisms" => "fillets/l-prism-convex-edges",
        "fillet-uneven-corners" => "fillets/three-radii-at-a-corner",
        "fillet-variable" => "fillets/variable-radius",
        "fillet-short-edges" => "fillets/tiny-rounds-beside-a-tiny-face",
        "fillet-swallowed-face" => "fillets/round-swallowing-a-tiny-face",
        "fillet-mixed-corners" => "fillets/concave-meets-convex-corner",
        "fillet-conic" => "fillets/conic-section",
        "fillet-setbacks" => "fillets/set-back-corner",
        "fillet-setbacks-g2" => "fillets/g2-set-back-corner",
        "fillet-faces" => "fillets/between-faces-across-a-chamfer",
        "full-rounds" => "fillets/full-round-rib",
        "fillet-over-limit" => "fillets/rejects-radius-past-half-width",
        "hole-rims" => match (act(c), kind(&c.a)) {
            (Some(Act::Fillet), "extrude") => "fillets/hole-rim",
            (Some(Act::Fillet), _) => "fillets/boss-foot",
            (_, "extrude") => "chamfers/hole-rim",
            _ => "chamfers/boss-foot",
        },
        // Chamfers.
        "chamfer-prisms" => "chamfers/prism-side-edges",
        "chamfer-rims" => "chamfers/cylinder-and-cone-rims",
        "chamfer-uneven-corners" => "chamfers/three-distances-at-a-corner",
        "chamfer-uneven" => "chamfers/two-unequal-distances",
        // Refusals of the blends and edits they belong to.
        "invalid-acts" => match act(c) {
            Some(Act::Fillet) => "fillets/rejects-radius-too-big-for-its-faces",
            Some(Act::Chamfer) => "chamfers/rejects-distance-too-big-for-its-faces",
            _ => "modify/rejects-shell-as-thick-as-the-part",
        },
        "invalid-new-acts" => match (act(c), kind(&c.a)) {
            (Some(Act::ChamferTwo), _) => "chamfers/rejects-two-distance-cut-deeper-than-a-face",
            (Some(Act::FilletVariable), _) => {
                "fillets/rejects-variable-radius-ball-that-cannot-fit"
            }
            (Some(Act::FilletRadii), _) => "fillets/rejects-one-of-several-radii-too-big",
            (Some(Act::ChamferDistances), _) => "chamfers/rejects-one-of-several-distances-too-big",
            (_, "box") => "modify/rejects-face-pushed-through-the-part",
            _ => "modify/rejects-step-raised-past-the-one-above",
        },
        // Changing a solid.
        "offsets" => match kind(&c.a) {
            "box" => "modify/offset-box",
            "sphere" => "modify/offset-sphere",
            _ => "modify/offset-cylinder-or-cone",
        },
        "offset-past-holes" => {
            let hole = match &c.a {
                Shape::Prim(Primitive::Extrude { profile, .. }) => match profile.loops.get(1) {
                    Some(lp) => match lp.first() {
                        Some(Seg::Arc { radius, .. }) => *radius,
                        _ => 0.0,
                    },
                    None => 0.0,
                },
                _ => 0.0,
            };
            match &c.feature {
                Some(f) if f.size < hole => "modify/offset-drilled-block-hole-narrows",
                _ => "modify/offset-drilled-block-hole-fills",
            }
        }
        "offset-l-prisms" => "modify/offset-l-prism",
        "shells" => "modify/shell",
        "shelled-rounds" => "modify/shell-rounded-box",
        "drafts" => "modify/draft",
        "closing-drafts" => "modify/draft-until-faces-meet",
        "offset-faces" => "modify/push-pull-faces",
        "closing-offsets" => "modify/push-faces-until-they-merge",
        "delete-faces" => "modify/delete-faces",
        "deleted-rim-rounds"
        | "deleted-rim-bevels"
        | "deleted-foot-rounds"
        | "deleted-dome-rounds"
        | "deleted-hemisphere-rounds"
        | "deleted-bore-rounds"
        | "deleted-bore-grooves"
        | "deleted-countersinks" => "modify/delete-faces-of-turned-parts",
        "deleted-bosses" | "deleted-root-rounds" | "deleted-inside-rounds" => {
            "modify/delete-faces-taking-material-away"
        }
        "frustum-shells-open-bottom" | "frustum-shells-open-top" | "frustum-shells-closed" => {
            "modify/shell-frustum"
        }
        "ball-shells" | "cone-shells-open-base" | "cone-shells-closed" => {
            "modify/shell-ball-or-cone"
        }
        "puck-shells-within-rounds" | "puck-shells-past-rounds" => "modify/shell-rounded-puck",
        "stepped-shells" | "stepped-shells-leaning-back" | "stepped-shells-leaning-over" => {
            "modify/shell-l-prism-open-at-its-step"
        }
        "offset-tori" | "offset-tori-past-the-hole" => "modify/offset-torus",
        "offset-root-rounds"
        | "offset-root-rounds-past"
        | "offset-cone-roots"
        | "offset-cone-roots-past" => "modify/offset-boss-with-a-root-round",
        "offset-cones-to-apex" => "modify/offset-cone-to-its-apex",
        "move-faces" => "modify/move-faces",
        "thicken" => "modify/thicken-faces",
        "swept-slides" => "modify/swept-solid-slid",
        "swept-turns" => "modify/swept-solid-turned",
        "swept-screws" => "modify/swept-solid-screwed",
        "swept-composed" => "modify/swept-solid-two-motions-at-once",
        "sewing" => "modify/sew-faces-into-a-solid",
        "transforms" => "modify/move-scale-and-mirror",
        "stretches" => "modify/non-uniform-scale-and-shear",
        // Queries.
        "normals" => "queries/normals-and-creases",
        "bounding-boxes" => "queries/bounding-box",
        "nearest-points" => "queries/closest-point",
        "nearest-in-holes" => "queries/closest-point-in-a-hole",
        "boundary-points" => "queries/classify-on-the-boundary",
        "splits" => "queries/split-by-a-plane",
        "sections" => "queries/plane-section-curves",
        "assemblies" => "queries/assembly-clash-detection",
        "curvatures" => match kind(&c.a) {
            "box" => "queries/curvature-on-box-faces",
            "cylinder" => "queries/curvature-on-cylinders",
            "cone" => "queries/curvature-on-cones",
            "sphere" => "queries/curvature-on-spheres",
            "torus" => "queries/curvature-on-tori",
            _ => "queries/curvature-in-holes",
        },
        "projections" => match kind(&c.a) {
            "box" => "queries/hidden-lines-of-a-box",
            "cylinder" => "queries/hidden-lines-of-a-cylinder",
            "sphere" => "queries/hidden-lines-of-a-sphere",
            "union" => "queries/hidden-lines-of-a-box-behind-a-box",
            _ => "queries/hidden-lines-of-a-drilled-box",
        },
        "displays" => match kind(&c.a) {
            "box" => "queries/display-of-a-box",
            "cylinder" => "queries/display-of-a-cylinder",
            "sphere" => "queries/display-of-a-sphere",
            "cone" => "queries/display-of-a-cone",
            _ => "queries/display-of-a-drilled-box",
        },
        // STEP.
        "step-corpus" => "step/read-ap214",
        "step-revolved" => "step/read-ap214-revolved-solids",
        "step-tessellated" => "step/read-tessellated-ap242",
        "step-nist" => "step/read-nist",
        "step-schemas" => match c.exact.step_as {
            Some(StepAs::Schema(crate::step::Schema::Ap203)) => "step/read-ap203",
            _ => "step/read-ap242",
        },
        _ => "unsorted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_has_a_test_under_a_known_directory() {
        let mut tests = std::collections::BTreeSet::new();
        for c in cases(1) {
            let top = c.test.split('/').next().unwrap_or("");
            assert!(
                DIRS.contains(&top),
                "{} ({}) has no test: {}",
                c.id,
                c.family,
                c.test
            );
            tests.insert(c.test);
        }
        // Names are lowercase words joined by hyphens, under one or two directories.
        for t in &tests {
            let parts: Vec<&str> = t.split('/').collect();
            assert!((2..=3).contains(&parts.len()), "{t}");
            assert!(
                t.chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || "-/".contains(ch)),
                "{t}"
            );
        }
    }
}

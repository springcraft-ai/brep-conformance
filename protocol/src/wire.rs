//! Reading the shapes back from the JSON [`Primitive::to_json`] writes.

use serde_json::Value;

use crate::Refusal;
use crate::model::{Frame, PathSeg, Primitive, Spline, Vec3};
use crate::motion::Motion;
use crate::path::PathCurve;
use crate::profile::{Profile, Seg};

fn num(v: &Value, key: &str) -> Result<f64, String> {
    v[key]
        .as_f64()
        .ok_or_else(|| format!("`{key}` is not a number"))
}

fn nums(v: &Value) -> Result<Vec<f64>, String> {
    v.as_array()
        .ok_or("expected an array of numbers")?
        .iter()
        .map(|x| x.as_f64().ok_or_else(|| "expected a number".to_string()))
        .collect()
}

/// An `integer` of doc/api.md: a whole number from 0 to 2⁵³, so `5.0` is `5`.
pub(crate) fn integer(v: &Value) -> Option<u64> {
    const MAX: u64 = 1 << 53;
    if let Some(n) = v.as_u64() {
        return (n <= MAX).then_some(n);
    }
    let d = v.as_f64()?;
    (d >= 0.0 && d <= MAX as f64 && d.fract() == 0.0).then_some(d as u64)
}

fn vec3(v: &Value) -> Result<Vec3, String> {
    let n = nums(v)?;
    n.try_into().map_err(|_| "expected three numbers".into())
}

fn vec2(v: &Value) -> Result<[f64; 2], String> {
    let n = nums(v)?;
    n.try_into().map_err(|_| "expected two numbers".into())
}

pub(crate) fn frame(v: &Value) -> Result<Frame, String> {
    Ok(Frame {
        origin: vec3(&v["origin"])?,
        x: vec3(&v["x"])?,
        y: vec3(&v["y"])?,
        z: vec3(&v["z"])?,
    })
}

fn spline3(v: &Value) -> Result<PathCurve, String> {
    Ok(PathCurve {
        degree: integer(&v["degree"]).ok_or("`degree` is not a whole number")? as usize,
        knots: nums(&v["knots"])?,
        ctrl: v["ctrl"]
            .as_array()
            .ok_or("`ctrl` is not an array")?
            .iter()
            .map(vec3)
            .collect::<Result<_, _>>()?,
    })
}

/// A motion of a kind this version does not know is one the adapter cannot do, and any other
/// misreading a request at fault.
pub(crate) fn motion(v: &Value) -> Result<Motion, Refusal> {
    let failed = |e: String| Refusal::failed(format!("motion: {e}"));
    if let Some(d) = v.get("slide") {
        return Ok(Motion::Slide {
            vector: vec3(d).map_err(failed)?,
        });
    }
    if let Some(t) = v.get("turn") {
        return Ok(Motion::Turn {
            origin: vec3(&t["origin"]).map_err(failed)?,
            axis: vec3(&t["axis"]).map_err(failed)?,
            angle: num(t, "angle").map_err(failed)?,
        });
    }
    if let Some(s) = v.get("screw") {
        return Ok(Motion::Screw {
            origin: vec3(&s["origin"]).map_err(failed)?,
            axis: vec3(&s["axis"]).map_err(failed)?,
            rise: num(s, "rise").map_err(failed)?,
            angle: num(s, "angle").map_err(failed)?,
        });
    }
    let Some(pair) = v.get("then") else {
        return Err(match v.as_object() {
            Some(o) if !o.is_empty() => Refusal::unsupported(format!(
                "unknown motion `{}`",
                o.keys().next().map_or("", String::as_str)
            )),
            _ => failed("a motion is a `slide`, a `turn`, a `screw` or a `then`".into()),
        });
    };
    let [first, second] = pair.as_array().map_or(&[][..], Vec::as_slice) else {
        return Err(failed("a `then` is two motions".into()));
    };
    Ok(Motion::Then(
        Box::new(motion(first)?),
        Box::new(motion(second)?),
    ))
}

impl Motion {
    pub fn from_json(v: &Value) -> Result<Motion, String> {
        motion(v).map_err(|e| e.reason)
    }
}

fn seg(v: &Value) -> Result<Seg, String> {
    if let Some(l) = v.get("line") {
        if l.as_array().map(Vec::len) != Some(2) {
            return Err("a line is two points".into());
        }
        return Ok(Seg::Line {
            a: vec2(&l[0])?,
            b: vec2(&l[1])?,
        });
    }
    if let Some(s) = v.get("spline") {
        return Ok(Seg::Spline(Spline {
            degree: integer(&s["degree"]).ok_or("`degree` is not a whole number")? as usize,
            knots: nums(&s["knots"])?,
            ctrl: s["ctrl"]
                .as_array()
                .ok_or("`ctrl` is not an array")?
                .iter()
                .map(vec2)
                .collect::<Result<_, _>>()?,
        }));
    }
    if let Some(c) = v.get("gn") {
        let order = integer(&c["order"]).ok_or("`order` is not a whole number")?;
        return Ok(Seg::Gn {
            a: vec2(&c["a"])?,
            apex: vec2(&c["apex"])?,
            b: vec2(&c["b"])?,
            order: u32::try_from(order).map_err(|e| e.to_string())?,
            w: num(c, "w")?,
        });
    }
    if let Some(c) = v.get("conic") {
        return Ok(Seg::Conic {
            a: vec2(&c["a"])?,
            apex: vec2(&c["apex"])?,
            b: vec2(&c["b"])?,
            w: num(c, "w")?,
        });
    }
    let a = v
        .get("arc")
        .ok_or("a segment is a `line`, an `arc`, a `conic` or a `spline`")?;
    Ok(Seg::Arc {
        center: vec2(&a["center"])?,
        radius: num(a, "radius")?,
        start: num(a, "start")?,
        sweep: num(a, "sweep")?,
    })
}

impl Profile {
    pub fn from_json(v: &Value) -> Result<Profile, String> {
        let loops = v
            .as_array()
            .ok_or("a profile is an array of loops")?
            .iter()
            .map(|l| {
                l.as_array()
                    .ok_or_else(|| "a loop is an array of segments".to_string())?
                    .iter()
                    .map(seg)
                    .collect()
            })
            .collect::<Result<_, String>>()?;
        Ok(Profile { loops })
    }
}

impl Primitive {
    /// The call that constructs the primitive: its JSON with the constructor's name as `cmd`.
    pub fn to_call(&self) -> Value {
        let mut v = self.to_json();
        let obj = v.as_object_mut().expect("a primitive is an object");
        let kind = obj.remove("kind").expect("a primitive has a kind");
        obj.insert("cmd".into(), kind);
        v
    }

    /// The primitive a shape's JSON or a constructor call describes: its kind is `kind`, or `cmd`.
    pub fn from_json(v: &Value) -> Result<Primitive, String> {
        let frame = frame(&v["frame"])?;
        let profile = || Profile::from_json(&v["profile"]);
        let kind = v.get("kind").or_else(|| v.get("cmd"));
        Ok(
            match kind.and_then(Value::as_str).ok_or("`kind` is missing")? {
                "box" => Primitive::Box {
                    frame,
                    size: vec3(&v["size"])?,
                },
                "cylinder" => Primitive::Cylinder {
                    frame,
                    radius: num(v, "radius")?,
                    height: num(v, "height")?,
                },
                "sphere" => Primitive::Sphere {
                    frame,
                    radius: num(v, "radius")?,
                },
                "torus" => Primitive::Torus {
                    frame,
                    major: num(v, "major")?,
                    minor: num(v, "minor")?,
                },
                "cone" => Primitive::Cone {
                    frame,
                    bottom: num(v, "bottom")?,
                    top: num(v, "top")?,
                    height: num(v, "height")?,
                },
                "extrude" => Primitive::Extrude {
                    frame,
                    profile: profile()?,
                    height: num(v, "height")?,
                },
                "revolve" => Primitive::Revolve {
                    frame,
                    profile: profile()?,
                    angle: num(v, "angle")?,
                },
                "screw" => Primitive::Screw {
                    frame,
                    profile: profile()?,
                    rise: num(v, "rise")?,
                    angle: num(v, "angle")?,
                },
                "loft" => Primitive::Loft {
                    frame,
                    profile: profile()?,
                    scale: num(v, "scale")?,
                    height: num(v, "height")?,
                    top: Profile::from_json(&v["top"])?,
                },
                "sweep" => Primitive::Sweep {
                    frame,
                    profile: profile()?,
                    path: v["path"]
                        .as_array()
                        .ok_or("`path` is not an array")?
                        .iter()
                        .map(|s| {
                            if let Some(l) = s.get("line") {
                                let length = l.as_f64().ok_or("a line's length is not a number")?;
                                return Ok(PathSeg::Line { length });
                            }
                            let a = s.get("arc").ok_or("a path piece is a `line` or an `arc`")?;
                            Ok(PathSeg::Arc {
                                radius: num(a, "radius")?,
                                angle: num(a, "angle")?,
                            })
                        })
                        .collect::<Result<_, String>>()?,
                },
                "sweep_path" => Primitive::SweepPath {
                    frame,
                    profile: profile()?,
                    path: spline3(&v["path"])?,
                    twist: v["twist"].as_f64().ok_or("`twist` is not a number")?,
                },
                "slab" => Primitive::Slab {
                    frame,
                    size: vec2(&v["size"])?,
                    knots_u: nums(&v["knots_u"])?,
                    knots_v: nums(&v["knots_v"])?,
                    heights: v["heights"]
                        .as_array()
                        .ok_or("`heights` is not an array")?
                        .iter()
                        .map(nums)
                        .collect::<Result<_, _>>()?,
                },
                k => return Err(format!("unknown constructor `{k}`")),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{circle, rounded};

    #[test]
    fn every_kind_reads_back_as_written() {
        let f = Frame::along([1.0, 2.0, 3.0], [0.0, 0.6, 0.8]);
        let profile = Profile {
            loops: vec![
                rounded(&[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]], &[0.0, 0.5, 0.0]),
                circle([2.0, 1.0], 0.4, false),
            ],
        };
        let all = [
            Primitive::Box {
                frame: f,
                size: [1.0, 2.0, 3.0],
            },
            Primitive::Cone {
                frame: f,
                bottom: 2.0,
                top: 0.0,
                height: 5.0,
            },
            Primitive::Screw {
                frame: f,
                profile: profile.clone(),
                rise: 0.3,
                angle: 9.0,
            },
            Primitive::Extrude {
                frame: f,
                height: 2.0,
                profile: Profile {
                    loops: vec![vec![Seg::Spline(Spline {
                        degree: 3,
                        knots: (0..11).map(f64::from).collect(),
                        ctrl: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]],
                    })]],
                },
            },
            Primitive::Loft {
                frame: f,
                top: profile.scaled(0.5),
                profile,
                scale: 0.5,
                height: 4.0,
            },
        ];
        for p in all {
            assert_eq!(Primitive::from_json(&p.to_json()), Ok(p.clone()));
            assert_eq!(Primitive::from_json(&p.to_call()), Ok(p));
        }
    }

    #[test]
    fn a_loft_reads_the_top_profile_it_is_sent() {
        // A top that is not the profile scaled by `scale`, so a reader that scales the profile
        // itself, instead of reading `top`, is caught.
        let square = |s: f64| Profile {
            loops: vec![rounded(
                &[[0.0, 0.0], [s, 0.0], [s, s], [0.0, s]],
                &[0.0; 4],
            )],
        };
        let mut call = serde_json::json!({
            "cmd": "loft",
            "frame": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0], "z": [0, 0, 1]},
            "profile": square(4.0).to_json(),
            "scale": 0.5,
            "height": 3.0,
        });
        call["top"] = square(1.0).to_json();
        let Ok(Primitive::Loft { top, .. }) = Primitive::from_json(&call) else {
            panic!("the loft did not read");
        };
        assert_eq!(top, square(1.0));
    }
}

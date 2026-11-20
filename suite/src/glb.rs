//! A case's views as one binary glTF: a scene per view, and in it a node per solid whose mesh holds
//! the solid's triangles and, as primitives of lines, its edges and its seams. Positions stay in
//! the suite's millimetres, z up, under a root node that scales them to glTF's metres and turns the
//! suite's z axis to glTF's y, which is up in every glTF viewer. They are quantized to 16 bits
//! over each solid's box (`KHR_mesh_quantization`). Each solid's node carries its role in
//! `extras.role`, so a reader can leave out the ghosts, which are open surfaces, not solids. Each
//! triangle vertex carries its face's index as `_FACE` and each edge vertex its edge's as `_EDGE`,
//! into the table of edges in the mesh's `extras`.

use std::collections::HashMap;

use brep_protocol::Display;
use serde_json::{Value, json};

/// Each role's colour as linear RGBA, in the report viewer's tones, and the edges'.
const MATERIALS: [(&str, [f64; 4]); 6] = [
    ("main", [0.202, 0.287, 0.371, 1.0]),
    ("alt", [0.479, 0.242, 0.078, 1.0]),
    ("ghost", [0.212, 0.468, 0.716, 0.14]),
    ("ghost2", [0.117, 0.456, 0.238, 0.14]),
    ("edge", [0.01, 0.01, 0.01, 1.0]),
    ("seam", [0.01, 0.01, 0.01, 1.0]),
];

/// What a line primitive's `extras.lines` says it draws: the edges between faces, or the seams
/// and the edges a split made, which mark no change in the shape.
const EDGES: &str = "edges";
const SEAMS: &str = "seams";

/// A quarter turn about x, as glTF's `[x, y, z, w]` quaternion: the suite's +z becomes +y.
const Z_UP_TO_Y_UP: [f64; 4] = [
    -std::f64::consts::FRAC_1_SQRT_2,
    0.0,
    0.0,
    std::f64::consts::FRAC_1_SQRT_2,
];

fn material(role: &str) -> usize {
    MATERIALS.iter().position(|(r, _)| *r == role).unwrap_or(0)
}

/// A solid drawn in a view: `key` names its geometry, so a solid drawn twice is stored once.
pub struct Solid<'a> {
    pub key: String,
    pub role: &'a str,
    pub display: &'a Display,
}

pub struct View<'a> {
    pub name: &'a str,
    pub note: Option<&'a str>,
    /// What the second tone marks, where the view has one.
    pub alt: Option<&'a str>,
    pub solids: Vec<Solid<'a>>,
}

#[derive(Default)]
struct Builder {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
    meshes: Vec<Value>,
}

/// A solid's primitives, its table of edges, and the translation and uniform scale its node takes
/// their quantized positions back to millimetres by.
struct Geometry {
    prims: Vec<Value>,
    edges: Value,
    translation: [f64; 3],
    scale: f64,
}

impl Builder {
    fn view(&mut self, bytes: &[u8], target: u32, stride: Option<usize>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        let mut v = json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len(),
                           "target": target});
        if let Some(stride) = stride {
            v["byteStride"] = json!(stride);
        }
        self.views.push(v);
        self.views.len() - 1
    }

    fn accessor(&mut self, a: Value) -> usize {
        self.accessors.push(a);
        self.accessors.len() - 1
    }

    /// Quantized positions as SHORT, each padded to four bytes as glTF aligns vertex attributes.
    fn positions(&mut self, v: &[[i16; 3]]) -> usize {
        let bytes: Vec<u8> = v
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 0])
            .flat_map(i16::to_le_bytes)
            .collect();
        let view = self.view(&bytes, 34962, Some(8));
        let lo: Vec<i16> = (0..3)
            .map(|k| v.iter().map(|p| p[k]).min().unwrap_or(0))
            .collect();
        let hi: Vec<i16> = (0..3)
            .map(|k| v.iter().map(|p| p[k]).max().unwrap_or(0))
            .collect();
        self.accessor(
            json!({"bufferView": view, "componentType": 5122, "count": v.len(),
                             "type": "VEC3", "min": lo, "max": hi}),
        )
    }

    /// Unit normals as normalized SHORT, padded to eight bytes.
    fn normals(&mut self, v: &[[f64; 3]]) -> usize {
        let bytes: Vec<u8> = v
            .iter()
            .flat_map(|n| {
                let s = n.map(|c| (c.clamp(-1.0, 1.0) * f64::from(i16::MAX)).round() as i16);
                [s[0], s[1], s[2], 0]
            })
            .flat_map(i16::to_le_bytes)
            .collect();
        let view = self.view(&bytes, 34962, Some(8));
        self.accessor(
            json!({"bufferView": view, "componentType": 5122, "normalized": true,
                             "count": v.len(), "type": "VEC3"}),
        )
    }

    /// One index per vertex as UNSIGNED_SHORT, padded to four bytes; none where an index would not
    /// fit, since glTF gives a vertex attribute no wider integer.
    fn tags(&mut self, v: &[u32]) -> Option<usize> {
        if v.iter().any(|&x| x > u32::from(u16::MAX)) {
            return None;
        }
        let bytes: Vec<u8> = v
            .iter()
            .flat_map(|&x| [x as u16, 0])
            .flat_map(u16::to_le_bytes)
            .collect();
        let view = self.view(&bytes, 34962, Some(4));
        Some(self.accessor(
            json!({"bufferView": view, "componentType": 5123, "count": v.len(),
                             "type": "SCALAR"}),
        ))
    }

    /// Indices as UNSIGNED_SHORT where every vertex fits below the restart value, else
    /// UNSIGNED_INT.
    fn indices(&mut self, i: &[u32], vertices: usize) -> usize {
        let (bytes, kind): (Vec<u8>, u32) = if vertices <= usize::from(u16::MAX) {
            (
                i.iter().flat_map(|&x| (x as u16).to_le_bytes()).collect(),
                5123,
            )
        } else {
            (i.iter().flat_map(|x| x.to_le_bytes()).collect(), 5125)
        };
        let view = self.view(&bytes, 34963, None);
        self.accessor(
            json!({"bufferView": view, "componentType": kind, "count": i.len(),
                             "type": "SCALAR"}),
        )
    }

    /// The display's faces as one triangle primitive, its edges as one line primitive and its seams
    /// and split edges as another, without materials, their positions quantized to 16 bits over
    /// its box.
    fn primitives(&mut self, d: &Display) -> Geometry {
        let mut positions: Vec<[f64; 3]> = Vec::new();
        let mut normals: Vec<[f64; 3]> = Vec::new();
        let mut triangles: Vec<u32> = Vec::new();
        let mut face_of: Vec<u32> = Vec::new();
        for (k, f) in d.faces.iter().enumerate() {
            let base = positions.len() as u32;
            positions.extend(&f.positions);
            normals.extend(&f.normals);
            face_of.extend(std::iter::repeat_n(k as u32, f.positions.len()));
            triangles.extend(f.triangles.iter().flatten().map(|i| i + base));
        }
        // Each kind of line: its positions, its segments, and the edge each vertex belongs to.
        #[derive(Default)]
        struct Lines {
            positions: Vec<[f64; 3]>,
            segments: Vec<u32>,
            edge_of: Vec<u32>,
        }
        impl Lines {
            fn add(&mut self, line: &[[f64; 3]], edge: u32) {
                if line.len() < 2 {
                    return;
                }
                let base = self.positions.len() as u32;
                self.positions.extend(line);
                self.edge_of.extend(std::iter::repeat_n(edge, line.len()));
                for k in 0..line.len() as u32 - 1 {
                    self.segments.extend([base + k, base + k + 1]);
                }
            }
        }
        let (mut drawn, mut quiet) = (Lines::default(), Lines::default());
        for (k, e) in d.edges.iter().enumerate() {
            let lines = if e.split { &mut quiet } else { &mut drawn };
            lines.add(&e.points, k as u32);
        }
        // A seam is no edge of the table: its vertices name the row past the table's last.
        for seam in &d.seams {
            quiet.add(seam, d.edges.len() as u32);
        }
        let edges: Vec<Value> = d
            .edges
            .iter()
            .map(|e| json!({"faces": e.faces, "turn": e.turn, "split": e.split}))
            .collect();
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for p in positions
            .iter()
            .chain(&drawn.positions)
            .chain(&quiet.positions)
        {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let translation = [0, 1, 2].map(|k| {
            let m = (lo[k] + hi[k]) / 2.0;
            if m.is_finite() { m } else { 0.0 }
        });
        let half = (0..3).map(|k| (hi[k] - lo[k]) / 2.0).fold(0.0, f64::max);
        let scale = if half > 0.0 {
            half / f64::from(i16::MAX)
        } else {
            1.0
        };
        let quantize = |v: &[[f64; 3]]| -> Vec<[i16; 3]> {
            v.iter()
                .map(|p| {
                    [0, 1, 2].map(|k| {
                        let q = ((p[k] - translation[k]) / scale).round();
                        q.clamp(-f64::from(i16::MAX), f64::from(i16::MAX)) as i16
                    })
                })
                .collect()
        };
        let mut prims = Vec::new();
        if !triangles.is_empty() {
            let p = self.positions(&quantize(&positions));
            let n = self.normals(&normals);
            let i = self.indices(&triangles, positions.len());
            let mut attributes = json!({"POSITION": p, "NORMAL": n});
            if let Some(f) = self.tags(&face_of) {
                attributes["_FACE"] = json!(f);
            }
            prims.push(json!({"attributes": attributes, "indices": i, "mode": 4}));
        }
        for (lines, kind) in [(drawn, EDGES), (quiet, SEAMS)] {
            if lines.segments.is_empty() {
                continue;
            }
            let p = self.positions(&quantize(&lines.positions));
            let i = self.indices(&lines.segments, lines.positions.len());
            let mut attributes = json!({"POSITION": p});
            if let Some(e) = self.tags(&lines.edge_of) {
                attributes["_EDGE"] = json!(e);
            }
            prims.push(json!({"attributes": attributes, "indices": i, "mode": 1,
                              "extras": {"lines": kind}}));
        }
        Geometry {
            prims,
            edges: json!(edges),
            translation,
            scale,
        }
    }

    /// A glTF mesh of the primitives in the role's material, its edges and seams in theirs, with
    /// the table of edges in its `extras`.
    fn mesh(&mut self, g: &Geometry, role: &str) -> usize {
        let prims: Vec<Value> = g
            .prims
            .iter()
            .map(|p| {
                let mut p = p.clone();
                let of = match p["extras"]["lines"].as_str() {
                    Some(SEAMS) => "seam",
                    Some(_) => "edge",
                    None => role,
                };
                p["material"] = json!(material(of));
                p
            })
            .collect();
        self.meshes
            .push(json!({"name": role, "primitives": prims, "extras": {"edges": g.edges}}));
        self.meshes.len() - 1
    }
}

/// The GLB of a case's views; `extras` goes on the file's root.
pub fn glb(views: &[View], extras: Value, generator: &str) -> Vec<u8> {
    let mut b = Builder::default();
    let mut nodes: Vec<Value> = Vec::new();
    let mut scenes: Vec<Value> = Vec::new();
    let mut geometry: HashMap<String, Geometry> = HashMap::new();
    let mut meshes: HashMap<(String, String), usize> = HashMap::new();
    for v in views {
        let mut children = Vec::new();
        for s in &v.solids {
            if !geometry.contains_key(&s.key) {
                let g = b.primitives(s.display);
                geometry.insert(s.key.clone(), g);
            }
            let g = &geometry[&s.key];
            if g.prims.is_empty() {
                continue;
            }
            let id = *meshes
                .entry((s.key.clone(), s.role.to_string()))
                .or_insert_with(|| b.mesh(g, s.role));
            nodes.push(
                json!({"name": s.role, "mesh": id, "extras": {"role": s.role},
                              "translation": g.translation, "scale": [g.scale, g.scale, g.scale]}),
            );
            children.push(nodes.len() - 1);
        }
        let mut root =
            json!({"name": v.name, "rotation": Z_UP_TO_Y_UP, "scale": [1e-3, 1e-3, 1e-3]});
        // glTF requires a node's `children` to be non-empty when present.
        if !children.is_empty() {
            root["children"] = json!(children);
        }
        nodes.push(root);
        let mut scene = json!({"name": v.name, "nodes": [nodes.len() - 1]});
        let mut extras = serde_json::Map::new();
        if let Some(note) = v.note {
            extras.insert("note".into(), json!(note));
        }
        if let Some(alt) = v.alt {
            extras.insert("alt".into(), json!(alt));
        }
        if !extras.is_empty() {
            scene["extras"] = Value::Object(extras);
        }
        scenes.push(scene);
    }
    let materials: Vec<Value> = MATERIALS
        .iter()
        .map(|(name, c)| {
            let mut m = json!({"name": name, "pbrMetallicRoughness": {"baseColorFactor": c,
                               "metallicFactor": 0.0, "roughnessFactor": 0.6}});
            if c[3] < 1.0 {
                m["alphaMode"] = json!("BLEND");
                m["doubleSided"] = json!(true);
            }
            m
        })
        .collect();
    let mut gltf = json!({
        "asset": {"version": "2.0", "generator": generator},
        "materials": materials, "extras": extras,
    });
    // glTF requires `scenes` and `nodes` to be non-empty when present.
    if !scenes.is_empty() {
        gltf["scene"] = json!(0);
        gltf["scenes"] = json!(scenes);
        gltf["nodes"] = json!(nodes);
    }
    if !b.meshes.is_empty() {
        while !b.bin.len().is_multiple_of(4) {
            b.bin.push(0);
        }
        gltf["extensionsUsed"] = json!(["KHR_mesh_quantization"]);
        gltf["extensionsRequired"] = json!(["KHR_mesh_quantization"]);
        gltf["meshes"] = json!(b.meshes);
        gltf["accessors"] = json!(b.accessors);
        gltf["bufferViews"] = json!(b.views);
        gltf["buffers"] = json!([{"byteLength": b.bin.len()}]);
    }
    let mut text = gltf.to_string().into_bytes();
    while !text.len().is_multiple_of(4) {
        text.push(b' ');
    }
    let bin_chunk = if b.bin.is_empty() { 0 } else { 8 + b.bin.len() };
    let total = 12 + 8 + text.len() + bin_chunk;
    let mut out = Vec::with_capacity(total);
    out.extend(b"glTF");
    out.extend(2u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((text.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(text);
    if !b.bin.is_empty() {
        out.extend((b.bin.len() as u32).to_le_bytes());
        out.extend(b"BIN\0");
        out.extend(b.bin);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use brep_protocol::{DisplayEdge, DisplayFace};

    /// The JSON chunk and the binary chunk of a GLB, checking its header.
    fn chunks(g: &[u8]) -> (Value, &[u8]) {
        let word = |i: usize| u32::from_le_bytes(g[i..i + 4].try_into().unwrap()) as usize;
        assert_eq!(&g[0..4], b"glTF");
        assert_eq!(word(4), 2);
        assert_eq!(word(8), g.len());
        let n = word(12);
        assert_eq!(&g[16..20], b"JSON");
        let json = serde_json::from_slice(&g[20..20 + n]).unwrap();
        let bin = if 20 + n < g.len() {
            assert_eq!(&g[24 + n..28 + n], b"BIN\0");
            &g[28 + n..28 + n + word(20 + n)]
        } else {
            &[][..]
        };
        (json, bin)
    }

    /// A tetrahedron's four faces and six edges, offset from the origin.
    fn tetra() -> Display {
        let p: [[f64; 3]; 4] = [
            [10.0, 20.0, 30.0],
            [14.0, 20.0, 30.0],
            [10.0, 24.0, 30.0],
            [10.0, 20.0, 34.0],
        ];
        let faces = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]
            .iter()
            .map(|t: &[usize; 3]| {
                DisplayFace::new(
                    t.iter().map(|&i| p[i]).collect(),
                    vec![[0.0, 0.0, 1.0]; 3],
                    vec![[0, 1, 2]],
                )
            })
            .collect();
        let corners = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let edges = [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]]
            .iter()
            .map(|e| {
                let faces = (0..4u32)
                    .filter(|&f| e.iter().all(|c| corners[f as usize].contains(c)))
                    .collect();
                DisplayEdge::new(vec![p[e[0]], p[e[1]]], faces, Some([1.0, 2.0]), false)
            })
            .collect();
        Display::new(faces, edges, Vec::new())
    }

    /// The accessor's values, read as UNSIGNED_SHORT at its view's stride.
    fn shorts(gltf: &Value, bin: &[u8], accessor: &Value) -> Vec<u16> {
        let a = &gltf["accessors"][accessor.as_u64().unwrap() as usize];
        assert_eq!(a["componentType"], 5123);
        let v = &gltf["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
        let at = v["byteOffset"].as_u64().unwrap() as usize;
        let step = v["byteStride"].as_u64().unwrap_or(2) as usize;
        (0..a["count"].as_u64().unwrap() as usize)
            .map(|i| u16::from_le_bytes([bin[at + i * step], bin[at + i * step + 1]]))
            .collect()
    }

    #[test]
    fn each_vertex_names_its_face_or_edge_and_seams_and_splits_are_lines_of_their_own() {
        let mut d = tetra();
        let (a, b) = ([10.0, 20.0, 30.0], [12.0, 22.0, 30.0]);
        d.edges.push(DisplayEdge::new(
            vec![a, b],
            vec![0, 0],
            Some([0.0; 2]),
            true,
        ));
        d.edges.push(DisplayEdge::bare(vec![a]));
        d.seams.push(vec![a, b, [14.0, 20.0, 30.0]]);
        let views = [View {
            name: "a",
            note: None,
            alt: None,
            solids: vec![Solid {
                key: "t".into(),
                role: "main",
                display: &d,
            }],
        }];
        let g = glb(&views, json!({}), "test");
        let (gltf, bin) = chunks(&g);
        let mesh = &gltf["meshes"][0];
        let prims = mesh["primitives"].as_array().unwrap();
        assert_eq!(prims.len(), 3);
        let faces = shorts(&gltf, bin, &prims[0]["attributes"]["_FACE"]);
        assert_eq!(faces, [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3]);
        assert_eq!(prims[1]["extras"]["lines"], "edges");
        let edges = shorts(&gltf, bin, &prims[1]["attributes"]["_EDGE"]);
        assert_eq!(edges, [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
        // The split edge keeps its row; the seam, no edge, names the row past the last.
        assert_eq!(prims[2]["extras"]["lines"], "seams");
        let quiet = shorts(&gltf, bin, &prims[2]["attributes"]["_EDGE"]);
        assert_eq!(quiet, [6, 6, 8, 8, 8]);
        let name = |p: &Value| {
            gltf["materials"][p["material"].as_u64().unwrap() as usize]["name"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(
            [name(&prims[0]), name(&prims[1]), name(&prims[2])],
            ["main", "edge", "seam"]
        );
        let table = mesh["extras"]["edges"].as_array().unwrap();
        assert_eq!(table.len(), 8);
        assert_eq!(
            table[0],
            json!({"faces": [0, 1], "turn": [1.0, 2.0], "split": false})
        );
        assert_eq!(
            table[6],
            json!({"faces": [0, 0], "turn": [0.0, 0.0], "split": true})
        );
        assert_eq!(table[7], json!({"faces": [], "turn": null, "split": false}));
    }

    #[test]
    fn a_normal_decodes_within_a_ten_thousandth_of_its_direction() {
        // Reflection stripes turn a normal's error into a visible wobble: 8 bits, a step of
        // about 0.45 degrees, wobbles; 16 bits does not.
        let l = 14f64.sqrt();
        let n = [1.0 / l, 2.0 / l, -3.0 / l];
        let d = Display::new(
            vec![DisplayFace::new(
                vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                vec![n; 3],
                vec![[0, 1, 2]],
            )],
            Vec::new(),
            Vec::new(),
        );
        let views = [View {
            name: "a",
            note: None,
            alt: None,
            solids: vec![Solid {
                key: "t".into(),
                role: "main",
                display: &d,
            }],
        }];
        let g = glb(&views, json!({}), "test");
        let (gltf, bin) = chunks(&g);
        let prim = &gltf["meshes"][0]["primitives"][0];
        let a = &gltf["accessors"][prim["attributes"]["NORMAL"].as_u64().unwrap() as usize];
        assert_eq!(a["componentType"], 5122);
        assert_eq!(a["normalized"], true);
        let v = &gltf["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
        let at = v["byteOffset"].as_u64().unwrap() as usize;
        let step = v["byteStride"].as_u64().unwrap() as usize;
        for i in 0..3 {
            for (k, &c) in n.iter().enumerate() {
                let o = at + i * step + 2 * k;
                let q = f64::from(i16::from_le_bytes([bin[o], bin[o + 1]])) / 32767.0;
                assert!((q - c).abs() <= 1e-4, "{i} {k}: {q} against {c}");
            }
        }
    }

    #[test]
    fn a_solid_drawn_twice_is_stored_once_and_decodes_to_its_millimetres() {
        let d = tetra();
        let views = [
            View {
                name: "a",
                note: Some("first"),
                alt: Some("second"),
                solids: vec![Solid {
                    key: "t".into(),
                    role: "main",
                    display: &d,
                }],
            },
            View {
                name: "b",
                note: None,
                alt: None,
                solids: vec![Solid {
                    key: "t".into(),
                    role: "main",
                    display: &d,
                }],
            },
        ];
        let g = glb(&views, json!({"id": "x"}), "test");
        let (gltf, bin) = chunks(&g);
        assert_eq!(gltf["extras"]["id"], "x");
        assert_eq!(gltf["scenes"].as_array().unwrap().len(), 2);
        assert_eq!(gltf["scenes"][0]["extras"]["note"], "first");
        assert_eq!(gltf["scenes"][0]["extras"]["alt"], "second");
        assert!(gltf["scenes"][1].get("extras").is_none());
        assert_eq!(gltf["meshes"].as_array().unwrap().len(), 1);
        let prims = &gltf["meshes"][0]["primitives"];
        assert_eq!(
            (prims[0]["mode"].as_u64(), prims[1]["mode"].as_u64()),
            (Some(4), Some(1))
        );
        for a in gltf["accessors"].as_array().unwrap() {
            let v = &gltf["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
            let end = v["byteOffset"].as_u64().unwrap() + v["byteLength"].as_u64().unwrap();
            assert!(end as usize <= bin.len());
        }
        // The first position, decoded through its node, is the tetrahedron's first corner.
        let a = &gltf["accessors"][prims[0]["attributes"]["POSITION"].as_u64().unwrap() as usize];
        let v = &gltf["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
        let at = v["byteOffset"].as_u64().unwrap() as usize;
        let q: Vec<f64> = (0..3)
            .map(|k| f64::from(i16::from_le_bytes([bin[at + 2 * k], bin[at + 2 * k + 1]])))
            .collect();
        let node = &gltf["nodes"][0];
        assert_eq!(node["extras"]["role"], "main");
        let s = node["scale"][0].as_f64().unwrap();
        let mm: Vec<f64> = (0..3)
            .map(|k| node["translation"][k].as_f64().unwrap() + s * q[k])
            .collect();
        for k in 0..3 {
            assert!((mm[k] - [10.0, 20.0, 30.0][k]).abs() <= s, "{mm:?}");
        }
        // The root turns the suite's z to glTF's y: (x, y, z) in millimetres is (x, z, -y) in
        // glTF's frame, scaled to metres.
        let root = gltf["nodes"].as_array().unwrap().last().unwrap();
        let [x, _, _, w] = [0, 1, 2, 3].map(|i| root["rotation"][i].as_f64().unwrap());
        let (c, sn) = (w * w - x * x, 2.0 * x * w);
        let up = [mm[0], c * mm[1] - sn * mm[2], sn * mm[1] + c * mm[2]];
        for k in 0..3 {
            assert!((up[k] - [10.0, 30.0, -20.0][k]).abs() <= 2.0 * s, "{up:?}");
        }
    }

    #[test]
    fn a_view_with_nothing_to_draw_writes_no_empty_lists() {
        let empty = Display::default();
        let views = [View {
            name: "empty",
            note: None,
            alt: None,
            solids: vec![Solid {
                key: "e".into(),
                role: "main",
                display: &empty,
            }],
        }];
        let g = glb(&views, json!({}), "test");
        let (gltf, bin) = chunks(&g);
        assert!(bin.is_empty());
        assert!(gltf["nodes"][0].get("children").is_none());
        let (gltf, _) = chunks(&glb(&[], json!({}), "test"));
        for k in ["scene", "scenes", "nodes", "meshes"] {
            assert!(gltf.get(k).is_none(), "{k}");
        }
    }
}

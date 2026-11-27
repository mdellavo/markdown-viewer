//! 3D models drawn as GitHub draws ```` ```stl ```` blocks: ASCII STL, and
//! also Wavefront OBJ, PLY (ASCII) and OFF blocks, and model files shown as
//! images (`![part](part.stl)`, binary STL too, through embeds.rs). The
//! model is drawn when the page is built, as an SVG of shaded
//! triangles seen from above at an angle, so it works offline, in exports
//! and in PDFs. In the viewer (ui.js), dragging rotates it: the figure
//! carries the model's vertices in `data-mdv-mesh` for that.

use std::fmt::Write;

use crate::render::escape_text;

const SIZE: f64 = 400.0;
/// Most triangles drawn.
pub const MAX_TRIANGLES: usize = 20_000;
/// Most triangles the viewer can rotate (their vertices go in the page).
const MAX_ROTATABLE: usize = 5_000;
/// The starting view: turned and tilted (radians).
pub const YAW: f64 = -0.6;
pub const PITCH: f64 = 1.0;

pub type V3 = [f64; 3];

/// The model formats, by code block language or file extension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Stl,
    Obj,
    Ply,
    Off,
    /// glTF: JSON (a block, or a `.gltf` file), or binary `.glb`.
    Gltf,
    /// 3MF (files only: they're zip archives).
    ThreeMf,
}

impl Format {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "stl" => Some(Self::Stl),
            "obj" => Some(Self::Obj),
            "ply" => Some(Self::Ply),
            "off" => Some(Self::Off),
            "gltf" | "glb" => Some(Self::Gltf),
            "3mf" => Some(Self::ThreeMf),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Stl => "STL",
            Self::Obj => "OBJ",
            Self::Ply => "PLY",
            Self::Off => "OFF",
            Self::Gltf => "glTF",
            Self::ThreeMf => "3MF",
        }
    }

    fn lang(self) -> &'static str {
        match self {
            Self::Stl => "stl",
            Self::Obj => "obj",
            Self::Ply => "ply",
            Self::Off => "off",
            Self::Gltf => "gltf",
            Self::ThreeMf => "3mf",
        }
    }

    fn parse(self, code: &str) -> Result<Vec<[V3; 3]>, String> {
        match self {
            Self::Stl => parse(code),
            Self::Obj => parse_obj(code),
            Self::Ply => parse_ply(code),
            Self::Off => parse_off(code),
            Self::Gltf => crate::modelformats::gltf_text(code),
            Self::ThreeMf => Err("3MF files are zip archives; show the file as an image".into()),
        }
    }
}

/// Render a ```stl block.
pub fn render(code: &str, sourcepos: &str) -> String {
    render_as(Format::Stl, code, sourcepos)
}

/// Render a ```obj block.
pub fn obj(code: &str, sourcepos: &str) -> String {
    render_as(Format::Obj, code, sourcepos)
}

/// Render a ```ply block.
pub fn ply(code: &str, sourcepos: &str) -> String {
    render_as(Format::Ply, code, sourcepos)
}

/// Render a ```gltf block (glTF JSON with its buffers as data URIs).
pub fn gltf(code: &str, sourcepos: &str) -> String {
    render_as(Format::Gltf, code, sourcepos)
}

/// Render a ```off block.
pub fn off(code: &str, sourcepos: &str) -> String {
    render_as(Format::Off, code, sourcepos)
}

fn render_as(format: Format, code: &str, sourcepos: &str) -> String {
    match format.parse(code) {
        Ok(tris) if !tris.is_empty() => draw(&tris, sourcepos),
        Ok(_) => invalid(format, code, sourcepos, "no faces to draw"),
        Err(e) => invalid(format, code, sourcepos, &e),
    }
}

/// A model file (`![alt](part.stl)`): binary STL, GLB and 3MF as well as
/// the text formats; a glTF file's other files are read from `dir`. `Err`
/// says why it can't be drawn.
pub fn file(
    format: Format,
    bytes: &[u8],
    dir: Option<&std::path::Path>,
    sourcepos: &str,
) -> Result<String, String> {
    let tris = match binary_stl(format, bytes) {
        Some(tris) => tris,
        None if format == Format::Gltf => crate::modelformats::gltf_file(bytes, dir)?,
        None if format == Format::ThreeMf => crate::modelformats::three_mf(bytes)?,
        None => {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| format!("not a text {} file", format.name()))?;
            format.parse(text)?
        }
    };
    if tris.is_empty() {
        return Err("no faces to draw".into());
    }
    Ok(draw(&tris, sourcepos))
}

/// Binary STL: an 80-byte header, a triangle count, then 50 bytes per
/// triangle (normal, three vertices, attributes). Recognised by its size,
/// since some binary files also start with "solid".
fn binary_stl(format: Format, bytes: &[u8]) -> Option<Vec<[V3; 3]>> {
    if format != Format::Stl || bytes.len() < 84 {
        return None;
    }
    let count = u32::from_le_bytes(bytes[80..84].try_into().ok()?) as usize;
    if bytes.len() != 84 + count.checked_mul(50)? {
        return None;
    }
    let f = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as f64;
    let mut out = Vec::new();
    for i in 0..count.min(MAX_TRIANGLES) {
        let at = 84 + i * 50 + 12;
        let v = |k: usize| [f(at + k * 12), f(at + k * 12 + 4), f(at + k * 12 + 8)];
        let t = [v(0), v(1), v(2)];
        if t.iter().flatten().all(|n| n.is_finite()) {
            out.push(t);
        }
    }
    Some(out)
}

fn invalid(format: Format, code: &str, sourcepos: &str, why: &str) -> String {
    format!(
        "<figure class=\"mdv-stl mdv-stl-invalid\"{sourcepos}><figcaption>Can't draw this {}: {}</figcaption>\
         <pre lang=\"{}\"><code>{}</code></pre></figure>\n",
        format.name(),
        escape_text(why),
        format.lang(),
        escape_text(code)
    )
}

/// Polygons (lists of vertex indices) as triangles, fanned from their
/// first corner.
fn fan(verts: &[V3], faces: &[Vec<usize>]) -> Result<Vec<[V3; 3]>, String> {
    let mut out = Vec::new();
    for face in faces {
        for i in 1..face.len().saturating_sub(1) {
            let get = |k: usize| {
                verts.get(face[k]).copied().ok_or_else(|| {
                    format!(
                        "a face uses vertex {}, but there are {}",
                        face[k] + 1,
                        verts.len()
                    )
                })
            };
            out.push([get(0)?, get(i)?, get(i + 1)?]);
            if out.len() >= MAX_TRIANGLES {
                return Ok(out);
            }
        }
    }
    Ok(out)
}

fn numbers<'a>(words: impl Iterator<Item = &'a str>) -> Option<V3> {
    let nums: Vec<f64> = words
        .take(3)
        .map(|w| w.parse().ok())
        .collect::<Option<_>>()?;
    match nums[..] {
        [x, y, z] if x.is_finite() && y.is_finite() && z.is_finite() => Some([x, y, z]),
        _ => None,
    }
}

/// Wavefront OBJ: `v x y z` vertices and `f a b c …` faces (indices from 1,
/// negative ones counting back, `a/t/n` forms allowed).
fn parse_obj(code: &str) -> Result<Vec<[V3; 3]>, String> {
    let mut verts = Vec::new();
    let mut faces = Vec::new();
    for (n, line) in code.lines().enumerate() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => verts.push(
                numbers(words)
                    .ok_or_else(|| format!("line {}: a vertex needs three numbers", n + 1))?,
            ),
            Some("f") => {
                let mut face = Vec::new();
                for w in words {
                    let i: i64 = w
                        .split('/')
                        .next()
                        .and_then(|i| i.parse().ok())
                        .ok_or_else(|| format!("line {}: {w:?} isn't a vertex number", n + 1))?;
                    let index = if i < 0 { verts.len() as i64 + i } else { i - 1 };
                    if index < 0 {
                        return Err(format!("line {}: no vertex {i}", n + 1));
                    }
                    face.push(index as usize);
                }
                faces.push(face);
            }
            _ => {}
        }
    }
    fan(&verts, &faces)
}

/// ASCII PLY: a header of elements and properties, then their rows. Uses
/// the vertices' x, y and z and the faces' index lists.
fn parse_ply(code: &str) -> Result<Vec<[V3; 3]>, String> {
    let mut lines = code.lines();
    if lines.next().map(str::trim) != Some("ply") {
        return Err("a PLY file starts with \"ply\"".into());
    }
    // Each element: name, count, property names (a list property is "list").
    let mut elements: Vec<(String, usize, Vec<String>)> = Vec::new();
    for line in lines.by_ref() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words[..] {
            ["format", f, ..] if f != "ascii" => {
                return Err(format!("{f} PLY isn't supported, only ascii"));
            }
            ["element", name, count] => elements.push((
                name.into(),
                count
                    .parse()
                    .map_err(|_| format!("bad element count {count:?}"))?,
                Vec::new(),
            )),
            ["property", "list", .., name] | ["property", _, name] => {
                if let Some(e) = elements.last_mut() {
                    let list = words[1] == "list";
                    e.2.push(if list {
                        format!("list:{name}")
                    } else {
                        name.into()
                    });
                }
            }
            ["end_header"] => break,
            _ => {}
        }
    }
    let mut verts = Vec::new();
    let mut faces = Vec::new();
    for (name, count, props) in &elements {
        for _ in 0..*count {
            let row = lines.next().ok_or("the file ends early")?;
            let nums: Vec<f64> = row
                .split_whitespace()
                .map(|w| w.parse().map_err(|_| format!("{w:?} isn't a number")))
                .collect::<Result<_, _>>()?;
            if name == "vertex" {
                let at = |p: &str| {
                    props
                        .iter()
                        .position(|x| x == p)
                        .and_then(|i| nums.get(i).copied())
                };
                match (at("x"), at("y"), at("z")) {
                    (Some(x), Some(y), Some(z))
                        if x.is_finite() && y.is_finite() && z.is_finite() =>
                    {
                        verts.push([x, y, z])
                    }
                    _ => return Err("a vertex needs x, y and z".into()),
                }
            } else if name == "face" {
                let n = *nums.first().ok_or("an empty face")? as usize;
                faces.push(nums.iter().skip(1).take(n).map(|i| *i as usize).collect());
            }
        }
    }
    fan(&verts, &faces)
}

/// OFF: an optional `OFF` line, the vertex and face counts, the vertices,
/// then faces as `n i1 … in`. `#` starts a comment.
fn parse_off(code: &str) -> Result<Vec<[V3; 3]>, String> {
    let mut rows = code
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty());
    let mut first = rows.next().ok_or("empty")?;
    if let Some(rest) = first.strip_prefix("OFF") {
        first = match rest.trim() {
            "" => rows.next().ok_or("no counts after OFF")?,
            r => r,
        };
    }
    let counts: Vec<usize> = first
        .split_whitespace()
        .filter_map(|w| w.parse().ok())
        .collect();
    let [nv, nf, ..] = counts[..] else {
        return Err("the counts line needs vertices and faces".into());
    };
    let mut verts = Vec::with_capacity(nv.min(1 << 20));
    for _ in 0..nv {
        let row = rows.next().ok_or("fewer vertices than the count")?;
        verts.push(numbers(row.split_whitespace()).ok_or("a vertex needs three numbers")?);
    }
    let mut faces = Vec::new();
    for _ in 0..nf {
        let row = rows.next().ok_or("fewer faces than the count")?;
        let nums: Vec<usize> = row
            .split_whitespace()
            .filter_map(|w| w.parse().ok())
            .collect();
        let n = *nums.first().ok_or("an empty face")?;
        faces.push(nums.iter().skip(1).take(n).copied().collect());
    }
    fan(&verts, &faces)
}

/// The triangles of an ASCII STL file (its `vertex` lines, in threes).
fn parse(code: &str) -> Result<Vec<[V3; 3]>, String> {
    if !code.trim_start().starts_with("solid") {
        return Err("an ASCII STL file starts with \"solid\"".into());
    }
    let mut verts: Vec<V3> = Vec::new();
    for (n, line) in code.lines().enumerate() {
        let mut words = line.split_whitespace();
        if words.next() != Some("vertex") {
            continue;
        }
        let nums: Vec<f64> = words.filter_map(|w| w.parse().ok()).collect();
        match nums[..] {
            [x, y, z] if x.is_finite() && y.is_finite() && z.is_finite() => verts.push([x, y, z]),
            _ => return Err(format!("line {}: a vertex needs three numbers", n + 1)),
        }
    }
    if !verts.len().is_multiple_of(3) {
        return Err("facets need three vertices each".into());
    }
    Ok(verts
        .chunks(3)
        .take(MAX_TRIANGLES)
        .map(|c| [c[0], c[1], c[2]])
        .collect())
}

/// Centre the model on the origin and scale it to fit a unit sphere.
fn normalise(tris: &[[V3; 3]]) -> Vec<[V3; 3]> {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for v in tris.iter().flatten() {
        for i in 0..3 {
            lo[i] = lo[i].min(v[i]);
            hi[i] = hi[i].max(v[i]);
        }
    }
    let c = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let r = tris
        .iter()
        .flatten()
        .map(|v| ((v[0] - c[0]).powi(2) + (v[1] - c[1]).powi(2) + (v[2] - c[2]).powi(2)).sqrt())
        .fold(0.0, f64::max)
        .max(1e-9);
    tris.iter()
        .map(|t| t.map(|v| [(v[0] - c[0]) / r, (v[1] - c[1]) / r, (v[2] - c[2]) / r]))
        .collect()
}

/// Turn around the z axis (yaw), then tilt towards the viewer (pitch).
/// Returns screen x, screen y (down) and depth (larger is nearer). The
/// same maths is in ui.js.
pub fn view(v: V3, yaw: f64, pitch: f64) -> V3 {
    let (sy, cy) = yaw.sin_cos();
    let x = v[0] * cy - v[1] * sy;
    let y = v[0] * sy + v[1] * cy;
    let z = v[2];
    let (sp, cp) = pitch.sin_cos();
    // Looking down the y axis, tilted so the top (z) leans back.
    let depth = -y * sp + z * cp;
    let up = y * cp + z * sp;
    [x, -up, depth]
}

/// One shaded triangle, ready to draw.
pub struct Face {
    pub points: [(f64, f64); 3],
    pub depth: f64,
    pub light: f64,
}

fn faces(tris: &[[V3; 3]], yaw: f64, pitch: f64) -> Vec<Face> {
    let half = SIZE / 2.0;
    let scale = half * 0.92;
    let mut out: Vec<Face> = tris
        .iter()
        .map(|t| {
            let p = t.map(|v| view(v, yaw, pitch));
            // The face's normal in view space, lit from the viewer's
            // upper left; both sides lit, so open meshes look right.
            let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
            let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
            let n = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
            let l = [-0.4, -0.5, 0.77];
            let dot = ((n[0] * l[0] + n[1] * l[1] + n[2] * l[2]) / len).abs();
            Face {
                points: [
                    (half + p[0][0] * scale, half + p[0][1] * scale),
                    (half + p[1][0] * scale, half + p[1][1] * scale),
                    (half + p[2][0] * scale, half + p[2][1] * scale),
                ],
                depth: (p[0][2] + p[1][2] + p[2][2]) / 3.0,
                light: 0.3 + 0.7 * dot,
            }
        })
        .collect();
    // Painter's order: farthest first.
    out.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    out
}

fn draw(tris: &[[V3; 3]], sourcepos: &str) -> String {
    // Rounded as the page carries them, so the viewer's redraw (from
    // data-mdv-mesh) matches this one exactly.
    let tris: Vec<[V3; 3]> = normalise(tris)
        .iter()
        .map(|t| t.map(|v| v.map(|n| (n * 1000.0).round() / 1000.0)))
        .collect();
    let mut html = String::new();
    // The vertices for rotating in the viewer, rounded to keep pages small.
    let mesh = if tris.len() <= MAX_ROTATABLE {
        let nums: Vec<String> = tris
            .iter()
            .flatten()
            .flatten()
            .map(|n| n.to_string())
            .collect();
        format!(" data-mdv-mesh=\"{}\"", nums.join(","))
    } else {
        String::new()
    };
    let _ = write!(
        html,
        "<figure class=\"mdv-stl\"{sourcepos}{mesh}><svg viewBox=\"0 0 {SIZE} {SIZE}\" role=\"img\" \
         aria-label=\"3D model, {} triangle{}\" xmlns=\"http://www.w3.org/2000/svg\">",
        tris.len(),
        if tris.len() == 1 { "" } else { "s" }
    );
    for f in faces(&tris, YAW, PITCH) {
        let [(ax, ay), (bx, by), (cx, cy)] = f.points;
        let _ = write!(
            html,
            "<polygon points=\"{ax:.1},{ay:.1} {bx:.1},{by:.1} {cx:.1},{cy:.1}\" style=\"--l:{:.2}\"/>",
            f.light
        );
    }
    html.push_str("</svg>");
    if !mesh.is_empty() {
        html.push_str(
            "<figcaption class=\"mdv-stl-hint\">Drag to turn; double-click to reset</figcaption>",
        );
    }
    html.push_str("</figure>\n");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE_CORNER: &str = "solid corner
facet normal 0 0 -1
  outer loop
    vertex 0 0 0
    vertex 1 0 0
    vertex 0 1 0
  endloop
endfacet
facet normal 0 -1 0
  outer loop
    vertex 0 0 0
    vertex 1 0 0
    vertex 0 0 1
  endloop
endfacet
endsolid corner
";

    #[test]
    fn draws_shaded_triangles_that_can_be_turned() {
        let html = render(CUBE_CORNER, " data-sourcepos=\"1:1-16:3\"");
        assert!(
            html.starts_with(
                "<figure class=\"mdv-stl\" data-sourcepos=\"1:1-16:3\" data-mdv-mesh=\""
            ),
            "{html}"
        );
        assert_eq!(html.matches("<polygon ").count(), 2);
        assert!(html.contains("aria-label=\"3D model, 2 triangles\""));
        assert!(html.contains("style=\"--l:"));
        assert!(html.contains("Drag to turn"));
        // 2 triangles × 3 vertices × 3 numbers.
        let mesh = html
            .split("data-mdv-mesh=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        assert_eq!(mesh.split(',').count(), 18);
        assert_eq!(html.trim_end().lines().count(), 1);
    }

    #[test]
    fn bad_input_shows_the_source_and_why() {
        assert!(render("not stl", "").contains("starts with \"solid\""));
        let html = render("solid x\nvertex 1 2\nendsolid", "");
        assert!(
            html.contains("line 2: a vertex needs three numbers"),
            "{html}"
        );
        assert!(render("solid empty\nendsolid empty\n", "").contains("no faces"));
    }

    #[test]
    fn nearer_faces_are_drawn_last() {
        let tris = normalise(&parse(CUBE_CORNER).unwrap());
        let f = faces(&tris, YAW, PITCH);
        assert!(f[0].depth <= f[1].depth);
        assert!(f.iter().all(|f| (0.3..=1.0).contains(&f.light)));
    }

    #[test]
    fn obj_ply_and_off_draw_the_same_square() {
        let o = "# a square\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1/1 2/2 3/3 -1\n";
        let p = "ply\nformat ascii 1.0\nelement vertex 4\nproperty float x\nproperty float y\n\
                   property float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n\
                   0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
        let f = "OFF\n# counts\n4 1 0\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
        for html in [obj(o, ""), ply(p, ""), off(f, "")] {
            assert_eq!(html.matches("<polygon ").count(), 2, "{html}");
            assert!(html.contains("2 triangles"));
        }
        assert!(
            super::obj("v 0 0 0\nf 1 2 3\n", "")
                .contains("Can't draw this OBJ: a face uses vertex 2")
        );
        assert!(super::ply("ply\nformat binary_little_endian 1.0\n", "").contains("only ascii"));
    }

    #[test]
    fn binary_stl_files_are_read() {
        let mut bytes = vec![0u8; 80];
        bytes.extend(1u32.to_le_bytes());
        for n in [
            0.0f32, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0,
        ] {
            bytes.extend(n.to_le_bytes());
        }
        bytes.extend([0, 0]);
        let html = file(Format::Stl, &bytes, None, "").unwrap();
        assert!(html.contains("3D model, 1 triangle\""), "{html}");
        assert!(
            file(Format::Stl, b"\xff\xfe", None, "")
                .unwrap_err()
                .contains("not a text STL")
        );
    }
}

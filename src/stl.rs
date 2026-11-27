//! ```` ```stl ```` blocks (ASCII STL) drawn as 3D models, as GitHub does.
//! The model is drawn when the page is built, as an SVG of shaded
//! triangles seen from above at an angle, so it works offline, in exports
//! and in PDFs. In the viewer (ui.js), dragging rotates it: the figure
//! carries the model's vertices in `data-mdv-mesh` for that.

use std::fmt::Write;

use crate::render::escape_text;

const SIZE: f64 = 400.0;
/// Most triangles drawn.
const MAX_TRIANGLES: usize = 20_000;
/// Most triangles the viewer can rotate (their vertices go in the page).
const MAX_ROTATABLE: usize = 5_000;
/// The starting view: turned and tilted (radians).
pub const YAW: f64 = -0.6;
pub const PITCH: f64 = 1.0;

type V3 = [f64; 3];

/// Render a ```stl block.
pub fn render(code: &str, sourcepos: &str) -> String {
    match parse(code) {
        Ok(tris) if !tris.is_empty() => draw(&tris, sourcepos),
        Ok(_) => invalid(code, sourcepos, "no facets to draw"),
        Err(e) => invalid(code, sourcepos, &e),
    }
}

fn invalid(code: &str, sourcepos: &str, why: &str) -> String {
    format!(
        "<figure class=\"mdv-stl mdv-stl-invalid\"{sourcepos}><figcaption>Can't draw this STL: {}</figcaption>\
         <pre lang=\"stl\"><code>{}</code></pre></figure>\n",
        escape_text(why),
        escape_text(code)
    )
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
fn view(v: V3, yaw: f64, pitch: f64) -> V3 {
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
        assert!(render("solid empty\nendsolid empty\n", "").contains("no facets"));
    }

    #[test]
    fn nearer_faces_are_drawn_last() {
        let tris = normalise(&parse(CUBE_CORNER).unwrap());
        let f = faces(&tris, YAW, PITCH);
        assert!(f[0].depth <= f[1].depth);
        assert!(f.iter().all(|f| (0.3..=1.0).contains(&f.light)));
    }
}

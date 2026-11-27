//! ```` ```geojson ```` and ```` ```topojson ```` blocks drawn as maps, as
//! GitHub does. GitHub puts them on map tiles from the web; pages here stay
//! offline, so the map is an SVG drawn when the page is built: the shapes in
//! Web Mercator (GitHub's projection) over a latitude/longitude grid, with
//! each feature's properties as its tooltip. Styles follow the simplestyle
//! properties GitHub supports (`stroke`, `stroke-width`, `stroke-opacity`,
//! `fill`, `fill-opacity`, `marker-color`).

use std::fmt::Write;

use serde_json::{Map, Value};

use crate::render::{escape_attr, escape_text};

const WIDTH: f64 = 800.0;
const MIN_HEIGHT: f64 = 260.0;
const MAX_HEIGHT: f64 = 480.0;
/// Most features drawn (a very large file would make a huge page).
const MAX_FEATURES: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Format {
    GeoJson,
    TopoJson,
}

/// Render a ```geojson block.
pub fn geojson(code: &str, sourcepos: &str) -> String {
    render(code, sourcepos, Format::GeoJson)
}

/// Render a ```topojson block.
pub fn topojson(code: &str, sourcepos: &str) -> String {
    render(code, sourcepos, Format::TopoJson)
}

fn render(code: &str, sourcepos: &str, format: Format) -> String {
    let parsed = serde_json::from_str::<Value>(code)
        .map_err(|e| format!("not valid JSON: {e}"))
        .and_then(|v| match format {
            Format::GeoJson => Ok(v),
            Format::TopoJson => topo_to_geo(&v),
        })
        .and_then(|v| collect(&v));
    match parsed {
        Ok(features) if !features.is_empty() => draw(&features, sourcepos),
        Ok(_) => invalid(code, sourcepos, format, "no geometry to draw"),
        Err(e) => invalid(code, sourcepos, format, &e),
    }
}

/// A block that couldn't be drawn: the source, with what went wrong.
fn invalid(code: &str, sourcepos: &str, format: Format, why: &str) -> String {
    let (lang, name) = match format {
        Format::GeoJson => ("geojson", "GeoJSON"),
        Format::TopoJson => ("topojson", "TopoJSON"),
    };
    format!(
        "<figure class=\"mdv-map mdv-map-invalid\"{sourcepos}><figcaption>Can't draw this {name}: {}</figcaption>\
         <pre lang=\"{lang}\"><code>{}</code></pre></figure>\n",
        escape_text(why),
        escape_text(code)
    )
}

/// Longitude, latitude.
type Pt = (f64, f64);

#[derive(Debug)]
enum Shape {
    Point(Pt),
    Line(Vec<Pt>),
    /// Outer ring, then holes.
    Polygon(Vec<Vec<Pt>>),
}

#[derive(Debug)]
struct Feature {
    shapes: Vec<Shape>,
    props: Map<String, Value>,
}

/// The features of a GeoJSON object (FeatureCollection, Feature or bare
/// geometry).
fn collect(v: &Value) -> Result<Vec<Feature>, String> {
    let mut out = Vec::new();
    let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "FeatureCollection" => {
            let features = v
                .get("features")
                .and_then(Value::as_array)
                .ok_or("a FeatureCollection needs \"features\"")?;
            for f in features.iter().take(MAX_FEATURES) {
                out.push(feature(f)?);
            }
        }
        "Feature" => out.push(feature(v)?),
        "" => return Err("no \"type\"".into()),
        _ => {
            let mut shapes = Vec::new();
            geometry(v, &mut shapes)?;
            out.push(Feature {
                shapes,
                props: Map::new(),
            });
        }
    }
    out.retain(|f| !f.shapes.is_empty());
    Ok(out)
}

fn feature(v: &Value) -> Result<Feature, String> {
    let mut shapes = Vec::new();
    if let Some(g) = v.get("geometry").filter(|g| !g.is_null()) {
        geometry(g, &mut shapes)?;
    }
    let props = v
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Ok(Feature { shapes, props })
}

fn geometry(g: &Value, out: &mut Vec<Shape>) -> Result<(), String> {
    let kind = g.get("type").and_then(Value::as_str).unwrap_or("");
    if kind == "GeometryCollection" {
        for child in g
            .get("geometries")
            .and_then(Value::as_array)
            .ok_or("a GeometryCollection needs \"geometries\"")?
        {
            geometry(child, out)?;
        }
        return Ok(());
    }
    let c = g
        .get("coordinates")
        .ok_or_else(|| format!("a {kind} needs \"coordinates\""))?;
    let bad = || format!("bad coordinates in a {kind}");
    match kind {
        "Point" => out.push(Shape::Point(point(c).ok_or_else(bad)?)),
        "MultiPoint" => {
            for p in c.as_array().ok_or_else(bad)? {
                out.push(Shape::Point(point(p).ok_or_else(bad)?));
            }
        }
        "LineString" => out.push(Shape::Line(line(c).ok_or_else(bad)?)),
        "MultiLineString" => {
            for l in c.as_array().ok_or_else(bad)? {
                out.push(Shape::Line(line(l).ok_or_else(bad)?));
            }
        }
        "Polygon" => out.push(Shape::Polygon(rings(c).ok_or_else(bad)?)),
        "MultiPolygon" => {
            for p in c.as_array().ok_or_else(bad)? {
                out.push(Shape::Polygon(rings(p).ok_or_else(bad)?));
            }
        }
        other => return Err(format!("unknown geometry type \"{other}\"")),
    }
    Ok(())
}

fn point(v: &Value) -> Option<Pt> {
    let a = v.as_array()?;
    let (x, y) = (a.first()?.as_f64()?, a.get(1)?.as_f64()?);
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

fn line(v: &Value) -> Option<Vec<Pt>> {
    v.as_array()?.iter().map(point).collect()
}

fn rings(v: &Value) -> Option<Vec<Vec<Pt>>> {
    v.as_array()?.iter().map(line).collect()
}

// ---- TopoJSON ---------------------------------------------------------------

/// A TopoJSON topology as GeoJSON: a FeatureCollection of every object.
fn topo_to_geo(topo: &Value) -> Result<Value, String> {
    if topo.get("type").and_then(Value::as_str) != Some("Topology") {
        return Err("a TopoJSON file has \"type\": \"Topology\"".into());
    }
    let transform = topo.get("transform").map(|t| {
        let pair = |k: &str, d: f64| -> (f64, f64) {
            let a = t.get(k).and_then(Value::as_array);
            let n = |i: usize| {
                a.and_then(|a| a.get(i))
                    .and_then(Value::as_f64)
                    .unwrap_or(d)
            };
            (n(0), n(1))
        };
        (pair("scale", 1.0), pair("translate", 0.0))
    });
    // Decode every arc to absolute positions.
    let arcs: Vec<Vec<Pt>> = topo
        .get("arcs")
        .and_then(Value::as_array)
        .ok_or("a Topology needs \"arcs\"")?
        .iter()
        .map(|arc| {
            let mut pts = Vec::new();
            let (mut x, mut y) = (0.0, 0.0);
            for p in arc.as_array().into_iter().flatten() {
                let Some((dx, dy)) = point(p) else { continue };
                match transform {
                    // Quantized: positions are deltas.
                    Some(((sx, sy), (tx, ty))) => {
                        x += dx;
                        y += dy;
                        pts.push((x * sx + tx, y * sy + ty));
                    }
                    None => pts.push((dx, dy)),
                }
            }
            pts
        })
        .collect();
    let objects = topo
        .get("objects")
        .and_then(Value::as_object)
        .ok_or("a Topology needs \"objects\"")?;
    let mut features = Vec::new();
    for object in objects.values() {
        topo_object(object, &arcs, transform, &mut features)?;
    }
    Ok(serde_json::json!({ "type": "FeatureCollection", "features": features }))
}

type Transform = Option<((f64, f64), (f64, f64))>;

fn topo_object(
    o: &Value,
    arcs: &[Vec<Pt>],
    transform: Transform,
    out: &mut Vec<Value>,
) -> Result<(), String> {
    if o.get("type").and_then(Value::as_str) == Some("GeometryCollection") {
        for g in o
            .get("geometries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            topo_object(g, arcs, transform, out)?;
        }
        return Ok(());
    }
    let geometry = topo_geometry(o, arcs, transform)?;
    out.push(serde_json::json!({
        "type": "Feature",
        "geometry": geometry,
        "properties": o.get("properties").cloned().unwrap_or(Value::Null),
    }));
    Ok(())
}

fn topo_geometry(o: &Value, arcs: &[Vec<Pt>], transform: Transform) -> Result<Value, String> {
    let kind = o.get("type").and_then(Value::as_str).unwrap_or("");
    let at = |p: &Value| -> Option<Value> {
        let (x, y) = point(p)?;
        let (x, y) = match transform {
            Some(((sx, sy), (tx, ty))) => (x * sx + tx, y * sy + ty),
            None => (x, y),
        };
        Some(serde_json::json!([x, y]))
    };
    // A line from arc indices; ~i (that is, -i-1) is arc i reversed.
    let chain = |ids: &Value| -> Option<Value> {
        let mut pts: Vec<Pt> = Vec::new();
        for id in ids.as_array()? {
            let i = id.as_i64()?;
            let (index, reversed) = if i < 0 {
                ((!i) as usize, true)
            } else {
                (i as usize, false)
            };
            let mut arc = arcs.get(index)?.clone();
            if reversed {
                arc.reverse();
            }
            if !pts.is_empty() {
                arc.remove(0);
            }
            pts.extend(arc);
        }
        Some(Value::Array(
            pts.into_iter()
                .map(|(x, y)| serde_json::json!([x, y]))
                .collect(),
        ))
    };
    let list = |v: &Value, f: &dyn Fn(&Value) -> Option<Value>| -> Option<Value> {
        Some(Value::Array(
            v.as_array()?.iter().map(f).collect::<Option<_>>()?,
        ))
    };
    let bad = || format!("bad {kind} in the topology");
    let coordinates = match kind {
        "Point" => o.get("coordinates").and_then(at),
        "MultiPoint" => o.get("coordinates").and_then(|c| list(c, &at)),
        "LineString" => o.get("arcs").and_then(chain),
        "MultiLineString" | "Polygon" => o.get("arcs").and_then(|a| list(a, &chain)),
        "MultiPolygon" => o
            .get("arcs")
            .and_then(|a| list(a, &|p: &Value| list(p, &chain))),
        "" => return Ok(Value::Null),
        other => return Err(format!("unknown geometry type \"{other}\"")),
    }
    .ok_or_else(bad)?;
    Ok(serde_json::json!({ "type": kind, "coordinates": coordinates }))
}

// ---- Drawing ----------------------------------------------------------------

/// Web Mercator, in degrees-like units (x = longitude).
fn project((lon, lat): Pt) -> (f64, f64) {
    let lat = lat.clamp(-85.0, 85.0).to_radians();
    let y = (std::f64::consts::FRAC_PI_4 + lat / 2.0)
        .tan()
        .ln()
        .to_degrees();
    (lon, y)
}

fn unproject_lat(y: f64) -> f64 {
    (2.0 * y.to_radians().exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees()
}

fn project_lat(lat: f64) -> f64 {
    project((0.0, lat)).1
}

struct View {
    min_x: f64,
    max_y: f64,
    scale: f64,
    height: f64,
}

impl View {
    fn xy(&self, p: Pt) -> (f64, f64) {
        let (x, y) = project(p);
        ((x - self.min_x) * self.scale, (self.max_y - y) * self.scale)
    }
}

fn draw(features: &[Feature], sourcepos: &str) -> String {
    // Bounds of everything, projected.
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut grow = |p: Pt| {
        let (x, y) = project(p);
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    };
    for f in features {
        for s in &f.shapes {
            match s {
                Shape::Point(p) => grow(*p),
                Shape::Line(l) => l.iter().copied().for_each(&mut grow),
                Shape::Polygon(r) => r.iter().flatten().copied().for_each(&mut grow),
            }
        }
    }
    // A margin, and a minimum size (a single point gets some surroundings).
    let span = (x1 - x0).max(y1 - y0).max(0.02);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (mut w, mut h) = ((x1 - x0).max(span * 0.3), (y1 - y0).max(span * 0.3));
    w *= 1.12;
    h *= 1.12;
    let height = (WIDTH * h / w).clamp(MIN_HEIGHT, MAX_HEIGHT);
    // Fit, then widen whichever way the box has room.
    let scale = (WIDTH / w).min(height / h);
    let (w, h) = (WIDTH / scale, height / scale);
    let view = View {
        min_x: cx - w / 2.0,
        max_y: cy + h / 2.0,
        scale,
        height,
    };

    let mut svg = String::new();
    let _ = write!(
        svg,
        "<figure class=\"mdv-map\"{sourcepos}><svg viewBox=\"0 0 {WIDTH} {height:.0}\" \
         role=\"img\" aria-label=\"Map with {} feature{}\" xmlns=\"http://www.w3.org/2000/svg\">",
        features.len(),
        if features.len() == 1 { "" } else { "s" }
    );
    grid(&mut svg, &view);
    // Areas first, then lines, then points on top.
    for pass in 0..3 {
        for f in features {
            let style = Style::from(&f.props);
            let title = tooltip(&f.props);
            for s in &f.shapes {
                match (pass, s) {
                    (0, Shape::Polygon(rings)) => {
                        let mut d = String::new();
                        for ring in rings {
                            path_data(&mut d, ring, &view, true);
                        }
                        let _ = write!(
                            svg,
                            "<path class=\"mdv-map-area\" d=\"{d}\" fill-rule=\"evenodd\"{}>{title}</path>",
                            style.attrs(true)
                        );
                    }
                    (1, Shape::Line(line)) => {
                        let mut d = String::new();
                        path_data(&mut d, line, &view, false);
                        let _ = write!(
                            svg,
                            "<path class=\"mdv-map-line\" d=\"{d}\"{}>{title}</path>",
                            style.attrs(false)
                        );
                    }
                    (2, Shape::Point(p)) => {
                        let (x, y) = view.xy(*p);
                        let fill = style
                            .marker
                            .as_ref()
                            .map(|c| format!(" style=\"fill:{c}\""))
                            .unwrap_or_default();
                        let _ = write!(
                            svg,
                            "<circle class=\"mdv-map-point\" cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"6\"{fill}>{title}</circle>"
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    svg.push_str("</svg></figure>\n");
    svg
}

fn path_data(d: &mut String, pts: &[Pt], view: &View, close: bool) {
    for (i, p) in pts.iter().enumerate() {
        let (x, y) = view.xy(*p);
        let _ = write!(d, "{}{x:.1} {y:.1}", if i == 0 { "M" } else { "L" });
    }
    if close && !pts.is_empty() {
        d.push('Z');
    }
}

/// Latitude and longitude lines at a round step, labelled at the edges.
fn grid(svg: &mut String, view: &View) {
    let lon0 = view.min_x;
    let lon1 = view.min_x + WIDTH / view.scale;
    let lat1 = unproject_lat(view.max_y);
    let lat0 = unproject_lat(view.max_y - view.height / view.scale);
    let step = nice_step((lon1 - lon0).max(lat1 - lat0) / 6.0);
    svg.push_str("<g class=\"mdv-map-grid\">");
    let mut lon = (lon0 / step).ceil() * step;
    while lon <= lon1 {
        let x = (lon - view.min_x) * view.scale;
        let _ = write!(svg, "<path d=\"M{x:.1} 0V{:.0}\"/>", view.height);
        // Labels along the bottom, where they fit.
        if (40.0..=WIDTH - 48.0).contains(&x) {
            let _ = write!(
                svg,
                "<text x=\"{:.1}\" y=\"{:.0}\">{}</text>",
                x + 3.0,
                view.height - 4.0,
                degrees(lon, 'E', 'W', step)
            );
        }
        lon += step;
    }
    let mut lat = (lat0 / step).ceil() * step;
    while lat <= lat1 {
        let y = (view.max_y - project_lat(lat)) * view.scale;
        let _ = write!(svg, "<path d=\"M0 {y:.1}H{WIDTH}\"/>");
        // Labels down the left, clear of the top edge and the bottom row.
        if (16.0..=view.height - 20.0).contains(&y) {
            let _ = write!(
                svg,
                "<text x=\"4\" y=\"{:.1}\">{}</text>",
                y - 3.0,
                degrees(lat, 'N', 'S', step)
            );
        }
        lat += step;
    }
    svg.push_str("</g>");
}

fn nice_step(raw: f64) -> f64 {
    let raw = raw.max(1e-6);
    let pow = 10f64.powf(raw.log10().floor());
    let m = raw / pow;
    let nice = if m < 1.5 {
        1.0
    } else if m < 3.5 {
        2.0
    } else if m < 7.5 {
        5.0
    } else {
        10.0
    };
    (nice * pow).min(30.0)
}

fn degrees(v: f64, pos: char, neg: char, step: f64) -> String {
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10()).ceil() as usize
    };
    let v = if v.abs() < step / 1000.0 { 0.0 } else { v };
    let hemi = if v > 0.0 {
        pos.to_string()
    } else if v < 0.0 {
        neg.to_string()
    } else {
        String::new()
    };
    format!("{:.*}°{hemi}", decimals, v.abs())
}

/// The feature's properties, one per line, as an SVG tooltip.
fn tooltip(props: &Map<String, Value>) -> String {
    let lines: Vec<String> = props
        .iter()
        .filter(|(k, _)| !STYLE_KEYS.contains(&k.as_str()))
        .map(|(k, v)| match v {
            Value::String(s) => format!("{k}: {s}"),
            other => format!("{k}: {other}"),
        })
        .collect();
    if lines.is_empty() {
        String::new()
    } else {
        format!("<title>{}</title>", escape_text(&lines.join("\n")))
    }
}

const STYLE_KEYS: &[&str] = &[
    "stroke",
    "stroke-width",
    "stroke-opacity",
    "fill",
    "fill-opacity",
    "marker-color",
    "marker-size",
    "marker-symbol",
];

/// simplestyle properties, checked to be plain colours and numbers.
#[derive(Default)]
struct Style {
    stroke: Option<String>,
    stroke_width: Option<f64>,
    stroke_opacity: Option<f64>,
    fill: Option<String>,
    fill_opacity: Option<f64>,
    marker: Option<String>,
}

impl Style {
    fn from(props: &Map<String, Value>) -> Self {
        let colour = |k: &str| props.get(k).and_then(Value::as_str).and_then(safe_colour);
        let number = |k: &str| {
            props
                .get(k)
                .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
                .filter(|n| n.is_finite() && *n >= 0.0)
        };
        Style {
            stroke: colour("stroke"),
            stroke_width: number("stroke-width").map(|n| n.min(20.0)),
            stroke_opacity: number("stroke-opacity").map(|n| n.min(1.0)),
            fill: colour("fill"),
            fill_opacity: number("fill-opacity").map(|n| n.min(1.0)),
            marker: colour("marker-color"),
        }
    }

    /// Inline styles (they win over the theme's classes).
    fn attrs(&self, area: bool) -> String {
        let mut css = String::new();
        if let Some(c) = &self.stroke {
            let _ = write!(css, "stroke:{c};");
        }
        if let Some(w) = self.stroke_width {
            let _ = write!(css, "stroke-width:{w};");
        }
        if let Some(o) = self.stroke_opacity {
            let _ = write!(css, "stroke-opacity:{o};");
        }
        if area {
            if let Some(c) = &self.fill {
                let _ = write!(css, "fill:{c};");
            }
            if let Some(o) = self.fill_opacity {
                let _ = write!(css, "fill-opacity:{o};");
            }
        }
        if css.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", escape_attr(&css))
        }
    }
}

/// `#rgb`, `#rrggbb` or a colour name; anything else is ignored.
fn safe_colour(s: &str) -> Option<String> {
    let s = s.trim();
    let hex = s
        .strip_prefix('#')
        .is_some_and(|h| matches!(h.len(), 3 | 6) && h.chars().all(|c| c.is_ascii_hexdigit()));
    let named = !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_alphabetic());
    (hex || named).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEO: &str = r##"{"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"name":"Boston <MA>","marker-color":"#ff0000"},
       "geometry":{"type":"Point","coordinates":[-71.06,42.36]}},
      {"type":"Feature","properties":{"stroke":"red;}","fill":"#0f0","fill-opacity":0.3},
       "geometry":{"type":"Polygon","coordinates":[[[-72,41],[-70,41],[-70,43],[-72,43],[-72,41]]]}},
      {"type":"Feature","properties":null,
       "geometry":{"type":"LineString","coordinates":[[-72,41],[-70,43]]}}]}"##;

    #[test]
    fn draws_points_lines_and_areas_with_styles() {
        let html = geojson(GEO, " data-sourcepos=\"1:1-9:3\"");
        assert!(
            html.starts_with("<figure class=\"mdv-map\" data-sourcepos=\"1:1-9:3\"><svg"),
            "{html}"
        );
        assert!(html.contains("aria-label=\"Map with 3 features\""));
        assert_eq!(html.matches("<circle class=\"mdv-map-point\"").count(), 1);
        assert_eq!(html.matches("class=\"mdv-map-area\"").count(), 1);
        assert_eq!(html.matches("class=\"mdv-map-line\"").count(), 1);
        assert!(
            html.contains("<title>name: Boston &lt;MA&gt;</title>"),
            "{html}"
        );
        assert!(html.contains("style=\"fill:#ff0000\""));
        // A colour that isn't one is dropped; good ones are kept.
        assert!(
            html.contains("style=\"fill:#0f0;fill-opacity:0.3;\""),
            "{html}"
        );
        assert!(!html.contains("red;}"));
        // The grid is labelled.
        assert!(
            html.contains("°N</text>") && html.contains("°W</text>"),
            "{html}"
        );
        // One element, on one line, for live updates.
        assert_eq!(html.trim_end().lines().count(), 1);
    }

    #[test]
    fn bad_input_shows_the_source_and_why() {
        let html = geojson("{nope", "");
        assert!(html.contains("mdv-map-invalid"));
        assert!(
            html.contains("Can't draw this GeoJSON: not valid JSON"),
            "{html}"
        );
        assert!(html.contains("<code>{nope</code>"));
        let html = geojson(r#"{"type":"Point","coordinates":["a"]}"#, "");
        assert!(html.contains("bad coordinates in a Point"), "{html}");
    }

    #[test]
    fn topojson_is_decoded() {
        // Two quantized arcs forming a square, and a point.
        let topo = r#"{"type":"Topology",
          "transform":{"scale":[0.01,0.01],"translate":[10,20]},
          "objects":{"shapes":{"type":"GeometryCollection","geometries":[
            {"type":"Polygon","arcs":[[0,1]],"properties":{"name":"square"}},
            {"type":"Point","coordinates":[50,50],"properties":{"name":"middle"}}]}},
          "arcs":[[[0,0],[100,0],[0,100]],[[100,100],[-100,0],[0,-100]]]}"#;
        let geo = topo_to_geo(&serde_json::from_str(topo).unwrap()).unwrap();
        let features = collect(&geo).unwrap();
        assert_eq!(features.len(), 2);
        let Shape::Polygon(rings) = &features[0].shapes[0] else {
            panic!()
        };
        let ring: Vec<(i64, i64)> = rings[0]
            .iter()
            .map(|(x, y)| ((x * 10.0).round() as i64, (y * 10.0).round() as i64))
            .collect();
        assert_eq!(
            ring,
            vec![(100, 200), (110, 200), (110, 210), (100, 210), (100, 200)]
        );
        let Shape::Point(p) = features[1].shapes[0] else {
            panic!()
        };
        assert_eq!(((p.0 * 10.0).round(), (p.1 * 10.0).round()), (105.0, 205.0));
        let html = topojson(topo, "");
        assert!(html.contains("<title>name: square</title>"));
        assert!(topojson(r#"{"type":"Feature"}"#, "").contains("\"type\": \"Topology\""));
    }
}

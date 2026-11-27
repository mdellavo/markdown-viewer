//! Other geographic formats, turned into GeoJSON for geo.rs to draw: GPX
//! (GPS tracks, routes and waypoints), KML (Google Earth placemarks) and WKT
//! (`POINT (…)`, `POLYGON ((…))` as databases write them).

use serde_json::{Map, Value, json};

/// GPX: waypoints as points, tracks and routes as lines, with their names
/// (and descriptions) as properties.
pub fn gpx(code: &str) -> Result<Value, String> {
    let doc = roxmltree::Document::parse(code).map_err(|e| format!("not valid XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "gpx" {
        return Err("a GPX file's root element is <gpx>".into());
    }
    let mut features = Vec::new();
    for node in root.children().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "wpt" => {
                if let Some(p) = lon_lat(&node) {
                    features.push(feature(json!({"type": "Point", "coordinates": p}), &node));
                }
            }
            "rte" => {
                let pts: Vec<_> = elements(&node, "rtept")
                    .filter_map(|n| lon_lat(&n))
                    .collect();
                if pts.len() > 1 {
                    features.push(feature(
                        json!({"type": "LineString", "coordinates": pts}),
                        &node,
                    ));
                }
            }
            "trk" => {
                let segs: Vec<Vec<_>> = elements(&node, "trkseg")
                    .map(|s| {
                        elements(&s, "trkpt")
                            .filter_map(|n| lon_lat(&n))
                            .collect::<Vec<_>>()
                    })
                    .filter(|s| s.len() > 1)
                    .collect();
                if !segs.is_empty() {
                    features.push(feature(
                        json!({"type": "MultiLineString", "coordinates": segs}),
                        &node,
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(json!({"type": "FeatureCollection", "features": features}))
}

fn elements<'a, 'i>(
    node: &roxmltree::Node<'a, 'i>,
    name: &'static str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> {
    node.children()
        .filter(move |n| n.is_element() && n.tag_name().name() == name)
}

fn lon_lat(node: &roxmltree::Node) -> Option<[f64; 2]> {
    let lat: f64 = node.attribute("lat")?.trim().parse().ok()?;
    let lon: f64 = node.attribute("lon")?.trim().parse().ok()?;
    (lat.is_finite() && lon.is_finite()).then_some([lon, lat])
}

fn child_text(node: &roxmltree::Node, name: &str) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

fn feature(geometry: Value, node: &roxmltree::Node) -> Value {
    let mut props = Map::new();
    for (key, tag) in [
        ("name", "name"),
        ("description", "desc"),
        ("description", "description"),
    ] {
        if let Some(t) = child_text(node, tag) {
            props.insert(key.into(), Value::String(t));
        }
    }
    json!({"type": "Feature", "geometry": geometry, "properties": props})
}

/// KML: every Placemark's Point, LineString, Polygon (with holes) or
/// MultiGeometry, with its name and description.
pub fn kml(code: &str) -> Result<Value, String> {
    let doc = roxmltree::Document::parse(code).map_err(|e| format!("not valid XML: {e}"))?;
    if doc.root_element().tag_name().name() != "kml" {
        return Err("a KML file's root element is <kml>".into());
    }
    let mut features = Vec::new();
    for mark in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Placemark")
    {
        let mut geoms = Vec::new();
        for g in mark.children().filter(|n| n.is_element()) {
            kml_geometry(&g, &mut geoms)?;
        }
        let geometry = match geoms.len() {
            0 => continue,
            1 => geoms.remove(0),
            _ => json!({"type": "GeometryCollection", "geometries": geoms}),
        };
        features.push(feature(geometry, &mark));
    }
    Ok(json!({"type": "FeatureCollection", "features": features}))
}

fn kml_geometry(g: &roxmltree::Node, out: &mut Vec<Value>) -> Result<(), String> {
    let coords = |n: &roxmltree::Node| -> Result<Vec<[f64; 2]>, String> {
        let text = n
            .descendants()
            .find(|c| c.is_element() && c.tag_name().name() == "coordinates")
            .and_then(|c| c.text())
            .unwrap_or("");
        text.split_whitespace()
            .map(|t| {
                let mut it = t.split(',').map(|v| v.parse::<f64>());
                match (it.next(), it.next()) {
                    (Some(Ok(lon)), Some(Ok(lat))) if lon.is_finite() && lat.is_finite() => {
                        Ok([lon, lat])
                    }
                    _ => Err(format!("{t:?} isn't longitude,latitude")),
                }
            })
            .collect()
    };
    match g.tag_name().name() {
        "Point" => {
            if let Some(p) = coords(g)?.first() {
                out.push(json!({"type": "Point", "coordinates": p}));
            }
        }
        "LineString" | "LinearRing" => {
            out.push(json!({"type": "LineString", "coordinates": coords(g)?}))
        }
        "Polygon" => {
            let mut rings = Vec::new();
            for boundary in ["outerBoundaryIs", "innerBoundaryIs"] {
                for b in g
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == boundary)
                {
                    rings.push(coords(&b)?);
                }
            }
            out.push(json!({"type": "Polygon", "coordinates": rings}));
        }
        "MultiGeometry" => {
            for c in g.children().filter(|n| n.is_element()) {
                kml_geometry(&c, out)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// WKT, one geometry per line (or one spread over several lines), e.g.
/// `POINT (30 10)` or `MULTIPOLYGON (((…)), ((…)))`. Z and M values are
/// ignored.
pub fn wkt(code: &str) -> Result<Value, String> {
    let mut p = Parser {
        s: code.as_bytes(),
        i: 0,
    };
    let mut geometries = Vec::new();
    loop {
        p.space();
        if p.i >= p.s.len() {
            break;
        }
        geometries.push(p.geometry()?);
        p.space();
        if p.peek() == Some(b';') || p.peek() == Some(b',') {
            p.i += 1;
        }
    }
    let features: Vec<Value> = geometries
        .into_iter()
        .map(|g| json!({"type": "Feature", "geometry": g, "properties": {}}))
        .collect();
    Ok(json!({"type": "FeatureCollection", "features": features}))
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn space(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            self.i += 1;
        }
    }

    fn word(&mut self) -> String {
        self.space();
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.i]).to_ascii_uppercase()
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.space();
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!(
                "expected {:?} at character {}",
                c as char,
                self.i + 1
            ))
        }
    }

    /// `(` a , b , … `)` with `item` parsing each element.
    fn list<T>(
        &mut self,
        mut item: impl FnMut(&mut Self) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        self.expect(b'(')?;
        let mut out = vec![item(self)?];
        loop {
            self.space();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                    out.push(item(self)?);
                }
                Some(b')') => {
                    self.i += 1;
                    return Ok(out);
                }
                _ => {
                    return Err(format!(
                        "expected \",\" or \")\" at character {}",
                        self.i + 1
                    ));
                }
            }
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        self.space();
        let start = self.i;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E'))
        {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()
            .and_then(|t| t.parse::<f64>().ok())
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("expected a number at character {}", start + 1))
    }

    /// x y, then any z and m.
    fn point(&mut self) -> Result<[f64; 2], String> {
        let x = self.number()?;
        let y = self.number()?;
        loop {
            self.space();
            match self.peek() {
                Some(c) if c.is_ascii_digit() || c == b'-' || c == b'.' => {
                    self.number()?;
                }
                _ => return Ok([x, y]),
            }
        }
    }

    fn points(&mut self) -> Result<Vec<[f64; 2]>, String> {
        self.list(Self::point)
    }

    fn rings(&mut self) -> Result<Vec<Vec<[f64; 2]>>, String> {
        self.list(Self::points)
    }

    fn geometry(&mut self) -> Result<Value, String> {
        let kind = self.word();
        // Dimension markers: POINT Z (…), POINT ZM (…).
        let mark = self.i;
        if !matches!(self.word().as_str(), "Z" | "M" | "ZM") {
            self.i = mark;
        }
        let mark = self.i;
        if self.word() == "EMPTY" {
            return Ok(Value::Null);
        }
        self.i = mark;
        Ok(match kind.as_str() {
            "POINT" => {
                let p = self.list(Self::point)?;
                json!({"type": "Point", "coordinates": p[0]})
            }
            "LINESTRING" => json!({"type": "LineString", "coordinates": self.points()?}),
            "POLYGON" => json!({"type": "Polygon", "coordinates": self.rings()?}),
            "MULTIPOINT" => {
                // Both MULTIPOINT (1 2, 3 4) and MULTIPOINT ((1 2), (3 4)).
                let pts = self.list(|p| {
                    p.space();
                    if p.peek() == Some(b'(') {
                        Ok(p.list(Self::point)?[0])
                    } else {
                        p.point()
                    }
                })?;
                json!({"type": "MultiPoint", "coordinates": pts})
            }
            "MULTILINESTRING" => json!({"type": "MultiLineString", "coordinates": self.rings()?}),
            "MULTIPOLYGON" => {
                json!({"type": "MultiPolygon", "coordinates": self.list(Self::rings)?})
            }
            "GEOMETRYCOLLECTION" => {
                json!({"type": "GeometryCollection", "geometries": self.list(Self::geometry)?})
            }
            "" => return Err(format!("expected a geometry at character {}", self.i + 1)),
            other => return Err(format!("{other} isn't a WKT geometry")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpx_tracks_routes_and_waypoints() {
        let g = gpx(r#"<?xml version="1.0"?><gpx version="1.1" xmlns="http://www.topografix.com/GPX/1/1">
            <wpt lat="42.36" lon="-71.06"><name>Boston</name></wpt>
            <trk><name>Ride</name><trkseg><trkpt lat="42" lon="-71"/><trkpt lat="42.1" lon="-71.1"/></trkseg></trk>
            <rte><rtept lat="1" lon="2"/><rtept lat="3" lon="4"/></rte></gpx>"#)
        .unwrap();
        let f = g["features"].as_array().unwrap();
        assert_eq!(f.len(), 3);
        assert_eq!(f[0]["properties"]["name"], "Boston");
        assert_eq!(f[0]["geometry"]["coordinates"], json!([-71.06, 42.36]));
        assert_eq!(f[1]["geometry"]["type"], "MultiLineString");
        assert!(gpx("<kml/>").unwrap_err().contains("<gpx>"));
    }

    #[test]
    fn kml_placemarks() {
        let k = kml(r#"<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
            <Placemark><name>Park</name><Polygon><outerBoundaryIs><LinearRing>
              <coordinates>-71,42,0 -70,42,0 -70,43,0 -71,42,0</coordinates>
            </LinearRing></outerBoundaryIs></Polygon></Placemark>
            <Placemark><Point><coordinates>-71.1,42.3</coordinates></Point></Placemark>
            </Document></kml>"#)
        .unwrap();
        let f = k["features"].as_array().unwrap();
        assert_eq!(f[0]["geometry"]["type"], "Polygon");
        assert_eq!(f[0]["properties"]["name"], "Park");
        assert_eq!(f[1]["geometry"]["coordinates"], json!([-71.1, 42.3]));
    }

    #[test]
    fn wkt_geometries() {
        let w = wkt("POINT (30 10)\nlinestring z (30 10 1, 10 30 2)\n\
                     MULTIPOINT (10 40, 40 30)\nMULTIPOLYGON (((30 20, 45 40, 10 40, 30 20)), ((15 5, 40 10, 10 20, 15 5)))\n")
            .unwrap();
        let f = w["features"].as_array().unwrap();
        assert_eq!(f.len(), 4);
        assert_eq!(
            f[1]["geometry"]["coordinates"],
            json!([[30.0, 10.0], [10.0, 30.0]])
        );
        assert_eq!(f[3]["geometry"]["type"], "MultiPolygon");
        assert!(wkt("CIRCLE (1 2)").unwrap_err().contains("CIRCLE isn't"));
        assert!(wkt("POINT (1)").unwrap_err().contains("expected a number"));
    }
}

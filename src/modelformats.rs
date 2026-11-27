//! More 3D model formats for stl.rs to draw: glTF (```` ```gltf ```` blocks
//! and `.gltf` files, with embedded or neighbouring buffers), binary glTF
//! (`.glb`) and 3MF (`.3mf`, a zip of XML). Only the shape is used:
//! materials, textures and animations are ignored. Coordinates come out
//! Z-up, as STL's are.

use std::io::Read;
use std::path::Path;

use serde_json::Value;

use crate::stl::{MAX_TRIANGLES, V3};

type Tris = Vec<[V3; 3]>;
type Matrix = [f64; 16];

const IDENTITY: Matrix = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// Column-major 4×4 matrices, as glTF writes them.
fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    let mut m = [0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            m[col * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum();
        }
    }
    m
}

fn apply(m: &Matrix, v: V3) -> V3 {
    [
        m[0] * v[0] + m[4] * v[1] + m[8] * v[2] + m[12],
        m[1] * v[0] + m[5] * v[1] + m[9] * v[2] + m[13],
        m[2] * v[0] + m[6] * v[1] + m[10] * v[2] + m[14],
    ]
}

/// A ```gltf block (buffers must be `data:` URIs).
pub fn gltf_text(code: &str) -> Result<Tris, String> {
    let json: Value = serde_json::from_str(code).map_err(|e| format!("not valid JSON: {e}"))?;
    gltf(&json, None, None)
}

/// A `.gltf` or `.glb` file; other buffers are read from `dir`.
pub fn gltf_file(bytes: &[u8], dir: Option<&Path>) -> Result<Tris, String> {
    if bytes.starts_with(b"glTF") {
        let (json, bin) = glb_chunks(bytes)?;
        return gltf(&json, Some(bin), dir);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "not a glTF file".to_string())?;
    let json: Value = serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
    gltf(&json, None, dir)
}

/// GLB: a 12-byte header, then a JSON chunk and an optional binary chunk.
fn glb_chunks(bytes: &[u8]) -> Result<(Value, &[u8]), String> {
    let u32_at = |at: usize| -> Result<usize, String> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .ok_or_else(|| "the GLB file ends early".to_string())
    };
    let mut at = 12;
    let mut json = None;
    let mut bin: &[u8] = &[];
    while at + 8 <= bytes.len() {
        let len = u32_at(at)?;
        let kind = u32_at(at + 4)?;
        let data = bytes
            .get(at + 8..at + 8 + len)
            .ok_or("a GLB chunk runs past the end")?;
        match kind {
            0x4E4F534A => {
                json = Some(
                    serde_json::from_slice::<Value>(data)
                        .map_err(|e| format!("the GLB's JSON isn't valid: {e}"))?,
                )
            }
            0x004E4942 => bin = data,
            _ => {}
        }
        at += 8 + len;
    }
    Ok((json.ok_or("the GLB has no JSON chunk")?, bin))
}

fn gltf(json: &Value, glb_bin: Option<&[u8]>, dir: Option<&Path>) -> Result<Tris, String> {
    let list = |key: &str| {
        json.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    // Load every buffer up front.
    let mut buffers: Vec<Vec<u8>> = Vec::new();
    for (i, b) in list("buffers").iter().enumerate() {
        let data = match b.get("uri").and_then(Value::as_str) {
            None => glb_bin
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("buffer {i} has no data"))?,
            Some(uri) if uri.starts_with("data:") => {
                let b64 = uri
                    .split_once(";base64,")
                    .ok_or_else(|| format!("buffer {i}: only base64 data URIs are read"))?
                    .1;
                base64(b64).ok_or_else(|| format!("buffer {i}: bad base64"))?
            }
            Some(uri) => {
                let dir = dir.ok_or_else(|| {
                    format!(
                        "buffer {i} is a separate file ({uri}); show the .gltf file as an image"
                    )
                })?;
                if uri.contains("://") || uri.starts_with('/') {
                    return Err(format!("buffer {i} isn't a local file"));
                }
                let path = percent_encoding::percent_decode_str(uri).decode_utf8_lossy();
                std::fs::read(dir.join(path.as_ref()))
                    .map_err(|e| format!("can't read {uri}: {e}"))?
            }
        };
        buffers.push(data);
    }
    let views = list("bufferViews");
    let accessors = list("accessors");
    let meshes = list("meshes");
    let nodes = list("nodes");

    // The bytes of an accessor's elements: (data, offset, stride).
    let read = |index: usize, want_vec3: bool| -> Result<Vec<f64>, String> {
        let a = accessors
            .get(index)
            .ok_or_else(|| format!("no accessor {index}"))?;
        let count = a.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
        let comp = a.get("componentType").and_then(Value::as_u64).unwrap_or(0);
        let kind = a.get("type").and_then(Value::as_str).unwrap_or("");
        let n = if want_vec3 { 3 } else { 1 };
        if (want_vec3 && kind != "VEC3") || (!want_vec3 && kind != "SCALAR") {
            return Err(format!("accessor {index} is {kind}"));
        }
        let size = match comp {
            5126 | 5125 => 4,
            5123 | 5122 => 2,
            5121 | 5120 => 1,
            _ => return Err(format!("accessor {index}: unknown component type {comp}")),
        };
        let Some(view_i) = a.get("bufferView").and_then(Value::as_u64) else {
            return Ok(vec![0.0; count * n]);
        };
        let view = views
            .get(view_i as usize)
            .ok_or_else(|| format!("no buffer view {view_i}"))?;
        let buffer = view.get("buffer").and_then(Value::as_u64).unwrap_or(0) as usize;
        let data = buffers
            .get(buffer)
            .ok_or_else(|| format!("no buffer {buffer}"))?;
        let start = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
            + a.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let stride = view
            .get("byteStride")
            .and_then(Value::as_u64)
            .map_or(size * n, |s| s as usize);
        let mut out = Vec::with_capacity(count * n);
        for i in 0..count {
            for k in 0..n {
                let at = start + i * stride + k * size;
                let b = data
                    .get(at..at + size)
                    .ok_or_else(|| format!("accessor {index} runs past its buffer"))?;
                out.push(match comp {
                    5126 => f32::from_le_bytes(b.try_into().unwrap()) as f64,
                    5125 => u32::from_le_bytes(b.try_into().unwrap()) as f64,
                    5123 => u16::from_le_bytes(b.try_into().unwrap()) as f64,
                    5122 => i16::from_le_bytes(b.try_into().unwrap()) as f64,
                    5121 => b[0] as f64,
                    _ => b[0] as i8 as f64,
                });
            }
        }
        Ok(out)
    };

    let mut out = Tris::new();
    let draw_mesh = |mesh_i: usize, m: &Matrix, out: &mut Tris| -> Result<(), String> {
        let mesh = meshes
            .get(mesh_i)
            .ok_or_else(|| format!("no mesh {mesh_i}"))?;
        for p in mesh
            .get("primitives")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if p.pointer("/extensions/KHR_draco_mesh_compression")
                .is_some()
            {
                return Err("Draco-compressed meshes aren't supported".into());
            }
            let mode = p.get("mode").and_then(Value::as_u64).unwrap_or(4);
            let Some(pos) = p.pointer("/attributes/POSITION").and_then(Value::as_u64) else {
                continue;
            };
            let flat = read(pos as usize, true)?;
            let verts: Vec<V3> = flat
                .chunks(3)
                .map(|c| {
                    // glTF is Y-up; models here are Z-up.
                    let v = apply(m, [c[0], c[1], c[2]]);
                    [v[0], -v[2], v[1]]
                })
                .collect();
            let indices: Vec<usize> = match p.get("indices").and_then(Value::as_u64) {
                Some(i) => read(i as usize, false)?
                    .into_iter()
                    .map(|n| n as usize)
                    .collect(),
                None => (0..verts.len()).collect(),
            };
            let get = |i: usize| {
                verts
                    .get(i)
                    .copied()
                    .ok_or_else(|| format!("an index ({i}) past the vertices"))
            };
            match mode {
                4 => {
                    for t in indices.chunks_exact(3) {
                        out.push([get(t[0])?, get(t[1])?, get(t[2])?]);
                    }
                }
                5 => {
                    for i in 2..indices.len() {
                        let (a, b) = if i % 2 == 0 {
                            (i - 2, i - 1)
                        } else {
                            (i - 1, i - 2)
                        };
                        out.push([get(indices[a])?, get(indices[b])?, get(indices[i])?]);
                    }
                }
                6 => {
                    for i in 2..indices.len() {
                        out.push([get(indices[0])?, get(indices[i - 1])?, get(indices[i])?]);
                    }
                }
                _ => {} // points and lines
            }
            if out.len() >= MAX_TRIANGLES {
                out.truncate(MAX_TRIANGLES);
                return Ok(());
            }
        }
        Ok(())
    };

    let scene = json.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
    let roots: Option<Vec<usize>> = json
        .pointer(&format!("/scenes/{scene}/nodes"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|n| n.as_u64().map(|n| n as usize))
                .collect()
        });
    match roots {
        Some(roots) => {
            let mut stack: Vec<(usize, Matrix, usize)> =
                roots.into_iter().map(|n| (n, IDENTITY, 0)).collect();
            while let Some((n, parent, depth)) = stack.pop() {
                let node = nodes.get(n).ok_or_else(|| format!("no node {n}"))?;
                if depth > 64 {
                    return Err("nodes nest too deeply".into());
                }
                let m = mul(&parent, &local_matrix(node));
                if let Some(mesh) = node.get("mesh").and_then(Value::as_u64) {
                    draw_mesh(mesh as usize, &m, &mut out)?;
                }
                for c in node
                    .get("children")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(c) = c.as_u64() {
                        stack.push((c as usize, m, depth + 1));
                    }
                }
                if out.len() >= MAX_TRIANGLES {
                    break;
                }
            }
        }
        None => {
            for i in 0..meshes.len() {
                draw_mesh(i, &IDENTITY, &mut out)?;
            }
        }
    }
    Ok(out)
}

/// A node's own transform: `matrix`, or translation × rotation × scale.
fn local_matrix(node: &Value) -> Matrix {
    let nums = |key: &str| -> Option<Vec<f64>> {
        node.get(key)?
            .as_array()?
            .iter()
            .map(Value::as_f64)
            .collect()
    };
    if let Some(m) = nums("matrix").filter(|m| m.len() == 16) {
        return m.try_into().unwrap();
    }
    let t = nums("translation")
        .filter(|v| v.len() == 3)
        .unwrap_or(vec![0.0; 3]);
    let q = nums("rotation")
        .filter(|v| v.len() == 4)
        .unwrap_or(vec![0.0, 0.0, 0.0, 1.0]);
    let s = nums("scale")
        .filter(|v| v.len() == 3)
        .unwrap_or(vec![1.0; 3]);
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let r = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
    ];
    [
        r[0] * s[0],
        r[1] * s[0],
        r[2] * s[0],
        0.0,
        r[3] * s[1],
        r[4] * s[1],
        r[5] * s[1],
        0.0,
        r[6] * s[2],
        r[7] * s[2],
        r[8] * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ]
}

/// Standard base64 (with or without padding).
pub fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// Biggest file taken out of a zip (against zip bombs).
const MAX_UNZIPPED: u64 = 256 * 1024 * 1024;

/// A file inside a zip archive, by name (case-insensitively).
fn unzip(zip: &[u8], name: &str) -> Result<Option<Vec<u8>>, String> {
    let u16_at = |at: usize| {
        zip.get(at..at + 2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()) as usize)
    };
    let u32_at = |at: usize| {
        zip.get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
    };
    let bad = || "not a valid zip file".to_string();
    // The end of central directory record is in the last 64 KiB.
    let low = zip.len().saturating_sub(65_557);
    let eocd = (low..zip.len().saturating_sub(21))
        .rev()
        .find(|&i| zip[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .ok_or_else(bad)?;
    let entries = u16_at(eocd + 10).ok_or_else(bad)?;
    let mut at = u32_at(eocd + 16).ok_or_else(bad)?;
    for _ in 0..entries {
        if u32_at(at) != Some(0x02014b50) {
            return Err(bad());
        }
        let method = u16_at(at + 10).ok_or_else(bad)?;
        let size = u32_at(at + 20).ok_or_else(bad)?;
        let name_len = u16_at(at + 28).ok_or_else(bad)?;
        let extra = u16_at(at + 30).ok_or_else(bad)?;
        let comment = u16_at(at + 32).ok_or_else(bad)?;
        let local = u32_at(at + 42).ok_or_else(bad)?;
        let entry = zip.get(at + 46..at + 46 + name_len).ok_or_else(bad)?;
        if String::from_utf8_lossy(entry).eq_ignore_ascii_case(name) {
            let ln = u16_at(local + 26).ok_or_else(bad)?;
            let le = u16_at(local + 28).ok_or_else(bad)?;
            let start = local + 30 + ln + le;
            let data = zip.get(start..start + size).ok_or_else(bad)?;
            let mut out = Vec::new();
            match method {
                0 => out.extend_from_slice(data),
                8 => {
                    flate2::read::DeflateDecoder::new(data)
                        .take(MAX_UNZIPPED)
                        .read_to_end(&mut out)
                        .map_err(|e| format!("can't unzip {name}: {e}"))?;
                }
                m => {
                    return Err(format!(
                        "{name} is compressed in a way that isn't supported ({m})"
                    ));
                }
            }
            return Ok(Some(out));
        }
        at += 46 + name_len + extra + comment;
    }
    Ok(None)
}

/// 3MF: the model in `3D/3dmodel.model`, its objects placed by the build
/// items (and objects made of other objects' components).
pub fn three_mf(bytes: &[u8]) -> Result<Tris, String> {
    let xml = unzip(bytes, "3D/3dmodel.model")?.ok_or("no 3D/3dmodel.model in the file")?;
    let xml = String::from_utf8(xml).map_err(|_| "the model isn't UTF-8".to_string())?;
    let doc = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            ..Default::default()
        },
    )
    .map_err(|e| format!("the model isn't valid XML: {e}"))?;
    let objects: Vec<roxmltree::Node> = doc
        .descendants()
        .filter(|n| n.has_tag_name("object") || n.tag_name().name() == "object")
        .collect();
    let object = |id: &str| objects.iter().find(|o| o.attribute("id") == Some(id));
    let mut out = Tris::new();
    // (object id, transform, depth)
    let mut todo: Vec<(String, Matrix, usize)> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "item")
        .filter_map(|i| {
            Some((
                i.attribute("objectid")?.to_string(),
                transform(i.attribute("transform")),
                0,
            ))
        })
        .collect();
    if todo.is_empty() {
        todo = objects
            .iter()
            .filter_map(|o| Some((o.attribute("id")?.to_string(), IDENTITY, 0)))
            .collect();
    }
    while let Some((id, m, depth)) = todo.pop() {
        if depth > 32 {
            return Err("components nest too deeply".into());
        }
        let Some(o) = object(&id) else {
            return Err(format!("no object {id}"));
        };
        let child = |name: &str| o.descendants().find(|n| n.tag_name().name() == name);
        if let Some(mesh) = child("mesh") {
            let verts: Vec<V3> = mesh
                .descendants()
                .filter(|n| n.tag_name().name() == "vertex")
                .map(|v| {
                    let c = |k| {
                        v.attribute(k)
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0)
                    };
                    apply(&m, [c("x"), c("y"), c("z")])
                })
                .collect();
            for t in mesh
                .descendants()
                .filter(|n| n.tag_name().name() == "triangle")
            {
                let i = |k| t.attribute(k).and_then(|s| s.parse::<usize>().ok());
                if let (Some(a), Some(b), Some(c)) = (i("v1"), i("v2"), i("v3"))
                    && let (Some(a), Some(b), Some(c)) = (verts.get(a), verts.get(b), verts.get(c))
                {
                    out.push([*a, *b, *c]);
                    if out.len() >= MAX_TRIANGLES {
                        return Ok(out);
                    }
                }
            }
        }
        for comp in o
            .descendants()
            .filter(|n| n.tag_name().name() == "component")
        {
            if let Some(cid) = comp.attribute("objectid") {
                todo.push((
                    cid.to_string(),
                    mul(&m, &transform(comp.attribute("transform"))),
                    depth + 1,
                ));
            }
        }
    }
    Ok(out)
}

/// A 3MF transform: 12 numbers, a 3×4 matrix applied to row vectors.
fn transform(attr: Option<&str>) -> Matrix {
    let nums: Vec<f64> = attr
        .unwrap_or("")
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    if nums.len() != 12 {
        return IDENTITY;
    }
    // Row-vector m00 m01 m02 / m10 … / translation → column-major.
    [
        nums[0], nums[1], nums[2], 0.0, nums[3], nums[4], nums[5], 0.0, nums[6], nums[7], nums[8],
        0.0, nums[9], nums[10], nums[11], 1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A triangle: positions (three VEC3 floats) and u16 indices, in one
    /// base64 buffer, moved by its node.
    fn triangle_gltf() -> String {
        let mut buf = Vec::new();
        for n in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            buf.extend(n.to_le_bytes());
        }
        for i in [0u16, 1, 2, 0] {
            buf.extend(i.to_le_bytes());
        }
        let b64 = encode(&buf);
        format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],
            "nodes":[{{"mesh":0,"translation":[5,0,0]}}],
            "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1}}]}}],
            "buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{b64}"}}],
            "bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":6}}],
            "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}},
                         {{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}]}}"#,
            buf.len()
        )
    }

    fn encode(b: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in b.chunks(3) {
            let n = (c[0] as u32) << 16
                | (*c.get(1).unwrap_or(&0) as u32) << 8
                | *c.get(2).unwrap_or(&0) as u32;
            for k in 0..4 {
                if k <= c.len() {
                    s.push(A[(n >> (18 - 6 * k) & 63) as usize] as char);
                } else {
                    s.push('=');
                }
            }
        }
        s
    }

    #[test]
    fn gltf_with_an_embedded_buffer() {
        let tris = gltf_text(&triangle_gltf()).unwrap();
        assert_eq!(tris.len(), 1);
        // Moved 5 along x; y-up (0,1,0) becomes z-up (0,0,1).
        assert_eq!(tris[0][1], [6.0, 0.0, 0.0]);
        assert_eq!(tris[0][2], [5.0, 0.0, 1.0]);
        assert!(
            gltf_text("{\"buffers\":[{\"uri\":\"a.bin\"}]}")
                .unwrap_err()
                .contains("separate file")
        );
    }

    #[test]
    fn glb_files() {
        let json: Value = serde_json::from_str(&triangle_gltf()).unwrap();
        let data = base64(
            json["buffers"][0]["uri"]
                .as_str()
                .unwrap()
                .split_once(",")
                .unwrap()
                .1,
        )
        .unwrap();
        let mut json = json;
        json["buffers"][0].as_object_mut().unwrap().remove("uri");
        let mut j = serde_json::to_vec(&json).unwrap();
        while !j.len().is_multiple_of(4) {
            j.push(b' ');
        }
        let mut bin = data;
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let mut glb = b"glTF".to_vec();
        glb.extend(2u32.to_le_bytes());
        glb.extend(((12 + 8 + j.len() + 8 + bin.len()) as u32).to_le_bytes());
        glb.extend((j.len() as u32).to_le_bytes());
        glb.extend(0x4E4F534Au32.to_le_bytes());
        glb.extend(&j);
        glb.extend((bin.len() as u32).to_le_bytes());
        glb.extend(0x004E4942u32.to_le_bytes());
        glb.extend(&bin);
        assert_eq!(gltf_file(&glb, None).unwrap().len(), 1);
    }

    /// A stored (uncompressed) zip holding one file.
    pub fn zip_one(name: &str, data: &[u8]) -> Vec<u8> {
        let mut z = Vec::new();
        z.extend(0x04034b50u32.to_le_bytes());
        z.extend([20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        z.extend([0; 4]); // crc (not checked)
        z.extend((data.len() as u32).to_le_bytes());
        z.extend((data.len() as u32).to_le_bytes());
        z.extend((name.len() as u16).to_le_bytes());
        z.extend(0u16.to_le_bytes());
        z.extend(name.as_bytes());
        z.extend(data);
        let cd = z.len();
        z.extend(0x02014b50u32.to_le_bytes());
        z.extend([20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        z.extend([0; 4]);
        z.extend((data.len() as u32).to_le_bytes());
        z.extend((data.len() as u32).to_le_bytes());
        z.extend((name.len() as u16).to_le_bytes());
        z.extend([0; 12]);
        z.extend(0u32.to_le_bytes());
        z.extend(name.as_bytes());
        let cd_len = z.len() - cd;
        z.extend(0x06054b50u32.to_le_bytes());
        z.extend([0, 0, 0, 0, 1, 0, 1, 0]);
        z.extend((cd_len as u32).to_le_bytes());
        z.extend((cd as u32).to_le_bytes());
        z.extend([0, 0]);
        z
    }

    #[test]
    fn three_mf_objects_and_components() {
        let model = r#"<?xml version="1.0"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
 <resources>
  <object id="1" type="model"><mesh>
   <vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices>
   <triangles><triangle v1="0" v2="1" v3="2"/></triangles>
  </mesh></object>
  <object id="2" type="model"><components><component objectid="1" transform="1 0 0 0 1 0 0 0 1 0 0 10"/></components></object>
 </resources>
 <build><item objectid="2" transform="1 0 0 0 1 0 0 0 1 3 0 0"/></build>
</model>"#;
        let tris = three_mf(&zip_one("3D/3dmodel.model", model.as_bytes())).unwrap();
        assert_eq!(
            tris,
            vec![[[3.0, 0.0, 10.0], [4.0, 0.0, 10.0], [3.0, 1.0, 10.0]]]
        );
        assert!(
            three_mf(&zip_one("other.txt", b"x"))
                .unwrap_err()
                .contains("no 3D/3dmodel.model")
        );
        assert!(three_mf(b"not a zip").unwrap_err().contains("zip"));
    }
}

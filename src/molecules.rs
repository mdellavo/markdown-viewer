//! Molecules from their files' coordinates, drawn as SVG at render time:
//! ```` ```mol ````/```` ```sdf ```` (MDL molfiles, V2000), ```` ```xyz ```` and
//! ```` ```pdb ```` blocks, and the same files shown as images (embeds.rs).
//!
//! A flat molfile (all z = 0, as chemical editors write 2D structures) is
//! drawn as a structure diagram: bonds as lines (double and triple bonds
//! doubled, wedges for stereo bonds), atoms other than carbon as labels.
//! 3D coordinates are drawn as balls and sticks seen at an angle; in the
//! viewer, dragging turns them (ui.js redraws from `data-mdv-atoms` and
//! `data-mdv-bonds` with the same maths). Bonds missing from the file (XYZ
//! has none, PDB lists only some) are found by distance.

use std::fmt::Write;

use crate::diagrams::invalid;
use crate::render::{escape_attr, escape_text};

/// Most atoms drawn, and the most that can be turned in the viewer.
const MAX_ATOMS: usize = 20_000;
const MAX_ROTATABLE: usize = 4_000;
const SIZE: f64 = 400.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Format {
    Mol,
    Xyz,
    Pdb,
}

impl Format {
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Mol => ("molfile", "mol"),
            Self::Xyz => ("XYZ file", "xyz"),
            Self::Pdb => ("PDB file", "pdb"),
        }
    }
}

pub fn from_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mol" | "sdf" | "xyz" | "pdb"
    )
}

#[derive(Debug, Clone)]
struct Atom {
    el: String,
    pos: [f64; 3],
    charge: i32,
}

#[derive(Debug, Clone, Copy)]
struct Bond {
    a: usize,
    b: usize,
    order: u8,
    /// Molfile stereo: 1 wedge, 6 hash.
    stereo: u8,
}

#[derive(Debug, Default)]
struct Molecule {
    name: String,
    atoms: Vec<Atom>,
    bonds: Vec<Bond>,
}

/// ```mol and ```sdf blocks.
pub fn mol(code: &str, sourcepos: &str) -> String {
    render(Format::Mol, code, sourcepos)
}

/// ```xyz blocks.
pub fn xyz(code: &str, sourcepos: &str) -> String {
    render(Format::Xyz, code, sourcepos)
}

/// ```pdb blocks.
pub fn pdb(code: &str, sourcepos: &str) -> String {
    render(Format::Pdb, code, sourcepos)
}

fn render(format: Format, code: &str, sourcepos: &str) -> String {
    match draw_text(format, code, sourcepos) {
        Ok(html) => html,
        Err(e) => {
            let (name, lang) = format.names();
            invalid(name, lang, code, sourcepos, &e)
        }
    }
}

/// A molecule file shown as an image (`![caffeine](caffeine.mol)`).
pub fn file(ext: &str, bytes: &[u8], sourcepos: &str) -> Result<String, String> {
    let format = match ext.to_ascii_lowercase().as_str() {
        "xyz" => Format::Xyz,
        "pdb" => Format::Pdb,
        _ => Format::Mol,
    };
    let text = std::str::from_utf8(bytes).map_err(|_| "not a text file".to_string())?;
    draw_text(format, text, sourcepos)
}

fn draw_text(format: Format, code: &str, sourcepos: &str) -> Result<String, String> {
    let mut m = match format {
        Format::Mol => parse_mol(code)?,
        Format::Xyz => parse_xyz(code)?,
        Format::Pdb => parse_pdb(code)?,
    };
    if m.atoms.is_empty() {
        return Err("no atoms".into());
    }
    let flat = m.atoms.iter().all(|a| a.pos[2].abs() < 1e-4);
    if format != Format::Mol || !flat {
        add_bonds_by_distance(&mut m);
    }
    Ok(if format == Format::Mol && flat {
        draw_2d(&m, sourcepos)
    } else {
        draw_3d(&m, sourcepos)
    })
}

fn element_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase(),
        None => String::new(),
    }
}

/// MDL molfile, V2000 (the first molecule of an SDF file).
fn parse_mol(code: &str) -> Result<Molecule, String> {
    let lines: Vec<&str> = code.lines().collect();
    let counts = lines
        .get(3)
        .ok_or("a molfile needs a header and a counts line")?;
    if counts.contains("V3000") {
        return Err("V3000 molfiles aren't supported, only V2000".into());
    }
    let field =
        |s: &str, a: usize, b: usize| s.get(a..b.min(s.len())).unwrap_or("").trim().to_string();
    let n_atoms: usize = field(counts, 0, 3).parse().map_err(|_| "bad atom count")?;
    let n_bonds: usize = field(counts, 3, 6).parse().map_err(|_| "bad bond count")?;
    let mut m = Molecule {
        name: lines[0].trim().to_string(),
        ..Default::default()
    };
    for i in 0..n_atoms.min(MAX_ATOMS) {
        let l = lines.get(4 + i).ok_or("fewer atoms than the count")?;
        let num = |a, b| -> Result<f64, String> {
            field(l, a, b)
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .ok_or_else(|| format!("atom {}: bad coordinates", i + 1))
        };
        let charge = match field(l, 36, 39).parse::<i32>().unwrap_or(0) {
            1 => 3,
            2 => 2,
            3 => 1,
            5 => -1,
            6 => -2,
            7 => -3,
            _ => 0,
        };
        m.atoms.push(Atom {
            pos: [num(0, 10)?, num(10, 20)?, num(20, 30)?],
            el: element_case(&field(l, 31, 34)),
            charge,
        });
    }
    for i in 0..n_bonds {
        let l = lines
            .get(4 + n_atoms + i)
            .ok_or("fewer bonds than the count")?;
        let n = |a, b| field(l, a, b).parse::<usize>().unwrap_or(0);
        let (a, b) = (n(0, 3), n(3, 6));
        if a == 0 || b == 0 || a > m.atoms.len() || b > m.atoms.len() {
            continue;
        }
        m.bonds.push(Bond {
            a: a - 1,
            b: b - 1,
            order: n(6, 9).clamp(1, 4) as u8,
            stereo: n(9, 12) as u8,
        });
    }
    // Charges given as "M  CHG" lines win.
    for l in lines.iter().skip(4 + n_atoms + n_bonds) {
        if l.starts_with("M  END") || l.starts_with("$$$$") {
            break;
        }
        if let Some(rest) = l.strip_prefix("M  CHG") {
            let nums: Vec<i32> = rest
                .split_whitespace()
                .filter_map(|w| w.parse().ok())
                .collect();
            for pair in nums.get(1..).unwrap_or(&[]).chunks(2) {
                if let [atom, charge] = pair
                    && let Some(a) = m.atoms.get_mut((*atom as usize).wrapping_sub(1))
                {
                    a.charge = *charge;
                }
            }
        }
    }
    Ok(m)
}

/// XYZ: an atom count, a comment line, then `element x y z` lines.
fn parse_xyz(code: &str) -> Result<Molecule, String> {
    let mut lines = code.lines();
    let count: usize = lines
        .next()
        .and_then(|l| l.trim().parse().ok())
        .ok_or("an XYZ file starts with the number of atoms")?;
    let mut m = Molecule {
        name: lines.next().unwrap_or("").trim().to_string(),
        ..Default::default()
    };
    for (i, l) in lines.take(count.min(MAX_ATOMS)).enumerate() {
        let w: Vec<&str> = l.split_whitespace().collect();
        let nums: Option<Vec<f64>> = w
            .get(1..4)
            .map(|n| n.iter().filter_map(|x| x.parse().ok()).collect());
        match (w.first(), nums.as_deref()) {
            (Some(el), Some([x, y, z])) => m.atoms.push(Atom {
                el: element_case(el.trim_end_matches(|c: char| c.is_ascii_digit())),
                pos: [*x, *y, *z],
                charge: 0,
            }),
            _ => {
                return Err(format!(
                    "line {}: expected an element and three numbers",
                    i + 3
                ));
            }
        }
    }
    Ok(m)
}

/// PDB: ATOM and HETATM records (first model only), and CONECT bonds.
fn parse_pdb(code: &str) -> Result<Molecule, String> {
    let mut m = Molecule::default();
    let mut serials = std::collections::HashMap::new();
    let mut conect = Vec::new();
    for l in code.lines() {
        let rec = l.get(..6).unwrap_or(l).trim_end();
        match rec {
            "HEADER" | "TITLE" if m.name.is_empty() => {
                m.name = l.get(10..).unwrap_or("").trim().to_string();
            }
            "ATOM" | "HETATM" if m.atoms.len() < MAX_ATOMS => {
                let col = |a: usize, b: usize| l.get(a..b.min(l.len())).unwrap_or("").trim();
                let coord = |a, b| col(a, b).parse::<f64>().ok().filter(|n| n.is_finite());
                let (Some(x), Some(y), Some(z)) = (coord(30, 38), coord(38, 46), coord(46, 54))
                else {
                    continue;
                };
                let mut el = col(76, 78).to_string();
                if el.is_empty() {
                    // From the atom name: " CA " is carbon alpha.
                    el = col(12, 16)
                        .trim_start_matches(|c: char| c.is_ascii_digit())
                        .chars()
                        .take(1)
                        .collect();
                }
                if let Ok(serial) = col(6, 11).parse::<usize>() {
                    serials.insert(serial, m.atoms.len());
                }
                m.atoms.push(Atom {
                    el: element_case(&el),
                    pos: [x, y, z],
                    charge: 0,
                });
            }
            "CONECT" => {
                let n: Vec<usize> = (0..5)
                    .filter_map(|i| {
                        l.get(6 + i * 5..11 + i * 5)
                            .and_then(|s| s.trim().parse().ok())
                    })
                    .collect();
                if let Some((first, rest)) = n.split_first() {
                    for r in rest {
                        conect.push((*first, *r));
                    }
                }
            }
            "ENDMDL" => break,
            _ => {}
        }
    }
    for (a, b) in conect {
        if let (Some(&a), Some(&b)) = (serials.get(&a), serials.get(&b))
            && a < b
        {
            m.bonds.push(Bond {
                a,
                b,
                order: 1,
                stereo: 0,
            });
        }
    }
    if m.atoms.is_empty() {
        return Err("no ATOM or HETATM records".into());
    }
    Ok(m)
}

/// Covalent radius (Å), for finding bonds.
fn covalent(el: &str) -> f64 {
    match el {
        "H" => 0.31,
        "C" => 0.76,
        "N" => 0.71,
        "O" => 0.66,
        "F" => 0.57,
        "P" => 1.07,
        "S" => 1.05,
        "Cl" => 1.02,
        "Br" => 1.20,
        "I" => 1.39,
        "B" => 0.84,
        "Si" => 1.11,
        "Se" => 1.20,
        "Na" | "K" | "Ca" | "Mg" => 1.6,
        "Fe" | "Zn" | "Cu" | "Mn" | "Co" | "Ni" => 1.3,
        _ => 1.2,
    }
}

/// Bonds between atoms closer than their radii (plus a margin), if the
/// file listed none for them. A grid keeps this fast for proteins.
fn add_bonds_by_distance(m: &mut Molecule) {
    let mut bonded = vec![false; m.atoms.len()];
    for b in &m.bonds {
        bonded[b.a] = true;
        bonded[b.b] = true;
    }
    let cell = 2.0;
    let key = |p: &[f64; 3]| {
        (
            (p[0] / cell).floor() as i64,
            (p[1] / cell).floor() as i64,
            (p[2] / cell).floor() as i64,
        )
    };
    let mut grid: std::collections::HashMap<(i64, i64, i64), Vec<usize>> = Default::default();
    for (i, a) in m.atoms.iter().enumerate() {
        grid.entry(key(&a.pos)).or_default().push(i);
    }
    let mut found = Vec::new();
    for (i, a) in m.atoms.iter().enumerate() {
        if bonded[i] {
            continue;
        }
        let (kx, ky, kz) = key(&a.pos);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &j in grid.get(&(kx + dx, ky + dy, kz + dz)).into_iter().flatten() {
                        if j <= i && !bonded[j] {
                            continue;
                        }
                        if j == i {
                            continue;
                        }
                        let b = &m.atoms[j];
                        let d2: f64 = (0..3).map(|k| (a.pos[k] - b.pos[k]).powi(2)).sum();
                        let max = covalent(&a.el) + covalent(&b.el) + 0.4;
                        if d2 > 0.16 && d2 < max * max {
                            found.push((i.min(j), i.max(j)));
                        }
                    }
                }
            }
        }
    }
    found.sort_unstable();
    found.dedup();
    for (a, b) in found {
        m.bonds.push(Bond {
            a,
            b,
            order: 1,
            stereo: 0,
        });
    }
}

/// Class for an element's colour (CPK-style colours in the CSS).
fn el_class(el: &str) -> &'static str {
    match el {
        "H" => "H",
        "C" => "C",
        "N" => "N",
        "O" => "O",
        "S" => "S",
        "P" => "P",
        "F" | "Cl" => "Cl",
        "Br" => "Br",
        "I" => "I",
        "Fe" | "Cu" | "Zn" | "Mn" | "Co" | "Ni" | "Na" | "K" | "Ca" | "Mg" => "M",
        _ => "X",
    }
}

fn caption(m: &Molecule) -> String {
    let n = m.atoms.len();
    let what = format!("{n} atom{}", if n == 1 { "" } else { "s" });
    if m.name.is_empty() {
        what
    } else {
        format!("{}, {what}", m.name)
    }
}

/// A structure diagram from 2D coordinates.
fn draw_2d(m: &Molecule, sourcepos: &str) -> String {
    // Hydrogens on carbon are left out, as structure diagrams do.
    let hidden: Vec<bool> = (0..m.atoms.len())
        .map(|i| {
            m.atoms[i].el == "H"
                && m.bonds.iter().any(|b| {
                    (b.a == i && m.atoms[b.b].el == "C") || (b.b == i && m.atoms[b.a].el == "C")
                })
        })
        .collect();
    let shown: Vec<usize> = (0..m.atoms.len()).filter(|&i| !hidden[i]).collect();
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for &i in &shown {
        for k in 0..2 {
            lo[k] = lo[k].min(m.atoms[i].pos[k]);
            hi[k] = hi[k].max(m.atoms[i].pos[k]);
        }
    }
    // Typical bond length, to scale to about 40 px.
    let lens: Vec<f64> = m
        .bonds
        .iter()
        .filter(|b| !hidden[b.a] && !hidden[b.b])
        .map(|b| dist2(&m.atoms[b.a].pos, &m.atoms[b.b].pos).sqrt())
        .collect();
    let bond_len = if lens.is_empty() {
        1.5
    } else {
        lens.iter().sum::<f64>() / lens.len() as f64
    }
    .max(1e-3);
    let scale = 40.0 / bond_len;
    let pad = 24.0;
    let w = (hi[0] - lo[0]) * scale + 2.0 * pad;
    let h = (hi[1] - lo[1]) * scale + 2.0 * pad;
    let xy = |p: &[f64; 3]| (pad + (p[0] - lo[0]) * scale, pad + (hi[1] - p[1]) * scale);
    // Centre of the molecule: double bonds' second line goes towards it.
    let labelled = |i: usize| {
        let a = &m.atoms[i];
        a.el != "C" || a.charge != 0 || !m.bonds.iter().any(|b| b.a == i || b.b == i)
    };
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg viewBox=\"0 0 {w:.1} {h:.1}\" width=\"{w:.0}\" role=\"img\" aria-label=\"{}\" \
         xmlns=\"http://www.w3.org/2000/svg\">",
        escape_attr(&caption(m))
    );
    for b in &m.bonds {
        if hidden[b.a] || hidden[b.b] {
            continue;
        }
        let (mut x1, mut y1) = xy(&m.atoms[b.a].pos);
        let (mut x2, mut y2) = xy(&m.atoms[b.b].pos);
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / len, dy / len);
        // Stop short of labels.
        if labelled(b.a) {
            x1 += ux * 8.0;
            y1 += uy * 8.0;
        }
        if labelled(b.b) {
            x2 -= ux * 8.0;
            y2 -= uy * 8.0;
        }
        let (nx, ny) = (-uy, ux);
        match (b.stereo, b.order) {
            (1, _) => {
                let _ = write!(
                    svg,
                    "<polygon class=\"mdv-mol-wedge\" points=\"{x1:.1},{y1:.1} {:.1},{:.1} {:.1},{:.1}\"/>",
                    x2 + nx * 3.5,
                    y2 + ny * 3.5,
                    x2 - nx * 3.5,
                    y2 - ny * 3.5
                );
            }
            (6, _) => {
                for k in 1..=6 {
                    let t = k as f64 / 6.0;
                    let (cx, cy) = (x1 + (x2 - x1) * t, y1 + (y2 - y1) * t);
                    let wdt = 3.5 * t;
                    let _ = write!(
                        svg,
                        "<line class=\"mdv-mol-bond\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>",
                        cx + nx * wdt,
                        cy + ny * wdt,
                        cx - nx * wdt,
                        cy - ny * wdt
                    );
                }
            }
            (_, order) => {
                let line = |svg: &mut String, off: f64, shorten: f64| {
                    let _ = write!(
                        svg,
                        "<line class=\"mdv-mol-bond\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>",
                        x1 + nx * off + ux * shorten,
                        y1 + ny * off + uy * shorten,
                        x2 + nx * off - ux * shorten,
                        y2 + ny * off - uy * shorten
                    );
                };
                match order {
                    2 => {
                        // Inside a ring, the second line goes inside it.
                        let side = ring_side(m, b, &hidden);
                        line(&mut svg, 0.0, 0.0);
                        line(&mut svg, side * 5.0, 5.0);
                    }
                    3 => {
                        line(&mut svg, 0.0, 0.0);
                        line(&mut svg, 4.5, 2.0);
                        line(&mut svg, -4.5, 2.0);
                    }
                    4 => {
                        line(&mut svg, 0.0, 0.0);
                        let _ = write!(
                            svg,
                            "<line class=\"mdv-mol-bond mdv-mol-aromatic\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>",
                            x1 + nx * 5.0 * ring_side(m, b, &hidden),
                            y1 + ny * 5.0 * ring_side(m, b, &hidden),
                            x2 + nx * 5.0 * ring_side(m, b, &hidden),
                            y2 + ny * 5.0 * ring_side(m, b, &hidden)
                        );
                    }
                    _ => line(&mut svg, 0.0, 0.0),
                }
            }
        }
    }
    for &i in &shown {
        if !labelled(i) {
            continue;
        }
        let a = &m.atoms[i];
        let (x, y) = xy(&a.pos);
        // Hydrogens the file leaves implicit, from the usual valence.
        let valence: i32 = match a.el.as_str() {
            "O" | "S" => 2,
            "N" | "P" => 3,
            _ => 0,
        };
        let bonded: f64 = m
            .bonds
            .iter()
            .filter(|b| b.a == i || b.b == i)
            .map(|b| match b.order {
                4 => 1.5,
                o => o as f64,
            })
            .sum();
        // A charge changes it: O⁻ takes one bond fewer, NH₄⁺ one more.
        let hs = (valence as f64 + a.charge as f64 - bonded).floor().max(0.0) as usize;
        let h_text = match hs {
            0 => String::new(),
            1 => "H".into(),
            n => format!("H<tspan class=\"mdv-mol-count\" dy=\"4\">{n}</tspan>"),
        };
        let x = if hs > 0 { x - 5.0 } else { x };
        let charge = match a.charge {
            0 => String::new(),
            1 => "+".into(),
            -1 => "−".into(),
            c if c > 0 => format!("{c}+"),
            c => format!("{}−", -c),
        };
        let _ = write!(
            svg,
            "<text class=\"mdv-mol-label mdv-el-{}\" x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"{}\">{}{h_text}{}</text>",
            el_class(&a.el),
            y + 5.0,
            // With hydrogens, keep the element itself over the atom.
            if hs > 0 { "start" } else { "middle" },
            escape_text(&a.el),
            if charge.is_empty() {
                String::new()
            } else {
                format!("<tspan class=\"mdv-mol-charge\" dy=\"-7\">{charge}</tspan>")
            }
        );
    }
    svg.push_str("</svg>");
    if !m.name.is_empty() {
        let _ = write!(svg, "<figcaption>{}</figcaption>", escape_text(&m.name));
    }
    format!("<figure class=\"mdv-diagram mdv-molecule\"{sourcepos}>{svg}</figure>\n")
}

fn dist2(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum()
}

/// Which side of a double bond its second line goes: towards the
/// neighbours' side (inside a ring), on screen (y flipped).
fn ring_side(m: &Molecule, b: &Bond, hidden: &[bool]) -> f64 {
    let p = |i: usize| m.atoms[i].pos;
    let (a, c) = (p(b.a), p(b.b));
    let (dx, dy) = (c[0] - a[0], c[1] - a[1]);
    let mut sum = 0.0;
    for o in &m.bonds {
        for (end, other) in [(o.a, o.b), (o.b, o.a)] {
            if (end == b.a || end == b.b) && other != b.a && other != b.b && !hidden[other] {
                let q = p(other);
                sum += dx * (q[1] - a[1]) - dy * (q[0] - a[0]);
            }
        }
    }
    // Screen y points down, so the side flips.
    if sum > 0.0 { -1.0 } else { 1.0 }
}

/// Ball radius (Å) for 3D drawings.
fn ball(el: &str) -> f64 {
    match el {
        "H" => 0.22,
        "C" | "N" | "O" | "F" => 0.36,
        _ => 0.45,
    }
}

/// Balls and sticks from 3D coordinates.
fn draw_3d(m: &Molecule, sourcepos: &str) -> String {
    // Centre on the origin, scale to fit the unit sphere (with the balls).
    let n = m.atoms.len() as f64;
    let c: [f64; 3] = std::array::from_fn(|k| m.atoms.iter().map(|a| a.pos[k]).sum::<f64>() / n);
    let r = m
        .atoms
        .iter()
        .map(|a| dist2(&a.pos, &c).sqrt() + ball(&a.el))
        .fold(0.0, f64::max)
        .max(1e-6);
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    let atoms: Vec<([f64; 3], f64, &'static str)> = m
        .atoms
        .iter()
        .map(|a| {
            (
                std::array::from_fn(|k| round((a.pos[k] - c[k]) / r)),
                round(ball(&a.el) / r),
                el_class(&a.el),
            )
        })
        .collect();
    let bonds: Vec<(usize, usize)> = m.bonds.iter().map(|b| (b.a, b.b)).collect();
    let data = if atoms.len() <= MAX_ROTATABLE {
        let a: Vec<String> = atoms
            .iter()
            .map(|(p, rad, el)| format!("{},{},{},{},{el}", p[0], p[1], p[2], rad))
            .collect();
        let b: Vec<String> = bonds.iter().map(|(a, b)| format!("{a},{b}")).collect();
        format!(
            " data-mdv-atoms=\"{}\" data-mdv-bonds=\"{}\"",
            a.join(";"),
            b.join(";")
        )
    } else {
        String::new()
    };
    let mut svg = format!(
        "<figure class=\"mdv-diagram mdv-molecule mdv-molecule-3d\"{sourcepos}{data}><svg viewBox=\"0 0 {SIZE} {SIZE}\" \
         role=\"img\" aria-label=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">",
        escape_attr(&caption(m))
    );
    svg.push_str(&scene(&atoms, &bonds, crate::stl::YAW, crate::stl::PITCH));
    svg.push_str("</svg>");
    let mut cap = escape_text(&caption(m));
    if !data.is_empty() {
        cap.push_str("<span class=\"mdv-mol-hint\"> · drag to turn; double-click to reset</span>");
    }
    let _ = writeln!(svg, "<figcaption>{cap}</figcaption></figure>");
    svg
}

/// The atoms and bonds seen from `yaw`/`pitch`, farthest first. ui.js
/// (`drawMolecule`) does the same.
fn scene(
    atoms: &[([f64; 3], f64, &str)],
    bonds: &[(usize, usize)],
    yaw: f64,
    pitch: f64,
) -> String {
    let half = SIZE / 2.0;
    let scale = half * 0.92;
    let p: Vec<[f64; 3]> = atoms
        .iter()
        .map(|(v, _, _)| crate::stl::view(*v, yaw, pitch))
        .collect();
    // (depth, kind 0 bond / 1 atom, index): bonds behind the atoms they join.
    let mut items: Vec<(f64, u8, usize)> = Vec::with_capacity(atoms.len() + bonds.len());
    for (i, b) in bonds.iter().enumerate() {
        items.push(((p[b.0][2] + p[b.1][2]) / 2.0 - 0.01, 0, i));
    }
    for (i, q) in p.iter().enumerate() {
        items.push((q[2], 1, i));
    }
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = String::new();
    for (_, kind, i) in items {
        if kind == 0 {
            let (a, b) = bonds[i];
            let _ = write!(
                out,
                "<line class=\"mdv-mol-stick\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>",
                half + p[a][0] * scale,
                half + p[a][1] * scale,
                half + p[b][0] * scale,
                half + p[b][1] * scale
            );
        } else {
            let _ = write!(
                out,
                "<circle class=\"mdv-el-{}\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\"/>",
                atoms[i].2,
                half + p[i][0] * scale,
                half + p[i][1] * scale,
                (atoms[i].1 * scale).max(1.0)
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ethanol, flat, with a charge line.
    const MOL: &str = "ethanol
  test

  3  2  0  0  0  0  0  0  0  0999 V2000
    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    1.2990    0.7500    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    2.5981    0.0000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0
  1  2  1  0
  2  3  2  0
M  CHG  1   3  -1
M  END
";

    #[test]
    fn flat_molfiles_are_structure_diagrams() {
        let html = mol(MOL, " data-sourcepos=\"1:1-12:3\"");
        assert!(
            html.starts_with(
                "<figure class=\"mdv-diagram mdv-molecule\" data-sourcepos=\"1:1-12:3\"><svg"
            ),
            "{html}"
        );
        // Two carbons unlabelled, oxygen labelled with its charge.
        assert_eq!(html.matches("mdv-mol-label").count(), 1);
        // O⁻ with one bond (a double bond) has no hydrogens.
        assert!(
            html.contains(">O<tspan class=\"mdv-mol-charge\" dy=\"-7\">−</tspan>"),
            "{html}"
        );
        let acid = mol(
            &MOL.replace("M  CHG  1   3  -1\n", "")
                .replace("  2  3  2  0", "  2  3  1  0"),
            "",
        );
        assert!(acid.contains("text-anchor=\"start\">OH</text>"), "{acid}");
        // One single bond, a double bond as two lines.
        assert_eq!(html.matches("<line class=\"mdv-mol-bond\"").count(), 3);
        assert!(html.contains("<figcaption>ethanol</figcaption>"));
        assert!(mol("x\n\n\n  0  0  0  0  0  0  0  0  0  0999 V3000\n", "").contains("V3000"));
    }

    #[test]
    fn xyz_and_pdb_are_balls_and_sticks() {
        let water = "3\nwater\nO 0 0 0.12\nH 0 0.76 -0.47\nH 0 -0.76 -0.47\n";
        let html = xyz(water, "");
        assert!(html.contains("mdv-molecule-3d"));
        assert_eq!(html.matches("<circle").count(), 3);
        // Both O–H bonds found by distance, not H–H.
        assert_eq!(
            html.matches("<line class=\"mdv-mol-stick\"").count(),
            2,
            "{html}"
        );
        assert!(html.contains("data-mdv-atoms=\"") && html.contains("data-mdv-bonds=\"0,1;0,2\""));
        assert!(html.contains("water, 3 atoms"));

        let pdb_text = "HEADER    TEST\n\
ATOM      1  N   GLY A   1       0.000   0.000   0.000  1.00  0.00           N\n\
ATOM      2  CA  GLY A   1       1.458   0.000   0.000  1.00  0.00           C\n\
HETATM    3  O   HOH A   2      10.000  10.000  10.000  1.00  0.00\n\
END\n";
        let html = pdb(pdb_text, "");
        assert_eq!(html.matches("<circle").count(), 3);
        assert_eq!(html.matches("mdv-mol-stick").count(), 1);
        assert!(
            html.contains("class=\"mdv-el-O\""),
            "element from the atom name"
        );
        assert!(xyz("two\n", "").contains("Can't draw this XYZ file"));
    }
}

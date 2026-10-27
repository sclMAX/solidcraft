//! Strict, pure profile DXF import; no topology repair or block expansion.
use crate::sketch2d::{MAX_DRAWING_BYTES, MAX_ENTITIES};
use crate::{Geom2, IoError, Result};
use solidcraft_geom::Vec2;

type Pair<'a> = (i32, &'a str);
fn invalid(message: impl Into<String>) -> IoError {
    IoError::Invalid(message.into())
}
fn number(code: i32, value: &str) -> Result<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && v.abs() < 1e12)
        .ok_or_else(|| invalid(format!("invalid profile DXF number for group {code}")))
}
fn one(p: &[Pair<'_>], code: i32) -> Result<Option<f64>> {
    let mut values = p.iter().filter(|(c, _)| *c == code);
    let value = values.next().map(|(_, v)| number(code, v)).transpose()?;
    if values.next().is_some() {
        return Err(invalid(format!("duplicate profile DXF group {code}")));
    }
    Ok(value)
}
fn required(p: &[Pair<'_>], kind: &str, code: i32) -> Result<f64> {
    one(p, code)?.ok_or_else(|| invalid(format!("profile DXF {kind} requires group {code}")))
}
fn flags(p: &[Pair<'_>], code: i32, allowed: i64) -> Result<()> {
    let n = one(p, code)?.unwrap_or(0.0);
    if n.fract() != 0.0 || (n as i64) & !allowed != 0 {
        return Err(invalid("unsupported profile DXF polyline flags"));
    }
    Ok(())
}
fn plane(p: &[Pair<'_>]) -> Result<()> {
    for code in [30, 31, 38, 39, 210, 220, 230, 67] {
        let default = if code == 230 { 1.0 } else { 0.0 };
        if one(p, code)?.is_some_and(|n| n != default) {
            return Err(invalid("profile DXF requires the XY plane and default extrusion"));
        }
    }
    Ok(())
}
fn widths(p: &[Pair<'_>]) -> Result<()> {
    if p.iter().any(|(c, v)| matches!(c, 40 | 41 | 43) && number(*c, v).is_ok_and(|n| n != 0.0)) {
        return Err(invalid("profile DXF does not support polyline widths"));
    }
    Ok(())
}
fn vertex(p: &[Pair<'_>], kind: &str, k: f64) -> Result<(Vec2, f64)> {
    let at = Vec2::new(required(p, kind, 10)? * k, required(p, kind, 20)? * k);
    if at.x.abs().max(at.y.abs()) >= 1e12 {
        return Err(invalid("profile DXF scaled coordinate exceeds limits"));
    }
    Ok((at, one(p, 42)?.unwrap_or(0.0)))
}
fn edges(verts: &[(Vec2, f64)], closed: bool, out: &mut Vec<Geom2>) -> Result<()> {
    if verts.len() < 2 {
        return Err(invalid("profile DXF polyline requires at least two vertices"));
    }
    let count = if closed { verts.len() } else { verts.len() - 1 };
    for i in 0..count {
        let (Some(&(a, b)), Some(&(z, _))) = (verts.get(i), verts.get((i + 1) % verts.len())) else { break };
        let chord = z - a;
        if chord.len() == 0.0 {
            return Err(invalid("profile DXF has a zero-length polyline edge"));
        }
        if b == 0.0 {
            out.push(Geom2::Line(a, z));
        } else {
            // Analytic bulge formula, with no small-bulge approximation or welding.
            let c = (a + z) * 0.5 + chord.perp() * ((1.0 / b - b) * 0.25);
            if !c.x.is_finite() || !c.y.is_finite() || c.x.abs().max(c.y.abs()) >= 1e150 {
                return Err(invalid("profile DXF bulge exceeds coordinate limits"));
            }
            out.push(if b > 0.0 { Geom2::Arc { c, a, b: z } } else { Geom2::Arc { c, a: z, b: a } });
        }
        if out.len() > MAX_ENTITIES {
            return Err(invalid("profile DXF is too complex"));
        }
    }
    Ok(())
}

/// Read supported planar profile geometry, normalized to millimetres.
/// Units are `mm`, `cm`, `m`, `in` or `ft`; an override wins over `$INSUNITS`.
/// This pure reader rejects unsupported source geometry and never repairs topology.
pub fn read_profile_dxf(text: &str, units: Option<&str>) -> Result<Vec<Geom2>> {
    if text.len() > MAX_DRAWING_BYTES {
        return Err(invalid("profile DXF file is too large"));
    }
    if text.starts_with("AutoCAD Binary DXF") {
        return Err(invalid("binary DXF is not supported; save as ASCII DXF"));
    }
    let mut p = Vec::new();
    let mut lines = text.lines();
    while let Some(c) = lines.next() {
        let v = lines.next().ok_or_else(|| invalid("incomplete profile DXF pair"))?.trim();
        let c = c.trim().parse::<i32>().ok().filter(|c| (0..=1071).contains(c)).ok_or_else(|| invalid("invalid profile DXF group code"))?;
        if matches!(c,10..=59|60..=99|110..=149|160..=179|210..=239|270..=299|370..=389|400..=409|420..=429|440..=459|1010..=1059|1070..=1071) {
            number(c, v)?;
        }
        p.push((c, v));
        if p.len() > 2_000_000 {
            return Err(invalid("profile DXF is too complex"));
        }
    }
    let mut section = None;
    let mut entities = None;
    let mut source_units = None;
    let mut eof = false;
    let mut i = 0;
    while let Some(&(c, v)) = p.get(i) {
        if eof {
            return Err(invalid("data after profile DXF EOF"));
        }
        match (c, v) {
            (0, "SECTION") => {
                if section.is_some() {
                    return Err(invalid("nested profile DXF section"));
                }
                let Some(&(2, name)) = p.get(i + 1) else {
                    return Err(invalid("missing profile DXF section name"));
                };
                section = Some((name, i + 2));
                i += 1;
            }
            (0, "ENDSEC") => {
                let Some((name, start)) = section.take() else {
                    return Err(invalid("unexpected profile DXF ENDSEC"));
                };
                if name == "ENTITIES" && entities.replace((start, i)).is_some() {
                    return Err(invalid("duplicate profile DXF ENTITIES section"));
                }
            }
            (0, "EOF") => {
                if section.is_some() {
                    return Err(invalid("unclosed profile DXF section"));
                }
                eof = true;
            }
            (9, "$INSUNITS") if section.is_some_and(|(s, _)| s == "HEADER") => {
                if source_units.is_some() {
                    return Err(invalid("duplicate profile DXF units"));
                }
                let Some(&(70, value)) = p.get(i + 1) else {
                    return Err(invalid("invalid profile DXF units header"));
                };
                source_units = Some(value);
            }
            (0, kind) if section.is_some_and(|(s, _)| s == "BLOCKS") => {
                if !matches!(kind, "BLOCK" | "ENDBLK") {
                    return Err(invalid("profile DXF does not support block contents"));
                }
            }
            (0, _) if section.is_none() => return Err(invalid("profile DXF entity outside section")),
            _ => {}
        }
        i += 1;
    }
    if !eof || section.is_some() {
        return Err(invalid("incomplete profile DXF structure"));
    }
    let k = match units {
        Some("mm") => 1.0,
        Some("cm") => 10.0,
        Some("m") => 1000.0,
        Some("in") => 25.4,
        Some("ft") => 304.8,
        Some(u) => return Err(invalid(format!("unsupported profile DXF units: {u}"))),
        None => match source_units {
            Some("1") => 25.4,
            Some("2") => 304.8,
            Some("4") => 1.0,
            Some("5") => 10.0,
            Some("6") => 1000.0,
            _ => return Err(invalid("profile DXF requires known units or an explicit override")),
        },
    };
    let (start, end) = entities.ok_or_else(|| invalid("no ENTITIES section"))?;
    let mut out = Vec::new();
    let mut cursor = start;
    let mut old_poly: Option<(Vec<(Vec2, f64)>, bool)> = None;
    let mut count = 0;
    while cursor < end {
        let Some(&(0, kind)) = p.get(cursor) else {
            return Err(invalid("profile DXF entity must start with group 0"));
        };
        let next = (cursor + 1..end).find(|&j| p.get(j).is_some_and(|(c, _)| *c == 0)).unwrap_or(end);
        let data = p.get(cursor + 1..next).unwrap_or_default();
        count += 1;
        if count > MAX_ENTITIES {
            return Err(invalid("profile DXF is too complex"));
        }
        plane(data)?;
        let allowed: &[i32] = match kind {
            "LINE" => &[10, 20, 11, 21],
            "ARC" => &[10, 20, 40, 50, 51],
            "CIRCLE" => &[10, 20, 40],
            "LWPOLYLINE" => &[10, 20, 40, 41, 42, 43],
            "POLYLINE" | "VERTEX" => &[10, 20, 40, 41, 42],
            _ => &[],
        };
        for &(code, _) in data {
            if matches!(code,10..=59|110..=149|210..=239) && !matches!(code, 30 | 31 | 38 | 39 | 210 | 220 | 230) && !allowed.contains(&code) {
                return Err(invalid(format!("unsupported profile DXF geometry group {code}")));
            }
        }
        if old_poly.is_some() && !matches!(kind, "VERTEX" | "SEQEND") {
            return Err(invalid("profile DXF POLYLINE requires SEQEND"));
        }
        match kind {
            "LINE" | "ARC" | "CIRCLE" => {
                let codes: &[i32] = match kind {
                    "LINE" => &[10, 20, 11, 21],
                    "ARC" => &[10, 20, 40, 50, 51],
                    _ => &[10, 20, 40],
                };
                let mut normalized = String::from("0\nSECTION\n2\nENTITIES\n");
                normalized.push_str(&format!("0\n{kind}\n"));
                for &code in codes {
                    let n = required(data, kind, code)?;
                    if code == 40 && n <= 0.0 {
                        return Err(invalid("profile DXF radius must be positive"));
                    }
                    let n = if matches!(code, 10 | 20 | 11 | 21 | 40) { n * k } else { n };
                    if n.abs() >= 1e12 {
                        return Err(invalid("profile DXF scaled coordinate exceeds limits"));
                    }
                    normalized.push_str(&format!("{code}\n{n}\n"));
                }
                normalized.push_str("0\nENDSEC\n0\nEOF\n");
                out.extend(crate::read_dxf(&normalized)?);
            }
            "POLYLINE" => {
                flags(data, 70, 1 | 128)?;
                widths(data)?;
                old_poly = Some((Vec::new(), one(data, 70)?.unwrap_or(0.0) as i64 & 1 != 0));
            }
            "VERTEX" => {
                flags(data, 70, 0)?;
                widths(data)?;
                let v = vertex(data, kind, k)?;
                old_poly.as_mut().ok_or_else(|| invalid("profile DXF orphan VERTEX"))?.0.push(v);
            }
            "SEQEND" => {
                let (verts, closed) = old_poly.take().ok_or_else(|| invalid("profile DXF orphan SEQEND"))?;
                edges(&verts, closed, &mut out)?;
            }
            "LWPOLYLINE" => {
                flags(data, 70, 1 | 128)?;
                widths(data)?;
                let mut verts = Vec::new();
                let mut head = 0;
                while let Some(rel) = data.get(head..).and_then(|s| s.iter().position(|(c, _)| *c == 10)) {
                    let at = head + rel;
                    let stop = data.get(at + 1..).and_then(|s| s.iter().position(|(c, _)| *c == 10)).map_or(data.len(), |n| at + 1 + n);
                    verts.push(vertex(data.get(at..stop).unwrap_or_default(), kind, k)?);
                    head = stop;
                }
                if required(data, kind, 90)? != verts.len() as f64 || data.iter().filter(|(c, _)| *c == 20).count() != verts.len() {
                    return Err(invalid("profile DXF polyline vertex count mismatch"));
                }
                if data.iter().take_while(|(c, _)| *c != 10).any(|(c, _)| matches!(c, 20 | 42)) {
                    return Err(invalid("invalid profile DXF vertex ordering"));
                }
                edges(&verts, one(data, 70)?.unwrap_or(0.0) as i64 & 1 != 0, &mut out)?;
            }
            _ => return Err(invalid(format!("profile DXF does not support entity {kind}"))),
        }
        cursor = next;
    }
    if old_poly.is_some() {
        return Err(invalid("profile DXF POLYLINE requires SEQEND"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use crate::{Geom2, read_profile_dxf};
    use solidcraft_geom::Vec2;

    fn drawing(body: &str) -> String {
        format!("0\nSECTION\n2\nENTITIES\n{body}0\nENDSEC\n0\nEOF\n")
    }
    const LINE: &str = "0\nLINE\n10\n0\n20\n0\n11\n2\n21\n0\n";

    #[test]
    fn profile_dxf_units_and_exact_geometry() {
        for (unit, scale) in [("mm", 1.0), ("cm", 10.0), ("m", 1000.0), ("in", 25.4), ("ft", 304.8)] {
            let g = read_profile_dxf(&drawing(LINE), Some(unit)).unwrap();
            assert!(matches!(g.as_slice(), [Geom2::Line(a,b)] if *a == Vec2::ZERO && *b == Vec2::new(2.0*scale,0.0)));
        }
    }

    #[test]
    fn profile_dxf_curves_and_polylines() {
        let curves = drawing("0\nCIRCLE\n10\n1\n20\n2\n40\n3\n0\nARC\n10\n0\n20\n0\n40\n2\n50\n0\n51\n90\n");
        let g = read_profile_dxf(&curves, Some("mm")).unwrap();
        assert!(
            matches!(g.as_slice(), [Geom2::Circle(c,r), Geom2::Arc {c:o,a,b}] if *c == Vec2::new(1.0,2.0) && *r == 3.0 && *o == Vec2::ZERO && *a == Vec2::new(2.0,0.0) && b.dist(Vec2::new(0.0,2.0)) < 1e-14)
        );
        for bulge in [1.0, -1.0, 2.0, 1e-13] {
            let lw = drawing(&format!("0\nLWPOLYLINE\n90\n2\n10\n0\n20\n0\n42\n{bulge}\n10\n2\n20\n0\n"));
            let old = drawing(&format!("0\nPOLYLINE\n70\n0\n0\nVERTEX\n10\n0\n20\n0\n42\n{bulge}\n0\nVERTEX\n10\n2\n20\n0\n0\nSEQEND\n"));
            for text in [lw, old] {
                let g = read_profile_dxf(&text, Some("mm")).unwrap();
                let expected = Vec2::new(1.0, (1.0 / bulge - bulge) * 0.5);
                assert!(
                    matches!(g.as_slice(), [Geom2::Arc {c,a,b}] if *c == expected && if bulge > 0.0 { *a == Vec2::ZERO && *b == Vec2::new(2.0,0.0) } else { *b == Vec2::ZERO && *a == Vec2::new(2.0,0.0) })
                );
            }
        }
        let closed = drawing("0\nLWPOLYLINE\n90\n3\n70\n1\n10\n0\n20\n0\n10\n2\n20\n0\n10\n0\n20\n2\n");
        assert_eq!(read_profile_dxf(&closed, Some("mm")).unwrap().len(), 3);
    }

    #[test]
    fn profile_dxf_source_units_and_blocks() {
        for (code, scale) in [(1, 25.4), (2, 304.8), (4, 1.0), (5, 10.0), (6, 1000.0)] {
            let text = format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{code}\n0\nENDSEC\n{}", drawing(LINE));
            assert!(matches!(read_profile_dxf(&text,None).unwrap().as_slice(), [Geom2::Line(_,b)] if *b == Vec2::new(2.0*scale,0.0)));
            assert!(matches!(read_profile_dxf(&text,Some("mm")).unwrap().as_slice(), [Geom2::Line(_,b)] if *b == Vec2::new(2.0,0.0)));
        }
        let empty = format!("0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\n*Model_Space\n0\nENDBLK\n0\nENDSEC\n{}", drawing(LINE));
        assert_eq!(read_profile_dxf(&empty, Some("mm")).unwrap().len(), 1);
        let filled = empty.replace("0\nENDBLK", &format!("{LINE}0\nENDBLK"));
        assert_eq!(read_profile_dxf(&filled, Some("mm")).unwrap_err().to_string(), "profile DXF does not support block contents");
    }

    #[test]
    fn profile_dxf_polyline_rejections_and_bounds() {
        let good = "0\nLWPOLYLINE\n90\n2\n10\n0\n20\n0\n10\n2\n20\n0\n";
        for (body, error) in [
            (good.replace("90\n2", "90\n3"), "profile DXF polyline vertex count mismatch"),
            (good.replace("90\n2", "90\n2\n43\n1"), "profile DXF does not support polyline widths"),
            (good.replace("90\n2", "90\n2\n70\n8"), "unsupported profile DXF polyline flags"),
            (format!("{good}210\n1\n"), "profile DXF requires the XY plane and default extrusion"),
            ("0\nPOLYLINE\n70\n16\n".into(), "unsupported profile DXF polyline flags"),
            ("0\nPOLYLINE\n".into(), "profile DXF POLYLINE requires SEQEND"),
            (good.replace("10\n2", "10\n0"), "profile DXF has a zero-length polyline edge"),
        ] {
            let text = drawing(&body);
            let before = text.clone();
            assert_eq!(read_profile_dxf(&text, Some("mm")).unwrap_err().to_string(), error);
            assert_eq!(text, before);
        }
        let big = " ".repeat(crate::MAX_DRAWING_BYTES + 1);
        assert_eq!(read_profile_dxf(&big, Some("mm")).unwrap_err().to_string(), "profile DXF file is too large");
        let many = drawing(&"0\nCIRCLE\n10\n0\n20\n0\n40\n1\n".repeat(crate::sketch2d::MAX_ENTITIES + 1));
        assert_eq!(read_profile_dxf(&many, Some("mm")).unwrap_err().to_string(), "profile DXF is too complex");
    }

    #[test]
    fn profile_dxf_rejects_without_mutation() {
        let cases = [
            (drawing(LINE), None, "profile DXF requires known units or an explicit override"),
            (drawing(LINE), Some("yd"), "unsupported profile DXF units: yd"),
            (drawing("0\nINSERT\n2\nsection\n"), Some("mm"), "profile DXF does not support entity INSERT"),
            (drawing("0\nELLIPSE\n"), Some("mm"), "profile DXF does not support entity ELLIPSE"),
            (drawing(&format!("{LINE}30\n1\n")), Some("mm"), "profile DXF requires the XY plane and default extrusion"),
            (drawing(&LINE.replace("2\n21", "nan\n21")), Some("mm"), "invalid profile DXF number for group 11"),
            (drawing(&LINE.replace("11\n2\n", "")), Some("mm"), "profile DXF LINE requires group 11"),
            ("bad\nvalue\n".into(), Some("mm"), "invalid profile DXF group code"),
            ("0\n".into(), Some("mm"), "incomplete profile DXF pair"),
        ];
        for (text, units, error) in cases {
            let before = text.clone();
            assert_eq!(read_profile_dxf(&text, units).unwrap_err().to_string(), error);
            assert_eq!(text, before);
        }
    }
}

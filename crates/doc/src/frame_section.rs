//! Strict, pure validation of reusable material sections. See `docs/profile-sections.md`.
use crate::{DocError, Result};
use solidcraft_geom::{Loop2, Region2, Seg2, Vec2};
use std::f64::consts::{PI, TAU};

const EPS: f64 = 1e-7;
const LIMIT: f64 = 1e6;
const MAX_SEGMENTS: usize = 512;
fn error(message: &str) -> DocError {
    DocError::Invalid(format!("frame section: {message}"))
}
fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(error(message)) }
}
fn point_ok(p: Vec2) -> bool {
    p.is_finite() && p.x.abs() <= LIMIT && p.y.abs() <= LIMIT
}

/// Construct one connected section without repairing or discarding any boundary.
pub fn section_from_loops(loops: Vec<Loop2>) -> Result<Region2> {
    let outer = inspect(&loops)?;
    let boundary = loops.get(outer).ok_or_else(|| error("empty section"))?.ccw();
    let holes = loops.iter().enumerate().filter(|(i, _)| *i != outer).map(|(_, l)| l.ccw().reversed()).collect();
    let region = Region2 { outer: boundary, holes };
    validate_section(&region)?;
    Ok(region)
}

/// Validate durable geometry, including topology and stored winding, after deserialization.
pub fn validate_section(region: &Region2) -> Result<()> {
    require(region.holes.len() < MAX_SEGMENTS, "segment budget")?;
    let count = std::iter::once(&region.outer)
        .chain(&region.holes)
        .try_fold(0usize, |n, l| n.checked_add(l.segs.len()))
        .ok_or_else(|| error("segment budget"))?;
    require(count <= MAX_SEGMENTS, "segment budget")?;
    let loops: Vec<_> = std::iter::once(region.outer.clone()).chain(region.holes.iter().cloned()).collect();
    require(inspect(&loops)? == 0, "outer boundary does not enclose holes")?;
    require(region.outer.signed_area() > 0.0 && region.holes.iter().all(|h| h.signed_area() < 0.0), "invalid orientation")
}

fn inspect(loops: &[Loop2]) -> Result<usize> {
    require(!loops.is_empty(), "empty section")?;
    require(loops.len() <= MAX_SEGMENTS, "segment budget")?;
    let count = loops.iter().try_fold(0usize, |n, l| n.checked_add(l.segs.len())).ok_or_else(|| error("segment budget"))?;
    require(count <= MAX_SEGMENTS, "segment budget")?;
    for l in loops {
        require(!l.segs.is_empty(), "empty loop")?;
        for s in &l.segs {
            let valid = match *s {
                Seg2::Line { a, b } => point_ok(a) && point_ok(b) && a.dist(b) > EPS,
                Seg2::Arc { center, radius, start, sweep } => {
                    point_ok(center)
                        && radius.is_finite()
                        && radius > EPS
                        && radius <= LIMIT
                        && start.is_finite()
                        && start.abs() <= TAU * 2.0
                        && sweep.is_finite()
                        && sweep.abs() > EPS
                        && sweep.abs() <= TAU
                        && center.x.abs() + radius <= LIMIT
                        && center.y.abs() + radius <= LIMIT
                }
                _ => return Err(error("unsupported segment kind")),
            };
            require(valid, "invalid segment")?;
        }
        require(l.is_closed(EPS), "open loop")?;
        for (i, a) in l.segs.iter().enumerate() {
            for (j, b) in l.segs.iter().enumerate().skip(i + 1) {
                let adjacent = j == i + 1 || (i == 0 && j + 1 == l.segs.len());
                let allowed = |p: Vec2| {
                    adjacent && ((p.dist(a.end()) <= EPS && p.dist(b.start()) <= EPS) || (p.dist(b.end()) <= EPS && p.dist(a.start()) <= EPS))
                };
                require(!contacts(*a, *b).iter().any(|p| !allowed(*p)), "self intersection")?;
            }
        }
        require(l.signed_area().is_finite() && l.signed_area().abs() > EPS * EPS, "invalid area")?;
    }
    for (i, a) in loops.iter().enumerate() {
        for b in loops.iter().skip(i + 1) {
            for sa in &a.segs {
                for sb in &b.segs {
                    require(contacts(*sa, *sb).is_empty(), "touching or crossing boundaries")?;
                }
            }
        }
    }
    let mut outer = None;
    for (i, l) in loops.iter().enumerate() {
        let p = l.segs.first().ok_or_else(|| error("empty loop"))?.start();
        let depth = loops.iter().enumerate().filter(|(j, other)| *j != i && inside(other, p)).count();
        require(depth <= 1, "nested holes or material islands")?;
        if depth == 0 {
            require(outer.is_none(), "disconnected exteriors")?;
            outer = Some(i);
        }
    }
    outer.ok_or_else(|| error("disconnected exteriors"))
}

// Analytic membership of a point on a supporting line/circle, with a conservative
// distance tolerance. No curve tessellation is used for topology.
fn on(s: Seg2, p: Vec2) -> bool {
    match s {
        Seg2::Line { a, b } => {
            let t = (p - a).dot(b - a) / (b - a).len2();
            (-EPS / a.dist(b)..=1.0 + EPS / a.dist(b)).contains(&t) && p.dist(a + (b - a) * t) <= EPS
        }
        Seg2::Arc { center, radius, start, sweep } => {
            let q = p - center;
            let d = ((q.y.atan2(q.x) - start) * sweep.signum()).rem_euclid(TAU);
            (q.len() - radius).abs() <= EPS && (d <= sweep.abs() + EPS / radius || TAU - d <= EPS / radius)
        }
        _ => false,
    }
}
fn contacts(a: Seg2, b: Seg2) -> Vec<Vec2> {
    let mut candidates = vec![a.start(), a.end(), a.mid(), b.start(), b.end(), b.mid()];
    match (a, b) {
        (Seg2::Line { a: p, b: q }, Seg2::Line { a: r, b: s }) => {
            let den = (q - p).cross(s - r);
            if den != 0.0 {
                candidates.push(p + (q - p) * ((r - p).cross(s - r) / den));
            }
        }
        (Seg2::Line { a: p, b: q }, Seg2::Arc { center, radius, .. }) | (Seg2::Arc { center, radius, .. }, Seg2::Line { a: p, b: q }) => {
            let d = q - p;
            let t = (center - p).dot(d) / d.len2();
            let foot = p + d * t;
            let h2 = radius * radius - (foot - center).len2();
            if h2 >= -EPS * radius {
                let dt = h2.max(0.0).sqrt() / d.len();
                candidates.extend([foot + d * dt, foot - d * dt]);
            }
        }
        (Seg2::Arc { center: p, radius: r, .. }, Seg2::Arc { center: q, radius: s, .. }) => {
            let d = p.dist(q);
            if d > EPS && d <= r + s + EPS && d + EPS >= (r - s).abs() {
                let x = (d * d + r * r - s * s) / (2.0 * d);
                let v = (q - p) / d;
                let h = (r * r - x * x).max(0.0).sqrt();
                candidates.extend([p + v * x + v.perp() * h, p + v * x - v.perp() * h]);
            }
        }
        _ => {}
    }
    candidates.into_iter().filter(|p| on(a, *p) && on(b, *p)).collect()
}

// Horizontal ray parity, splitting circular arcs at their vertical extrema so
// each piece has one monotone y interval. Half-open intervals count joints once.
fn inside(l: &Loop2, p: Vec2) -> bool {
    let mut parity = false;
    for s in &l.segs {
        match *s {
            Seg2::Line { a, b } => {
                if (a.y > p.y) != (b.y > p.y) && a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) > p.x {
                    parity = !parity;
                }
            }
            Seg2::Arc { center, radius, start, sweep } => {
                let mut cuts = vec![0.0, 1.0];
                for k in -10..=10 {
                    let t = (PI * 0.5 + k as f64 * PI - start) / sweep;
                    if t > 0.0 && t < 1.0 {
                        cuts.push(t);
                    }
                }
                cuts.sort_by(f64::total_cmp);
                for w in cuts.windows(2) {
                    let (mut a, mut b) = (s.point_at(w[0]), s.point_at(w[1]));
                    // Trigonometric roundoff at pi/2 multiples must not double-count
                    // a ray through the circle's horizontal diameter.
                    if (a.y - center.y).abs() < radius * 1e-14 {
                        a.y = center.y;
                    }
                    if (b.y - center.y).abs() < radius * 1e-14 {
                        b.y = center.y;
                    }
                    if (a.y > p.y) != (b.y > p.y) {
                        let mid = start + sweep * (w[0] + w[1]) * 0.5;
                        let x = center.x + mid.cos().signum() * (radius * radius - (p.y - center.y).powi(2)).max(0.0).sqrt();
                        if x > p.x {
                            parity = !parity;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    parity
}

#[cfg(test)]
mod tests {
    use crate::frame_section::{section_from_loops, validate_section};
    use solidcraft_geom::{Loop2, Region2, Seg2, Vec2};
    fn circle(x: f64, r: f64) -> Loop2 {
        Loop2::circle(Vec2::new(x, 0.0), r)
    }
    fn reject(loops: Vec<Loop2>, message: &str) {
        let before = loops.clone();
        assert_eq!(section_from_loops(loops.clone()).unwrap_err().to_string(), format!("frame section: {message}"));
        assert_eq!(loops, before);
    }
    #[test]
    fn normalizes_and_revalidates_material() {
        let outer = circle(0.0, 10.0);
        let hole = circle(0.0, 2.0);
        let result = section_from_loops(vec![hole.clone(), outer.reversed()]).unwrap();
        assert_eq!(result, Region2 { outer: outer.reversed().reversed(), holes: vec![hole.reversed()] });
        assert_eq!(validate_section(&result), Ok(()));
        assert!((result.area() - 96.0 * std::f64::consts::PI).abs() < 1e-10);
        let several = section_from_loops(vec![outer, circle(-4.0, 1.0), circle(4.0, 1.0)]).unwrap();
        assert_eq!(several.holes.len(), 2);
        assert_eq!(validate_section(&several), Ok(()));
    }
    #[test]
    fn lines_arcs_and_durable_contract() {
        let square = Loop2::polygon(&[Vec2::new(-5.0, -5.0), Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0), Vec2::new(-5.0, 5.0)]);
        let result = section_from_loops(vec![square.clone(), circle(0.0, 1.0)]).unwrap();
        assert_eq!(result.outer, square);
        assert_eq!(result.holes, vec![circle(0.0, 1.0).reversed()]);
        assert_eq!(validate_section(&result), Ok(()));
        reject(vec![square.clone(), circle(4.0, 1.0)], "touching or crossing boundaries");
        let invalid = Region2 { outer: square.reversed(), holes: vec![] };
        assert_eq!(validate_section(&invalid).unwrap_err().to_string(), "frame section: invalid orientation");
        reject(vec![circle(0.0, 1.0); 257], "segment budget");
        reject(vec![Loop2 { segs: vec![Seg2::Cubic { p0: Vec2::ZERO, p1: Vec2::X, p2: Vec2::Y, p3: Vec2::ZERO }] }], "unsupported segment kind");
        reject(vec![Loop2::polygon(&[Vec2::ZERO, Vec2::X, Vec2::new(2.0, 0.0)])], "self intersection");
        let mut arc = circle(0.0, 1.0);
        arc.segs = vec![Seg2::Arc { center: Vec2::ZERO, radius: 1.0, start: 0.0, sweep: std::f64::consts::TAU }];
        assert_eq!(section_from_loops(vec![arc.clone()]).unwrap(), Region2 { outer: arc, holes: vec![] });
    }
    #[test]
    fn rejects_invalid_topology() {
        reject(vec![], "empty section");
        reject(vec![circle(0.0, 10.0), circle(0.0, 5.0), circle(0.0, 1.0)], "nested holes or material islands");
        reject(vec![circle(0.0, 1.0), circle(4.0, 1.0)], "disconnected exteriors");
        for x in [0.0, 1.0, 2.0] {
            reject(vec![circle(0.0, 1.0), circle(x, 1.0)], "touching or crossing boundaries");
        }
        reject(vec![Loop2 { segs: vec![Seg2::Line { a: Vec2::ZERO, b: Vec2::X }] }], "open loop");
        reject(vec![Loop2::default()], "empty loop");
        reject(vec![circle(0.0, 0.0)], "invalid segment");
        reject(vec![circle(0.0, f64::INFINITY)], "invalid segment");
        reject(vec![circle(0.0, 1e7)], "invalid segment");
        let bow = Loop2::polygon(&[Vec2::ZERO, Vec2::new(2.0, 2.0), Vec2::new(0.0, 2.0), Vec2::new(2.0, 0.0)]);
        reject(vec![bow], "self intersection");
        let invalid = Region2 { outer: circle(0.0, 1.0), holes: vec![circle(4.0, 1.0).reversed()] };
        let before = invalid.clone();
        assert_eq!(validate_section(&invalid).unwrap_err().to_string(), "frame section: disconnected exteriors");
        assert_eq!(invalid, before);
    }
}

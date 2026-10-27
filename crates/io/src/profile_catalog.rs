//! Portable structural sections, independent of source files and platform storage.
use crate::{Geom2, IoError, Result};
use serde::{Deserialize, Serialize};
use solidcraft_doc::frame::SavedProfile;
use solidcraft_doc::frame_section::section_from_loops;
use solidcraft_geom::{Loop2, Region2, Seg2};
use std::f64::consts::TAU;

pub const FORMAT: &str = "solidcraft-profiles/1";
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 128;
const MAX_SEGMENTS: usize = 512;
const JOIN_TOL: f64 = 1e-7;
fn invalid(message: impl Into<String>) -> IoError {
    IoError::Invalid(message.into())
}

/// A self-contained catalog. Coordinates are always millimetres.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub format: String,
    pub units: String,
    pub entries: Vec<SavedProfile>,
}
impl Catalog {
    pub fn new(entries: Vec<SavedProfile>) -> Result<Self> {
        let catalog = Self { format: FORMAT.into(), units: "mm".into(), entries };
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn validate(&self) -> Result<()> {
        if self.format != FORMAT || self.units != "mm" || self.entries.len() > MAX_ENTRIES {
            return Err(invalid("unsupported profile catalog (solidcraft-profiles/1, mm, at most 128 entries)"));
        }
        let mut names = std::collections::BTreeSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if !names.insert(&entry.name) {
                return Err(invalid("duplicate profile catalog name"));
            }
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(invalid("profile catalog exceeds 4 MiB"));
        }
        let catalog: Self = serde_json::from_slice(bytes).map_err(|e| invalid(format!("invalid profile catalog: {e}")))?;
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| invalid(e.to_string()))?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid("profile catalog exceeds 4 MiB"));
        }
        Ok(bytes)
    }
}

/// Join supported source curves without welding or dropping any of them.
/// Ambiguous junctions, open chains and invalid nested material are errors.
pub fn section_from_geometry(geometry: Vec<Geom2>) -> Result<Region2> {
    if geometry.len() > MAX_SEGMENTS {
        return Err(invalid("frame section: segment budget"));
    }
    let mut loops = Vec::new();
    let mut edges = Vec::new();
    for item in geometry {
        match item {
            Geom2::Circle(center, radius) => loops.push(Loop2::circle(center, radius)),
            Geom2::Line(a, b) => edges.push(Seg2::Line { a, b }),
            Geom2::Arc { c, a, b } => {
                let radius = a.dist(c);
                if !radius.is_finite() || (b.dist(c) - radius).abs() > JOIN_TOL {
                    return Err(invalid("frame section: invalid circular arc"));
                }
                let start = (a - c).angle();
                let sweep = ((b - c).angle() - start).rem_euclid(TAU);
                edges.push(Seg2::Arc { center: c, radius, start, sweep });
            }
            _ => return Err(invalid("frame section: unsupported source curve")),
        }
    }
    if edges.len().saturating_add(loops.len().saturating_mul(2)) > MAX_SEGMENTS {
        return Err(invalid("frame section: segment budget"));
    }
    let mut edges: Vec<Option<Seg2>> = edges.into_iter().map(Some).collect();
    while let Some(first) = edges.iter().position(Option::is_some) {
        let edge = edges.get_mut(first).and_then(Option::take).ok_or_else(|| invalid("frame section: missing edge"))?;
        let start = edge.start();
        let mut end = edge.end();
        let mut chain = vec![edge];
        while end.dist(start) > JOIN_TOL {
            let mut found = None;
            for (i, edge) in edges.iter().enumerate().filter_map(|(i, e)| e.as_ref().map(|e| (i, e))) {
                let forward = end.dist(edge.start()) <= JOIN_TOL;
                let reverse = end.dist(edge.end()) <= JOIN_TOL;
                if forward || reverse {
                    if found.is_some() || (forward && reverse) {
                        return Err(invalid("frame section: ambiguous junction"));
                    }
                    found = Some((i, reverse));
                }
            }
            let (i, reverse) = found.ok_or_else(|| invalid("frame section: open loop"))?;
            let edge = edges.get_mut(i).and_then(Option::take).ok_or_else(|| invalid("frame section: missing edge"))?;
            let edge = if reverse { edge.reversed() } else { edge };
            end = edge.end();
            chain.push(edge);
        }
        loops.push(Loop2 { segs: chain });
    }
    section_from_loops(loops).map_err(Into::into)
}

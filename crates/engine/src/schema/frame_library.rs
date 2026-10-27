//! Contracts for the portable structural profile library.
#[cfg(test)]
#[path = "frame_library_tests.rs"]
mod tests;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Inspect the reusable profiles stored in this design.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct LibraryParams {}

/// Read one planar ASCII DXF section into the design's profile library.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct DxfParams {
    /// Unique profile name, 1–128 bytes.
    pub name: String,
    /// Inline ASCII DXF. Give text or path, exactly one.
    pub text: Option<String>,
    /// Source DXF path, accessed through the platform VFS.
    pub path: Option<String>,
    /// Explicit source units: mm, cm, m, in or ft; overrides INSUNITS.
    pub units: Option<String>,
    /// Profile origin in normalized millimetres. Default: [0, 0].
    pub origin: Option<[f64; 2]>,
    /// Replace an existing name. Default: false; existing members remain unchanged.
    pub replace: Option<bool>,
    /// Optional revision from preview; refuse if the source or options changed.
    pub expected_revision: Option<String>,
}

/// Save a built-in parametric profile as a reusable section snapshot.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SaveParams {
    /// Unique profile name, 1–128 bytes.
    pub name: String,
    /// Built-in profile kind and dimensions; resolved in the design's units.
    pub profile: Value,
    /// Replace an existing name. Default: false.
    pub replace: Option<bool>,
}

/// Import a versioned portable profile catalog, atomically.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ImportParams {
    /// Inline catalog JSON. Give text or path, exactly one.
    pub text: Option<String>,
    /// Catalog path, accessed through the platform VFS.
    pub path: Option<String>,
    /// Replace duplicate names. Default: false; member snapshots stay unchanged.
    pub replace: Option<bool>,
}

/// Export the design's reusable profiles without changing the design.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ExportParams {
    /// Optional JSON destination. Without one, returns the catalog text only.
    pub path: Option<String>,
    /// Replace an existing destination. Default: false.
    pub overwrite: Option<bool>,
}

/// A self-contained, revision-pinned section in millimetres.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Entry {
    pub name: String,
    pub source: String,
    pub revision: String,
    /// Closed line/arc exterior and holes; solidcraft-profiles/1 geometry.
    pub region: Value,
}
impl Entry {
    pub fn from_profile(profile: &solidcraft_doc::frame::SavedProfile) -> crate::Result<Self> {
        Ok(Self {
            name: profile.name.clone(),
            source: profile.source.clone(),
            revision: profile.revision.clone(),
            region: serde_json::to_value(&profile.region).map_err(|e| crate::EngineError::Other(e.to_string()))?,
        })
    }
}
/// Analytic section properties, in mm², mm and mm⁴ respectively.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Section {
    pub area: f64,
    pub centroid: [f64; 2],
    pub ixx: f64,
    pub iyy: f64,
    pub ixy: f64,
    pub width: f64,
    pub height: f64,
}
/// The saved profile and its analytic properties.
#[derive(Debug, Serialize, JsonSchema)]
pub struct EntryResult {
    pub entry: Entry,
    pub section: Section,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}
/// All reusable section definitions currently stored in the design.
#[derive(Debug, Serialize, JsonSchema)]
pub struct LibraryResult {
    pub entries: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}
/// Number of entries imported from the catalog.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ImportResult {
    pub entries: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}
/// A portable catalog and its optional written destination.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ExportResult {
    pub text: String,
    pub entries: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

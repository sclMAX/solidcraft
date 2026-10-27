//! Reusable, self-contained structural sections. No source file is needed after import.
use super::CommandSpec;
use crate::params::bad;
use crate::schema::frame_library::*;
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session};
use serde::de::DeserializeOwned;
use serde_json::Value;
use solidcraft_doc::frame::{self, SavedProfile};
use solidcraft_geom::Vec2;
use solidcraft_io::profile_catalog::{Catalog, MAX_BYTES, MAX_ENTRIES, section_from_geometry};

const LIBRARY: TypedContract<LibraryParams> = TypedContract::new();
const DXF: TypedContract<DxfParams> = TypedContract::new();
const SAVE: TypedContract<SaveParams> = TypedContract::new();
const IMPORT: TypedContract<ImportParams> = TypedContract::new();
const EXPORT: TypedContract<ExportParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("frame.profile.library", "Saved Profiles", crate::checked!("frame.profile.library", library))
        .at("WELDMENTS", "CREATE").icon("library").noundo().contract(&LIBRARY).output(output_schema_for::<LibraryResult>)
        .params("Returns entries: the design's reusable profiles, each with name, source, revision and region in mm"),
    CommandSpec::new("frame.profile.inspect_dxf", "Preview Profile DXF", crate::checked!("frame.profile.inspect_dxf", inspect_dxf))
        .icon("inspect").noundo().doc_only().contract(&DXF).output(output_schema_for::<EntryResult>)
        .params("name; text | path: planar ASCII DXF; units?: mm|cm|m|in|ft; origin?: [x,y] in normalized mm; replace? and expected_revision? are accepted for an identical import request. Validates and returns a section and content revision without changing the design"),
    CommandSpec::new("frame.profile.import_dxf", "Import Profile DXF", crate::checked!("frame.profile.import_dxf", import_dxf))
        .icon("import").doc_only().contract(&DXF).output(output_schema_for::<EntryResult>)
        .params("name; text | path: planar ASCII DXF; units?: mm|cm|m|in|ft (overrides INSUNITS); origin?: [x,y] in normalized mm; replace?: bool (default false); expected_revision?: content revision from preview (rejects changed input). One connected section with holes; no blocks, islands, repairs or unsupported curves. Returns entry and section properties"),
    CommandSpec::new("frame.profile.save", "Save Profile", crate::checked!("frame.profile.save", save))
        .icon("library").doc_only().contract(&SAVE).output(output_schema_for::<EntryResult>)
        .params("name; profile: built-in kind and dimensions in design units; replace?: bool (default false). Saves resolved mm geometry; existing members are unchanged. Returns entry and section properties"),
    CommandSpec::new("frame.profile.import", "Import Profile Library", crate::checked!("frame.profile.import", import))
        .icon("import").doc_only().contract(&IMPORT).output(output_schema_for::<ImportResult>)
        .params("text | path: solidcraft-profiles/1 JSON; replace?: bool (default false). Validates all entries before changing the design. Returns entries count"),
    CommandSpec::new("frame.profile.export", "Export Profile Library", crate::checked!("frame.profile.export", export))
        .icon("export").noundo().doc_only().contract(&EXPORT).output(output_schema_for::<ExportResult>)
        .params("path?: JSON destination (omitted: text only); overwrite?: bool (default false). Returns catalog text, entries count and optional path. Does not change the design"),
];

fn request<T: DeserializeOwned>(p: &Value, cmd: &str) -> Result<T> {
    serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))
}
fn reply(value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).map_err(|e| crate::EngineError::Other(e.to_string()))
}
fn input(text: Option<String>, path: Option<String>, max: usize, cmd: &str) -> Result<String> {
    match (text, path) {
        (Some(text), None) if text.len() <= max => Ok(text),
        (Some(_), None) => Err(bad(cmd, "profile input exceeds byte limit")),
        (None, Some(path)) => {
            if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
                return Err(bad(cmd, "invalid profile source path"));
            }
            let bytes = solidcraft_io::vfs::read(&path, max).map_err(|e| bad(cmd, e.to_string()))?;
            String::from_utf8(bytes).map_err(|_| bad(cmd, "profile input must be UTF-8 text"))
        }
        _ => Err(bad(cmd, "give text or path, exactly one")),
    }
}
fn admit(s: &Session, name: &str, replace: bool, cmd: &str) -> Result<()> {
    if s.doc.frame_profiles.contains_key(name) {
        if !replace {
            return Err(bad(cmd, "profile name already exists; set replace to true"));
        }
    } else if s.doc.frame_profiles.len() >= MAX_ENTRIES {
        return Err(bad(cmd, "profile library exceeds 128 entries"));
    }
    Ok(())
}
fn entry_reply(profile: &SavedProfile) -> Result<Value> {
    let props = frame::section(&profile.region);
    reply(EntryResult {
        entry: Entry::from_profile(profile)?,
        section: Section {
            area: props.area,
            centroid: props.centroid,
            ixx: props.ixx,
            iyy: props.iyy,
            ixy: props.ixy,
            width: props.width,
            height: props.height,
        },
        param_warnings: Vec::new(),
    })
}
fn library(s: &mut Session, _: &Value) -> Result<Value> {
    let entries = s.doc.frame_profiles.values().map(Entry::from_profile).collect::<Result<Vec<_>>>()?;
    reply(LibraryResult { entries, param_warnings: Vec::new() })
}
fn dxf_entry(request: DxfParams, cmd: &str) -> Result<SavedProfile> {
    let text = input(request.text, request.path, solidcraft_io::MAX_DRAWING_BYTES, cmd)?;
    let geometry = solidcraft_io::read_profile_dxf(&text, request.units.as_deref()).map_err(|e| bad(cmd, e.to_string()))?;
    let mut region = section_from_geometry(geometry).map_err(|e| bad(cmd, e.to_string()))?;
    if let Some([x, y]) = request.origin {
        if !x.is_finite() || !y.is_finite() || x.abs().max(y.abs()) > 1e6 {
            return Err(bad(cmd, "origin must be finite normalized millimetres within ±1e6"));
        }
        region = frame::placed(&region, Vec2::new(x, y), Vec2::ZERO, 0.0);
    }
    let profile = SavedProfile::new(request.name, "DXF".into(), region).map_err(|e| bad(cmd, e.to_string()))?;
    if request.expected_revision.as_ref().is_some_and(|expected| *expected != profile.revision) {
        return Err(bad(cmd, "profile changed after preview; inspect it again"));
    }
    Ok(profile)
}
fn inspect_dxf(_: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "frame.profile.inspect_dxf";
    let profile = dxf_entry(request(p, cmd)?, cmd)?;
    entry_reply(&profile)
}
fn import_dxf(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "frame.profile.import_dxf";
    let request: DxfParams = request(p, cmd)?;
    let replace = request.replace.unwrap_or(false);
    let profile = dxf_entry(request, cmd)?;
    admit(s, &profile.name, replace, cmd)?;
    let out = entry_reply(&profile)?;
    s.doc_mut().frame_profiles.insert(profile.name.clone(), profile);
    Ok(out)
}
fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "frame.profile.save";
    let request: SaveParams = request(p, cmd)?;
    let spec: frame::ProfileSpec = serde_json::from_value(request.profile).map_err(|e| bad(cmd, e.to_string()))?;
    let profile = spec
        .resolve(|_, expr| s.doc.eval(&s.doc.with_design_unit(expr, solidcraft_doc::expr::Kind::Length), solidcraft_doc::expr::Kind::Length))
        .map_err(|e| bad(cmd, e.to_string()))?;
    let region = profile.region().map_err(|e| bad(cmd, e.to_string()))?;
    let profile = SavedProfile::new(request.name, "Parametric preset".into(), region).map_err(|e| bad(cmd, e.to_string()))?;
    admit(s, &profile.name, request.replace.unwrap_or(false), cmd)?;
    let out = entry_reply(&profile)?;
    s.doc_mut().frame_profiles.insert(profile.name.clone(), profile);
    Ok(out)
}
fn import(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "frame.profile.import";
    let request: ImportParams = request(p, cmd)?;
    let text = input(request.text, request.path, MAX_BYTES, cmd)?;
    let catalog = Catalog::decode(text.as_bytes()).map_err(|e| bad(cmd, e.to_string()))?;
    for profile in &catalog.entries {
        admit(s, &profile.name, request.replace.unwrap_or(false), cmd)?;
    }
    let mut entries = s.doc.frame_profiles.clone();
    let count = catalog.entries.len();
    for profile in catalog.entries {
        entries.insert(profile.name.clone(), profile);
    }
    if entries.len() > MAX_ENTRIES {
        return Err(bad(cmd, "profile library exceeds 128 entries"));
    }
    let out = reply(ImportResult { entries: count, param_warnings: Vec::new() })?;
    s.doc_mut().frame_profiles = entries;
    Ok(out)
}
fn export(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "frame.profile.export";
    let request: ExportParams = request(p, cmd)?;
    let catalog = Catalog::new(s.doc.frame_profiles.values().cloned().collect()).map_err(|e| bad(cmd, e.to_string()))?;
    let bytes = catalog.encode().map_err(|e| bad(cmd, e.to_string()))?;
    if let Some(path) = &request.path {
        if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) || !path.to_ascii_lowercase().ends_with(".json") {
            return Err(bad(cmd, "profile export path must be a JSON file"));
        }
        solidcraft_io::vfs::check(path).map_err(|e| bad(cmd, e.to_string()))?;
        #[cfg(not(target_arch = "wasm32"))]
        let written = if request.overwrite.unwrap_or(false) {
            solidcraft_io::write_atomic(std::path::Path::new(path), &bytes, false)
        } else {
            solidcraft_io::safe_write::create_atomic(std::path::Path::new(path), &bytes)
        };
        #[cfg(target_arch = "wasm32")]
        let written = if !request.overwrite.unwrap_or(false) && solidcraft_io::vfs::exists(path) {
            Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "profile library file already exists"))
        } else {
            solidcraft_io::vfs::write(path, &bytes)
        };
        written.map_err(|e| bad(cmd, e.to_string()))?;
    }
    reply(ExportResult {
        text: String::from_utf8(bytes).map_err(|e| bad(cmd, e.to_string()))?,
        entries: catalog.entries.len(),
        path: request.path,
        param_warnings: Vec::new(),
    })
}

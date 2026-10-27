use super::*;
use crate::Session;
use crate::schema::tests::{assert_keys, assert_strict_keeps_todays_errors, conforms};
use crate::schema::{ParamCheck, input_schema_for, output_schema_for};
use serde_json::{Value, json};

fn reply_conforms<T: schemars::JsonSchema>(value: &Value) {
    let schema = output_schema_for::<T>();
    assert!(conforms(schema.as_value(), schema.as_value(), value), "{value}");
}

#[test]
fn schemas_and_the_key_check_agree() {
    assert_keys(input_schema_for::<LibraryParams>(), &[]);
    assert_keys(input_schema_for::<DxfParams>(), &["expected_revision", "name", "origin", "path", "replace", "text", "units"]);
    assert_keys(input_schema_for::<SaveParams>(), &["name", "profile", "replace"]);
    assert_keys(input_schema_for::<ImportParams>(), &["path", "replace", "text"]);
    assert_keys(input_schema_for::<ExportParams>(), &["overwrite", "path"]);
}

#[test]
fn strict_keeps_profile_library_errors() {
    assert_strict_keeps_todays_errors(&[
        ("frame.profile.save", json!({"name":"Bar", "profile":{"kind":"unknown"}})),
        ("frame.profile.import_dxf", json!({"name":"DXF"})),
        ("frame.profile.import", json!({"text":"{}"})),
        ("frame.profile.export", json!({"path":"not-json.dxf"})),
    ]);
}

#[test]
fn replies_match_the_output_schemas() {
    let mut s = Session { param_check: ParamCheck::Warn, ..Default::default() };

    let entry = s.execute("frame.profile.save", &json!({"name":"Bar", "profile":{"kind":"bar", "width":10,"height":20}})).unwrap();
    assert!(entry.get("param_warnings").is_none(), "{entry}");
    reply_conforms::<EntryResult>(&entry);
    let library = s.execute("frame.profile.library", &json!({})).unwrap();
    assert!(library.get("param_warnings").is_none());
    reply_conforms::<LibraryResult>(&library);
    let export = s.execute("frame.profile.export", &json!({})).unwrap();
    assert!(export.get("param_warnings").is_none());
    reply_conforms::<ExportResult>(&export);
    let imported = s.execute("frame.profile.import", &json!({"text":export["text"],"replace":true})).unwrap();
    assert!(imported.get("param_warnings").is_none());
    reply_conforms::<ImportResult>(&imported);
    let dxf = "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n0\n20\n0\n40\n1\n0\nENDSEC\n0\nEOF\n";
    let preview = s.execute("frame.profile.inspect_dxf", &json!({"name":"Disk","text":dxf,"units":"mm"})).unwrap();
    reply_conforms::<EntryResult>(&preview);
    let entry = s.execute("frame.profile.import_dxf", &json!({"name":"Disk","text":dxf,"units":"mm","origin":[0,0]})).unwrap();
    assert!(entry.get("param_warnings").is_none());
    reply_conforms::<EntryResult>(&entry);
}

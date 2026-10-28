use crate::test_support::run_all;
use serde_json::json;
use solidcraft_engine::Session;

use super::*;
use crate::Services;
use crate::dialogs::{Dialog, apply_commands, for_feature as edit_dialog};

fn app_with(cmds: &[(&str, Value)]) -> SolveApp {
    let mut s = Session::default();
    for (id, p) in cmds {
        s.execute(id, p).unwrap();
    }
    SolveApp::new(s, Services::default())
}

/// A 600 × 400 rectangle on XY as the frame sketch `Frame`, and, drawn after it on YZ, a ring of
/// two circles as the section sketch `Ring` (its circles' ids are not the frame's line ids).
fn frame_app() -> SolveApp {
    app_with(&[
        ("sketch.create", json!({"plane": "XY", "name": "Frame"})),
        ("sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [600, 400]})),
        ("sketch.finish", json!({})),
        ("sketch.create", json!({"plane": "YZ", "name": "Ring"})),
        ("sketch.circle.center", json!({"center": [0, 0], "radius": 20})),
        ("sketch.circle.center", json!({"center": [0, 0], "radius": 17})),
        ("sketch.finish", json!({})),
    ])
}

fn sketch_id(app: &SolveApp, name: &str) -> u64 {
    app.session.doc.find_feature(name).unwrap().id
}

fn frame_lines(app: &SolveApp) -> Vec<String> {
    app.session.doc.sketch(sketch_id(app, "Frame")).unwrap().curves.iter().map(|c| c.id.clone()).collect()
}

fn body_names(app: &SolveApp) -> Vec<String> {
    app.session.world_state().bodies.iter().map(|b| b.name.clone()).collect()
}

/// The dialog started with the given lines picked.
fn dialog_on(app: &mut SolveApp, lines: &[String]) -> Dialog {
    app.session.selection = lines.iter().map(|id| Sel::SketchCurve { id: id.clone(), sketch: None }).collect();
    app.start("frame.member");
    app.dialog.clone().unwrap()
}

fn member(d: &mut Dialog) -> &mut Member {
    match &mut d.kind {
        Kind::Frame(m) => m,
        k => panic!("{k:?}"),
    }
}

#[test]
fn saved_profile_library_uses_shared_pickers_and_explicit_preview() {
    let mut h = crate::scenario::Harness::new();
    let entities = "0\nCIRCLE\n10\n0\n20\n0\n40\n10\n0\nCIRCLE\n10\n0\n20\n0\n40\n8\n";
    let dxf = format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n");
    std::fs::write(h.dir.join("tube.dxf"), dxf).unwrap();
    h.step(&json!({"call":"ui.home","params":{"open":false}})).unwrap();
    h.step(&json!({"start":"frame.profile.library"})).unwrap();
    h.step(&json!({"widget":"Import DXF…"})).unwrap();
    h.step(&json!({"choose_file":"tube.dxf"})).unwrap();
    h.step(&json!({"widget":"Choose DXF…"})).unwrap();
    h.frames(3);
    let revision = h.app.run("document.revision", json!({})).unwrap();
    h.step(&json!({"widget":"Validate and preview"})).unwrap();
    assert!(h.app.run("frame.profile.library", json!({})).unwrap()["entries"].as_array().unwrap().is_empty());
    assert_eq!(h.app.run("document.revision", json!({})).unwrap(), revision);
    h.step(&json!({"widget":"Save section"})).unwrap();
    let library = h.app.run("frame.profile.library", json!({})).unwrap();
    assert_eq!(library["entries"][0]["name"], "tube");
    assert_eq!(library["entries"][0]["region"]["holes"].as_array().unwrap().len(), 1);
    h.step(&json!({"widget":"Export library…"})).unwrap();
    h.frames(3);
    let path = h.dir.join("profiles.solidcraft-profiles.json");
    if !path.is_file() {
        h.step(&json!({"debug":"widgets"})).unwrap();
        h.step(&json!({"debug":"handles"})).unwrap();
        eprintln!("picker busy: {}, dialog: {:?}", h.app.file_picker_busy(), h.app.dialog.as_ref().map(|d| &d.kind));
    }
    assert!(path.is_file(), "export picker must create the catalog");
    h.app.run("edit.undo", json!({})).unwrap();
    assert!(h.app.run("frame.profile.library", json!({})).unwrap()["entries"].as_array().unwrap().is_empty());
    h.step(&json!({"choose_file":"profiles.solidcraft-profiles.json"})).unwrap();
    h.step(&json!({"widget":"Import library…"})).unwrap();
    h.frames(3);
    assert_eq!(h.app.run("frame.profile.library", json!({})).unwrap(), library);
}

#[test]
fn saved_profile_explorer_defaults_to_custom_and_uses_wide_card() {
    let mut app = frame_app();
    app.start("frame.profile.library");
    let dialog = app.dialog.as_ref().unwrap();
    let Kind::ProfileLibrary(library) = &dialog.kind else { panic!("profile library expected") };
    assert_eq!(library.family, 0, "DXF import remains immediately available");
    assert_eq!(crate::dialogs::card_width(dialog), crate::theme::Tokens::get().form_card_width * 2.8);
}

#[test]
fn saved_profile_explorer_family_navigation_does_not_change_document() {
    let mut h = crate::scenario::Harness::new();
    h.step(&json!({"call":"ui.home","params":{"open":false}})).unwrap();
    h.step(&json!({"start":"frame.profile.library"})).unwrap();
    let revision = h.app.run("document.revision", json!({})).unwrap();
    h.step(&json!({"click":{"handle":"profile_library:family:3"}})).unwrap();
    h.frames(2);
    assert_eq!(h.app.run("document.revision", json!({})).unwrap(), revision);
    assert!(h.app.run("frame.profile.library", json!({})).unwrap()["entries"].as_array().unwrap().is_empty());
}

#[test]
fn saved_profile_library_opens_without_mutating_design() {
    let mut app = frame_app();
    let before = app.run("document.revision", json!({})).unwrap();
    app.start("frame.profile.library");
    assert!(app.dialog.is_some(), "Saved Profiles must open its shared desktop/web editor");
    assert_eq!(app.run("document.revision", json!({})).unwrap(), before);
}

#[test]
fn saved_profile_member_edit_keeps_its_pinned_revision() {
    let mut app = frame_app();
    app.run("frame.profile.save", json!({"name":"Bar","profile":{"kind":"bar","width":10,"height":20}})).unwrap();
    app.run("frame.member", json!({"sketch":"Frame","profile_entry":"Bar"})).unwrap();
    let feature = app.session.doc.features.last().unwrap().id;
    let mut dialog = edit_dialog(&app, feature, None).unwrap();
    let pinned = member(&mut dialog).saved.clone().expect("saved member keeps its section");
    app.run("frame.profile.save", json!({"name":"Bar","replace":true,"profile":{"kind":"bar","width":30,"height":20}})).unwrap();
    member(&mut dialog).angle = "90 deg".into();
    let commands = apply_commands(&app, &dialog).unwrap();
    assert_eq!(commands[0].1["params"]["profile_snapshot"], serde_json::to_value(pinned).unwrap());
    assert!(commands[0].1["params"].get("profile_entry").is_none());
    run_all(&mut app, commands);
    let measured = app.run("inspect.measure", json!({})).unwrap();
    assert!((measured["total"]["volume_mm3"].as_f64().unwrap() - 400000.0).abs() < 1e-4);
}

/// Picked lines and the default built-in profile make one member a line; Edit Feature refills
/// the dialog and runs `timeline.redefine`, and the members keep their names.
#[test]
fn members_from_picked_lines_and_a_built_in_profile() {
    let mut app = frame_app();
    let lines = frame_lines(&app);
    let mut d = dialog_on(&mut app, &lines[..2]);
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].0, "frame.member");
    let p = &c[0].1;
    assert_eq!(p["sketch"], json!(sketch_id(&app, "Frame")), "the frame sketch, not the newer section sketch: {p}");
    assert_eq!(p["lines"], json!(lines[..2]));
    assert_eq!(p["profile"], json!({"kind": "rect_tube", "width": "40 mm", "height": "40 mm", "wall": "3 mm"}));
    assert!(p.get("anchor").is_none() && p.get("angle").is_none() && p.get("mirror").is_none() && p.get("body_name").is_none(), "{p}");
    // Another built-in profile and its dimensions; an optional one left empty takes its default.
    let m = member(&mut d);
    m.kind = frame::profile_kinds().iter().position(|k| k.kind == "angle").unwrap();
    m.dims[m.kind][2] = "4 mm".into();
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].1["profile"], json!({"kind": "angle", "width": "50 mm", "height": "50 mm", "thickness": "4 mm"}));
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Angle 50x50x4", "Angle 50x50x4 (2)"]);
    app.run("body.rename", json!({"body": "Angle 50x50x4 (2)", "name": "Rail"})).unwrap();
    let f = app.session.doc.features.last().unwrap().id;
    let mut e = edit_dialog(&app, f, None).unwrap();
    assert_eq!(e.inputs[0].items.len(), 2);
    let m = member(&mut e);
    assert_eq!((frame::profile_kinds()[m.kind].kind, m.dims[m.kind][2].as_str()), ("angle", "4 mm"));
    m.dims[m.kind][2] = "5 mm".into();
    let c = apply_commands(&app, &e).unwrap();
    assert_eq!(c[0].0, "timeline.redefine");
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Angle 50x50x4", "Rail"], "the members keep their names through the edit");
}

/// A section picked in another sketch is the profile, the picks naming its regions; All Lines,
/// anchor, angle, mirror and name reach the command and come back when editing.
#[test]
fn a_picked_section_and_the_placement() {
    let mut app = frame_app();
    let lines = frame_lines(&app);
    let ring = sketch_id(&app, "Ring");
    let mut d = dialog_on(&mut app, &lines[..1]);
    // The ring between the circles: the region whose interior point lies between radii 17 and 20.
    let ss = app.session.world_state().sketch(ring).unwrap().clone();
    let index = ss.profiles.iter().position(|p| (p.region.interior_point().len() - 18.5).abs() < 1.5).unwrap();
    d.inputs[1].items = vec![Sel::Profile { sketch: ring, index }];
    let m = member(&mut d);
    m.all = true;
    m.anchor = ANCHORS.iter().position(|a| a.0 == "bottom").unwrap();
    m.angle = "15 deg".into();
    m.mirror = true;
    m.name = " Rail ".into();
    let c = apply_commands(&app, &d).unwrap();
    let p = &c[0].1;
    assert_eq!(p["sketch"], json!(sketch_id(&app, "Frame")));
    assert!(p.get("lines").is_none() && p.get("profile").is_none(), "{p}");
    assert_eq!((&p["profile_sketch"], &p["profiles"]), (&json!(ring), &json!([index])));
    assert_eq!((&p["anchor"], &p["angle"], &p["mirror"], &p["body_name"]), (&json!("bottom"), &json!("15 deg"), &json!(true), &json!("Rail")));
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Rail", "Rail (2)", "Rail (3)", "Rail (4)"]);
    let f = app.session.doc.features.last().unwrap().id;
    let mut e = edit_dialog(&app, f, None).unwrap();
    assert_eq!(e.inputs[0].items.len(), 4, "every line of the sketch shows as picked");
    assert_eq!(e.inputs[1].items, [Sel::Profile { sketch: ring, index }]);
    let m = member(&mut e);
    assert!(m.all && m.mirror && m.name == "Rail" && m.angle == "15 deg" && ANCHORS[m.anchor].0 == "bottom", "{m:?}");
}

/// A member whose section is a whole profile sketch keeps it when edited: the dialog shows the
/// sketch rather than picking every region of it, which would fill the tube's hole.
#[test]
fn a_whole_sketch_section_stays_whole_through_an_edit() {
    let mut app = frame_app();
    app.run("frame.member", json!({"sketch": "Frame", "profile_sketch": "Ring"})).unwrap();
    let volume = |app: &mut SolveApp| app.session.execute("inspect.measure", &json!({})).unwrap()["bodies"][0]["volume_mm3"].as_f64().unwrap();
    let before = volume(&mut app);
    let f = app.session.doc.features.last().unwrap().id;
    let mut e = edit_dialog(&app, f, None).unwrap();
    assert!(e.inputs[1].items.is_empty());
    assert_eq!(member(&mut e).whole, Some(sketch_id(&app, "Ring")));
    member(&mut e).angle = "30 deg".into();
    let c = apply_commands(&app, &e).unwrap();
    assert!(c[0].1["params"].get("profiles").is_none(), "{}", c[0].1);
    run_all(&mut app, c);
    assert!((volume(&mut app) - before).abs() < 1e-6 * before, "still the ring, not the filled circle");
    // Unticked, the section is a built-in profile again.
    let mut e = edit_dialog(&app, f, None).unwrap();
    member(&mut e).whole = None;
    assert_eq!(apply_commands(&app, &e).unwrap()[0].1["params"]["profile"]["kind"], "rect_tube");
}

/// Without lines there is nothing to place; lines and the section in one sketch are refused.
#[test]
fn missing_or_mixed_picks_are_refused() {
    let mut app = frame_app();
    let d = dialog_on(&mut app, &[]);
    assert!(apply_commands(&app, &d).unwrap_err().contains("lines"));
    let ring = sketch_id(&app, "Ring");
    let circles: Vec<String> = app.session.doc.sketch(ring).unwrap().curves.iter().map(|c| c.id.clone()).collect();
    let mut d = dialog_on(&mut app, &circles[..1]);
    d.inputs[1].items = vec![Sel::Profile { sketch: ring, index: 0 }];
    assert!(apply_commands(&app, &d).is_err());
}

/// Lines picked in the view keep their sketch: two 3D sketches both number their lines `w1`…, and
/// the members go on the lines that were clicked, not on the newer sketch's lines of those ids.
#[test]
fn picked_lines_stay_in_the_sketch_they_were_picked_in() {
    let mut app = app_with(&[
        ("sketch.create", json!({"plane": "XY", "name": "Uprights"})),
        ("sketch.options", json!({"sketch_3d": true})),
        ("sketch.line3d", json!({"points": [[0, 0, 0], [0, 0, 500]]})),
        ("sketch.line3d", json!({"points": [[300, 0, 0], [300, 0, 500]]})),
        ("sketch.finish", json!({})),
        ("sketch.create", json!({"plane": "XY", "name": "Rails"})),
        ("sketch.options", json!({"sketch_3d": true})),
        ("sketch.line3d", json!({"points": [[0, 0, 250], [300, 0, 250]]})),
        ("sketch.line3d", json!({"points": [[0, 0, 480], [300, 0, 480]]})),
        ("sketch.finish", json!({})),
    ]);
    let (uprights, rails) = (sketch_id(&app, "Uprights"), sketch_id(&app, "Rails"));
    let wires = |id: u64| -> Vec<String> { app.session.doc.sketch(id).unwrap().wires.iter().map(|w| w.id.clone()).collect() };
    let ids = wires(uprights);
    assert_eq!(ids, wires(rails), "the two sketches share their line ids");
    app.start("frame.member");
    let mut d = app.dialog.clone().unwrap();
    d.inputs[0].items = ids.iter().map(|id| Sel::ExternalSketchCurve { sketch: uprights, id: id.clone() }).collect();
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].1["sketch"], json!(uprights));
    run_all(&mut app, c.clone());
    // Two posts the height of the uprights, not two rails.
    let m = app.session.execute("inspect.measure", &json!({})).unwrap();
    for b in m["bodies"].as_array().unwrap() {
        let (lo, hi) = (b["bbox"]["min"][2].as_f64().unwrap(), b["bbox"]["max"][2].as_f64().unwrap());
        assert!(lo.abs() < 1e-6 && (hi - 500.0).abs() < 1e-6, "{b}");
    }
    // Lines picked in two sketches are refused.
    d.inputs[0].items.push(Sel::ExternalSketchCurve { sketch: rails, id: ids[0].clone() });
    assert!(apply_commands(&app, &d).unwrap_err().contains("one sketch"));
}

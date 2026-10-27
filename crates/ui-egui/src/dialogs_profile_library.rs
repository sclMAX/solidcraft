//! Shared desktop/browser editor for reusable structural sections.
use crate::{SolveApp, dialogs::Kind, ui_kit};
use serde_json::{Value, json};
use solidcraft_engine::doc::frame::{self, SavedProfile};
use solidcraft_engine::geom::{Region2, Vec2};
use std::sync::Arc;

const UNITS: [&str; 6] = ["", "mm", "cm", "m", "in", "ft"];

#[derive(Clone, Debug)]
pub struct Library {
    token: Arc<()>,
    pub name: String,
    pub path: String,
    pub units: usize,
    pub origin: [String; 2],
    pub replace: bool,
    pub overwrite: bool,
    pub search: String,
    pub selected: Option<String>,
    pub preview: Option<SavedProfile>,
    binding: Option<Value>,
    pub note: Option<String>,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            token: Arc::new(()),
            name: String::new(),
            path: String::new(),
            units: 0,
            origin: ["0".into(), "0".into()],
            replace: false,
            overwrite: false,
            search: String::new(),
            selected: None,
            preview: None,
            binding: None,
            note: None,
        }
    }
}
impl Library {
    fn draft(&self) -> Result<Value, String> {
        let x = self.origin[0].parse::<f64>().ok().filter(|v| v.is_finite());
        let y = self.origin[1].parse::<f64>().ok().filter(|v| v.is_finite());
        let (Some(x), Some(y)) = (x, y) else { return Err(crate::i18n::tr("profile_library.invalid_origin").into()) };
        let mut p = json!({"name":self.name.trim(),"path":self.path,"origin":[x,y]});
        if let Some(units) = UNITS.get(self.units).filter(|u| !u.is_empty()) {
            p["units"] = json!(units);
        }
        Ok(p)
    }
}
fn editor<'a>(app: &'a mut SolveApp, token: &Arc<()>) -> Option<&'a mut Library> {
    let Kind::ProfileLibrary(library) = &mut app.dialog.as_mut()?.kind else { return None };
    Arc::ptr_eq(token, &library.token).then_some(library)
}
fn choose_dxf(app: &mut SolveApp, token: Arc<()>) {
    let origin = app.session.doc.clone();
    app.pick_open_for(&["dxf"], move |app, path| {
        if !Arc::ptr_eq(&origin, &app.session.doc) {
            return;
        }
        let Some(path) = path else { return };
        if let Some(library) = editor(app, &token) {
            if library.name.trim().is_empty() {
                library.name = std::path::Path::new(&path).file_stem().and_then(|s| s.to_str()).unwrap_or("Profile").to_owned();
            }
            library.path = path;
            library.preview = None;
            library.binding = None;
        }
    });
}
fn import_catalog(app: &mut SolveApp, token: Arc<()>, replace: bool) {
    let origin = app.session.doc.clone();
    app.pick_open_for(&["json"], move |app, path| {
        if !Arc::ptr_eq(&origin, &app.session.doc) || editor(app, &token).is_none() {
            return;
        }
        let Some(path) = path else { return };
        let result = app.run("frame.profile.import", json!({"path":path,"replace":replace}));
        let note = result.map(|_| crate::i18n::tr("profile_library.imported").to_owned()).unwrap_or_else(|e| e);
        if let Some(library) = editor(app, &token) {
            library.note = Some(note);
        }
    });
}
fn export_catalog(app: &mut SolveApp, token: Arc<()>, overwrite: bool) {
    let origin = app.session.doc.clone();
    app.pick_save("profiles.solidcraft-profiles.json", &["json"], move |app, path| {
        if !Arc::ptr_eq(&origin, &app.session.doc) || editor(app, &token).is_none() {
            return;
        }
        let Some(path) = path else { return };
        let result = app.run("frame.profile.export", json!({"path":path,"overwrite":overwrite}));
        let note = result.map(|_| crate::i18n::tr("profile_library.exported").to_owned()).unwrap_or_else(|e| e);
        if let Some(library) = editor(app, &token) {
            library.note = Some(note);
        }
    });
}

pub fn rows(app: &mut SolveApp, ui: &mut egui::Ui, library: &mut Library) {
    ui_kit::text(ui, app.tr("profile_library.storage"), ui_kit::Text::Hint("profile_storage"));
    // Keep catalog actions before the variable-height list and preview.
    ui.horizontal(|ui| {
        let import = ui_kit::button(ui, app.tr("profile_library.import_catalog"), ui_kit::Button::Normal);
        crate::scenario::publish_handle("profile_library:import", import.rect.center());
        if import.clicked() {
            import_catalog(app, library.token.clone(), library.replace);
        }
        let export = ui_kit::button(ui, app.tr("profile_library.export_catalog"), ui_kit::Button::Normal);
        crate::scenario::publish_handle("profile_library:export", export.rect.center());
        if export.clicked() {
            export_catalog(app, library.token.clone(), library.overwrite);
        }
    });
    ui_kit::checkbox(ui, app.tr("profile_library.replace"), &mut library.replace);
    ui_kit::checkbox(ui, app.tr("profile_library.overwrite"), &mut library.overwrite);
    ui_kit::text_row(ui, app.tr("profile_library.name"), &mut library.name, "");
    ui_kit::text_row(ui, app.tr("profile_library.file"), &mut library.path, "");
    let choose = ui_kit::button(ui, app.tr("profile_library.choose_dxf"), ui_kit::Button::Normal);
    crate::scenario::publish_handle("profile_library:choose", choose.rect.center());
    if choose.clicked() {
        choose_dxf(app, library.token.clone());
    }
    let labels = [app.tr("profile_library.source_units"), "mm", "cm", "m", "in", "ft"];
    ui_kit::choice_row(ui, app.tr("profile_library.units"), "profile_source_units", &mut library.units, &labels);
    ui_kit::text_row(ui, app.tr("profile_library.origin_x"), &mut library.origin[0], "0");
    ui_kit::text_row(ui, app.tr("profile_library.origin_y"), &mut library.origin[1], "0");
    let draft = library.draft();
    if library.binding.as_ref() != draft.as_ref().ok() {
        library.preview = None;
        library.binding = None;
    }
    let preview =
        ui_kit::button(ui, app.tr("profile_library.preview"), ui_kit::Button::Enabled(!library.name.trim().is_empty() && !library.path.is_empty()));
    crate::scenario::publish_handle("profile_library:preview", preview.rect.center());
    if preview.clicked() {
        let result = draft.clone().and_then(|p| app.run("frame.profile.inspect_dxf", p));
        match result {
            Ok(result) => match result.get("entry").cloned().and_then(|v| serde_json::from_value::<SavedProfile>(v).ok()) {
                Some(profile) => {
                    library.preview = Some(profile);
                    library.binding = draft.clone().ok();
                    library.note = None;
                }
                None => library.note = Some(app.tr("profile_library.invalid_preview").into()),
            },
            Err(error) => library.note = Some(error),
        }
    }
    let import = ui_kit::button(ui, app.tr("profile_library.save_section"), ui_kit::Button::Enabled(library.preview.is_some()));
    crate::scenario::publish_handle("profile_library:save", import.rect.center());
    if import.clicked()
        && let Some(profile) = &library.preview
    {
        let result = draft.and_then(|mut p| {
            p["replace"] = json!(library.replace);
            p["expected_revision"] = json!(profile.revision);
            app.run("frame.profile.import_dxf", p)
        });
        match result {
            Ok(_) => {
                library.selected = Some(profile.name.clone());
                library.note = Some(app.tr("profile_library.imported").into());
            }
            Err(error) => {
                library.note = Some(error);
                library.preview = None;
                library.binding = None;
            }
        }
    }
    ui_kit::text_row(ui, app.tr("profile_library.search"), &mut library.search, "");
    let query = library.search.to_lowercase();
    let entries: Vec<_> = app.session.doc.frame_profiles.values().filter(|p| p.name.to_lowercase().contains(&query)).collect();
    if entries.is_empty() {
        ui_kit::text(ui, app.tr("profile_library.empty"), ui_kit::Text::Hint("profile_empty"));
    }
    for entry in entries {
        let style = if library.selected.as_ref() == Some(&entry.name) { ui_kit::Button::SmallSelected } else { ui_kit::Button::Small };
        if ui_kit::button(ui, &entry.name, style).clicked() {
            library.selected = Some(entry.name.clone());
            library.preview = None;
            library.binding = None;
        }
    }
    let shown = library.preview.as_ref().or_else(|| library.selected.as_ref().and_then(|name| app.session.doc.frame_profiles.get(name)));
    if let Some(profile) = shown {
        preview_region(ui, &profile.region);
        let section = frame::section(&profile.region);
        let properties = crate::i18n::format(
            "profile_library.properties",
            &[("area", &format!("{:.3}", section.area)), ("width", &format!("{:.3}", section.width)), ("height", &format!("{:.3}", section.height))],
        );
        ui_kit::text(ui, &properties, ui_kit::Text::Hint("profile_properties"));
    }
    if let Some(note) = &library.note {
        ui_kit::text(ui, note, ui_kit::Text::Hint("profile_note"));
    }
}

// A passive section diagram, not a navigable viewport. Sampling is display-only;
// saved geometry and validation retain the analytic line/arc representation.
fn preview_region(ui: &mut egui::Ui, region: &Region2) {
    let t = crate::theme::Tokens::get();
    let paths: Vec<Vec<Vec2>> = std::iter::once(&region.outer)
        .chain(&region.holes)
        .map(|l| l.segs.iter().flat_map(|s| (0..=32).map(move |i| s.point_at(i as f64 / 32.0))).collect())
        .collect();
    let points: Vec<_> = paths.iter().flatten().copied().collect();
    let Some(&first) = points.first() else { return };
    let (lo, hi) =
        points.iter().fold((first, first), |(lo, hi), p| (Vec2::new(lo.x.min(p.x), lo.y.min(p.y)), Vec2::new(hi.x.max(p.x), hi.y.max(p.y))));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(t.form_card_width), t.form_card_width * 0.5), egui::Sense::hover());
    let inner = rect.shrink(t.space_12);
    let scale = (inner.width() as f64 / (hi.x - lo.x).max(1e-7)).min(inner.height() as f64 / (hi.y - lo.y).max(1e-7));
    let center = (lo + hi) * 0.5;
    for path in paths {
        let points: Vec<_> =
            path.into_iter().map(|p| rect.center() + egui::vec2(((p.x - center.x) * scale) as f32, (-(p.y - center.y) * scale) as f32)).collect();
        ui.painter().add(egui::Shape::line(points, t.primary_border(t.form_primary)));
    }
}

//! Shared desktop/browser editor for reusable structural sections.
use crate::{SolveApp, dialogs::Kind, ui_kit};
use serde_json::{Value, json};
use solidcraft_engine::doc::frame::{self, SavedProfile};
use solidcraft_engine::geom::{Region2, Vec2};
use std::sync::Arc;

const FAMILIES: [(&str, &str); 9] = [
    ("profile_library.family_custom", "profile_library.family_custom_desc"),
    ("profile_library.family_bar", "profile_library.family_bar_desc"),
    ("profile_library.family_round_bar", "profile_library.family_round_bar_desc"),
    ("profile_library.family_rect_tube", "profile_library.family_rect_tube_desc"),
    ("profile_library.family_round_tube", "profile_library.family_round_tube_desc"),
    ("profile_library.family_angle", "profile_library.family_angle_desc"),
    ("profile_library.family_channel", "profile_library.family_channel_desc"),
    ("profile_library.family_i_beam", "profile_library.family_i_beam_desc"),
    ("profile_library.family_c", "profile_library.family_c_desc"),
];

#[derive(Clone, Debug)]
struct DimensionRow {
    code: String,
    description: String,
    values: Vec<String>,
}

fn illustrative_rows() -> Vec<Vec<DimensionRow>> {
    let defaults: &[&[f64]] = &[
        &[],
        &[40., 40., 2.],
        &[40.],
        &[40., 40., 4., 2., 2.],
        &[40., 4.],
        &[40., 40., 6., 2., 2.],
        &[50., 100., 5., 8., 2., 2.],
        &[60., 100., 5., 8., 2., 2.],
        &[50., 100., 4., 15., 2.],
    ];
    defaults
        .iter()
        .enumerate()
        .map(|(i, values)| {
            if i == 0 {
                return Vec::new();
            }
            let number = match i {
                6 => 7,
                7 => 6,
                _ => i,
            };
            let mut rows = vec![DimensionRow {
                code: format!("DEMO-{number}01"),
                description: crate::i18n::tr("profile_library.illustrative_row").into(),
                values: values.iter().map(ToString::to_string).collect(),
            }];
            if i == 3 {
                for (code, dimensions) in [("DEMO-302", [20., 20., 2., 2., 0.]), ("DEMO-310", [100., 100., 10., 10., 2.])] {
                    rows.push(DimensionRow {
                        code: code.into(),
                        description: crate::i18n::tr("profile_library.illustrative_row").into(),
                        values: dimensions.iter().map(ToString::to_string).collect(),
                    });
                }
            }
            rows
        })
        .collect()
}

const UNITS: [&str; 6] = ["", "mm", "cm", "m", "in", "ft"];

#[derive(Clone, Debug)]
pub struct Library {
    token: Arc<()>,
    pub name: String,
    pub path: String,
    pub units: usize,
    pub origin: [String; 2],
    pub replace: bool,
    pub catalog_replace: bool,
    pub overwrite: bool,
    pub importing_dxf: bool,
    pub search: String,
    pub family: usize,
    family_search: String,
    size_search: String,
    sizes: Vec<Vec<DimensionRow>>,
    editing: Option<(Option<usize>, DimensionRow)>,
    sort: usize,
    descending: bool,
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
            catalog_replace: false,
            overwrite: false,
            importing_dxf: false,
            search: String::new(),
            family: 0,
            family_search: String::new(),
            size_search: String::new(),
            sizes: illustrative_rows(),
            editing: None,
            sort: 0,
            descending: false,
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
    let t = crate::theme::Tokens::get();

    // Top bar: note on left, catalog import/export on right
    ui.horizontal(|ui| {
        ui_kit::text(ui, app.tr("profile_library.storage"), ui_kit::Text::Hint("profile_storage"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let export = ui_kit::button(ui, app.tr("profile_library.export_catalog"), ui_kit::Button::Normal);
            crate::scenario::publish_handle("profile_library:export", export.rect.center());
            if export.clicked() {
                export_catalog(app, library.token.clone(), library.overwrite);
            }
            let import = ui_kit::button(ui, app.tr("profile_library.import_catalog"), ui_kit::Button::Normal);
            crate::scenario::publish_handle("profile_library:import", import.rect.center());
            if import.clicked() {
                import_catalog(app, library.token.clone(), library.catalog_replace);
            }
        });
    });
    ui.separator();

    ui.horizontal_top(|ui| {
        // Left column: section families navigation
        let nav_width = (t.form_card_width * 0.95).max(250.0);
        ui.allocate_ui_with_layout(egui::vec2(nav_width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui_kit::text(ui, app.tr("profile_library.families"), ui_kit::Text::Heading("families"));
            ui_kit::text_row(ui, app.tr("profile_library.family_search"), &mut library.family_search, "");
            let query = library.family_search.to_lowercase();
            let visible: Vec<_> = FAMILIES
                .iter()
                .enumerate()
                .filter(|(_, (key, desc))| app.tr(key).to_lowercase().contains(&query) || app.tr(desc).to_lowercase().contains(&query))
                .collect();
            ui_kit::text(
                ui,
                &crate::i18n::format("profile_library.family_count", &[("count", &visible.len().to_string())]),
                ui_kit::Text::Hint("family_count"),
            );
            ui.add_space(t.space_4);

            egui::ScrollArea::vertical().max_height(ui.available_height().max(350.0)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = t.space_6;
                for (index, (key, desc)) in visible {
                    let is_selected = library.family == index;
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width() - 4.0, 44.0), egui::Sense::click());
                    crate::scenario::publish_handle(&format!("profile_library:family:{index}"), rect.center());
                    if response.clicked() {
                        library.family = index;
                        library.editing = None;
                        library.size_search.clear();
                    }
                    let bg = if is_selected {
                        t.selected_bg
                    } else if response.hovered() {
                        t.control_fill
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let border_stroke = if is_selected { t.primary_border(t.form_primary) } else { egui::Stroke::NONE };
                    ui.painter().rect(rect, t.radius_md, bg, border_stroke, egui::StrokeKind::Inside);

                    let icon_rect = egui::Rect::from_min_size(rect.min + egui::vec2(t.space_6, 6.0), egui::vec2(32.0, 32.0));
                    paint_family_icon(ui.painter(), icon_rect, index, is_selected);

                    let text_rect =
                        egui::Rect::from_min_max(egui::pos2(rect.min.x + 46.0, rect.min.y + 4.0), egui::pos2(rect.max.x - 4.0, rect.max.y - 4.0));
                    let title_font = crate::fonts::semibold(t.font_label);
                    let desc_font = egui::FontId::proportional(t.font_small);
                    let title_color = if is_selected { t.selected_ink } else { t.text };
                    let desc_color = if is_selected { t.selected_ink.gamma_multiply(0.85) } else { t.form_dim };

                    ui.painter().text(egui::pos2(text_rect.left(), text_rect.top()), egui::Align2::LEFT_TOP, app.tr(key), title_font, title_color);
                    ui.painter().text(
                        egui::pos2(text_rect.left(), text_rect.top() + 18.0),
                        egui::Align2::LEFT_TOP,
                        app.tr(desc),
                        desc_font,
                        desc_color,
                    );
                }
            });
        });

        ui.separator();

        // Right column: family detail, table or custom cards
        ui.vertical(|ui| {
            if library.family == 0 {
                custom_rows(app, ui, library);
            } else {
                standard_rows(app, ui, library);
            }
        });
    });
}

fn custom_rows(app: &mut SolveApp, ui: &mut egui::Ui, library: &mut Library) {
    let t = crate::theme::Tokens::get();

    ui.horizontal(|ui| {
        ui_kit::text(ui, app.tr("profile_library.custom"), ui_kit::Text::Heading("custom"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !library.importing_dxf {
                let import_btn = ui_kit::button(ui, app.tr("profile_library.import_dxf"), ui_kit::Button::Normal);
                crate::scenario::publish_handle("profile_library:import_dxf_btn", import_btn.rect.center());
                if import_btn.clicked() {
                    library.importing_dxf = true;
                    library.preview = None;
                }
            }
        });
    });

    if library.importing_dxf {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui_kit::text(ui, app.tr("cmd.frame.profile.import_dxf"), ui_kit::Text::Heading("dxf_title"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui_kit::button(ui, app.tr("profile_library.back_to_profiles"), ui_kit::Button::Small).clicked() {
                        library.importing_dxf = false;
                    }
                });
            });
            ui.separator();
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
            ui_kit::checkbox(ui, app.tr("profile_library.replace"), &mut library.replace);
            let draft = library.draft();
            if library.binding.as_ref() != draft.as_ref().ok() {
                library.preview = None;
                library.binding = None;
            }
            ui.horizontal(|ui| {
                let preview = ui_kit::button(
                    ui,
                    app.tr("profile_library.preview"),
                    ui_kit::Button::Enabled(!library.name.trim().is_empty() && !library.path.is_empty()),
                );
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
                            library.importing_dxf = false;
                        }
                        Err(error) => {
                            library.note = Some(error);
                            library.preview = None;
                            library.binding = None;
                        }
                    }
                }
            });
            if let Some(profile) = library.preview.as_ref() {
                preview_region(ui, &profile.region);
                let section = frame::section(&profile.region);
                let properties = crate::i18n::format(
                    "profile_library.properties",
                    &[
                        ("area", &format!("{:.3}", section.area)),
                        ("width", &format!("{:.3}", section.width)),
                        ("height", &format!("{:.3}", section.height)),
                    ],
                );
                ui_kit::text(ui, &properties, ui_kit::Text::Hint("profile_properties"));
            }
        });
        if let Some(note) = &library.note {
            ui_kit::text(ui, note, ui_kit::Text::Hint("profile_note"));
        }
        return;
    }

    ui_kit::text_row(ui, app.tr("profile_library.custom_search"), &mut library.search, "");
    let query = library.search.to_lowercase();
    let entries: Vec<_> = app.session.doc.frame_profiles.values().filter(|p| p.name.to_lowercase().contains(&query)).collect();
    if entries.is_empty() {
        ui_kit::text(ui, app.tr("profile_library.custom_empty"), ui_kit::Text::Hint("profile_empty"));
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        let card_w = (ui.available_width() * 0.48 - t.space_6).max(140.0);
        for pair in entries.chunks(2) {
            ui.horizontal_top(|ui| {
                for entry in pair {
                    let is_sel = library.selected.as_ref() == Some(&entry.name);
                    let frame_stroke = if is_sel { t.primary_border(t.form_primary) } else { t.container_border(t.border) };
                    let frame_bg = if is_sel { t.selected_bg } else { t.control_fill };
                    egui::Frame::new().fill(frame_bg).stroke(frame_stroke).corner_radius(t.radius_md).inner_margin(t.space_10).show(ui, |ui| {
                        ui.set_width(card_w);
                        preview_region(ui, &entry.region);
                        ui.add_space(t.space_4);
                        ui_kit::text(ui, &entry.name, ui_kit::Text::Heading("custom_card_name"));
                        let section = frame::section(&entry.region);
                        let properties = crate::i18n::format(
                            "profile_library.properties",
                            &[
                                ("area", &format!("{:.1}", section.area)),
                                ("width", &format!("{:.1}", section.width)),
                                ("height", &format!("{:.1}", section.height)),
                            ],
                        );
                        ui_kit::text(ui, &properties, ui_kit::Text::Hint("profile_properties"));
                        ui.add_space(t.space_4);
                        ui.horizontal(|ui| {
                            let style = if is_sel { ui_kit::Button::SmallSelected } else { ui_kit::Button::Small };
                            if ui_kit::button(ui, app.tr("profile_library.select"), style).clicked() {
                                library.selected = Some(entry.name.clone());
                            }
                        });
                    });
                }
            });
            ui.add_space(t.space_6);
        }
    });

    if let Some(note) = &library.note {
        ui_kit::text(ui, note, ui_kit::Text::Hint("profile_note"));
    }
}

fn standard_rows(app: &SolveApp, ui: &mut egui::Ui, library: &mut Library) {
    let family = library.family;
    let Some((key, _)) = FAMILIES.get(family) else { return };
    ui_kit::text(ui, app.tr(key), ui_kit::Text::Heading("standard_family"));
    ui_kit::text(ui, app.tr("profile_library.schematic"), ui_kit::Text::Hint("schematic"));
    schematic(ui, family);
    ui_kit::text(ui, app.tr("profile_library.illustrative"), ui_kit::Text::Hint("illustrative"));
    ui_kit::text(ui, app.tr("profile_library.dimensions"), ui_kit::Text::Heading("dimensions"));
    ui_kit::text_row(ui, app.tr("profile_library.size_search"), &mut library.size_search, "");
    let Some(rows) = library.sizes.get_mut(family) else { return };
    let template = illustrative_rows().get(family).and_then(|r| r.first()).cloned();
    if ui_kit::button(ui, app.tr("profile_library.add"), ui_kit::Button::Normal).clicked() {
        library.editing = template.map(|mut row| {
            row.code.clear();
            (None, row)
        });
    }
    let count = rows.first().map_or_else(|| template_dimension_count(family), |row| row.values.len());
    let dimension_labels: Vec<_> = (0..count).map(|i| dimension_letter(family, i)).collect();
    ui_kit::text(ui, &dimension_labels.join(" · "), ui_kit::Text::Hint("dimension_legend"));
    let mut labels = vec![app.tr("profile_library.code").to_owned(), app.tr("profile_library.description").to_owned()];
    labels.extend(dimension_labels.iter().map(|s| format!("{s} (mm)")));
    labels.push(app.tr("profile_library.actions").into());
    let columns: Vec<_> = labels.iter().map(|label| ui_kit::Column::new(label, 1.0)).collect();
    let query = library.size_search.to_lowercase();
    let mut visible: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| format!("{} {} {}", r.code, r.description, r.values.join(" ")).to_lowercase().contains(&query))
        .collect();
    visible.sort_by(|(_, a), (_, b)| {
        let order = match library.sort {
            0 => a.code.cmp(&b.code),
            1 => a.description.cmp(&b.description),
            i => a
                .values
                .get(i - 2)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.)
                .total_cmp(&b.values.get(i - 2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.)),
        };
        if library.descending { order.reverse() } else { order }
    });
    ui_kit::text(ui, &crate::i18n::format("profile_library.size_count", &[("count", &visible.len().to_string())]), ui_kit::Text::Hint("size_count"));
    let mut delete = None;
    ui_kit::table_custom(ui, "profile_dimensions", &columns, app.tr("profile_library.empty"), |ui, table| {
        table.headers(ui, |ui, column| {
            if column + 1 == labels.len() {
                return false;
            }
            if ui_kit::button(ui, &labels[column], ui_kit::Button::Small).clicked() {
                library.descending = library.sort == column && !library.descending;
                library.sort = column;
            }
            true
        });
        for (index, row) in visible {
            table.row(ui, &index.to_string(), false, |cells| {
                cells.cell(|ui| {
                    ui_kit::text(ui, &row.code, ui_kit::Text::Cell);
                });
                cells.cell(|ui| {
                    ui_kit::text(ui, &row.description, ui_kit::Text::Cell);
                });
                for value in &row.values {
                    cells.cell(|ui| {
                        ui_kit::text(ui, value, ui_kit::Text::MonoCell);
                    });
                }
                cells.cell(|ui| {
                    if ui_kit::button(ui, app.tr("profile_library.edit"), ui_kit::Button::Small).clicked() {
                        library.editing = Some((Some(index), row.clone()));
                    }
                    if ui_kit::button(ui, app.tr("profile_library.delete"), ui_kit::Button::Small).clicked() {
                        delete = Some(index);
                    }
                });
            });
        }
    });
    if let Some(index) = delete {
        rows.remove(index);
        library.editing = None;
    }
    let mut commit = false;
    let mut cancel = false;
    if let Some((index, row)) = &mut library.editing {
        ui.separator();
        ui_kit::text_row(ui, app.tr("profile_library.code"), &mut row.code, "");
        ui_kit::text_row(ui, app.tr("profile_library.description"), &mut row.description, "");
        for (i, value) in row.values.iter_mut().enumerate() {
            ui_kit::text_row(ui, &dimension_letter(family, i), value, "mm");
        }
        let valid = !row.code.trim().is_empty()
            && !row.description.trim().is_empty()
            && !rows.iter().enumerate().any(|(i, r)| Some(i) != *index && r.code == row.code.trim())
            && row.values.iter().enumerate().all(|(i, v)| {
                v.parse::<f64>().is_ok_and(|n| n.is_finite() && if dimension_letter(family, i).starts_with('R') { n >= 0. } else { n > 0. })
            });
        commit = ui_kit::button(ui, app.tr("profile_library.save_row"), ui_kit::Button::Enabled(valid)).clicked();
        cancel = ui_kit::button(ui, app.tr("profile_library.cancel"), ui_kit::Button::Normal).clicked();
    }
    if commit && let Some((index, mut row)) = library.editing.take() {
        row.code = row.code.trim().into();
        if let Some(target) = index.and_then(|i| rows.get_mut(i)) {
            *target = row;
        } else {
            rows.push(row);
        }
    }
    if cancel {
        library.editing = None;
    }
}

fn template_dimension_count(family: usize) -> usize {
    illustrative_rows().get(family).and_then(|r| r.first()).map_or(0, |r| r.values.len())
}

fn base_dimension_count(family: usize) -> usize {
    match family {
        1 => 2,
        2 => 1,
        3 => 3,
        4 => 2,
        5 => 3,
        6 | 7 | 8 => 4,
        _ => 0,
    }
}

fn dimension_letter(family: usize, index: usize) -> String {
    let base = base_dimension_count(family);
    if index < base { char::from(b'A' + index as u8).to_string() } else { format!("R{}", index - base + 1) }
}

fn paint_family_icon(p: &egui::Painter, rect: egui::Rect, family: usize, selected: bool) {
    let t = crate::theme::Tokens::get();
    let center = rect.center();
    let s = rect.width() * 0.36;
    let stroke = egui::Stroke::new(1.6, if selected { t.selected_ink } else { t.form_primary });
    let fill = if selected { t.selected_bg } else { t.control_fill };

    match family {
        0 => {
            // Custom / Others (stepped / irregular hollow shape)
            let pts = vec![
                center + egui::vec2(-s * 0.8, -s),
                center + egui::vec2(s * 0.25, -s),
                center + egui::vec2(s * 0.25, -s * 0.25),
                center + egui::vec2(s * 0.8, -s * 0.25),
                center + egui::vec2(s * 0.8, s),
                center + egui::vec2(-s * 0.8, s),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
            p.rect(
                egui::Rect::from_center_size(center + egui::vec2(-s * 0.25, s * 0.25), egui::vec2(s * 0.45, s * 0.45)),
                0.0,
                egui::Color32::TRANSPARENT,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        1 => {
            // Flat / square bar
            p.rect(egui::Rect::from_center_size(center, egui::vec2(s * 1.8, s * 1.8)), t.radius_sm, fill, stroke, egui::StrokeKind::Inside);
        }
        2 => {
            // Round bar
            p.circle(center, s * 0.9, fill, stroke);
        }
        3 => {
            // Rect tube (hollow box)
            p.rect(egui::Rect::from_center_size(center, egui::vec2(s * 1.8, s * 1.8)), t.radius_sm, fill, stroke, egui::StrokeKind::Inside);
            p.rect(
                egui::Rect::from_center_size(center, egui::vec2(s * 1.0, s * 1.0)),
                0.0,
                egui::Color32::TRANSPARENT,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        4 => {
            // Round tube (hollow circle)
            p.circle(center, s * 0.9, fill, stroke);
            p.circle(center, s * 0.55, egui::Color32::TRANSPARENT, stroke);
        }
        5 => {
            // Angle (L-shape)
            let pts = vec![
                center + egui::vec2(-s, -s),
                center + egui::vec2(-s * 0.35, -s),
                center + egui::vec2(-s * 0.35, s * 0.35),
                center + egui::vec2(s, s * 0.35),
                center + egui::vec2(s, s),
                center + egui::vec2(-s, s),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
        }
        6 => {
            // Channels / UPN (U-shape)
            let pts = vec![
                center + egui::vec2(s * 0.8, -s),
                center + egui::vec2(-s * 0.8, -s),
                center + egui::vec2(-s * 0.8, s),
                center + egui::vec2(s * 0.8, s),
                center + egui::vec2(s * 0.8, s * 0.45),
                center + egui::vec2(-s * 0.25, s * 0.45),
                center + egui::vec2(-s * 0.25, -s * 0.45),
                center + egui::vec2(s * 0.8, -s * 0.45),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
        }
        7 => {
            // I-Beam / IPN (I-shape)
            let pts = vec![
                center + egui::vec2(-s * 0.8, -s),
                center + egui::vec2(s * 0.8, -s),
                center + egui::vec2(s * 0.8, -s * 0.5),
                center + egui::vec2(s * 0.2, -s * 0.5),
                center + egui::vec2(s * 0.2, s * 0.5),
                center + egui::vec2(s * 0.8, s * 0.5),
                center + egui::vec2(s * 0.8, s),
                center + egui::vec2(-s * 0.8, s),
                center + egui::vec2(-s * 0.8, s * 0.5),
                center + egui::vec2(-s * 0.2, s * 0.5),
                center + egui::vec2(-s * 0.2, -s * 0.5),
                center + egui::vec2(-s * 0.8, -s * 0.5),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
        }
        _ => {
            // C-section
            let pts = vec![
                center + egui::vec2(s * 0.8, -s * 0.35),
                center + egui::vec2(s * 0.8, -s),
                center + egui::vec2(-s * 0.8, -s),
                center + egui::vec2(-s * 0.8, s),
                center + egui::vec2(s * 0.8, s),
                center + egui::vec2(s * 0.8, s * 0.35),
                center + egui::vec2(s * 0.45, s * 0.35),
                center + egui::vec2(s * 0.45, s * 0.6),
                center + egui::vec2(-s * 0.3, s * 0.6),
                center + egui::vec2(-s * 0.3, -s * 0.6),
                center + egui::vec2(s * 0.45, -s * 0.6),
                center + egui::vec2(s * 0.45, -s * 0.35),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
        }
    }
}

fn schematic(ui: &mut egui::Ui, family: usize) {
    let t = crate::theme::Tokens::get();
    let width = ui.available_width().min(t.form_card_width * 1.6);
    let height = (t.control_height * 5.0).max(150.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let center = rect.center() + egui::vec2(-10.0, 0.0);
    let scale = t.control_height * 1.3;
    let stroke = t.primary_border(t.form_primary);
    let dim_stroke = egui::Stroke::new(1.0, t.form_dim);
    let dim_font = egui::FontId::monospace(t.font_small);
    let p = ui.painter();

    match family {
        1 => {
            // Bar (solid rectangle)
            let box_rect = egui::Rect::from_center_size(center, egui::vec2(scale * 1.8, scale * 1.8));
            p.rect(box_rect, t.radius_sm, t.selected_bg, stroke, egui::StrokeKind::Inside);
            // Dim A (bottom)
            let y = box_rect.bottom() + 14.0;
            p.line_segment([egui::pos2(box_rect.left(), box_rect.bottom() + 2.0), egui::pos2(box_rect.left(), y + 3.0)], dim_stroke);
            p.line_segment([egui::pos2(box_rect.right(), box_rect.bottom() + 2.0), egui::pos2(box_rect.right(), y + 3.0)], dim_stroke);
            p.line_segment([egui::pos2(box_rect.left(), y), egui::pos2(box_rect.right(), y)], dim_stroke);
            p.text(egui::pos2(box_rect.center().x, y + 2.0), egui::Align2::CENTER_TOP, "A", dim_font.clone(), t.form_dim);
            // Dim B (right)
            let x = box_rect.right() + 14.0;
            p.line_segment([egui::pos2(box_rect.right() + 2.0, box_rect.top()), egui::pos2(x + 3.0, box_rect.top())], dim_stroke);
            p.line_segment([egui::pos2(box_rect.right() + 2.0, box_rect.bottom()), egui::pos2(x + 3.0, box_rect.bottom())], dim_stroke);
            p.line_segment([egui::pos2(x, box_rect.top()), egui::pos2(x, box_rect.bottom())], dim_stroke);
            p.text(egui::pos2(x + 4.0, box_rect.center().y), egui::Align2::LEFT_CENTER, "B", dim_font.clone(), t.form_dim);
        }
        2 => {
            // Round bar (solid circle)
            let r = scale * 0.9;
            p.circle(center, r, t.selected_bg, stroke);
            p.line_segment([center - egui::vec2(r, 0.0), center + egui::vec2(r, 0.0)], dim_stroke);
            p.text(center + egui::vec2(0.0, -8.0), egui::Align2::CENTER_BOTTOM, "Ø A", dim_font.clone(), t.form_dim);
        }
        3 => {
            // Square/rect tube (hollow box)
            let outer = egui::Rect::from_center_size(center, egui::vec2(scale * 1.8, scale * 1.8));
            let inner = egui::Rect::from_center_size(center, egui::vec2(scale * 1.1, scale * 1.1));
            p.rect(outer, t.radius_sm, t.selected_bg, stroke, egui::StrokeKind::Inside);
            p.rect(inner, t.radius_sm.saturating_div(2), ui.visuals().window_fill(), stroke, egui::StrokeKind::Inside);
            // Dim A (bottom)
            let y = outer.bottom() + 14.0;
            p.line_segment([egui::pos2(outer.left(), outer.bottom() + 2.0), egui::pos2(outer.left(), y + 3.0)], dim_stroke);
            p.line_segment([egui::pos2(outer.right(), outer.bottom() + 2.0), egui::pos2(outer.right(), y + 3.0)], dim_stroke);
            p.line_segment([egui::pos2(outer.left(), y), egui::pos2(outer.right(), y)], dim_stroke);
            p.text(egui::pos2(outer.center().x, y + 2.0), egui::Align2::CENTER_TOP, "A", dim_font.clone(), t.form_dim);
            // Dim B (right)
            let x = outer.right() + 14.0;
            p.line_segment([egui::pos2(outer.right() + 2.0, outer.top()), egui::pos2(x + 3.0, outer.top())], dim_stroke);
            p.line_segment([egui::pos2(outer.right() + 2.0, outer.bottom()), egui::pos2(x + 3.0, outer.bottom())], dim_stroke);
            p.line_segment([egui::pos2(x, outer.top()), egui::pos2(x, outer.bottom())], dim_stroke);
            p.text(egui::pos2(x + 4.0, outer.center().y), egui::Align2::LEFT_CENTER, "B", dim_font.clone(), t.form_dim);
            // Dim C (wall)
            p.line_segment([egui::pos2(outer.left(), center.y), egui::pos2(inner.left(), center.y)], dim_stroke);
            p.text(egui::pos2(outer.left() - 4.0, center.y), egui::Align2::RIGHT_CENTER, "C", dim_font.clone(), t.form_dim);
        }
        4 => {
            // Round tube (hollow circle)
            let r_out = scale * 0.95;
            let r_in = scale * 0.65;
            p.circle(center, r_out, t.selected_bg, stroke);
            p.circle(center, r_in, ui.visuals().window_fill(), stroke);
            p.line_segment([center - egui::vec2(r_out, 0.0), center + egui::vec2(r_out, 0.0)], dim_stroke);
            p.text(center + egui::vec2(0.0, -8.0), egui::Align2::CENTER_BOTTOM, "Ø A", dim_font.clone(), t.form_dim);
            p.text(center + egui::vec2(r_out + 6.0, 0.0), egui::Align2::LEFT_CENTER, "B (wall)", dim_font.clone(), t.form_dim);
        }
        5 => {
            // Angle (L-shape)
            let pts = vec![
                center + egui::vec2(-scale, -scale),
                center + egui::vec2(-scale * 0.4, -scale),
                center + egui::vec2(-scale * 0.4, scale * 0.4),
                center + egui::vec2(scale, scale * 0.4),
                center + egui::vec2(scale, scale),
                center + egui::vec2(-scale, scale),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
            let y = center.y + scale + 14.0;
            p.line_segment([egui::pos2(center.x - scale, y), egui::pos2(center.x + scale, y)], dim_stroke);
            p.text(egui::pos2(center.x, y + 2.0), egui::Align2::CENTER_TOP, "A", dim_font.clone(), t.form_dim);
            let x = center.x - scale - 14.0;
            p.line_segment([egui::pos2(x, center.y - scale), egui::pos2(x, center.y + scale)], dim_stroke);
            p.text(egui::pos2(x - 4.0, center.y), egui::Align2::RIGHT_CENTER, "B", dim_font.clone(), t.form_dim);
            p.text(center + egui::vec2(0.0, scale * 0.7), egui::Align2::CENTER_CENTER, "C", dim_font.clone(), t.form_dim);
        }
        6 => {
            // Channels / UPN (U-shape)
            let pts = vec![
                center + egui::vec2(scale * 0.7, -scale),
                center + egui::vec2(-scale * 0.7, -scale),
                center + egui::vec2(-scale * 0.7, scale),
                center + egui::vec2(scale * 0.7, scale),
                center + egui::vec2(scale * 0.7, scale * 0.6),
                center + egui::vec2(-scale * 0.2, scale * 0.6),
                center + egui::vec2(-scale * 0.2, -scale * 0.6),
                center + egui::vec2(scale * 0.7, -scale * 0.6),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
            let y = center.y + scale + 14.0;
            p.line_segment([egui::pos2(center.x - scale * 0.7, y), egui::pos2(center.x + scale * 0.7, y)], dim_stroke);
            p.text(egui::pos2(center.x, y + 2.0), egui::Align2::CENTER_TOP, "A", dim_font.clone(), t.form_dim);
            let x = center.x - scale * 0.7 - 14.0;
            p.line_segment([egui::pos2(x, center.y - scale), egui::pos2(x, center.y + scale)], dim_stroke);
            p.text(egui::pos2(x - 4.0, center.y), egui::Align2::RIGHT_CENTER, "B", dim_font.clone(), t.form_dim);
            p.text(center + egui::vec2(-scale * 0.45, 0.0), egui::Align2::CENTER_CENTER, "C", dim_font.clone(), t.form_dim);
            p.text(center + egui::vec2(scale * 0.3, -scale * 0.8), egui::Align2::CENTER_CENTER, "D", dim_font.clone(), t.form_dim);
        }
        7 => {
            // I-Beam / IPN (I-shape)
            let pts = vec![
                center + egui::vec2(-scale * 0.7, -scale),
                center + egui::vec2(scale * 0.7, -scale),
                center + egui::vec2(scale * 0.7, -scale * 0.6),
                center + egui::vec2(scale * 0.15, -scale * 0.6),
                center + egui::vec2(scale * 0.15, scale * 0.6),
                center + egui::vec2(scale * 0.7, scale * 0.6),
                center + egui::vec2(scale * 0.7, scale),
                center + egui::vec2(-scale * 0.7, scale),
                center + egui::vec2(-scale * 0.7, scale * 0.6),
                center + egui::vec2(-scale * 0.15, scale * 0.6),
                center + egui::vec2(-scale * 0.15, -scale * 0.6),
                center + egui::vec2(-scale * 0.7, -scale * 0.6),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
            let y = center.y + scale + 14.0;
            p.line_segment([egui::pos2(center.x - scale * 0.7, y), egui::pos2(center.x + scale * 0.7, y)], dim_stroke);
            p.text(egui::pos2(center.x, y + 2.0), egui::Align2::CENTER_TOP, "A", dim_font.clone(), t.form_dim);
            let x = center.x + scale * 0.7 + 14.0;
            p.line_segment([egui::pos2(x, center.y - scale), egui::pos2(x, center.y + scale)], dim_stroke);
            p.text(egui::pos2(x + 4.0, center.y), egui::Align2::LEFT_CENTER, "B", dim_font.clone(), t.form_dim);
            p.text(center + egui::vec2(0.0, 0.0), egui::Align2::CENTER_CENTER, "C", dim_font.clone(), t.form_dim);
            p.text(center + egui::vec2(0.0, -scale * 0.8), egui::Align2::CENTER_CENTER, "D", dim_font.clone(), t.form_dim);
        }
        8 => {
            // C-section (C-shape with return lips)
            let pts = vec![
                center + egui::vec2(scale * 0.7, -scale * 0.4),
                center + egui::vec2(scale * 0.7, -scale),
                center + egui::vec2(-scale * 0.7, -scale),
                center + egui::vec2(-scale * 0.7, scale),
                center + egui::vec2(scale * 0.7, scale),
                center + egui::vec2(scale * 0.7, scale * 0.4),
                center + egui::vec2(scale * 0.5, scale * 0.4),
                center + egui::vec2(scale * 0.5, scale * 0.7),
                center + egui::vec2(-scale * 0.3, scale * 0.7),
                center + egui::vec2(-scale * 0.3, -scale * 0.65),
                center + egui::vec2(scale * 0.5, -scale * 0.65),
                center + egui::vec2(scale * 0.5, -scale * 0.4),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
            let y = center.y + scale + 14.0;
            p.line_segment([egui::pos2(center.x - scale * 0.7, y), egui::pos2(center.x + scale * 0.7, y)], dim_stroke);
            p.text(egui::pos2(center.x, y + 2.0), egui::Align2::CENTER_TOP, "A", dim_font.clone(), t.form_dim);
            let x = center.x - scale * 0.7 - 14.0;
            p.line_segment([egui::pos2(x, center.y - scale), egui::pos2(x, center.y + scale)], dim_stroke);
            p.text(egui::pos2(x - 4.0, center.y), egui::Align2::RIGHT_CENTER, "B", dim_font.clone(), t.form_dim);
            p.text(center + egui::vec2(-scale * 0.5, 0.0), egui::Align2::CENTER_CENTER, "C", dim_font.clone(), t.form_dim);
            p.text(center + egui::vec2(scale * 0.85, scale * 0.55), egui::Align2::LEFT_CENTER, "D", dim_font.clone(), t.form_dim);
        }
        _ => {}
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

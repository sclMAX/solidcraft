//! Frame Member (`frame.member`): structural members along the lines of a frame sketch. The lines
//! are picked in the view; the section is a built-in profile with its dimensions, or the profiles
//! picked in another sketch. Anchor, angle and mirror place the section on each line.

use serde_json::{Map, Value, json};
use solidcore_record::Vec2;
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_engine::doc::frame::{self, Anchor, MemberProfile, SectionView};
use solidcraft_engine::doc::{FeatureKind, ProfileSel};

use crate::SolveApp;
use crate::dialogs::{Kind, check, curves_sketch, form_row, profile_indices, put, shown_expr};
use crate::selection::{PROFILES, SKETCH_LINES, SelInput, SelectionExt as _};
use crate::ui_kit::{choice_row, expr_row, hint_row, text_row};

/// The anchors as `frame.member` names them, and their labels, in `frame::Anchor::ALL` order
/// (the hot points in the view, `place_gizmo::frame_anchor`, go by it).
pub(crate) const ANCHORS: &[(&str, &str)] = &[
    ("origin", "Origin"),
    ("center", "Center"),
    ("centroid", "Centroid"),
    ("top_left", "Top Left"),
    ("top", "Top Middle"),
    ("top_right", "Top Right"),
    ("left", "Left Middle"),
    ("right", "Right Middle"),
    ("bottom_left", "Bottom Left"),
    ("bottom", "Bottom Middle"),
    ("bottom_right", "Bottom Right"),
];

/// The anchor at `index` in [`ANCHORS`], whose order is `Anchor::ALL`'s.
pub(crate) fn anchor_at(index: usize) -> Option<Anchor> {
    Anchor::ALL.get(index).copied()
}

/// The index of `anchor` in [`ANCHORS`] (the inverse of [`anchor_at`]).
pub(crate) fn anchor_index(anchor: Anchor) -> Option<usize> {
    Anchor::ALL.iter().position(|a| *a == anchor)
}

/// How near (mm) a `locate` worked out again from the offsets must come to the one the dialog
/// holds for that one to stay: float noise does not rewrite a feature's located point.
const SAME_LOCATE: f64 = 1e-5;

/// The Frame Member dialog's values (the picks are its selection inputs).
#[derive(Clone, Debug)]
pub struct Member {
    /// Every line of the frame sketch, now and as lines are added, not only the lines picked.
    pub all: bool,
    /// The built-in profile (an index into `frame::profile_kinds`), used when no section is picked.
    pub kind: usize,
    /// Each built-in profile's dimensions, its required ones then its optional ones; kept per
    /// profile so switching back finds them again. An empty optional one takes its default.
    pub dims: Vec<Vec<String>>,
    /// A profile sketch whose whole drawn section is the member's (nested loops alternating), as
    /// a feature made that way keeps it while no section is picked.
    pub whole: Option<u64>,
    pub anchor: usize,
    /// Offset A and B: how far the section is moved from its anchor, along its own axes as drawn
    /// (A its width, B its height; not the world's X and Y; both empty: none). The user's
    /// intention: kept as the dialog's `locate` (`sync_offset`), which the engine reads, and
    /// worked out again into it when the profile, Anchor, Mirror or Angle change.
    pub offset: [String; 2],
    /// The offset text `locate` was last worked out from (or shown from): a different text is an
    /// edit to apply.
    offset_applied: [String; 2],
    /// The `locate` this dialog last wrote or showed as offsets: a different one was set
    /// elsewhere (Edit Feature, a script) and shows as the offsets from the anchor.
    locate_written: Option<[f64; 2]>,
    /// The parameters without `locate` (`base_params`) `locate` was last worked out for: other
    /// ones (a dimension, the Anchor, Mirror) work it out again from the offsets.
    params_applied: Option<String>,
    pub angle: String,
    pub mirror: bool,
    pub name: String,
}

impl Member {
    fn new() -> Member {
        let dims = frame::profile_kinds()
            .iter()
            .map(|k| {
                let example = serde_json::to_value(&k.example).unwrap_or(Value::Null);
                let given = k.required.iter().map(|d| example.get(*d).and_then(Value::as_f64).map(|v| format!("{v} mm")).unwrap_or_default());
                given.chain(k.optional.iter().map(|_| String::new())).collect()
            })
            .collect();
        // A rectangular tube is the most common member.
        let kind = frame::profile_kinds().iter().position(|k| k.kind == "rect_tube").unwrap_or(0);
        Member {
            all: false,
            kind,
            dims,
            whole: None,
            anchor: 0,
            offset: Default::default(),
            offset_applied: Default::default(),
            locate_written: None,
            params_applied: None,
            angle: String::new(),
            mirror: false,
            name: String::new(),
        }
    }
}

/// The dialog for Frame Member.
pub fn start(_app: &SolveApp, id: &str) -> Option<(Kind, Vec<SelInput>)> {
    (id == "frame.member")
        .then(|| (Kind::Frame(Member::new()), vec![SelInput::new("Lines", SKETCH_LINES, true), SelInput::new("Section (optional)", PROFILES, true)]))
}

fn tr(text: &str) -> &str {
    solidcraft_ui_text::tr_label(text)
}

/// The dialog's own rows (after the selection inputs); true when Enter was pressed in a value.
pub fn rows(app: &SolveApp, ui: &mut egui::Ui, m: &mut Member, inputs: &[SelInput], extra: &mut Map<String, Value>) -> bool {
    let mut enter = false;
    form_row(ui, "All Lines", |ui| check(ui, &mut m.all));
    let picked = !crate::selection::items(inputs, 1).is_empty();
    if let Some(sketch) = m.whole.filter(|_| !picked) {
        let name = app.session.doc.feature(sketch).map(|f| f.name.clone()).unwrap_or_default();
        let mut on = true;
        form_row(ui, "Section", |ui| crate::ui_kit::checkbox(ui, &name, &mut on));
        if !on {
            m.whole = None;
        }
    } else if !picked {
        let kinds = frame::profile_kinds();
        let labels: Vec<&str> = kinds.iter().map(|k| k.label).collect();
        choice_row(ui, "Profile", "frame_profile", &mut m.kind, &labels);
        if let (Some(k), Some(values)) = (kinds.get(m.kind), m.dims.get_mut(m.kind)) {
            for (i, (name, v)) in k.required.iter().chain(k.optional).zip(values.iter_mut()).enumerate() {
                let label = frame::dimension_label(name);
                enter |= if i < k.required.len() { expr_row(ui, label, v) } else { hint_row(ui, label, v, tr("default")) };
            }
        }
    }
    let anchors: Vec<&str> = ANCHORS.iter().map(|a| a.1).collect();
    choice_row(ui, "Anchor", "frame_anchor", &mut m.anchor, &anchors);
    let [a, b] = &mut m.offset;
    enter |= hint_row(ui, "ui.frame_offset_a", a, "0 mm");
    enter |= hint_row(ui, "ui.frame_offset_b", b, "0 mm");
    enter |= hint_row(ui, "Angle", &mut m.angle, "0 deg");
    form_row(ui, "Mirror", |ui| check(ui, &mut m.mirror));
    enter |= text_row(ui, "Name", &mut m.name, tr("from the profile")).0;
    sync_offset(app, m, inputs, extra);
    enter
}

/// Keep Offset A and B and the dialog's `locate` in step. The offsets are the user's intention:
/// typed (or set by an arrow or the marker in the view), they set `locate` (the anchor's point
/// minus the offset; both empty: none), and a later change of the profile, Anchor, Mirror or
/// Angle works `locate` out again from them. A `locate` set otherwise (Edit Feature, a script)
/// shows as the offsets from the anchor. A click on a hot point empties both.
fn sync_offset(app: &SolveApp, m: &mut Member, inputs: &[SelInput], extra: &mut Map<String, Value>) {
    let held = located(extra);
    let edited = m.offset != m.offset_applied;
    // Within float noise of the `locate` written: not set elsewhere (an arrow's move must not
    // rewrite the other offset's text from it).
    let elsewhere = match (held, m.locate_written) {
        (Some(h), Some(w)) => !same_locate(h, w),
        (h, w) => h.is_some() != w.is_some(),
    };
    if !edited && elsewhere {
        let offset = match held {
            None => Some(Default::default()),
            Some(at) => base_view(app, m, inputs, extra)
                .and_then(|v| offset_of(&v, m.anchor, at))
                .map(|o| o.map(|v| crate::length_input::model(&app.session.doc, nanometre(v)))),
        };
        // No section to read it on yet: tried again as values change.
        if let Some(offset) = offset {
            m.offset_applied = offset.clone();
            m.offset = offset;
            m.locate_written = held;
            m.params_applied = base_key(app, m, inputs, extra);
        }
        return;
    }
    let key = base_key(app, m, inputs, extra);
    if !edited && key == m.params_applied {
        return;
    }
    let at = match offset_mm(app, m) {
        // Half typed: `locate` stays as it was, and with it the section in the view; OK refuses.
        Err(_) => return,
        Ok(None) => None,
        // No section yet (no line, an unfinished value), or past the largest profile: tried
        // again as values change, rather than taken as applied and later wiped; OK refuses.
        Ok(Some(off)) => match base_view(app, m, inputs, extra).and_then(|v| locate_for(&v, m.anchor, off)) {
            Some(at) => Some(at),
            None => return,
        },
    };
    match at {
        Some(at) if !held.is_some_and(|h| same_locate(h, at)) => {
            extra.insert("locate".into(), json!(at));
        }
        Some(_) => {}
        None => {
            extra.remove("locate");
        }
    }
    m.offset_applied = m.offset.clone();
    m.locate_written = located(extra);
    m.params_applied = key;
}

/// The dialog's `locate`, when it holds one.
fn located(extra: &Map<String, Value>) -> Option<[f64; 2]> {
    extra.get("locate").and_then(Value::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]))
}

fn same_locate(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() <= SAME_LOCATE && (a[1] - b[1]).abs() <= SAME_LOCATE
}

/// `v` (mm) rounded to a nanometre, without a negative zero: an offset worked out from a
/// `locate` shows as `50 mm`, not as the float noise of the section's layout (`49.99999999 mm`).
fn nanometre(v: f64) -> f64 {
    (v * 1e6).round() / 1e6 + 0.0
}

/// Offset A and B in mm; none when both are empty (an empty one is 0).
fn offset_mm(app: &SolveApp, m: &Member) -> Result<Option<[f64; 2]>, String> {
    let [a, b] = [m.offset[0].trim(), m.offset[1].trim()];
    if a.is_empty() && b.is_empty() {
        return Ok(None);
    }
    let mm = |t: &str| -> Result<f64, String> {
        if t.is_empty() {
            return Ok(0.0);
        }
        crate::length_input::eval(&app.session.doc, t, ValueKind::Length)
            .ok()
            .filter(|v| frame::within_profile_size(*v))
            .ok_or_else(|| format!("Offset A and Offset B must be lengths within {} mm", frame::MAX_PROFILE_SIZE))
    };
    Ok(Some([mm(a)?, mm(b)?]))
}

/// The dialog's parameters without a located point: what the anchors are read from (they do not
/// move with one).
fn base_params(app: &SolveApp, m: &Member, inputs: &[SelInput], extra: &Map<String, Value>) -> Result<Value, String> {
    let mut p = params(app, m, inputs, extra)?;
    if let Value::Object(o) = &mut p {
        o.remove("locate");
    }
    Ok(p)
}

fn base_key(app: &SolveApp, m: &Member, inputs: &[SelInput], extra: &Map<String, Value>) -> Option<String> {
    base_params(app, m, inputs, extra).ok().map(|p| p.to_string())
}

/// The section the dialog's values make without a located point, from the section cache the
/// view's hot points share.
fn base_view(app: &SolveApp, m: &Member, inputs: &[SelInput], extra: &Map<String, Value>) -> Option<SectionView> {
    crate::place_gizmo::frame_anchor::section_view(app, &base_params(app, m, inputs, extra).ok()?).ok()
}

/// The `locate` that moves the section by Offset A and B (`a`, `b`) from anchor `anchor`: the
/// anchor's point minus the offset is the point on the line. None past the largest profile.
pub(crate) fn locate_for(v: &SectionView, anchor: usize, [a, b]: [f64; 2]) -> Option<[f64; 2]> {
    let p = frame::anchor_point(&v.regions, anchor_at(anchor)?);
    let at = v.locate_of(Vec2::new(p.x - a, p.y - b));
    at.iter().all(|c| frame::within_profile_size(*c)).then_some(at)
}

/// How far `locate` moves the section from anchor `anchor` (the inverse of `locate_for`).
fn offset_of(v: &SectionView, anchor: usize, at: [f64; 2]) -> Option<[f64; 2]> {
    let (p, q) = (frame::anchor_point(&v.regions, anchor_at(anchor)?), v.locate_point(at));
    let off = [p.x - q.x, p.y - q.y];
    off.iter().all(|c| c.is_finite()).then_some(off)
}

/// The `locate` typed Offset A and B (`off`) give with the dialog's values now; refused, naming
/// the fields, when they cannot be applied, so OK never runs a `locate` left from other values.
fn applied_locate(app: &SolveApp, m: &Member, inputs: &[SelInput], extra: &Map<String, Value>, off: [f64; 2]) -> Result<[f64; 2], String> {
    let view = crate::place_gizmo::frame_anchor::section_view(app, &base_params(app, m, inputs, extra)?)
        .map_err(|e| format!("Offset A and Offset B cannot be applied: {e}"))?;
    locate_for(&view, m.anchor, off).ok_or_else(|| {
        format!("Offset A and Offset B move the section's point on the line past the largest profile ({} mm)", frame::MAX_PROFILE_SIZE)
    })
}

/// The command the dialog's OK runs (`extra`: parameters kept from the feature being edited).
pub fn commands(app: &SolveApp, m: &Member, inputs: &[SelInput], extra: &Map<String, Value>) -> Result<Vec<(String, Value)>, String> {
    let off = offset_mm(app, m)?;
    let mut p = params(app, m, inputs, extra)?;
    if let Some(off) = off {
        let at = applied_locate(app, m, inputs, extra, off)?;
        if !located(extra).is_some_and(|h| same_locate(h, at)) {
            p["locate"] = json!(at);
        }
    }
    Ok(vec![("frame.member".to_string(), p)])
}

/// The `frame.member` parameters (Offset A and B are in them as `locate`, through `extra`); the
/// view's hot points draw them, a half-typed offset leaving the last `locate` in them.
pub(crate) fn params(app: &SolveApp, m: &Member, inputs: &[SelInput], extra: &Map<String, Value>) -> Result<Value, String> {
    let section = crate::selection::items(inputs, 1).profiles();
    let section_sketch = section.first().map(|x| x.0).or(m.whole);
    if section.iter().any(|x| Some(x.0) != section_sketch) {
        return Err("the section's profiles must be in one sketch".into());
    }
    // A line picked in the view comes with its sketch (line ids are a sketch's own); a line
    // selected before the dialog opened is looked for in the newest sketch with it, not the
    // section's.
    let picks = crate::selection::items(inputs, 0);
    let mut lines = Vec::new();
    let mut known = None;
    for pick in picks {
        match pick {
            Sel::ExternalSketchCurve { sketch, id } | Sel::SketchCurve { id, sketch: Some(sketch) } => {
                if known.is_some_and(|k| k != *sketch) {
                    return Err("the lines must be in one sketch".into());
                }
                known = Some(*sketch);
                lines.push(id.clone());
            }
            Sel::SketchCurve { id, sketch: None } => lines.push(id.clone()),
            _ => {}
        }
    }
    if lines.is_empty() {
        return Err("select the frame sketch's lines first".into());
    }
    let sketch = known.or_else(|| curves_sketch(app, &lines, section_sketch)).ok_or("the lines must be in one sketch, not the section's")?;
    let mut p = json!({"sketch": sketch});
    if !m.all {
        p["lines"] = json!(lines);
    }
    if let Some(ps) = section_sketch {
        p["profile_sketch"] = json!(ps);
        if !section.is_empty() {
            p["profiles"] = json!(section.iter().map(|x| x.1).collect::<Vec<_>>());
        }
    } else {
        let kinds = frame::profile_kinds();
        let (k, values) = kinds.get(m.kind).zip(m.dims.get(m.kind)).ok_or("choose a profile")?;
        let mut profile = json!({"kind": k.kind});
        for (name, v) in k.required.iter().chain(k.optional).zip(values) {
            if !v.trim().is_empty() {
                profile[*name] = json!(v);
            }
        }
        p["profile"] = profile;
    }
    if m.anchor != 0
        && let Some((anchor, _)) = ANCHORS.get(m.anchor)
    {
        p["anchor"] = json!(anchor);
    }
    put(&mut p, &[("angle", &m.angle), ("body_name", m.name.trim())]);
    if m.mirror {
        p["mirror"] = json!(true);
    }
    if let Value::Object(o) = &mut p {
        for (key, v) in extra {
            o.entry(key.clone()).or_insert_with(|| v.clone());
        }
    }
    Ok(p)
}

/// The dialog editing a frame member feature, filled from it.
pub fn for_feature(app: &SolveApp, kind: &FeatureKind) -> Option<(Kind, Vec<SelInput>, Map<String, Value>)> {
    let FeatureKind::FrameMember { path_sketch, lines, profile, anchor, locate, angle, mirror, body_name, .. } = kind else { return None };
    let (_, mut inputs) = start(app, "frame.member")?;
    let st = app.session.model.state();
    let mut m = Member::new();
    m.all = lines.is_empty();
    let ids = if m.all { frame::member_line_ids(&st, kind).unwrap_or_default() } else { lines.clone() };
    if let Some(i) = inputs.get_mut(0) {
        i.items = ids.into_iter().map(|id| Sel::ExternalSketchCurve { sketch: path_sketch.0, id }).collect();
    }
    match profile {
        MemberProfile::Library { profile } => {
            let kinds = frame::profile_kinds();
            if let Some((index, k)) = kinds.iter().enumerate().find(|(_, k)| k.kind == profile.kind)
                && let Some(values) = m.dims.get_mut(index)
            {
                m.kind = index;
                for (name, v) in k.required.iter().chain(k.optional).zip(values.iter_mut()) {
                    *v = profile.dims.get(*name).map(|e| shown_expr(e, frame::dimension_kind(name))).unwrap_or_default();
                }
            }
        }
        MemberProfile::Sketch { sketch, profiles: ProfileSel::All } => m.whole = Some(sketch.0),
        MemberProfile::Sketch { sketch, profiles } => {
            let picks = st.sketch(sketch.0).map(|ss| profile_indices(ss, profiles)).unwrap_or_default();
            if let Some(i) = inputs.get_mut(1) {
                i.items = picks.into_iter().map(|index| Sel::Profile { sketch: sketch.0, index }).collect();
            }
        }
    }
    m.anchor = anchor_index(*anchor).unwrap_or(0);
    m.angle = angle.as_deref().map(|a| shown_expr(a, ValueKind::Angle)).unwrap_or_default();
    m.mirror = *mirror;
    m.name = body_name.clone().unwrap_or_default();
    // A located point stays as the feature has it (the member stays where it is attached); Offset
    // A and B show it from the anchor, worked out once here (or once a section can be laid out).
    let mut extra = Map::new();
    if let Some(at) = locate {
        extra.insert("locate".into(), json!(at));
        sync_offset(app, &mut m, &inputs, &mut extra);
    }
    Some((Kind::Frame(m), inputs, extra))
}

#[cfg(test)]
#[path = "dialogs_frame_tests.rs"]
mod tests;

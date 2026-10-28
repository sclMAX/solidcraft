# Reusable structural profiles

A saved profile is a self-contained cross-section in millimetres, not a link to a
DXF file, directory or online catalog. It is stored with the design. Each member
copies the selected definition and revision: replacing a library entry does not
change members already created from it.

## Import a section without redrawing it

Open **WELDMENTS → CREATE → Saved Profiles**. Choose a DXF, give the section a
name, confirm source units and optionally choose its origin. **Validate and
preview** checks the entire section without changing the design. **Save section**
imports that exact content revision; if the file changed, preview it again.
The editor also searches saved profiles and imports/exports portable catalogs.
Frame Member offers a **Saved section** selector; existing members retain their
original section when edited.

The family navigation can be resized horizontally. Standard families show filled
schematic sections and a dimension guide beside the diagram when space permits,
or below it in narrower panes. The illustrative millimetre dimensions are not
certified; edits to their temporary rows are discarded when the dialog closes.
Row actions use edit and delete icons with descriptive tooltips. Adding or editing a
row opens a centred editor with the selected family's reference schematic. Save
commits a valid row; Cancel, the close button or Escape discards the draft while
leaving the library open. Codes must be unique and descriptions nonblank;
dimensions must be finite and positive, except radii may be zero.

`frame.profile.inspect_dxf` and `frame.profile.import_dxf` accept `text` or `path`,
exactly one, plus `name`. Inspection is read-only; import optionally takes
`expected_revision` from inspection and refuses changed source content.
An explicit `units` (`mm`, `cm`, `m`, `in`, `ft`) overrides the drawing's units.
Unknown or absent units require an explicit choice. `origin: [x,y]`, in normalized
millimetres, optionally establishes the profile's insertion origin.

```json
{"command":"frame.profile.import_dxf","params":{"path":"section.dxf","name":"My tube","units":"mm"}}
{"command":"frame.member","params":{"sketch":"Frame","profile_entry":"My tube"}}
```

The strict reader accepts planar ASCII DXF lines, circular arcs, circles and
supported polylines. No block references or nonempty block definitions are
accepted. Empty layout block metadata carries no profile geometry. Binary DXF,
DWG, text, splines, ellipses, nondefault extrusion systems, 3D geometry and wide
polylines are rejected, not silently flattened or approximated. If necessary,
export a supported planar ASCII DXF from the drawing editor first.

A profile contains exactly one closed exterior and zero or more holes. Two
concentric circles form a tube. Three concentric circles introduce a disconnected
material island and are rejected. Several separate holes inside one exterior are
valid; separate material regions, touching/crossing boundaries and open contours
are not. Import does not weld points or repair contours. See
[reader limitations](profile-dxf-reader.md) and [section validation](profile-sections.md)
for the numerical tolerance and complexity limits.

## Save a built-in profile

```json
{"command":"frame.profile.save","params":{"name":"Flat 40x5","profile":{"kind":"bar","width":40,"height":5}}}
```

Dimensions are resolved using the current design's units and parameters. The
saved section is a snapshot, not a live parameter expression. Use the existing
parametric `frame.member` input when the member must follow parameter changes.

## Share or back up a library

- `frame.profile.library {}` lists saved names, source descriptions, revisions and
  geometry. The existing `frame.profile.list` still lists built-in families.
- `frame.profile.export {}` returns catalog JSON as `text`; optional `path` writes
  a `.json` file. Existing files are preserved unless `overwrite: true` is explicit.
- `frame.profile.import {"text": "…"}` or `{"path":"profiles.json"}` validates
  an entire catalog before adding any entry.
- Import/save refuses duplicate names unless `replace: true` is explicit.

The catalog format is `solidcraft-profiles/1`, with `units: "mm"`, at most 128
entries and 4 MiB encoded input. Entries contain a name, source description,
SHA-256 content revision and normalized `region` (exterior and holes). Revision
validation detects mismatched content; it is not authentication or a license.
Only share drawings and profile data you have permission to redistribute.

## Desktop and browser storage

Engine commands use the same portable data and VFS on both platforms. Browser
files enter through its upload picker and exports leave through downloads. Saved
profiles travel inside the design; the browser host's normal design persistence
also retains them. A downloaded library is a separate backup. Browser storage
may be unavailable, cleared or evicted; local persistence is not cloud sync or a
guarantee of permanent retention.

Programmatic commands never open a picker or dialog. Import failure leaves the
design unchanged; adding/replacing profiles is undoable. `profile_snapshot` on
`frame.member` is the self-contained revision form used when editing existing
members, so editing a member does not rebind it to an updated library entry.

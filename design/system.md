# SolidCraft light UI — source sync

Not a proposed starter. Scoped token subset for profile-library mockups; no application changes.

## Sources
- `assets/styles/ui/palettes.rs`: `Tokens::light()` values below, logical points mapped to CSS px.
- `crates/ui-egui/src/theme.rs`: token semantics, border fallbacks and shared styling.
- `docs/ui-kit.md`: Panel/Window composition and controls.
- `crates/ui-egui/src/dialogs_profile_library.rs`: existing interaction semantics.

## Colors
Token | Value
--- | ---
`app_bar` | `#343840`
`app_bar_text` | `#e6eaf2`
`toolbar` | `#f6f7f9`
`panel` | `#fafbfc`
`panel_header` | `#eceef2`
`border` | `#d0d4dc`
`text` | `#262a32`
`form_dim` | `#4a5868`
`form_primary` | `#236ead`
`on_accent` | `#ffffff`
`selected_ink` | `#1a507f`
`selected_bg` | `#e8f1fa`
`dialog_body` | `#ffffff`
`control_border` | `#c3cbd6`
`form_error` | `#a1342c`
`form_error_fill` | `#fcefed`
`form_success` | `#276b2c`
`form_success_fill` | `#e5f2e6`
`viewport_top` | `#f0f3f7`
`viewport_bottom` | `#cbd2dd`
`grid` | `#c4cad4`
`body` | `#b0bac6`
`body_edge` | `#282c34`
`accent` | `#2678da`

All color rows source: `assets/styles/ui/palettes.rs`, `Tokens::light()`. Names retain source roles: app chrome, neutral surfaces, text, form states, viewport geometry. Use form_dim rather than text_dim for readable small hints.

## Typography, spacing and radii
Token | Value (px mapping)
--- | ---
`space_2` | 2.0
`space_3` | 3.0
`space_4` | 4.0
`space_6` | 6.0
`space_8` | 8.0
`space_10` | 10.0
`space_12` | 12.0
`space_14` | 14.0
`space_16` | 16.0
`space_24` | 24.0
`radius_sm` | 4
`radius_md` | 6
`radius_lg` | 8
`radius_xl` | 10
`control_radius` | 6
`control_height` | 28.0
`form_header_height` | 40.0
`form_footer_height` | 48.0
`font_body` | 12.0
`font_small` | 11.0
`font_label` | 12.5
`font_button` | 13.0
`font_heading` | 14.0
`font_title` | 22.0

All numeric rows source: `assets/styles/ui/palettes.rs`, `Tokens::light()`. `theme.rs::apply` uses space_6/space_4 gaps and space_6/space_3 button padding.

## Browser font mapping
`--font-ui: system-ui,sans-serif`; `--font-mono: ui-monospace,monospace`. These are explicitly mockup mappings, not claims about Rust embedded font identity (not established by the inspected sources). Numeric properties use tabular figures.

## Components
`docs/ui-kit.md`: one outer surface; 40px header, 48px footer, 28px controls, 12px body inset, 6px control radius. Selected rows use selected_bg/selected_ink; primary buttons use form_primary/on_accent; errors include text and form_error_fill. Border fallback is 1px (`theme.rs`). No shadow used; source shell_shadow is offset space_2, blur space_8, app_bar gamma ×0.18, intentionally omitted in these flat boards. Board geometry and SVG dimensions are illustrative layout, not new app tokens.

## Self-check
All boards repeat this scoped color/dimension set and the two browser font mappings. No external resources. No application functionality or default change.

## Dark palette — profile library baseline
Source: `assets/styles/ui/palettes.rs`, `Tokens::dark()`; inherited dimensions and browser font mappings above remain unchanged.

Token | Value
--- | ---
`app_bar` | `#1c1f26`
`toolbar` / `panel` / `dialog_body` / `dialog_bg` | `#272b34`
`panel_header` / `dialog_footer` | `#2f343e`
`border` / `dialog_border` | `#3f4552`
`dialog_rule` | `#353b47`
`text` / `form_ink` / `dialog_label` | `#dee2e8`
`text_dim` | `#949ba8`
`form_dim` | `#b4bbc8`
`form_primary` / `accent` | `#3a88ee`
`selected_ink` | `#9cc7f5`
`selected_bg` | `#2a466e`
`control_fill` | `#1c1f26`
`control_border` | `#4a5262`
`icon` | `#d0d6e2`
`field` | `#1e2229`
`viewport_top` / `viewport_bottom` | `#2e3440`
`app_bar_text` (inherited from light) | `#e6eaf2`


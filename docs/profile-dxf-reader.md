# Strict profile DXF reader

```rust,ignore
let geometry = solidcraft_io::read_profile_dxf(text, Some("mm"))?;
```

`read_profile_dxf(&str, Option<&str>) -> solidcraft_io::Result<Vec<Geom2>>`
is a pure, portable text reader. It does not access files, modify a document,
expand blocks, weld points, repair boundaries, or validate connected sections.
The caller must validate closed contours, holes and material connectivity.
Direct DWG and binary DXF are not supported.

## Units and geometry

Results use millimetres. An explicit `mm`, `cm`, `m`, `in` or `ft` overrides
`$INSUNITS`. Without an override, the source must declare one of those units
(DXF codes 4, 5, 6, 1 and 2 respectively). Unitless, missing and unknown units
fail with `profile DXF requires known units or an explicit override`.

Supported entities are XY `LINE`, `ARC`, `CIRCLE`, `LWPOLYLINE` and ordinary
2D `POLYLINE` / `VERTEX` / `SEQEND`. Positive and negative bulges remain circular
arcs, including major arcs and small nonzero bulges. Open polylines remain open;
closed flags add the closing edge. No curve is approximated by a spline.
Validated lines, circles and arcs reuse the generic DXF reader with normalized
input; polyline bulges use their analytic centre formula to avoid the generic
reader's small-bulge approximation and short-edge removal.

## Deliberate limitations

- Rejects INSERT and all nonempty block definitions, including unused definitions.
  Empty layout block metadata is harmless and accepted.
- Requires zero Z/elevation/thickness and the default extrusion `(0, 0, 1)`.
  Nondefault OCS, paper-space geometry, wide/variable-width polylines, fitted,
  3D, mesh and polyface variants are rejected rather than flattened or skipped.
- Rejects unsupported entities, including POINT, TEXT, ELLIPSE and SPLINE.
- Requires complete code/value pairs, valid numeric values, complete sections,
  EOF, mandatory coordinates and unambiguous scalar fields. No permissive skips.
- Limits input to 64 MiB, two million pairs, 200,000 entity records and 200,000
  output primitives. Source numeric magnitudes and scaled coordinates must be
  less than `1e12`; derived bulge centres must be finite and below `1e150`.

Errors use `IoError::Invalid`. Failure returns no partial geometry and cannot
mutate source text or caller state. Generic `read_dxf` and existing frame import
behaviour are unchanged; callers must explicitly select this stricter route.

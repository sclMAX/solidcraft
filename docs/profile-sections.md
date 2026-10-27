# Reusable profile sections

`solidcraft_doc::frame_section::section_from_loops` constructs one material
region from unordered closed loops. It preserves every boundary, normalizing the
exterior counter-clockwise and holes clockwise. Two concentric circles form a
valid tube; three concentric circles are rejected as nested material. Several
disjoint direct child holes are allowed. Disconnected exteriors, crossing or
touching boundaries, self intersections and open loops are errors, not repairs.

`validate_section` must run again after loading a stored `Region2`. It validates
geometry, topology, the designated exterior and winding without mutating input.
Neither API changes sketches or invokes the kernel. Legacy frame and generic DXF
paths are unchanged. These functions accept lines and circular arcs only; other
segment types return `frame section: unsupported segment kind`.

## Resource and numeric contract

Coordinates, circle extents and radii are bounded to 1,000,000 mm. All numbers
must be finite. Radii and line lengths exceed 1e-7 mm; arc sweep magnitudes are
in (1e-7, one turn], and start angles are within two turns of zero. Closure and
boundary contact use a conservative 1e-7 mm tolerance. Signed area magnitude
must exceed 1e-14 square mm. No input coordinates are welded or moved.

At most 512 segments total (and 512 loops) are accepted. This bounds unordered
segment comparisons to 512*511/2 = 130,816, each with constant-size analytic
line/line, line/circle or circle/circle calculations. Containment is quadratic
in this same bounded input size. Circular ray intersections are analytic, split
at vertical extrema, not polyline sampling. These floating-point predicates are
not an arbitrary-precision geometry certificate; near-coincident geometry is
conservatively rejected within the tolerance. Encoded-input byte and allocation
limits remain the caller's responsibility before deserialization.

Errors use `DocError::Invalid` and begin `frame section: `. The pure APIs leave
caller-owned state unchanged on success and failure.

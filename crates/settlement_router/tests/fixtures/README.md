# Settlement wire fixtures

`schema_identities.v1.json` records the original serializer and decoder identities
of all 15 settlement wire owners. `scalar_frames.v1.json` retains complete root,
Option and Vec frames for the two manually encoded time scalars. The timestamp
is 1,700,000,000,000 milliseconds; the signed duration is −37 seconds.

These bytes were captured from unchanged production code with pinned Rust 1.93.1.
The source fingerprint was
`57c2f92d2e42f01e9401197403161bfb39115d32b744f3aa281368afeadd0bca`;
only test probes were appended to the original settlement crate. Both directions
agreed for every owner, and all 27 capture probes passed. The retained local
source, compiler artifacts, commands and logs are under
`target/architecture-redesign/norito-identity-cutover/settlement-reference-capture/`.

The scalar owners keep their distinct nominal frame identities despite storing
the same eight-byte payload width. Tests also cover canonical container bytes,
every frame truncation, wrong frame identity, timestamp range rejection and
signed duration extremes. Capturing fixtures does not qualify node execution.

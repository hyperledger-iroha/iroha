# Configuration scalar frames

`config_scalar_frames.jsonl` retains the original compiler observations for
`DurationMs(42 ms)` and `Bytes(1024)`: nominal identities, both directional hashes
and complete canonical frames. Its SHA-256 is
`103b086c2555a3fdca12b0862a5f29d8254aa6c81adaeedc6e98b0d868f12c63`.

The capture used the original codec on source fingerprint
`78657f2527d66598b1698bba0b6c7042c7e9d09f93041ea58417f5d1b709c6f6`.
Source, compiler artifacts and runtime evidence are retained under ignored
`target/architecture-redesign/norito-identity-cutover/model-owner-reference-capture/`.
The unit tests compare declared identities and complete frames, decode the
captured bytes, and reject truncation, changed owners and trailing bytes.
These fixtures are independent of future crate or module locations.

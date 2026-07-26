# Codec Samples

This directory stores sample JSON files and their Norito-encoded binary counterparts used by the `kagami advanced codec` tool (enable the `codec` feature when running `kagami`).

## Regenerating binaries

Use the codec sample regeneration tool to keep these binaries in sync with their JSON sources:

```bash
cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml --
```

The tool encodes each typed sample with the current Norito implementation and
writes the resulting `.bin` into this directory. Run it whenever the generated
Iroha schema descriptors or a sample type changes.

## Requirements

- Each generated binary must start with the `NRT0` header identifying a Norito archive.
- Each JSON/binary pair must round-trip through its registered Iroha type. The
  generated descriptor inventory in `docs/source/references/schema.json` must
  be regenerated with `kagami advanced schema` and remain non-empty.

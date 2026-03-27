# Codec Samples

This directory stores sample JSON files and their Norito-encoded binary counterparts used by the `kagami advanced codec` tool (enable the `codec` feature when running `kagami`).

## Regenerating binaries

Use the codec sample regeneration tool to keep these binaries in sync with their JSON sources:

```bash
cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml --
```

The tool invokes `kagami advanced codec json-to-norito` for each JSON file and writes the resulting `.bin` into this directory. Run it whenever the JSON schema or the sample JSON changes.

## Requirements

- Each generated binary must start with the `NRT0` header identifying a Norito archive.
- The encoded data must validate against the JSON schema in `docs/source/references/schema.json`.

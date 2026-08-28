# Codec Samples

This directory stores sample JSON files and their Norito-encoded binary counterparts used by the always-available `kagami advanced codec` tool.

## Regenerating binaries

Use the ignored fixture test to keep the registered account/domain binaries in sync with their JSON sources:

```bash
cargo test --locked -p iroha_kagami --test codec regenerate_codec_samples -- --ignored --exact
```

The test encodes each supported typed sample with the current Norito implementation and
writes the resulting `.bin` into this directory. Run it whenever the generated
Iroha schema descriptors or a sample type changes.

## Requirements

- Each generated binary must start with the `NRT0` header identifying a Norito archive.
- Each regenerated account/domain JSON/binary pair must round-trip through its registered Iroha type. The
  generated descriptor inventory in `specs/references/schema.json` must
  be regenerated with `kagami advanced schema` and remain non-empty.

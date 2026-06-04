# ISO 20022 XSD Fixtures

This directory stores offline XSD fixtures used by the ISO 20022 parser and
rail-profile tests.

The files under `iso/` are Standards Editor generated ISO 20022 schema files
mirrored from the Apache-2.0 licensed `moov-io/fedwire20022` repository:

- `pacs.008.001.08.xsd`
- `pacs.009.001.08.xsd`
- `pacs.002.001.10.xsd`
- `pacs.004.001.10.xsd`
- `camt.056.001.08.xsd`

Tests use these full XSDs as stable MDR-derived fixtures for namespace,
`Document` root, and live rail profile admission coverage. Runtime validation
does not fetch schemas over the network.

`fixture_manifest.json` records how checked-in XML fixtures map to the checked-in
XSD corpus, plus reviewed schema-pending exceptions. All checked-in XSDs have
standalone XML fixtures, so `--require-fixture-for-schema` is expected to pass.
Verify the manifest offline with:

```bash
python3 scripts/iso_xsd_fixture_verify.py \
  --manifest fixtures/iso20022/xsd/fixture_manifest.json
```

The verifier is a structural manifest preflight, not a full XSD validator. It
checks XSD target namespaces, `Document` payload roots, XML fixture namespaces,
fixture payload roots, canonical lowercase ISO message definition ids, schema
paths staying under this `xsd/` tree, fixture paths staying under
`fixtures/iso20022/`, and SHA-256 digests. Strict release modes are available:
`--require-schema-backed-fixtures` rejects XML fixtures whose official XSD is not
checked in, and `--require-fixture-for-schema` rejects XSDs without a standalone
checked-in XML fixture. The schema-backed strict mode still fails until the
remaining legacy payment-return, securities, and collateral official XSD
packages are checked in, including the `colr.012.001.05` collateral
substitution confirmation package. The older local `colr.007` collateral
fixture is kept only as a legacy parser fixture and is not part of the
production XSD manifest.

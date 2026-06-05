# ISO 20022 XSD Fixtures

This directory stores offline XSD fixtures used by the ISO 20022 parser and
rail-profile tests.

The files under `iso/` are Standards Editor generated ISO 20022 schema files
mirrored from Apache-2.0 licensed Moov repositories:

- From `moov-io/fedwire20022`: `pacs.008.001.08.xsd`,
  `pacs.009.001.08.xsd`, `pacs.002.001.10.xsd`,
  `pacs.004.001.10.xsd`, and `camt.056.001.08.xsd`
- From `moov-io/iso20022`: `camt.056.001.09.xsd`

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
`fixtures/iso20022/`, SHA-256 digests, and optional offline XSD validation with
`xmllint --nonet`. It also rejects XSD files that contain known restricted
Standards Editor redistribution terms, so candidate public mirrors with
embedded no-redistribution license text must not be imported as release
fixtures. Each checked-in schema entry also carries offline source provenance:
a canonical GitHub repository URL, lowercase commit, source path, SPDX license,
and source SHA-256 that must match the checked-in XSD bytes. It can also read
the embedded default rail profile catalog
from `crates/iroha_core/src/iso_bridge/profiles.rs` with `--profile-catalog` and
record which concrete advertised message versions are backed by checked-in XSD
fixtures. Summaries bind the manifest SHA-256, each schema and fixture SHA-256,
per-schema source provenance, and, when a profile catalog is supplied, both the
profile source-file SHA-256 and embedded catalog JSON SHA-256. Profile catalog
checks fail closed on
non-canonical profile ids, malformed message family ids, unsupported directions,
empty message-profile/version lists, duplicate profile ids, duplicate
profile/message/direction entries, and duplicate concrete versions. Strict
release modes are available:
`--require-schema-backed-fixtures` rejects XML fixtures whose official XSD is not
checked in, `--require-fixture-for-schema` rejects XSDs without a standalone
checked-in XML fixture, `--require-profile-schema-backed-versions` rejects
profile-advertised concrete message versions without schema-backed fixtures, and
`--validate-xml-schema` validates every schema-backed XML fixture against its
checked-in XSD. The schema-backed strict modes still fail until the remaining
profile-advertised payment, securities, and collateral official XSD packages are
checked in, including the `colr.012.001.05` collateral substitution confirmation
package. The older local `colr.007` collateral fixture is kept only as a legacy
parser fixture and is not part of the production XSD manifest.

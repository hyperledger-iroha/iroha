# ISO 20022 XSD Fixtures

This directory stores offline XSD fixtures used by the ISO 20022 parser and
rail-profile tests.

The files under `iso/` are Standards Editor generated ISO 20022 schema files
mirrored from Apache-2.0 licensed upstream repositories:

- From `moov-io/fedwire20022`: `pacs.008.001.08.xsd`,
  `pacs.009.001.08.xsd`, `pacs.002.001.10.xsd`,
  `pacs.004.001.10.xsd`, and `camt.056.001.08.xsd`
- From `moov-io/iso20022`: `camt.056.001.09.xsd`
- From `prog-nov/iso20022-messages-for-go`: `pacs.004.001.09.xsd`

Tests use these full XSDs as stable MDR-derived fixtures for namespace,
`Document` root, and live rail profile admission coverage. Runtime validation
does not fetch schemas over the network.

`fixture_manifest.json` records how checked-in XML fixtures map to the checked-in
XSD corpus, plus reviewed schema-pending exceptions and audited public
candidate sources that are blocked by redistribution terms. The
`blocked_schema_sources` key must be present even when the reviewed blocker list
is intentionally empty, so absence cannot be interpreted as a clean production
gap review. All checked-in XSDs have standalone XML fixtures, so
`--require-fixture-for-schema` is expected to pass. Verify the manifest offline
with:

```bash
python3 scripts/iso_xsd_fixture_verify.py \
  --manifest fixtures/iso20022/xsd/fixture_manifest.json \
  --profile-catalog crates/iroha_core/src/iso_bridge/profiles.rs
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
and source SHA-256 that must match the checked-in XSD bytes. Source repository
coordinates must not use placeholder owners or names such as `example`,
`dummy`, `fake`, `sample`, or `template`, and source/schema/fixture paths must
not carry identifier-style secret-looking material. Schema entries and blocked
candidate entries must record `source` explicitly; omitted `source` and
explicit `source: null` are rejected separately. Blocked source
entries use the same canonical GitHub/path/SHA checks, record audited
restriction markers without checking in the restricted XSD bytes, and must
correspond to a current missing fixture/schema-only gap or, with
`--profile-catalog`, a current missing profile-version gap. The verifier can
also read the embedded default rail profile catalog
from `crates/iroha_core/src/iso_bridge/profiles.rs` with `--profile-catalog` and
record which concrete advertised message versions are backed by checked-in XSD
fixtures. Summaries bind the manifest SHA-256, each schema and fixture SHA-256,
per-schema source provenance, audited blocked-source provenance, and, when a
profile catalog is supplied, both the profile source-file SHA-256 and embedded
catalog JSON SHA-256. The `profile_catalog` key is always recorded; it is
`null` only when no profile catalog was checked, and readiness requires that
explicit state plus the manifest path to remain present. Profile catalog
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
checked in. Remaining blockers include restricted public candidates for
`pacs.002.001.12`, `pacs.008.001.10`, and `pacs.009.001.10`, plus unavailable
redistributable securities and collateral lifecycle packages such as
`sese.023.001.11`, `sese.024.001.10`, `sese.025.001.11`, and
`colr.012.001.05`. The older local `colr.007` collateral fixture is kept only
as a legacy parser fixture and is not part of the production XSD manifest.

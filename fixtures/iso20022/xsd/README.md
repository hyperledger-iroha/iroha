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
XSD corpus. The `blocked_schema_sources` and `pending_schema_sources` keys must
be present even when their reviewed lists are intentionally empty, so absence
cannot be interpreted as a clean production gap review. All checked-in XSDs have
standalone XML fixtures, so `--require-fixture-for-schema` is expected to pass.
Summaries generated from this repository fixture manifest intentionally record
`ok: false`; production evidence must come from an operator-supplied official
MDR/XSD package manifest outside checked-in fixture coordinates.
Verify the manifest offline with:

```bash
python3 scripts/iso_xsd_fixture_verify.py \
  --manifest fixtures/iso20022/xsd/fixture_manifest.json \
  --profile-catalog crates/iroha_core/src/iso_bridge/profiles.rs
```

If `--summary-out` is supplied, the output must be separate from the manifest
and profile-catalog inputs plus every discovered schema or XML fixture file,
including hard-linked aliases, so verifier evidence cannot overwrite or reuse
source material paths. Existing summary-output parents and leaves are
preflighted without creating missing parent directories before manifest loading
and optional `xmllint` validation.

The verifier is a structural manifest preflight, not a full XSD validator. It
checks XSD target namespaces, `Document` payload roots, XML fixture namespaces,
fixture payload roots, canonical lowercase ISO message definition ids, unique
fixture `message_def_id` values, schema paths staying under this `xsd/` tree,
fixture paths staying under `fixtures/iso20022/`, SHA-256 digests, and optional
offline XSD validation with
`xmllint --nonet`. It also rejects XSD files that contain known restricted
Standards Editor redistribution terms, so candidate public mirrors with
embedded no-redistribution license text must not be imported as release
fixtures. Each checked-in schema entry also carries offline source provenance:
a canonical GitHub repository URL, lowercase commit, source path, SPDX license,
and source SHA-256 that must match the checked-in XSD bytes. Source repository
coordinates must not use placeholder owners or names such as `example`,
`dummy`, `fake`, `sample`, or `template`, and source/schema/fixture paths must
not carry identifier-style secret-looking material. Schema entries, blocked
candidate entries, and official pending-source entries must record `source`
explicitly; omitted `source` and explicit `source: null` are rejected
separately. Blocked source entries use the same canonical GitHub/path/SHA
checks, record audited restriction markers without checking in the restricted
XSD bytes, must use unique `message_def_id` values, and must correspond to a
current missing fixture/schema-only gap or, with `--profile-catalog`, a current
missing profile-version gap. The first-release blocked public candidates are
also exact-pinned by `message_def_id` to their audited repository, commit,
source path, SHA-256, and restriction marker list, and message definitions
already tracked as official ISO pending sources cannot be relabelled as
blocked candidates. Pending source entries record official ISO
catalogue or archive coordinates plus the direct `/message/<id>/download` URL
for XSD downloads whose redistributable schema bytes are not checked in yet;
they must use byte-stable official `www.iso20022.org` catalogue and download
paths with no percent escapes, archive catalogue URLs must use the canonical
raw `page=<nonzero decimal>` query, canonical ISO-style message names ending in
`VNN` with suffixes that match their `message_def_id` versions, unique
`message_def_id`, message-name, and direct download URL values, bounded
catalogue/download URLs, canonical submitting organisation labels with bounded
ISO-style punctuation and no URL/contact, semicolon, placeholder, or
path-smuggling material, and a current missing schema/profile gap. XML
fixture entries that record a reviewed
`missing_schema_reason` must not use a `message_def_id` that already has a
checked-in schema; such fixtures must either reference that schema or use a
genuinely missing schema package. The verifier can
also read the embedded default rail profile catalog
from `crates/iroha_core/src/iso_bridge/profiles.rs` with `--profile-catalog` and
record which concrete advertised message versions are backed by checked-in XSD
fixtures. Reviewed `missing_schema_fixtures` and `schema_only_entries` are
replayed in fixture/schema order, and profile-catalog `versions`,
`missing_schema_versions`, and top-level `missing_profile_schema_versions` are
replayed in profile-catalog order, so archived evidence cannot be reordered
without detection. Version-3 summaries also include a unique
`missing_profile_schema_message_ids` gap index with the per-message profile
version count and whether the message definition has reviewed missing-schema,
schema-only, blocked-source, or pending-source evidence, ordered by canonical message
definition id for deterministic replay. They also expose the raw-derived
`unreviewed_profile_schema_message_id_count` and
`unreviewed_profile_schema_message_ids` fields so archived XSD evidence binds
the unique profile message definitions that still have no reviewed gap
evidence. Summaries bind the manifest SHA-256, each schema and fixture SHA-256,
per-schema source provenance, audited blocked-source provenance, audited
pending-source catalogue/download provenance, and, when a
profile catalog is supplied, both the profile source-file SHA-256 and embedded
catalog JSON SHA-256. Final readiness rejects manifest digest reuse across
schema, fixture, blocked-source, profile-catalog source, and profile-catalog JSON
roles. Pending-source probe summaries are versioned separately; version 2 records the
schema-root `target_namespace` extracted from the bounded sample and only treats
a row as reachable when it equals
`urn:iso:std:iso:20022:tech:xsd:<message_def_id>`. Final readiness replays that
binding and recomputes the summary `ok` verdict before pending-source probe
evidence can cover a reviewed gap. The
`profile_catalog` key is always recorded; it is
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
checked-in XSD. For the first release, the checked-in release manifest and the
embedded default rail profile catalog advertise only schema-backed concrete
message definitions. The current checked-in manifest/profile pair passes all
strict XSD gates with 7 schemas, 7 schema-backed XML fixtures, and 31
schema-backed profile-advertised concrete versions, but remains repository test
material and therefore does not produce a production `ok: true` summary.
Broader runtime fixtures
for securities and collateral lifecycle parsing, including `sese.*` and
`colr.*` samples, remain outside the production XSD manifest and default rail
catalog until redistributable official schemas are checked in. Restricted public
candidates such as `pacs.002.001.12`, `pacs.008.001.10`, and
`pacs.009.001.10` are likewise not release-advertised by default profiles. The
older local `colr.007` collateral fixture is kept only as retired negative
parser coverage and is not part of the production XSD manifest.
Final readiness also emits a non-overridable
`xsd.unreviewed_profile_schema_message_ids` blocker for unique
profile-advertised message definitions that have no checked-in schema and no
reviewed missing-schema, schema-only, blocked-source, or pending-source
evidence. The current checked-in corpus has zero unreviewed unique profile
gaps. Readiness derives that blocker from raw profile-version gaps and reviewed
evidence, not from the summary aggregate's own reviewed flags. Readiness
verifies the direct unreviewed-profile count/list in the XSD summary against
those same raw gaps and also exposes the same raw-derived unique list in each
public `xsd_summaries[]` rollup.

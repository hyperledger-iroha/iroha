---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/chunker-profile-authoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c9ee151879e08f7160aba921a3fe4a3f8f217e5b758e94d409a373064ba9ba3
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
id: chunker-profile-authoring
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
:::

# SoraFS chunker profile authoring guide

یہ گائیڈ وضاحت کرتی ہے کہ SoraFS کے لیے نئے chunker profiles کیسے تجویز اور publish کیے جائیں۔
یہ architecture RFC (SF-1) اور registry reference (SF-2a) کو
concrete authoring requirements، validation steps اور proposal templates کے ساتھ مکمل کرتی ہے۔
Canonical مثال کے لیے دیکھیں
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
اور متعلقہ dry-run log
`docs/source/sorafs/reports/sf1_determinism.md` میں۔

## Overview

ہر profile جو registry میں داخل ہوتی ہے اسے یہ کرنا ہوگا:

- deterministic CDC parameters اور multihash settings کو architectures کے درمیان یکساں طور پر advertise کرنا؛
- replayable fixtures (Rust/Go/TS JSON + fuzz corpora + PoR witnesses) فراہم کرنا جنہیں downstream SDKs بغیر bespoke tooling کے verify کر سکیں؛
- council review سے پہلے deterministic diff suite پاس کرنا۔

نیچے دی گئی checklist پر عمل کریں تاکہ ایک ایسا proposal تیار ہو جو ان قواعد پر پورا اترے۔

## Registry charter snapshot

Proposal draft کرنے سے پہلے تصدیق کریں کہ یہ registry charter کے مطابق ہے جسے
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` enforce کرتا ہے:

- Profile IDs مثبت integers ہوتے ہیں جو بغیر gaps کے monotonic طور پر بڑھتے ہیں۔
- کوئی alias کسی دوسرے canonical handle سے collide نہیں کر سکتا اور ایک سے زیادہ بار ظاہر نہیں ہو سکتا۔
- Aliases non-empty ہوں اور whitespace سے trim ہوں۔

Handy CLI helpers:

```bash
# تمام registered descriptors کی JSON listing (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Candidate default profile کے لیے metadata emit کریں (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

یہ commands proposals کو registry charter کے مطابق رکھتے ہیں اور governance discussions کے لیے درکار canonical metadata فراہم کرتے ہیں۔

## Required metadata

| Field | Description | Example (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | متعلقہ profiles کے لیے logical grouping۔ | `sorafs` |
| `name` | human-readable label۔ | `sf1` |
| `semver` | parameter set کے لیے semantic version string۔ | `1.0.0` |
| `profile_id` | Monotonic numeric identifier جو profile کے land ہونے پر assign ہوتا ہے۔ اگلا id reserve کریں مگر موجودہ نمبرز reuse نہ کریں۔ | `1` |
| `profile.min_size` | chunk length کی minimum حد bytes میں۔ | `65536` |
| `profile.target_size` | chunk length کی target حد bytes میں۔ | `262144` |
| `profile.max_size` | chunk length کی maximum حد bytes میں۔ | `524288` |
| `profile.break_mask` | rolling hash کے لیے adaptive mask (hex)۔ | `0x0000ffff` |
| `profile.polynomial` | gear polynomial constant (hex)۔ | `0x3da3358b4dc173` |
| `gear_seed` | 64 KiB gear table derive کرنے کے لیے seed۔ | `sorafs-v1-gear` |
| `chunk_multihash.code` | per-chunk digests کے لیے multihash code۔ | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | canonical fixtures bundle کا digest۔ | `13fa...c482` |
| `fixtures_root` | regenerated fixtures رکھنے والی relative directory۔ | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | deterministic PoR sampling کے لیے seed (`splitmix64`)۔ | `0xfeedbeefcafebabe` (example) |

Metadata کو proposal document اور generated fixtures دونوں میں شامل ہونا چاہیے تاکہ registry، CLI tooling اور governance automation بغیر manual cross-referencing کے values confirm کر سکیں۔ اگر شک ہو تو chunk-store اور manifest CLIs کو `--json-out=-` کے ساتھ چلائیں تاکہ computed metadata review notes میں stream ہو سکے۔

### CLI اور registry touchpoints

- `sorafs_manifest_chunk_store --profile=<handle>` — proposed parameters کے ساتھ chunk metadata، manifest digest اور PoR checks دوبارہ چلائیں۔
- `sorafs_manifest_chunk_store --json-out=-` — chunk-store report کو stdout پر stream کریں تاکہ automated comparisons ہو سکیں۔
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirm کریں کہ manifests اور CAR plans canonical handle اور aliases embed کرتے ہیں۔
- `sorafs_manifest_stub --plan=-` — پچھلا `chunk_fetch_specs` واپس feed کریں تاکہ change کے بعد offsets/digests verify ہوں۔

Command output (digests, PoR roots, manifest hashes) کو proposal میں ریکارڈ کریں تاکہ reviewers انہیں verbatim reproduce کر سکیں۔

## Determinism & validation checklist

1. **Fixtures regenerate کریں**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Parity suite چلائیں** — `cargo test -p sorafs_chunker` اور cross-language diff harness (`crates/sorafs_chunker/tests/vectors.rs`) نئے fixtures کے ساتھ green ہونا چاہیے۔
3. **Fuzz/back-pressure corpora replay کریں** — `cargo fuzz list` اور streaming harness (`fuzz/sorafs_chunker`) کو regenerated assets کے خلاف چلائیں۔
4. **Proof-of-Retrievability witnesses verify کریں** — `sorafs_manifest_chunk_store --por-sample=<n>` proposed profile کے ساتھ چلائیں اور roots کو fixture manifest سے match کریں۔
5. **CI dry run** — `ci/check_sorafs_fixtures.sh` لوکل چلائیں؛ script کو نئے fixtures اور موجودہ `manifest_signatures.json` کے ساتھ succeed ہونا چاہیے۔
6. **Cross-runtime confirmation** — یقینی بنائیں کہ Go/TS bindings regenerated JSON consume کریں اور identical chunk boundaries اور digests emit کریں۔

Commands اور resulting digests کو proposal میں دستاویزی کریں تاکہ Tooling WG بغیر guesswork کے انہیں دوبارہ چلا سکے۔

### Manifest / PoR confirmation

Fixtures regenerate کرنے کے بعد مکمل manifest pipeline چلائیں تاکہ CAR metadata اور PoR proofs consistent رہیں:

```bash
# نئے profile کے ساتھ chunk metadata + PoR validate کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# manifest + CAR generate کریں اور chunk fetch specs capture کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# محفوظ fetch plan کے ساتھ دوبارہ چلائیں (stale offsets سے بچاتا ہے)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Input file کو اپنے fixtures میں استعمال ہونے والے کسی representative corpus سے بدلیں
(مثلاً 1 GiB deterministic stream) اور resulting digests کو proposal کے ساتھ attach کریں۔

## Proposal template

Proposals کو `ChunkerProfileProposalV1` Norito records کے طور پر `docs/source/sorafs/proposals/` میں check in کیا جاتا ہے۔ نیچے JSON template expected شکل دکھاتا ہے (اپنی values سے replace کریں):


Matching Markdown report (`determinism_report`) فراہم کریں جس میں command output، chunk digests اور validation کے دوران پائی گئی deviations شامل ہوں۔

## Governance workflow

1. **Proposal + fixtures کے ساتھ PR submit کریں۔** Generated assets، Norito proposal، اور `chunker_registry_data.rs` updates شامل کریں۔
2. **Tooling WG review۔** Reviewers validation checklist دوبارہ چلاتے ہیں اور confirm کرتے ہیں کہ proposal registry rules کے مطابق ہے (id reuse نہیں، determinism satisfied)۔
3. **Council envelope۔** Approve ہونے کے بعد council members proposal digest (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) پر sign کرتے ہیں اور signatures کو profile envelope میں append کرتے ہیں جو fixtures کے ساتھ رکھا جاتا ہے۔
4. **Registry publish۔** Merge سے registry، docs اور fixtures update ہوتے ہیں۔ Default CLI پچھلے profile پر رہتا ہے جب تک governance migration کو ready قرار نہ دے۔

## Authoring tips

- Power-of-two کی even bounds ترجیح دیں تاکہ edge-case chunking behavior کم ہو۔
- Gear table seeds کو human-readable مگر globally unique رکھیں تاکہ audit trails آسان ہوں۔
- Benchmarking artifacts (مثلاً throughput comparisons) کو `docs/source/sorafs/reports/` میں محفوظ کریں تاکہ مستقبل میں reference ہو سکے۔

Rollout کے دوران operational expectations کے لیے migration ledger دیکھیں
(`docs/source/sorafs/migration_ledger.md`)۔ Runtime conformance rules کے لیے
`docs/source/sorafs/chunker_conformance.md` دیکھیں۔

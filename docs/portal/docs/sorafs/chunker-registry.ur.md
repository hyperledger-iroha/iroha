<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2895818612da6e0afbeb5bb44da3a6ecef6cab0aefd401ed79b4236df1dd9d18
source_last_modified: "2025-11-08T20:22:25.757062+00:00"
translation_last_reviewed: 2025-12-29
---

---
id: chunker-registry
title: SoraFS chunker profile registry
sidebar_label: Chunker registry
description: SoraFS chunker registry کے لیے profile IDs، parameters اور negotiation plan۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/chunker_registry.md` کی عکاسی کرتا ہے۔ جب تک legacy Sphinx documentation set مکمل طور پر ریٹائر نہ ہو جائے دونوں نقول کو ہم آہنگ رکھیں۔
:::

## SoraFS chunker profile registry (SF-2a)

SoraFS stack chunking behavior کو ایک چھوٹے namespaced registry کے ذریعے negotiate کرتا ہے۔
ہر profile deterministic CDC parameters، semver metadata اور expected digest/multicodec assign کرتا ہے جو manifests اور CAR archives میں استعمال ہوتا ہے۔

Profile authors کو
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
میں مطلوبہ metadata، validation checklist اور proposal template دیکھنا چاہیے قبل اس کے کہ وہ نئی entries submit کریں۔
جب governance تبدیلی approve کر دے تو
[registry rollout checklist](./chunker-registry-rollout-checklist.md) اور
[staging manifest playbook](./staging-manifest-playbook) کے مطابق fixtures کو staging اور production میں promote کریں۔

### Profiles

| Namespace | Name | SemVer | Profile ID | Min (bytes) | Target (bytes) | Max (bytes) | Break mask | Multihash | Aliases | Notes |
|-----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 fixtures میں استعمال ہونے والا canonical profile |

Registry code میں `sorafs_manifest::chunker_registry` کے طور پر موجود ہے (جسے [`chunker_registry_charter.md`](./chunker-registry-charter.md) govern کرتا ہے)۔ ہر entry ایک `ChunkerProfileDescriptor` کے طور پر ظاہر ہوتی ہے جس میں:

* `namespace` – متعلقہ profiles کی logical grouping (مثلاً `sorafs`)۔
* `name` – انسان کے لیے readable profile label (`sf1`, `sf1-fast`, …)۔
* `semver` – parameter set کے لیے semantic version string۔
* `profile` – اصل `ChunkProfile` (min/target/max/mask)۔
* `multihash_code` – chunk digests بناتے وقت استعمال ہونے والا multihash (`0x1f`
  SoraFS default کے لیے)۔

Manifest `ChunkingProfileV1` کے ذریعے profiles کو serialize کرتا ہے۔ یہ structure registry metadata
(namespace, name, semver) کو raw CDC parameters اور اوپر دکھائی گئی alias list کے ساتھ record کرتا ہے۔
Consumers کو پہلے `profile_id` کے ذریعے registry lookup کرنا چاہیے اور اگر unknown IDs آئیں تو inline parameters پر fallback کرنا چاہیے؛
alias list اس بات کو یقینی بناتی ہے کہ HTTP clients `Accept-Chunker` میں legacy handles guessing کے بغیر بھیج سکیں۔
Registry charter rules کے مطابق canonical handle (`namespace.name@semver`) کو `profile_aliases` میں پہلی entry ہونا چاہیے، اور اس کے بعد کوئی بھی legacy alias آتا ہے۔

Registry کو tooling سے inspect کرنے کے لیے helper CLI چلائیں:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

CLI کے وہ تمام flags جو JSON لکھتے ہیں (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) path کے طور پر `-` قبول کرتے ہیں، جس سے payload stdout پر stream ہوتا ہے بجائے فائل بنانے کے۔
یہ tooling میں data pipe کرنا آسان بناتا ہے جبکہ main report کو پرنٹ کرنے والا default behavior برقرار رہتا ہے۔

### Compatibility matrix & rollout plan


نیچے دی گئی جدول `sorafs.sf1@1.0.0` کے لیے core components میں موجودہ support status دکھاتی ہے۔ "Bridge" سے مراد CARv1 + SHA-256 compatibility lane ہے جس کے لیے explicit client negotiation (`Accept-Chunker` + `Accept-Digest`) درکار ہے۔

| Component | Status | Notes |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Supported | Canonical handle + aliases validate کرتا ہے، `--json-out=-` کے ذریعے reports stream کرتا ہے، اور `ensure_charter_compliance()` کے ذریعے registry charter enforce کرتا ہے۔ |
| `sorafs_manifest_stub` | ⚠️ Legacy | Legacy manifest builder؛ CAR/manifest packaging کے لیے `iroha sorafs toolkit pack` استعمال کریں اور deterministic revalidation کے لیے `--plan=-` رکھیں۔ |
| `sorafs_provider_advert_stub` | ⚠️ Legacy | صرف offline validation helper؛ provider adverts کو publishing pipeline میں بنائیں اور `/v1/sorafs/providers` کے ذریعے validate کریں۔ |
| `sorafs_fetch` (developer orchestrator) | ✅ Supported | `chunk_fetch_specs` پڑھتا ہے، `range` capability payloads سمجھتا ہے، اور CARv2 output assemble کرتا ہے۔ |
| SDK fixtures (Rust/Go/TS) | ✅ Supported | `export_vectors` کے ذریعے regenerate ہوتی ہیں؛ canonical handle ہر alias list میں پہلے آتا ہے اور council envelopes کے ذریعے signed ہوتا ہے۔ |
| Torii gateway profile negotiation | ✅ Supported | `Accept-Chunker` grammar مکمل طور پر implement کرتا ہے، `Content-Chunker` headers شامل کرتا ہے، اور CARv1 bridge صرف explicit downgrade requests پر expose کرتا ہے۔ |
| CARv1 bridge (`sha2-256`) | ⚠️ Transitional | Legacy clients کے لیے دستیاب جب request canonical profile اور `Accept-Digest: sha2-256` دونوں advertise کرے؛ responses میں `Content-Chunker: ...;legacy=true` شامل ہوتا ہے۔ |

Telemetry rollout:

- **Chunk fetch telemetry** — Iroha CLI `sorafs toolkit pack` chunk digests، CAR metadata اور PoR roots emit کرتا ہے تاکہ dashboards میں ingest ہو سکیں۔
- **Provider adverts** — advert payloads capability اور alias metadata شامل کرتے ہیں؛ `/v1/sorafs/providers` کے ذریعے coverage validate کریں (مثلاً `range` capability کی موجودگی)۔
- **Gateway monitoring** — operators کو `Content-Chunker`/`Content-Digest` pairings report کرنی چاہیے تاکہ unexpected downgrades detect ہوں؛ bridge usage سے توقع ہے کہ deprecation سے پہلے صفر کی طرف جائے گا۔

Deprecation policy: جب کوئی successor profile ratify ہو جائے تو dual-publish window schedule کریں (proposal میں documented) قبل اس کے کہ `sorafs.sf1@1.0.0` کو registry میں deprecated mark کیا جائے اور production gateways سے CARv1 bridge ہٹا دیا جائے۔

کسی مخصوص PoR witness کو inspect کرنے کے لیے chunk/segment/leaf indices دیں اور چاہیں تو proof کو disk پر persist کریں:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

آپ numeric id (`--profile-id=1`) یا registry handle (`--profile=sorafs.sf1@1.0.0`) کے ذریعے profile منتخب کر سکتے ہیں؛ handle والی شکل scripts کے لیے آسان ہے جو governance metadata سے namespace/name/semver براہ راست پاس کرتی ہیں۔

`--promote-profile=<handle>` استعمال کریں تاکہ JSON metadata block emit ہو (تمام registered aliases سمیت) جسے نئے default profile کو promote کرتے وقت `chunker_registry_data.rs` میں paste کیا جا سکے:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Main report (اور optional proof file) میں root digest، sampled leaf bytes (hex-encoded) اور segment/chunk sibling digests شامل ہوتے ہیں تاکہ verifiers 64 KiB/4 KiB layers کو `por_root_hex` value کے خلاف rehash کر سکیں۔

کسی موجودہ proof کو payload کے خلاف validate کرنے کے لیے path کو
`--por-proof-verify` کے ذریعے دیں (CLI `"por_proof_verified": true` تب add کرتا ہے جب witness computed root سے match کرے):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Batch sampling کے لیے `--por-sample=<count>` استعمال کریں اور optional seed/output path دیں۔ CLI deterministic ordering (`splitmix64` seeded) guarantee کرتا ہے اور جب request available leaves سے زیادہ ہو تو transparently truncate کرتا ہے:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub یہی data mirror کرتا ہے، جو pipelines میں `--chunker-profile-id` selection کو script کرنے کے لیے convenient ہے۔ دونوں chunk store CLIs canonical handle form (`--profile=sorafs.sf1@1.0.0`) بھی accept کرتے ہیں تاکہ build scripts numeric IDs hard-code کرنے سے بچ سکیں:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

`handle` field (`namespace.name@semver`) وہی ہے جو CLIs `--profile=…` کے ذریعے accept کرتے ہیں، اس لیے اسے automation میں براہ راست copy کرنا محفوظ ہے۔

### Negotiating chunkers

Gateways اور clients provider adverts کے ذریعے supported profiles advertise کرتے ہیں:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (registry کے ذریعے implicit)
    capabilities: [...]
}
```

Multi-source chunk scheduling `range` capability کے ذریعے announce ہوتی ہے۔ CLI اسے `--capability=range[:streams]` کے ساتھ accept کرتا ہے، جہاں optional numeric suffix provider کی preferred range-fetch concurrency encode کرتا ہے (مثلاً `--capability=range:64` 64-stream budget advertise کرتا ہے)۔ جب یہ omit ہو تو consumers advert میں کہیں اور شائع شدہ general `max_streams` hint پر fallback کرتے ہیں۔

CAR data request کرتے وقت clients کو `Accept-Chunker` header بھیجنا چاہیے جو preference order میں `(namespace, name, semver)` tuples list کرے:

```
Accept-Chunker: sorafs.sf1;version=1.0.0, legacy.fastcdc;version=0.9.0
```

Gateways mutually supported profile منتخب کرتے ہیں (default `sorafs.sf1@1.0.0`) اور فیصلہ `Content-Chunker` response header کے ذریعے reflect کرتے ہیں۔ Manifests منتخب profile embed کرتے ہیں تاکہ downstream nodes HTTP negotiation پر انحصار کیے بغیر chunk layout validate کر سکیں۔

### CAR compatibility

Canonical manifest envelope CIDv1 roots کے ساتھ `dag-cbor` (`0x71`) استعمال کرتا ہے۔ Legacy compatibility کے لیے ہم CARv1+SHA-2 export path برقرار رکھتے ہیں:

* **Primary path** – CARv2، BLAKE3 payload digest (`0x1f` multihash)،
  `MultihashIndexSorted`، اور chunk profile اوپر کے مطابق record ہوتا ہے۔
* **Legacy bridge** – CARv1، SHA-256 payload digest (`0x12` multihash)۔ Servers اس variant کو تب surface کر سکتے ہیں جب client `Accept-Chunker` omit کرے یا `Accept-Digest: sha2-256` request کرے۔

Manifests ہمیشہ CARv2/BLAKE3 commitment advertise کرتے ہیں۔ Legacy lanes compatibility کے لیے اضافی headers دیتے ہیں لیکن canonical digest کو replace نہیں کرنا چاہیے۔

### Conformance

* `sorafs.sf1@1.0.0` profile public fixtures (`fixtures/sorafs_chunker`) اور `fuzz/sorafs_chunker` کے تحت register corpora سے match کرتا ہے۔ End-to-end parity Rust، Go اور Node میں دیے گئے tests سے exercise کی جاتی ہے۔
* `chunker_registry::lookup_by_profile` assert کرتا ہے کہ descriptor parameters `ChunkProfile::DEFAULT` سے match کریں تاکہ accidental divergence سے بچا جا سکے۔
* `iroha sorafs toolkit pack` اور `sorafs_manifest_stub` سے بنے manifests میں registry metadata شامل ہوتی ہے۔

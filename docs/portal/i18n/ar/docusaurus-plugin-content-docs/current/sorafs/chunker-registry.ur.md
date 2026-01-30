---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-registry
title: SoraFS chunker profile registry
sidebar_label: Chunker registry
description: SoraFS chunker registry کے لیے profile IDs، parameters اور negotiation plan۔
---

:::note مستند ماخذ
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

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub یہی data mirror کرتا ہے، جو pipelines میں `--chunker-profile-id` selection کو script کرنے کے لیے convenient ہے۔ دونوں chunk store CLIs canonical handle form (`--profile=sorafs.sf1@1.0.0`) بھی accept کرتے ہیں تاکہ build scripts numeric IDs hard-code کرنے سے بچ سکیں:

```
```

`handle` field (`namespace.name@semver`) وہی ہے جو CLIs `--profile=…` کے ذریعے accept کرتے ہیں، اس لیے اسے automation میں براہ راست copy کرنا محفوظ ہے۔

### Negotiating chunkers

Gateways اور clients provider adverts کے ذریعے supported profiles advertise کرتے ہیں:

```
```

Multi-source chunk scheduling `range` capability کے ذریعے announce ہوتی ہے۔ CLI اسے `--capability=range[:streams]` کے ساتھ accept کرتا ہے، جہاں optional numeric suffix provider کی preferred range-fetch concurrency encode کرتا ہے (مثلاً `--capability=range:64` 64-stream budget advertise کرتا ہے)۔ جب یہ omit ہو تو consumers advert میں کہیں اور شائع شدہ general `max_streams` hint پر fallback کرتے ہیں۔

CAR data request کرتے وقت clients کو `Accept-Chunker` header بھیجنا چاہیے جو preference order میں `(namespace, name, semver)` tuples list کرے:

```

Gateways mutually supported profile منتخب کرتے ہیں (default `sorafs.sf1@1.0.0`) اور فیصلہ `Content-Chunker` response header کے ذریعے reflect کرتے ہیں۔ Manifests منتخب profile embed کرتے ہیں تاکہ downstream nodes HTTP negotiation پر انحصار کیے بغیر chunk layout validate کر سکیں۔



* **Primary path** – CARv2، BLAKE3 payload digest (`0x1f` multihash)،
  `MultihashIndexSorted`، اور chunk profile اوپر کے مطابق record ہوتا ہے۔


### Conformance

* `sorafs.sf1@1.0.0` profile public fixtures (`fixtures/sorafs_chunker`) اور `fuzz/sorafs_chunker` کے تحت register corpora سے match کرتا ہے۔ End-to-end parity Rust، Go اور Node میں دیے گئے tests سے exercise کی جاتی ہے۔
* `chunker_registry::lookup_by_profile` assert کرتا ہے کہ descriptor parameters `ChunkProfile::DEFAULT` سے match کریں تاکہ accidental divergence سے بچا جا سکے۔
* `iroha app sorafs toolkit pack` اور `sorafs_manifest_stub` سے بنے manifests میں registry metadata شامل ہوتی ہے۔

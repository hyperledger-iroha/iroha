---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/chunker-conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38fe05ff8e9a85d6742f48a7cfa2002078a016652d0562b0356923a9e3816e16
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
id: chunker-conformance
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
:::

یہ regeneration workflow، signing policy، اور verification steps بھی document کرتی ہے تاکہ SDKs میں fixture consumers sync میں رہیں۔

## Canonical profile

- Input seed (hex): `0000000000dec0ded`
- Target size: 262144 bytes (256 KiB)
- Minimum size: 65536 bytes (64 KiB)
- Maximum size: 524288 bytes (512 KiB)
- Rolling polynomial: `0x3DA3358B4DC173`
- Gear table seed: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

Reference implementation: `sorafs_chunker::chunk_bytes_with_digests_profile`.
کسی بھی SIMD acceleration کو یکساں boundaries اور digests پیدا کرنے چاہئیں۔

## Fixture bundle

`cargo run --locked -p sorafs_chunker --bin export_vectors` fixtures کو regenerate
کرتا ہے اور درج ذیل فائلیں `fixtures/sorafs_chunker/` میں بناتا ہے:

- `sf1_profile_v1.{json,rs,ts,go}` — Rust, TypeScript, اور Go consumers کے لیے canonical chunk boundaries۔ ہر فائل
  آتے ہیں (مثلاً `sorafs.sf1@1.0.0`، پھر `sorafs.sf1@1.0.0`)۔ یہ ordering
  `ensure_charter_compliance` کے ذریعے enforce ہوتی ہے اور اسے تبدیل نہیں کیا جا سکتا۔
- `manifest_blake3.json` — BLAKE3-verified manifest جو ہر fixture فائل کو cover کرتا ہے۔
- `manifest_signatures.json` — manifest digest پر council signatures (Ed25519)۔
- `sf1_profile_v1_backpressure.json` اور `fuzz/` کے اندر raw corpora —
  deterministic streaming scenarios جو chunker back-pressure tests میں استعمال ہوتے ہیں۔

### Signing policy

Fixture regeneration **لازم** طور پر valid council signature شامل کرے۔ generator unsigned output کو reject کرتا ہے جب تک `--allow-unsigned` واضح طور پر نہ دیا جائے (صرف مقامی تجربات کے لیے)۔ Signature envelopes append-only ہوتے ہیں اور signer کے لحاظ سے deduplicate ہوتے ہیں۔

Council signature شامل کرنے کے لیے:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verification

CI helper `ci/check_sorafs_fixtures.sh` generator کو `--locked` کے ساتھ دوبارہ چلاتا ہے۔
اگر fixtures میں drift ہو یا signatures missing ہوں تو job fail ہو جاتا ہے۔ اس script کو
nightly workflows میں اور fixtures changes submit کرنے سے پہلے استعمال کریں۔

Manual verification steps:

1. `cargo test -p sorafs_chunker` چلائیں۔
2. `ci/check_sorafs_fixtures.sh` لوکل چلائیں۔
3. تصدیق کریں کہ `git status -- fixtures/sorafs_chunker` صاف ہے۔

## Upgrade playbook

نیا chunker profile propose کرتے وقت یا SF1 اپڈیٹ کرتے وقت:

یہ بھی دیکھیں: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) تاکہ
metadata requirements، proposal templates اور validation checklists مل سکیں۔

1. نئے parameters کے ساتھ `ChunkProfileUpgradeProposalV1` (RFC SF-1 دیکھیں) تیار کریں۔
2. `export_vectors` کے ذریعے fixtures regenerate کریں اور نیا manifest digest ریکارڈ کریں۔
3. مطلوبہ council quorum کے ساتھ manifest sign کریں۔ تمام signatures
   `manifest_signatures.json` میں append ہونی چاہئیں۔
4. متاثرہ SDK fixtures (Rust/Go/TS) اپڈیٹ کریں اور cross-runtime parity یقینی بنائیں۔
5. اگر parameters بدلیں تو fuzz corpora regenerate کریں۔
6. اس گائیڈ میں نیا profile handle، seeds، اور digest اپڈیٹ کریں۔
7. تبدیلی کو اپڈیٹڈ tests اور roadmap updates کے ساتھ submit کریں۔

اگر اس عمل کے بغیر chunk boundaries یا digests تبدیل کیے جائیں تو وہ invalid ہیں اور merge نہیں ہونے چاہئیں۔

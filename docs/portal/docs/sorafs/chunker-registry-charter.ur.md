---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1200f94dd0dec510a4578581abd7b31e313d53eb7697b8f1d300b3e378c51c37
source_last_modified: "2025-11-08T20:16:30.133276+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: chunker-registry-charter
title: SoraFS chunker registry charter
sidebar_label: Chunker registry charter
description: Chunker profile submissions اور approvals کے لیے governance charter۔
---

:::note مستند ماخذ
:::

# SoraFS chunker registry governance charter

> **Ratified:** 2025-10-29 by the Sora Parliament Infrastructure Panel (see
> `docs/source/sorafs/council_minutes_2025-10-29.md`). کسی بھی ترمیم کے لیے باضابطہ governance ووٹ درکار ہے؛
> implementation ٹیموں کو اس دستاویز کو اس وقت تک normative سمجھنا ہوگا جب تک کوئی نئی charter منظور نہ ہو۔

یہ charter SoraFS chunker registry کو evolve کرنے کے لیے process اور roles define کرتی ہے۔
یہ [Chunker Profile Authoring Guide](./chunker-profile-authoring.md) کی تکمیل کرتی ہے اور یہ بیان کرتی ہے کہ نئے
profiles کیسے propose، review، ratify اور بالآخر deprecate ہوتے ہیں۔

## Scope

یہ charter `sorafs_manifest::chunker_registry` کی ہر entry پر لاگو ہے اور
ہر اس tooling پر بھی جو registry استعمال کرتا ہے (manifest CLI, provider-advert CLI,
SDKs). یہ alias اور handle invariants enforce کرتی ہے جنہیں
`chunker_registry::ensure_charter_compliance()` چیک کرتا ہے:

- Profile IDs مثبت integers ہوتے ہیں جو monotonic طور پر بڑھتے ہیں۔
- Canonical handle `namespace.name@semver` **لازم** ہے کہ `profile_aliases` میں پہلی
- Alias strings trim کی جاتی ہیں، unique ہوتی ہیں، اور دوسری entries کے canonical handles سے collide نہیں کرتیں۔

## Roles

- **Author(s)** – proposal تیار کرتے ہیں، fixtures regenerate کرتے ہیں، اور determinism evidence جمع کرتے ہیں۔
- **Tooling Working Group (TWG)** – published checklists کے ذریعے proposal validate کرتا ہے اور registry invariants برقرار رکھتا ہے۔
- **Governance Council (GC)** – TWG report کا review کرتا ہے، proposal envelope پر sign کرتا ہے، اور publication/deprecation timelines approve کرتا ہے۔
- **Storage Team** – registry implementation maintain کرتا ہے اور documentation updates publish کرتا ہے۔

## Lifecycle workflow

1. **Proposal submission**
   - Author authoring guide کی validation checklist چلاتا ہے اور
     `docs/source/sorafs/proposals/` کے تحت `ChunkerProfileProposalV1` JSON بناتا ہے۔
   - CLI output شامل کریں:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Fixtures، proposal، determinism report، اور registry updates پر مشتمل PR submit کریں۔

2. **Tooling review (TWG)**
   - Validation checklist دوبارہ چلائیں (fixtures, fuzz, manifest/PoR pipeline).
   - `cargo test -p sorafs_car --chunker-registry` چلائیں اور یقینی بنائیں کہ
     `ensure_charter_compliance()` نئی entry کے ساتھ پاس ہو۔
   - CLI behavior (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) کی توثیق کریں کہ یہ updated aliases اور handles دکھاتا ہے۔
   - Findings اور pass/fail status پر مشتمل مختصر رپورٹ تیار کریں۔

3. **Council approval (GC)**
   - TWG report اور proposal metadata کا review کریں۔
   - Proposal digest (`blake3("sorafs-chunker-profile-v1" || bytes)`) پر sign کریں اور
     signatures کو council envelope میں شامل کریں جو fixtures کے ساتھ رکھا جاتا ہے۔
   - Vote outcome کو governance minutes میں ریکارڈ کریں۔

4. **Publication**
   - PR merge کریں، اور اپڈیٹ کریں:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentation (`chunker_registry.md`, authoring/conformance guides).
     - Fixtures اور determinism reports.
   - Operators اور SDK teams کو نئے profile اور planned rollout کے بارے میں اطلاع دیں۔

5. **Deprecation / Sunset**
   - جو proposals موجودہ profile کو replace کرتی ہیں ان میں dual-publish window
     (grace periods) اور upgrade plan شامل ہونا چاہیے۔
     اور migration ledger کو update کریں۔

6. **Emergency changes**
   - Removal یا hotfixes کے لیے council vote اور اکثریتی approval درکار ہے۔
   - TWG کو risk mitigation steps document کرنے اور incident log update کرنے ہوں گے۔

## Tooling expectations

- `sorafs_manifest_chunk_store` اور `sorafs_manifest_stub` expose کرتے ہیں:
  - Registry inspection کے لیے `--list-profiles`.
  - Profile promote کرتے وقت canonical metadata block بنانے کے لیے `--promote-profile=<handle>`.
  - Reports کو stdout پر stream کرنے کے لیے `--json-out=-`، تاکہ reproducible review logs ممکن ہوں۔
- `ensure_charter_compliance()` relevant binaries کے startup پر چلایا جاتا ہے
  (`manifest_chunk_store`, `provider_advert_stub`). CI tests کو fail ہونا چاہیے اگر
  نئی entries charter کی خلاف ورزی کریں۔

## Record keeping

- تمام determinism reports کو `docs/source/sorafs/reports/` میں محفوظ کریں۔
- Chunker فیصلوں کا حوالہ دینے والی council minutes
  `docs/source/sorafs/migration_ledger.md` میں موجود ہیں۔
- ہر بڑے registry change کے بعد `roadmap.md` اور `status.md` اپڈیٹ کریں۔

## References

- Authoring guide: [Chunker Profile Authoring Guide](./chunker-profile-authoring.md)
- Conformance checklist: `docs/source/sorafs/chunker_conformance.md`
- Registry reference: [Chunker Profile Registry](./chunker-registry.md)

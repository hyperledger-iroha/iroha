---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 74b0ef4843c441003cd6630f35e0deac4a736adad450270047a739c1b1d0a6fc
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: SNNet-9 compliance policies کو attestation digests اور evidence capture کے ساتھ activate کرنے کی checklist.
---

SNNet-9 compliance configuration کو repeatable اور audit-friendly طریقے سے roll out کرنے کے لئے یہ brief استعمال کریں۔ jurisdictional review کے ساتھ اسے جوڑیں تاکہ ہر operator وہی digests اور evidence layout استعمال کرے۔

## Steps

1. **Assemble config**
   - `governance/compliance/soranet_opt_outs.json` import کریں۔
   - اپنی `operator_jurisdictions` کو attestation digests کے ساتھ merge کریں جو
     [jurisdictional review](gar-jurisdictional-review) میں شائع ہیں۔
2. **Validate**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Optional: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Capture evidence**
   - `artifacts/soranet/compliance/<YYYYMMDD>/` کے تحت محفوظ کریں:
     - `config.json` (final compliance block)
     - `attestations.json` (URIs + digests)
     - validation logs
     - signed PDFs/Norito envelopes کے references
4. **Activate**
   - rollout کو tag کریں (`gar-opt-out-<date>`)، orchestrator/SDK configs redeploy کریں،
     اور تصدیق کریں کہ `compliance_*` events متوقع logs میں ظاہر ہو رہے ہیں۔
5. **Close out**
   - evidence bundle کو Governance Council کے ساتھ فائل کریں۔
   - activation window اور approvers کو GAR logbook میں درج کریں۔
   - jurisdictional review table سے next-review dates schedule کریں۔

## Quick checklist

- [ ] `jurisdiction_opt_outs` canonical catalogue سے match کرتا ہے۔
- [ ] Attestation digests بالکل درست کاپی ہوئے۔
- [ ] Validation commands چلائے گئے اور محفوظ ہوئے۔
- [ ] Evidence bundle `artifacts/soranet/compliance/<date>/` میں محفوظ ہے۔
- [ ] Rollout tag اور GAR logbook اپ ڈیٹ ہیں۔
- [ ] Next-review reminders سیٹ ہیں۔

## See also

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)

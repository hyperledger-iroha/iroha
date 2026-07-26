---
lang: dz
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 ཡུ་རོབ་མཐུན་ཚོགས་ཀྱི་མིང་རྟགས་ཨོཕ་མེམོ་ — ༢༠༢༦.༡ ཇི་ཨེ་ (ཨེན་ཌོའིཌ་ཨེསི་ཌི་ཀེ་)

## བཅུད༌བསྡུ

- **གསར་བཏོན་ / རླངས་འཁོར་:** ༢༠༢༦.༡ ཇི་ཨེ་ (ཨེན་ཌོའིཌ་ཨེསི་ཌི་ཀེ་)
- **བསྐྱར་ཞིབ་ཚེས་གྲངས་:** ༢༠༢༦-༠༤-༡༥
- **གྲོས་དཔོན་ / བསྐྱར་ཞིབ་པ་:** སོ་ཕི་ཡ་མར་ཊིན་ — མཐུན་སྒྲིག་དང་ཁྲིམས་ལུགས།
- **Scope:** ETN 319 401 ཉེན་སྲུང་དམིགས་ཚད། GDPR DPIA བཅུད་བསྡུས་, SBOM བདེན་དཔང་, AND6 ཐབས་འཕྲུལ་-བརྟག་དཔྱད་ཁང་།
- **འབྲེལ་མཐུད་ཅན་གྱི་ཤོག་འཛིན་:** `_android-device-lab` / AND6-DR-202602, AND6 གཞུང་སྐྱོང་རྗེས་འདེད་ (`GOV-AND6-2026Q1`)

## ཅ་རྙིང་དཔྱད་གཞི།

| ཅ་ཆས། | SHA-256 | ས་གནས་ / འབྲེལ་མཐུད། | དྲན་ཐོ། |
|--------------------------------------------------------------------------------------
| Sigstore | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | མཐུན་སྒྲིག་ ༢༠༢༦.༡ ཇི་ཨེ་ གསར་བཏོན་ངོས་འཛིན་དང་ ཉེན་བརྡའི་དཔེ་ཚད་ ཌེལ་ཊས་ (Torii NRPC ཁ་སྐོང་)། |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | གཞི་བསྟུན་ AND7 བརྒྱུད་འཕྲིན་སྲིད་བྱུས་ (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore བཱན་ཌལ་ (I `android-sdk-release#4821`). | CycloneDX + འབྱུང་ཁུངས་བསྐྱར་ཞིབ་འབད་ཡོདཔ། matches བུའིཊ་ལཱ་ `android-sdk-release#4821`. |
| སྒྲུབ་བྱེད་དྲན་ཐོ། | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (གྲལ་ཐིག་ `android-device-lab-failover-20260220`) | དྲན་ཐོ་ཚུ་ བཟུང་ཡོད་པའི་ བཱན་ཌལ་ཧེ་ཤེ་ + ཤོང་ཚད་པར་ལེན་ + དྲན་ཐོ་ཐོ་བཀོད་ཚུ་ ངེས་གཏན་བཟོཝ་ཨིན། |
| ཐབས་འཕྲུལ་བརྟག་དཔྱད་ཁང་གི་ གློ་བུར་གྱི་ བུན་ཌལ་ | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ཧ་ཤི་འདི་ `bundle-manifest.json` ལས་བཏོན་ཡོདཔ་ཨིན། ཤོག་འཛིན་ AND6-DR-202602 གིས་ ཁྲིམས་དོན་/ཁྲིམས་བཤེར་ལུ་ ལགཔ་བཀལ་ཡོདཔ་ཨིན། |

## ཐོང་མ་དང་ཁྱད་པར།

- བཀག་ཆ་འབད་ནིའི་གནད་དོན་ ངོས་འཛིན་མ་འབད་བས། ETSI/GDPR དགོས་མཁོ་དང་མཐུན་སྒྲིག་འབད་ནི། AND7 བརྡ་འཕྲིན་འདྲ་མཉམ་གྱི་ DPIA བཅུད་བསྡུས་ནང་ བཀོད་ཡོད་པའི་ཁར་ ཁ་སྐོང་དགོཔ་མེདཔ་སྦེ་ བཀོད་དེ་ཡོདཔ་ཨིན་པས།
- གྲོས་འཆར། བལྟ་རྟོག་ལས་རིམ་གྱི་ DR-2026-05-Q2 དམག་སྦྱོང་ (Tticket AND6-DR-202605) དང་ ཤུལ་མའི་གཞུང་ནང་ ཞིབ་དཔྱད་ས་ཚིགས་ཀྱི་ཧེ་མ་ སྒྲུབ་བྱེད་དྲན་ཐོ་ནང་ མཐུད་རྐྱེན་བྱུང་ཡོདཔ།

## གནང༌བ

- **གྲོས་ཆོད་:** ཆ་འཇོག་འབད་ཡོདཔ།
- **མཚན་རྟགས་ / དུས་ཚོད་མཚོན་རྟགས།:* _སོ་ཕི་ཡ་ མར་ཊིན་ (ཌི་ཇི་ཊལ་སྦེ་ གཞུང་སྐྱོང་དྲྭ་ཚིགས་བརྒྱུད་དེ་ མཚན་རྟགས་བཀོད་ཡོདཔ།, 2026-04-15 14:32 UTC)_
- **རྗེས་སུ་འཇུག་པའི་ཇོ་བདག་:** ཐབས་འཕྲུལ་བརྟག་དཔྱད་ཁང་གི་ཨོཔ་ (DR-2026-05-Q2 སྒྲུབ་བྱེད་བང་མཛོད་ ༢༠༢༦-༠༥-༣༡ གི་ཧེ་མ་ བཀྲམ་སྤེལ་འབདཝ་ཨིན།)
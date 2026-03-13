---
lang: dz
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC ཉེན་སྲུང་ཚད་འཛིན་ཞིབ་དཔྱད་ཐོ་ཡིག་ — Android SDK

| ཕིལཌ་ | གནས་གོང་ |
|-------|--|-------------------------------------------------------------------------
| ཐོན་རིམ་ | ༠.༡ (༢༠༢༦-༠༢-༡༢) |
| ཁྱབ་ཁོངས། | Android SDK + བཀོལ་སྤྱོད་ལག་ཆས་ ཇ་པཱན་གྱི་དངུལ་འབྲེལ་བཀྲམ་སྤེལ་ནང་ལག་ལེན་འཐབ་མི་ ལག་ཆས་ཚུ་ |
| ཇོ་བདག་ | བསྟར་སྤྱོད་དང་ཁྲིམས་མཐུན་ (Daniel Park), Android ལས་རིམ་འགོ་ཁྲིད་ |

## ཚད་འཛིན་མེ་ཊིགསི།

| FISC ཚད་འཛིན་ | ལག་ལེན་འཐབ་ཐངས་ཁ་གསལ། | སྒྲུབ་བྱེད་ / གཞི་བསྟུན་ | གནས་ཚད་ |
|--------------|-----------------------|-----------------------|--------|
| **རིམ་ལུགས་རིམ་སྒྲིག་ཆ་ཚང་** | `ClientConfig` གིས་ ཧ་ཤིང་དང་ ལས་རིམ་བདེན་དཔྱད་ དེ་ལས་ ལྷག་ནི་རྐྱངམ་ཅིག་ རན་ཊའིམ་འཛུལ་སྤྱོད་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན། རིམ་སྒྲིག་ལོག་མངོན་གསལ་འཐུས་ཤོར་ཚུ་གིས་ རན་དེབ་ནང་ ཡིག་ཐོག་ལུ་བཀོད་ཡོད་པའི་ `android.telemetry.config.reload` བྱུང་ལས་ཚུ་ imit ཨིན། | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ ལག་བསྟར་འབད་ཡོདཔ། |
| **འཛུལ་སྤྱོད་ཚད་འཛིན་དང་བདེན་བཤད་** | SDK གུས་ཞབས་ Torii TLS སྲིད་བྱུས་དང་ `/v2/pipeline` མཚན་རྟགས་བཀོད་ཡོད། བཀོལ་སྤྱོད་པའི་ལས་ཀ་ གཞི་བསྟུན་རྒྱབ་སྐྱོར་འབདཝ་ཨིན། | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (ལས་ཀའི་རྒྱུན་རིམ།) | ✅ ལག་བསྟར་འབད་ཡོདཔ། |
| **ཀིརིཔ་ཊོ་གཱར་ཕི་ལྡེ་མིག་འཛིན་སྐྱོང་** | StrongBox-preferred providers, attestation validation, and device matrix coverage ensure KMS compliance. བདེན་དཔང་འབད་མི་ ལག་ཆས་ཚུ་ `artifacts/android/attestation/` གི་འོག་ལུ་ གཏན་མཛོད་དང་ གྲ་སྒྲིག་མེ་ཊིགསི་ནང་ བརྟག་ཞིབ་འབད་ཡོདཔ་ཨིན། | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ ལག་བསྟར་འབད་ཡོདཔ། |
| **ལྡེ་མིག་བརྐྱབ་ནི་དང་ ལྟ་རྟོག་འབད་ནི་ དེ་ལས་ བཀག་བཞག་ནི་** | བརྒྱུད་འཕྲིན་བསྐྱར་བཟོ་སྲིད་བྱུས་འདི་གིས་ ཚོར་ཤུགས་ཅན་གྱི་གནས་སྡུད་དང་ བཱ་ཀེཊི་སི་ ཐབས་འཕྲུལ་གྱི་ཁྱད་ཆོས་ དེ་ལས་ བཀག་འཛིན་ཚུ་ བསྟར་སྤྱོད་འབདཝ་ཨིན། (༧/༣༠/༩༠/༣༦༥ གི་སྒོ་སྒྲིག་ཚུ།) རྒྱབ་སྐྱོར་ Playbook §8 གིས་ ཌེཤ་བོརཌ་ཚད་གཞི་ཚུ་ འགྲེལ་བཤད་རྐྱབ་ཨིན། `telemetry_override_log.md` ནང་ཐོ་བཀོད་འབད་ཡོདཔ། | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ ལག་བསྟར་འབད་ཡོདཔ། |
| **བཀོལ་སྤྱོད་དང་བསྒྱུར་བཅོས་འཛིན་སྐྱོང་** | ཇི་ཨེ་ བཤག་བཅོས་བྱ་རིམ་ (རྒྱབ་སྐྱོར་པ་ལེབ་བུཀ་ §༧.༢) དང་ `status.md` དུས་མཐུན་བཟོ་བའི་ ལམ་སྟོན་གསར་བཏོན་གྲ་སྒྲིག་འབད་ནི། སྒྲུབ་བྱེད་བཏོན་པའི་སྒྲུབ་བྱེད་ (SBOM, Sigstore བུནཌལ་) `docs/source/compliance/android/eu/sbom_attestation.md` བརྒྱུད་དེ་མཐུད་ཡོད། | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ ལག་བསྟར་འབད་ཡོདཔ། |
| **གདོང་ལེན་དང་སྙན་ཞུ།** | Playbook གིས་ ཚབས་ཆེན་གྱི་མེ་ཊིགསི་དང་ ཨེསི་ཨེལ་ཨེ་ལན་འདེབས་སྒོ་སྒྲིག་ དེ་ལས་ བསྟར་སྤྱོད་བརྡ་དོན་རིམ་པ་ཚུ་ ངེས་འཛིན་འབདཝ་ཨིན། ཊེ་ལི་མི་ཊི་རི་གིས་ མཁའ་འགྲུལ་པ་གི་ཧེ་མ་ སྐྱེ་འཕེལ་འབད་ཚུགསཔ་སྦེ་ ངེས་གཏན་བཟོཝ་ཨིན། | `docs/source/android_support_playbook.md` §§༤–༩; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ ལག་བསྟར་འབད་ཡོདཔ། |
| **གནས་སྡུད་སྡོད་གནས་ / ས་གནས་** | ཆ་འཇོག་གྲུབ་པའི་ཊོ་ཀི་ཡོ་ལུང་ཕྱོགས་ནང་ ཇེ་པི་བཀྲམ་སྤེལ་ཚུ་གི་དོན་ལུ་ ཊེ་ལི་མི་ཊི་བསྡུ་ལེན་པ་ཚུ་ གཡོག་བཀོལ་ནི། StrongBox གིས་ ལུང་ཕྱོགས་ནང་ལུ་ གསོག་འཇོག་འབད་དེ་ མཉམ་འབྲེལ་གྱི་ཤོག་འཛིན་ཚུ་ལས་ གཞི་བསྟུན་འབད་དེ་ཡོདཔ་ཨིན། ས་གནས་ཀྱི་འཆར་གཞི་གིས་ བེ་ཊ་ (AND5) གི་ཧེ་མ་ ཇ་པཱན་ནང་ ཡིག་ཆ་ཚུ་ ངེས་གཏན་བཟོཝ་ཨིན། | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §༥; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 ཡར་རྒྱས་ནང་ (ས་གནས་ཀྱི་གནས་སྟངས།) |

## བསྐྱར་ཞིབ་ཀྱི་དྲན་ཐོ།

- ཚད་འཛིན་མཉམ་འབྲེལ་པ་བཙུགས་པའི་ཧེ་མ་ གཱ་ལེགསི་ཨེསི་༢༣/ཨེསི་༢༤ གི་དོན་ལུ་ ཐབས་འཕྲུལ་-མེ་ཊིགསི་ཐོ་བཀོད་ཚུ་ བདེན་དཔྱད་འབད་ (གྲ་སྒྲིག་ཡིག་ཆ་ `s23-strongbox-a`, `s24-strongbox-a` ལུ་བལྟ།)
- ཇེ་པི་བཀྲམ་སྤེལ་ཚུ་ནང་ ཊེ་ལི་མི་ཊི་བསྡུ་ལེན་འབད་མི་ཚུ་གིས་ ཌི་པི་ཨའི་ཨེ་ (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) ནང་ ངེས་འཛིན་འབད་ཡོད་པའི་ བཀག་བཞག་/ བཀག་ཆ་འབད་ཡོད་པའི་ཚད་མ་དེ་ བསྟར་སྤྱོད་འབདཝ་ཨིན།
- དངུལ་ཁང་མཉམ་འབྲེལ་པ་ཚུ་གིས་ བརྟག་ཞིབ་ཐོ་ཡིག་འདི་ བསྐྱར་ཞིབ་འབད་ཚརཝ་ད་ ཕྱི་ཁའི་རྩིས་ཞིབ་པ་ཚུ་ལས་ བདེན་དཔྱད་འབད་ནི།
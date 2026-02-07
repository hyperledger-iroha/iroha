---
lang: dz
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2025-12-29T18:16:35.929201+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox གི་བདེན་དཔང་སྒྲུབ་བྱེད་ — ཇ་པཱན།

| ཕིལཌ་ | གནས་གོང་ |
|-------|--|-------------------------------------------------------------------------
| བརྟག་ཞིབ་སྒོ་སྒྲིག་ | ༢༠༢༦-༠༢-༡༠ – ༢༠༢༦-༠༢-༡༢ |
| ཅ་དངོས་ས་ཆ། | `artifacts/android/attestation/<device-tag>/<date>/` (`docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` རེ་ལུ་ བང་རིམ་རྩ་སྒྲིག་) |
| ལག་ཆས་བཟོ་ནི། | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| བསྐྱར་ཞིབ་པ་ | མཐུན་རྐྱེན་བརྟག་དཔྱད་ཁང་འགོ་ཁྲིད་ མཐུན་སྒྲིག་དང་ཁྲིམས་མཐུན་ (JP) |

## 1. འཛིན་བཅོས།

༡ StrongBox matrix ནང་ཐོ་བཀོད་འབད་ཡོད་པའི་ ཐབས་འཕྲུལ་རེ་རེ་ནང་ གདོང་ལེན་ཅིག་བཏོན་ཞིནམ་ལས་ བདེན་ཁུངས་བཀལ་བའི་ བང་རིམ་འདི་ བཟུང་ཚུགས།
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. བུནཌལ་མེ་ཊ་ཌེ་ཊ་ (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`)
༣ བཱན་ཌལ་ཆ་མཉམ་ལོག་སྟེ་བདེན་དཔྱད་འབད་ནིའི་དོན་ལུ་ སི་ཨའི་གྲོགས་རམ་པ་འདི་གཡོག་བཀོལ།
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. ཐབས་འཕྲུལ་བཅུད་བསྡུས་ (༢༠༢༦-༠༢-༡༢)

| ཐབས་འཕྲུལ་གྱི་རྟགས་ | དཔེ་སྟོན་ / StrongBox | བུནཌལ་འགྲུལ་ལམ་ | གྲུབ་འབྲས། | དྲན་ཐོ། |
|------------------------------------------------------------------------------- |
| `pixel6-strongbox-a` | པིག་སེལ་ ༦ / ཊེན་སོར་ G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ བརྒྱུད་སྤྲོད་ (མཉེན་ཆས་རྒྱབ་སྐྱོར) | གདོང་ལེན་བཀག་ཆ་ OS ཐབས་འཕྲུལ་ ༢༠༢༥-༠༣-༠༥། |
| `pixel7-strongbox-a` | པིག་སེལ་ ༧ / ཊེན་སོར་ ཇི་༢ | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ བརྒལ་ | Primary CI ལེན་འདེམས་ངོ་; དྲོད་ཚད་ཀྱི་ནང་འཁོད་ལུ་། |
| `pixel8pro-strongbox-a` | པིག་སེལ་ ༨ པོརོ་ / ཊེན་སོར་ G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ བརྒྱུད་སྤྲོད་ (བསྐྱར་རྟགས) | USB-C ལྟེ་བ་ཚབ་བཙུགས་ཡོདཔ། Buildkite `android-strongbox-attestation#221` གིས་ བརྒལ་བའི་བཱན་ཌལ་འདི་ བཟུང་ཡོདཔ་ཨིན། |
| `s23-strongbox-a` | གཱ་ལེག་སི་ཨེས་༢༣ / སྣཔ་དྲ་གཱོན་ ༨ ཇེན་ ༢ | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ བརྒལ་ | ནོག་སི་བདེན་དཔང་གསལ་སྡུད་ནང་འདྲེན་འབད་ཡོདཔ། ༢༠༢༦-༠༢-༠༩. |
| `s24-strongbox-a` | གཱ་ལེག་སི་ཨེསི་༢༤ / སྣཔ་དྲ་གཱོན་ ༨ ཇེན་ ༣ | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ བརྒལ་ | ནོགསི་བདེན་དཔང་གསལ་སྡུད་ནང་འདྲེན་འབད་ཡོདཔ། CI ད་ལྟ་ལྗང་ཁུ། |

ཐབས་འཕྲུལ་གྱི་ངོ་རྟགས་སབ་ཁྲ་ `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` ལུ།

## 3. དཔྱད་གཞིའི་དཔྱད་གཞི།

- [x] `result.json` བདེན་བཤད་འབད་ `strongbox_attestation: true` དང་ ལག་ཁྱེར་ཚུ་ བློ་གཏད་ཅན་གྱི་རྩ་བ་ལུ་སྟོནམ་ཨིན།
- [x] བཱའིཊིསི་མཐུན་སྒྲིག་འདི་གིས་ བུའིལཌི་ཀི་ཊི་གིས་ `android-strongbox-attestation#219` གཡོག་བཀོལཝ་ཨིན་ (འགོ་ཐོག་བཤུད་སྒྲིལ་) དང་ `#221` (Pixel 8 Pro retect + S24 བཟུང་ཚད་) གཡོག་བཀོལཝ་ཨིན།
- [x] ལོག་སྟེ་གཡོག་བཀོལ་བའི་ Pixel 8 མཐུན་སྒྲིག་འབད་བའི་ཤུལ་ལས་ པྲོ་བཟུང་།
- [x] སྐར་ཚོམ་གསལ་སྡུད་ཆ་འཇོག་འདི་ལྷོད་པའི་སྐབས་ གཱ་ལེག་སི་ཨེསི་༢༤ ཆ་ཚང་བཟུང་། (ཇོ་བདག་: ཐབས་འཕྲུལ་བརྟག་དཔྱད་ཁང་། ༢༠༢༦-༠༢-༡༣ མཇུག་བསྡུ་ཡོདཔ།)

## ༤ བཀྲམ་སྤེལ།

- བཅུད་བསྡུས་འདི་མཉམ་སྦྲགས་འབད། མཉམ་འབྲེལ་སྒྲིག་བཀོད་ཐུམ་སྒྲིལ་ཚུ་ལུ་ (FISC ཞིབ་དཔྱད་ཐོ་ཡིག་ §Data residency) ལུ་ སྙན་ཞུ་ཚིག་ཡིག་ཡིག་སྣོད་གསརཔ་འདི་ མཉམ་སྦྲགས་འབད།
- ཁྲིམས་ལུགས་རྩིས་ཞིབ་ལུ་ལན་འདེབས་འབད་བའི་སྐབས་ གཞི་བསྟུན་ལམ་ཚུ་ རྒྱབ་རྟེན་འབད་ནི། གསང་བཟོས་རྒྱུ་ལམ་ཚུ་གི་ཕྱི་ཁར་ལག་ཁྱེར་ངོ་མ་ཚུ་ བརྒྱུད་སྤྲོད་མི་འབད།

## 5. ལོག བསྒྱུར་བ།

| ཚེས་གྲངས་ | བསྒྱུར་བཅོས་ | རྩོམ་པ་པོ |
|-------|-------------|------------------------------------------ |
| ༢༠༢༦-༠༢-༡༢ | འགོ་བཙུགས་ཇེ་པི་ བཱན་ཌལ་ འཛིན་བཟུང་ + སྙན་ཞུ། | ཐབས་འཕྲུལ་བརྟག་དཔྱད་ཁང་ Ops |
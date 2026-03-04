---
lang: dz
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2025-12-29T18:16:35.924058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Device ལབ་ ལག་ཆས་ ཧུཀ་ (AND6)

གཞི་བསྟུན་འདི་གིས་ ལམ་གྱི་ས་ཁྲ་འདི་ མཇུག་བསྡུཝ་ཨིན།
AND6 གི་གདོང་ཕྱོགས་ཀྱི་ ལག་ཆས་ཚུ་” གིས་ ག་དེ་སྦེ་ བཀག་བཞག་ཡོདཔ་ཨིན་ན་ འགྲེལ་བཤད་རྐྱབ་ཨིན།
ཐབས་འཕྲུལ་གྱི་བརྟག་དཔྱད་ཁང་གིས་ ཊེ་ལི་མི་ཊི་དང་ གྱལ་རིམ་ དེ་ལས་ བདེན་ཁུངས་བཀལ་ཡོད་པའི་ ཅ་ཆས་ཚུ་ བཟུང་དགོཔ་ཨིན།
AND6 དང་མཐུན་པའི་ བརྟག་ཞིབ་ཐོ་ཡིག་དང་ སྒྲུབ་བྱེད་དྲན་དེབ་ དེ་ལས་ གཞུང་སྐྱོང་སྦུང་ཚན་ཚུ་ ཅོག་འཐདཔ་ཨིན།
determistic ལས་ཀའི་རྒྱུན་རིམ། དྲན་ཐོ་འདི་བཀག་བཞག་བྱ་རིམ་དང་གཅིག་ཁར་ཆ་སྒྲིག་འབད།
(`device_lab_reservation.md`) དང་ བསྐྱར་སྦྱོང་འཆར་གཞི་བརྩམ་པའི་སྐབས་ འཐུས་ཤོར་གྱི་རའུན་དེབ་ཚུ།

## དམིགས་ཡུལ་དང་ཁྱབ་ཁོངས།

- ** Determistic སྒྲུབ་བྱེད་** – ལག་ཆས་ཐོན་འབྲས་ཆ་མཉམ་ འོག་ལུ་སྡོད་དོ་ཡོདཔ་ཨིན།
  `artifacts/android/device_lab/<slot-id>/` དང་SHA-256 དང་མཉམ་དུ་རྩིས་ཞིབ་པ་ཚུ་མངོན་ཡོད།
  འཚོལ་ཞིབ་ཚུ་ ལོག་མ་བཙུགས་པར་ ཁྱབ་སྤེལ་འབད་ཚུགས།
- **Script-དང་པ་ལཱ་གི་རྒྱུན་རིམ་** - ད་ལྟོ་ཡོད་པའི་གྲོགས་རམ་པ་ཚུ་ལོག་ལག་ལེན་འཐབ།
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  བེ་སི་པོཀ་ཨེ་ཌི་བི་བརྡ་བཀོད་ཀྱི་ཚབ་ལུ།
- **ཞིབ་དཔྱད་ཐོ་ཡིག་ཚུ་མཉམ་འབྱུང་** – གཡོག་བཀོལ་བའི་གཞི་བསྟུན་རེ་རེ་བཞིན་ ཡིག་ཆ་འདི་ནང་ལས་ ལས་ གཞི་བསྟུན་འབདཝ་ཨིན།
  AND6 བསྟུན་པའི་ བརྟག་ཞིབ་ཐོ་ཡིག་དང་ ཅ་རྙིང་ཚུ་ ༡༠ ལུ་ བཀལ་ཡོདཔ་ཨིན།
  `docs/source/compliance/android/evidence_log.csv`.

## ཅ་རྙིང་སྒྲིག་བསྒྲགས།

༡ བཀག་ཆ་འབད་ནིའི་ཤོག་བྱང་དང་མཐུན་པའི་ ཁྱད་པར་ཅན་གྱི་ས་སྒོ་ངོས་འཛིན་འབད་མི་ཅིག་ འདམ་ཁ་རྐྱབ། དཔེར་ན།
   `2026-05-12-slot-a`.
2. ཚད་ལྡན་སྣོད་ཐོ་ཚུ་ སོན་བཏབ་ནི།

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. མཐུན་སྒྲིག་སྣོད་འཛིན་ (དཔེར་ན་ ) མཐུན་སྒྲིག་སྣོད་འཛིན་ནང་ བརྡ་བཀོད་དྲན་ཐོ་ཆ་མཉམ་སྲུངས།
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
༤ SHA-256 འདི་ སྒོ་བསྡམས་པའི་ཤུལ་ལས་ གསལ་སྟོན་འབདཝ་ཨིན།

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## ལག་ཆས་མེ་རིགས།

| ཕོལོ་ | བརྡ་བཀོད་(ཚུ་) | ཐོན་འབྲས་གནས་ཁོངས་ | དྲན་ཐོ། |
|------------------------------------------------------ |
| བརྒྱུད་འཕྲིན་བསྐྱར་བཟོ་ + གནས་ཚད་བང་སྒྲིག་ | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | འགོ་བཙུགས་དང་མཇུག་བསྡུའི་ནང་རྒྱུག་འགྲན། CLI stdout ལུ་ `status.log` ལུ་མཉམ་སྦྲགས་འབད། |
| གྱལ་རིམ་ + ཟང་ཟིང་སྔོན་འགྲོའི་ | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256`, | མེ་ལོང་མཐོང་སྣང་ `readiness/labs/telemetry_lab_01.md` ལས་; ཐབས་འཕྲུལ་རེ་རེ་གི་དོན་ལུ་ env var རྒྱ་སྐྱེད་འབད། |
| ཨོ་ཝར་རིཌ་ལེཌ་ཇར་ བཞུ་བཅོས་ | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | བཀག་ཆ་མེད་པའི་སྐབས་ལུ་ཡང་དགོཔ་ཨིན། ཀླད་ཀོར་གནས་སྟངས་ལུ་བདེན་ཁུངས་བཀལ། |
| StrongBox / TEE བདེན་དཔང་། | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | གསོག་འཇོག་འབད་ཡོད་པའི་ཐབས་འཕྲུལ་རེ་རེ་གི་དོན་ལུ་ (`android_strongbox_device_matrix.md` ནང་ མཐུན་སྒྲིག་མིང་ཚུ་) བསྐྱར་ལོག་འབད། |
| CI harness ངོས་འཛིན་འགྱུར་ལྡོག་ | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI སྐྱེལ་བཙུགས་འབད་བའི་སྒྲུབ་བྱེད་གཅིག་པ་ཚུ་ བཟུང་ཡོདཔ་ཨིན། འདྲ་མཉམ་གྱི་དོན་ལུ་ ལག་ཐོག་རྒྱུག་ནི་ནང་ཚུདཔ་ཨིན། |
| Lint / བརྟེན་པའི་གཞི་རྟེན་ཐིག་ | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | གྱང་ཁོག་རེ་ལུ་ཚར་གཅིག་གཡོག་བཀོལ། བཅུད་བསྡུས་འདི་ བསྟར་སྤྱོད་ཀྱི་ཐུམ་སྒྲིལ་ནང་ བཀོད། |

## ཚད་ལྡན་གྱི་ས་ཆའི་བྱ་རིམ།1. **འཕུར་འགྲུལ་མ་འབད་བའི་ཧེ་མའི་ (T-24h)** – བཀག་ཆ་ཤོག་བྱང་གཞི་བསྟུན་ཚུ་ ངེས་གཏན་བཟོ།
   ཡིག་ཆ་དང་ ཐབས་འཕྲུལ་མེ་ཊིགསི་ཐོ་བཀོད་ཚུ་དུས་མཐུན་བཟོ་ནི་དང་ སོན་རིགས་རྩ་བའི་སོན་ཚུ་དུས་མཐུན་བཟོ་ནི།
2. ** slot* སྐབས་** ལུ།
   - ཊེ་ལི་མི་ཊི་བཱན་ཌལ་ + གྱལ་ཕྱིར་འདྲེན་བརྡ་བཀོད་ཚུ་ དང་པ་ར་ གཡོག་བཀོལ། མཐར༌འཁྱོལ
     `--note <ticket>` ལས་ `device_lab_reservation.md` ལས་ དྲན་ཐོ་།
     བྱུང་རྐྱེན་ཨའི་ཌི་འདི་གཞི་བསྟུན་འབདཝ་ཨིན།
   - ཐབས་འཕྲུལ་རེ་ལུ་ བདེན་ཁུངས་བཀལ་ཡོད་པའི་ཡིག་གཟུགས་ཚུ་ རི་གཱར་འབད། ཧར་ནེས་ཀྱིས་ཐོན་པའི་སྐབས།
     `.zip`, དེ་ ཅ་རྙིང་རྩ་བའི་ནང་ལུ་འདྲ་བཤུས་རྐྱབས་ཞིནམ་ལས་ གིཊི་ཨེསི་ཨེ་ལུ་དཔར་བསྐྲུན་འབད་ཡོད་པའི་ གིཊི་ཨེསི་འདི་ཐོ་བཀོད་འབད།
     ཡིག་གཟུགས་ཀྱི་མཐའ།
   - CI ཡིན་ན་ཡང་ `make android-lint` ལག་ལེན་འཐབ་དགོ།
     ཧེ་མ་ལས་རང་ བརྒྱུགས་ཡོདཔ་ཨིན། རྩིས་ཞིབ་པ་ཚུ་གིས་ བརྗེ་སོར་གྱི་དྲན་ཐོ་ཅིག་ རེ་བ་བསྐྱེདཔ་ཨིན།
3. **པོསཊི་-རན**
   - `sha256sum.txt` དང་ `README.md` (རང་དབང་ཅན་གྱི་དྲན་འཛིན་) ནང་ན་ བཏོན་གཏང་།
     སྣོད་འཛིན་ བཀོལ་སྤྱོད་འབད་ཡོད་པའི་བརྡ་བཀོད་ཚུ་ བཅུད་བསྡུས་འབད་དོ།
   - གྲལ་ཐིག་ཅིག་ `docs/source/compliance/android/evidence_log.csv` ལུ་ མཉམ་སྦྲགས་འབད།
     slot ID དང་ ཧེཤ་གསལ་སྟོན་འགྲུལ་ལམ་ བུའིལ་ཀི་ཊི་གཞི་བསྟུན་ (གལ་སྲིད་ཡོད་ན་) དང་ གསར་ཕྱོགས།
     ཐབས་འཕྲུལ་-བརྟག་དཔྱད་ནུས་ཤུགས་བརྒྱ་ཆ་ བཀག་འཛིན་ཟླ་ཐོའི་ཕྱིར་འདྲེན་ལས་།
   - `_android-device-lab` ཤོག་འཛིན་ནང་ ས་སྒོ་གི་སྣོད་འཛིན་འདི་ འབྲེལ་མཐུད་འབད།, AND6
     བརྟག་ཞིབ་ཐོ་ཡིག་, དང་ `docs/source/android_support_playbook.md` གསར་བཏོན་སྙན་ཞུ།

## འཐུས་ཤོར་འཛིན་སྐྱོང་དང་ ཡར་འཛེག་པ།

- བརྡ་བཀོད་གང་རུང་ཅིག་འཐུས་ཤོར་བྱུང་པ་ཅིན་ `logs/` གི་འོག་ལུ་ stderr outpross འདི་བཟུང་ཞིནམ་ལས་ རྗེས་སུ་འབྲང་།
  `device_lab_reservation.md` §6 ནང་ཡར་འཕར་གྱི་ཐེམ་སྐད། §6.
- ཀིའུ་ཨའེའུ་ཡང་ན་ ཊེ་ལི་མི་ཊི་མ་ལངམ་ཚུ་གིས་ སྤྱི་ལོ་ ༢༠༢༠ ལུ་ བཀག་ཆ་འབད་བའི་གནས་རིམ་འདི་ དེ་འཕྲོ་ལས་ དྲན་འཛིན་འབད་དགོ།
  `docs/source/sdk/android/telemetry_override_log.md` དང་ ས་སྒོ་ཨའི་ཌི་ལུ་གཞི་བསྟུན་འབད།
  དེ་འབདཝ་ལས་ གཞུང་སྐྱོང་འདི་གིས་ དམག་སྦྱོང་འདི་ འཚོལ་ཞིབ་འབད་ཚུགས།
- ཉེས་འགེལ་གྱི་ ལོག་ལྟའི་ཚུ་ སྤྱི་ལོ་༢༠༠༨ ལུ་ ཐོ་བཀོད་འབད་དགོ།
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  འཐུས་ཤོར་བྱུང་བའི་ཐབས་འཕྲུལ་རིམ་སྒྲིག་དང་ གོང་ལུ་སྒྲ་བཟུང་འབད་ཡོད་པའི་ བཱན་ཌལ་འགྲུལ་ལམ་ཚུ་དང་གཅིག་ཁར།

## སྙན་ཞུའི་ཐོ་འགོད་འབད་ནི།

ས་སྒོ་འདི་མཇུག་བསྡུ་རྟགས་མ་བཀོད་པའི་ཧེ་མ་ འོག་གི་གཞི་བསྟུན་ཚུ་ དུས་མཐུན་བཟོ་ཡོདཔ་ཨིན།

- `docs/source/compliance/android/and6_compliance_checklist.md` — རྟགས་བཀོད།
  ལག་ཆས་གྲལ་ཐིག་མཇུག་བསྡུ་ཞིནམ་ལས་ ས་སྒོ་ཨའི་ཌི་འདི་དྲན་འཛིན་འབད།
- `docs/source/compliance/android/evidence_log.csv` — དང་གཅིག་ཁར་ ཐོ་བཀོད་འདི་ ཁ་སྐོང་/དུས་མཐུན་བཟོ་ནི།
  the slot hash དང་ནུས་ཤུགས།
- `_android-device-lab` ཤོག་འཛིན་ — ཅ་རྙིང་འབྲེལ་མཐུད་དང་ བཱའིཊ་ཀིཊ་ལཱ་གི་ངོ་རྟགས་ཚུ་ མཉམ་སྦྲགས་འབད་ཡོདཔ།
- `status.md` — ཤུལ་མའི་ Android གི་གྲ་སྒྲིག་བཞུ་བཅོས་ནང་ དྲན་ཐོ་ཐུང་ཀུ་ཅིག་ ཚུད་དགོ།
  ལམ་སྟོན་ལྷག་མི་ཚུ་གིས་ སྒྲུབ་བྱེད་གསརཔ་ག་ཅི་གིས་ བཏོན་ཡོདཔ་ཨིན་ན་ ཤེས་ཚུགས།

འདི་གི་ཤུལ་ལས་ AND6 གི་ “ཐབས་འཕྲུལ་བརྟག་དཔྱད་ཁང་+ ལག་ཆས་ཚུ་” བཞགཔ་ཨིན།
རྩིས་ཞིབ་འབད་ཚུགསཔ་དང་ བཀོད་སྒྲིག་དང་ ལག་ལེན་འཐབ་ནིའི་བར་ན་ ལག་ཐོག་ལས་ ཁ་སྟོར་འགྱོ་ནི་ལས་ བཀག་ཆ་འབདཝ་ཨིན།
དང་སྙན་ཞུ་འབད་དོ།
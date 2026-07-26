---
lang: my
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

# Android စက်ပစ္စည်း ဓာတ်ခွဲခန်း ကိရိယာ ချိတ်များ (AND6)

ဤအကိုးအကားသည် လမ်းပြမြေပုံလုပ်ဆောင်မှုကို ပိတ်လိုက်သည် “ကျန်ရှိသော စက်ကိရိယာ-ဓာတ်ခွဲခန်းကို အဆင့်သတ်မှတ်ခြင်း/
AND6 စတင်ပွဲမတိုင်မီ ကိရိယာတန်ဆာပလာ ချိတ်သည်။ သိမ်းထားပုံကို ရှင်းပြသည်။
စက်ပစ္စည်း-ဓာတ်ခွဲခန်းအပေါက်သည် တယ်လီမီတာ၊ တန်းစီခြင်းနှင့် သက်သေအထောက်အထားများကို ဖမ်းယူရမည်ဖြစ်ပါသည်။
AND6 လိုက်နာမှုစစ်ဆေးချက်စာရင်း၊ အထောက်အထားမှတ်တမ်းနှင့် အုပ်ချုပ်မှုပက်ကေ့ဂျ်များသည် တူညီပါသည်။
သတ်မှတ်ထားသော အလုပ်အသွားအလာ။ ကြိုတင်စာရင်းသွင်းခြင်းလုပ်ငန်းစဉ်နှင့် ဤမှတ်စုကို တွဲချိတ်ပါ။
အစမ်းလေ့ကျင့်မှုများ စီစဉ်သည့်အခါ (`device_lab_reservation.md`) နှင့် ပျက်ကွက်မှု ပြေးစာအုပ်။

## ပန်းတိုင်နှင့် နယ်ပယ်

- ** အဆုံးအဖြတ် အထောက်အထားများ ** - ကိရိယာ တန်ဆာပလာများ အားလုံးသည် အောက်တွင် ရှိနေသည်။
  SHA-256 ပါသော `artifacts/android/device_lab/<slot-id>/` သည် စာရင်းစစ်သူများ၊
  probes ကို ပြန်မလုပ်ဆောင်ဘဲ အစုအဝေးများကို ကွဲပြားနိုင်သည်။
- **Script-first workflow** – ရှိပြီးသား အကူအညီများကို ပြန်သုံးပါ။
  (`ci/run_android_telemetry_chaos_prep.sh`၊
  `scripts/android_keystore_attestation.sh`၊ `scripts/android_override_tool.sh`)
  bespoke adb command များအစား
- **စစ်ဆေးသည့်စာရင်းများသည် တစ်ပြိုင်တည်းရှိနေသည်** – လည်ပတ်မှုတိုင်းသည် ဤစာရွက်စာတမ်းမှ ကိုးကားသည်။
  AND6 လိုက်နာမှု စစ်ဆေးရေးစာရင်းနှင့် ဆက်စပ်ပစ္စည်းများကို ဖြည့်စွက်သည်။
  `docs/source/compliance/android/evidence_log.csv`။

## Artifact Layout

1. ကြိုတင်မှာကြားထားသည့်လက်မှတ်နှင့် ကိုက်ညီသည့် သီးခြားအထိုင်အမှတ်အသားတစ်ခုကို ရွေးပါ၊ ဥပမာ။
   `2026-05-12-slot-a`။
2. စံလမ်းညွှန်များကို ကြည့်ပါ။

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. ကိုက်ညီသောဖိုင်တွဲအတွင်း အမိန့်ပေးမှတ်တမ်းတိုင်းကို သိမ်းဆည်းပါ (ဥပမာ။
   `telemetry/status.ndjson`၊ `attestation/pixel8pro.log`)။
4. အပေါက်ပိတ်သည်နှင့် SHA-256 ကို ဖမ်းယူပါ-

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Instrumentation Matrix

| စီးဆင်းမှု | အမိန့်(များ) | အထွက်တည်နေရာ | မှတ်စုများ |
|------|------------------|-----------------|------|
| တယ်လီမီတာ တုံ့ပြန်မှု + အခြေအနေ အစုအဝေး | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | slot ၏အစနှင့်အဆုံးတွင် run; CLI stdout ကို `status.log` သို့ ပူးတွဲပါ။ |
| စောင့်ဆိုင်းနေသော လူတန်း + ပရမ်းပတာ ပြင်ဆင်မှု | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | `readiness/labs/telemetry_lab_01.md` မှ Mirrors ScenarioD အပေါက်ရှိ စက်တိုင်းအတွက် env var ကို တိုးချဲ့ပါ။ |
| လယ်ဂျာ ချေဖျက်မှု | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | အစားထိုးခြင်းများ မလုပ်ဆောင်သော်လည်း လိုအပ်ပါသည်။ သုညအခြေအနေကို သက်သေပြပါ။ |
| StrongBox / TEE အထောက်အထား | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | လက်ဝယ်ထားရှိသော စက်တစ်ခုစီအတွက် ထပ်လုပ်ပါ (`android_strongbox_device_matrix.md` တွင် အမည်များနှင့် ကိုက်ညီသည်)။ |
| CI harness သက်သေ ဆုတ်ယုတ်မှု | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI အပ်လုဒ်လုပ်သည့် တူညီသောအထောက်အထားများကို ဖမ်းယူပါ။ symmetry အတွက် manual run များ ပါဝင်သည်။ |
| Lint / dependency baseline | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | အေးခဲသောဝင်းဒိုးတွင် တစ်ကြိမ်လုပ်ဆောင်ပါ။ လိုက်နာမှု packets တွင် အကျဉ်းချုပ်ကို ကိုးကားပါ။ |

## Standard Slot လုပ်ထုံးလုပ်နည်း1. **လေယာဉ်အကြို (T-24h)** - ကြိုတင်မှာကြားထားသည့် လက်မှတ်ကိုးကားချက်ကို အတည်ပြုပါ
   စာရွက်စာတမ်း၊ စက်ပစ္စည်း matrix entry ကို အပ်ဒိတ်လုပ်ပြီး artifact root ကို မျိုးစေ့ချပါ။
2. **အထိုင်အတွင်း**
   - ပထမဦးစွာ telemetry bundle + တန်းစီထုတ်ယူမှုအမိန့်များကို run ။ သည်း
     `--note <ticket>` မှ `ci/run_android_telemetry_chaos_prep.sh` ထို့ကြောင့် မှတ်တမ်း
     အဖြစ်အပျက် ID ကိုရည်ညွှန်းသည်။
   - စက်တစ်ခုစီအတွက် သက်သေခံ script များကို အစပျိုးပါ။ ကြိုးက ထွက်လာတဲ့အခါ
     `.zip`၊ ၎င်းကို artefact root တွင်ကူးယူပြီး Git SHA တွင် ရိုက်နှိပ်ထားသော မှတ်တမ်းကို
     ဇာတ်ညွှန်း၏အဆုံး။
   - CI ဖြစ်လျှင်ပင် `make android-lint` ကို overridden အကျဉ်းချုပ်လမ်းကြောင်းဖြင့် လုပ်ဆောင်ပါ
     ပြေးနေပြီ၊ စာရင်းစစ်များသည် per-slot မှတ်တမ်းတစ်ခုကို မျှော်လင့်ကြသည်။
၃။ **အပြေးလွန်ခြင်း**
   - အပေါက်အတွင်း၌ `sha256sum.txt` နှင့် `README.md` (အခမဲ့ပုံစံမှတ်စုများ) ကိုထုတ်ပါ
     ကွပ်မျက်ခံရသောအမိန့်များကိုအကျဉ်းချုပ်ဖော်ပြထားသောဖိုင်တွဲ။
   - အတန်းကို `docs/source/compliance/android/evidence_log.csv` နှင့်တွဲထည့်ပါ။
     slot ID၊ hash manifest လမ်းကြောင်း၊ Buildkite ရည်ညွှန်းချက်များ (ရှိပါက) နှင့် နောက်ဆုံး
     ကြိုတင်မှာယူမှုပြက္ခဒိန် တင်ပို့မှုမှ စက်ပစ္စည်း-ဓာတ်ခွဲခန်း စွမ်းဆောင်ရည် ရာခိုင်နှုန်း။
   - `_android-device-lab` လက်မှတ်၊ AND6 ရှိ slot ဖိုဒါကို ချိတ်ဆက်ပါ။
     စစ်ဆေးစာရင်းနှင့် `docs/source/android_support_playbook.md` ထုတ်ပြန်ချက် အစီရင်ခံစာ။

## ပျက်ကွက်မှုကို ကိုင်တွယ်ခြင်းနှင့် မြှင့်တင်ခြင်း။

- မည်သည့် command မှ အဆင်မပြေပါက၊ `logs/` အောက်တွင် stderr output ကိုဖမ်းပြီး လိုက်နာပါ။
  `device_lab_reservation.md` §6 ရှိ အရှိန်မြှင့်လှေကား။
- တန်းစီခြင်း သို့မဟုတ် တယ်လီမီတာ ပြတ်တောက်မှုများသည် override အခြေအနေကို ချက်ချင်းမှတ်သားသင့်သည်။
  `docs/source/sdk/android/telemetry_override_log.md` နှင့် slot ID ကိုကိုးကားပါ။
  ဒါမှ အုပ်ချုပ်မှုဟာ ညီညွှတ်မှုကို ခြေရာခံနိုင်မှာပါ။
- အထောက်အထား ဆုတ်ယုတ်မှုများကို မှတ်တမ်းတင်ထားရမည်။
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  မအောင်မြင်သော စက်ပစ္စည်း အမှတ်စဉ်များနှင့် အထက်တွင် မှတ်တမ်းတင်ထားသော အတွဲလမ်းကြောင်းများ။

## အစီရင်ခံချက်စာရင်း

အပေါက်ကို ပြီးမြောက်ကြောင်း အမှတ်အသားမပြုမီ၊ အောက်ပါကိုးကားချက်များကို အပ်ဒိတ်လုပ်ထားကြောင်း အတည်ပြုပါ။

- `docs/source/compliance/android/and6_compliance_checklist.md` — အမှတ်အသားပြုပါ။
  ကိရိယာတန်ဆာပလာအတန်းကို ပြီးမြောက်ပြီး အထိုင် ID ကို မှတ်သားပါ။
- `docs/source/compliance/android/evidence_log.csv` — ထည့်သွင်းမှုနှင့်အတူ ထည့်သွင်း/အပ်ဒိတ်လုပ်ပါ။
  slot hash နှင့် စွမ်းရည်ဖတ်ရှုခြင်း။
- `_android-device-lab` လက်မှတ် — artefact links များနှင့် Buildkite အလုပ် ID များကို ပူးတွဲပါ ။
- `status.md` — လာမည့် Android အဆင်သင့်အစီအစဥ်တွင် အတိုချုံးမှတ်စု ထည့်သွင်းပါ
  လမ်းပြမြေပုံဖတ်သူများသည် မည်သည့်အပေါက်မှ နောက်ဆုံးထွက်ရှိထားသည့် အထောက်အထားများကို သိရှိကြသည်။

ဤလုပ်ငန်းစဉ်ကို လိုက်နာခြင်းဖြင့် AND6 ၏ "စက်ပစ္စည်း-ဓာတ်ခွဲခန်း + ကိရိယာချိတ်တွဲများ" ကို သိမ်းဆည်းထားသည်။
သမိုင်းမှတ်တိုင်ကို စာရင်းစစ်နိုင်ပြီး ကြိုတင်စာရင်းသွင်းခြင်း၊ လုပ်ဆောင်ခြင်းကြားတွင် လူကိုယ်တိုင် ကွဲပြားမှုကို တားဆီးပေးသည်။
အစီရင်ခံခြင်း။
---
lang: my
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android ဖြန့်ချိမှုစာရင်း (AND6)

ဤစစ်ဆေးမှုစာရင်းသည် **AND6 — CI နှင့် Compliance Hardening** ဂိတ်များမှ ဖမ်းယူပါသည်။
`roadmap.md` (§ဦးစားပေး 5)။ ၎င်းသည် Android SDK ထုတ်ဝေမှုများကို Rust နှင့် ချိန်ညှိပေးသည်။
CI အလုပ်အကိုင်များ၊ လိုက်နာမှုဆိုင်ရာပစ္စည်းများကို စာလုံးပေါင်းခြင်းဖြင့် RFC ၏မျှော်လင့်ချက်များကို ထုတ်ပြန်ခြင်း၊
GA မတိုင်မီ ပူးတွဲထားရမည့် စက်ပစ္စည်းဓာတ်ခွဲခန်း အထောက်အထားများနှင့် သက်သေအစုအစည်းများ၊
LTS သို့မဟုတ် hotfix ရထားသည် ရှေ့သို့ ရွေ့လျားနေသည်။

ဤစာရွက်စာတမ်းကို တွဲသုံးပါ-

- `docs/source/android_support_playbook.md` — ထုတ်ပြက္ခဒိန်၊ SLA နှင့်
  ကြီးထွားမှုသစ်ပင်။
- `docs/source/android_runbook.md` — နေ့စဉ် လုပ်ငန်းလည်ပတ်မှုဆိုင်ရာ စာအုပ်များ။
- `docs/source/compliance/android/and6_compliance_checklist.md` — ထိန်းညှိကိရိယာ
  ပစ္စည်းစာရင်း။
- `docs/source/release_dual_track_runbook.md` — dual-track ထုတ်ဝေမှု အုပ်ချုပ်ရေး။

## 1. Stage Gates တစ်ချက်ကြည့်လိုက်ပါ။

| ဇာတ်ခုံ | လိုအပ်သောဂိတ်များ | အထောက်အထား |
|--------|----------------|----------------|
| **T−7 ရက် (အအေးမကြို)** | ညစဉ် `ci/run_android_tests.sh` အစိမ်းရောင် 14 ရက်; `ci/check_android_fixtures.sh`၊ `ci/check_android_samples.sh`၊ နှင့် `ci/check_android_docs_i18n.sh` သွားတာ၊ lint/dependency စကင်န်များကို တန်းစီထားသည်။ | Buildkite ဒက်ရှ်ဘုတ်များ၊ fixture diff အစီရင်ခံစာ၊ နမူနာဖန်သားပြင်ဓာတ်ပုံများ။ |
| **T−3 ရက် (RC ပရိုမိုးရှင်း)** | စက်ပစ္စည်း-ဓာတ်ခွဲခန်း ကြိုတင်မှာကြားမှုကို အတည်ပြုပြီး၊ StrongBox သက်သေခံချက် CI လည်ပတ်မှု (`scripts/android_strongbox_attestation_ci.sh`); စီစဉ်ထားသော ဟာ့ဒ်ဝဲတွင် လေ့ကျင့်ထားသော စက်ရုပ်/ကိရိယာအစုံအလင်များ၊ `./gradlew lintRelease ktlintCheck detekt dependencyGuard` ကားသန့်။ | စက်ပစ္စည်း matrix CSV၊ အထောက်အထားအစုအဝေး မန်နီးဖက်စ်၊ `artifacts/android/lint/<version>/` အောက်တွင် သိမ်းဆည်းထားသော Gradle အစီရင်ခံစာများ။ |
| **T−1 ရက် (သွား/မသွားရန်)** | တယ်လီမီတာ တုံ့ပြန်မှု အခြေအနေ အစုအဝေးကို ပြန်လည်ဆန်းသစ်ပြီး (`scripts/telemetry/check_redaction_status.py --write-cache`); `and6_compliance_checklist.md` နှုန်းဖြင့် မွမ်းမံထားသော လိုက်နာမှုဆိုင်ရာ ပစ္စည်းများ၊ သက်သေပြလေ့ကျင့်မှု (`scripts/android_sbom_provenance.sh --dry-run`) ပြီးပါပြီ။ | `docs/source/compliance/android/evidence_log.csv`၊ တယ်လီမီတာ အခြေအနေ JSON၊ သက်သေပြချက် ခြောက်သွေ့သော မှတ်တမ်း။ |
| **T0 (GA/LTS ဖြတ်တောက်မှု)** | `scripts/publish_android_sdk.sh --dry-run` ပြီးစီးပြီး၊ သက်သေ + SBOM လက်မှတ်ရေးထိုးခဲ့သည်; ထုတ်ယူထားသော စစ်ဆေးစာရင်းကို ထုတ်ပေးပြီး go/no-go minutes တွင် ပူးတွဲပါရှိသည်။ `ci/sdk_sorafs_orchestrator.sh` မီးခိုးငွေ့ အလုပ်စိမ်း။ | RFC ပူးတွဲပါဖိုင်များ၊ Sigstore အတွဲ၊ `artifacts/android/` အောက်တွင် မွေးစားခြင်းဆိုင်ရာ ပစ္စည်းများကို ထုတ်ဝေပါ။ |
| **T+1 ရက် (ဖြတ်တောက်ပြီးနောက်)** | Hotfix အဆင်သင့်စစ်ဆေးပြီး (`scripts/publish_android_sdk.sh --validate-bundle`); ဒက်ရှ်ဘုတ် ကွဲပြားမှုများကို သုံးသပ်ပြီး (`ci/check_android_dashboard_parity.sh`); `status.md` သို့ အထောက်အထား အစုံလိုက် အပ်လုဒ်လုပ်ထားသည်။ | ဒက်ရှ်ဘုတ် ကွဲပြားသော ထုတ်ယူမှု၊ `status.md` သွင်းမှုသို့ လင့်ခ်၊ သိမ်းဆည်းထားသော ထုတ်လွှတ်မှု ပက်ကတ်။ |

## 2. CI & Quality Gate Matrix| ဂိတ် | Command(s) / Script | မှတ်စုများ |
|--------|--------------------|------|
| ယူနစ် + ပေါင်းစပ်စစ်ဆေးမှုများ | `ci/run_android_tests.sh` (္စ `ci/run_android_tests.sh`) | `artifacts/android/tests/test-summary.json` + စမ်းသပ်မှုမှတ်တမ်းကို ထုတ်လွှတ်သည်။ Norito ကုဒ်ဒက်၊ တန်းစီ၊ StrongBox လှည့်ကွက်နှင့် Torii ကုဒ်ဆွဲကိရိယာ စမ်းသပ်မှုများ ပါဝင်သည်။ ညစဉ်နှင့် တဂ်မတင်မီ လိုအပ်သည်။ |
| Fixture parity | `ci/check_android_fixtures.sh` (္စ `scripts/check_android_fixtures.py`) | အသစ်ထုတ်ထားသော Norito ပွဲစဉ်များသည် Rust canonical set နှင့် ကိုက်ညီကြောင်း သေချာပါစေ။ ဂိတ်ပျက်သွားသောအခါ JSON diff ကို ပူးတွဲပါ။ |
| နမူနာအက်ပ်များ | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}` ကိုတည်ဆောက်ပြီး `scripts/android_sample_localization.py` မှတစ်ဆင့် ဒေသန္တရပြုလုပ်ထားသော ဖန်သားပြင်ဓာတ်ပုံများကို အတည်ပြုသည်။ |
| Docs/I18N | `ci/check_android_docs_i18n.sh` | README + ဒေသန္တရ အမြန်စတင်မှုများကို စောင့်ကြပ်သည်။ ထုတ်ဝေမှုဌာနခွဲတွင် doc တည်းဖြတ်ပြီးနောက် ထပ်မံလုပ်ဆောင်ပါ။ |
| ဒက်ရှ်ဘုတ် parity | `ci/check_android_dashboard_parity.sh` | CI/ပို့ကုန်မက်ထရစ်များသည် Rust အမျိုးအစားများနှင့် ကိုက်ညီကြောင်း အတည်ပြုသည်။ T+1 အတည်ပြုမှုအတွင်း လိုအပ်သည်။ |
| SDK မွေးစားခြင်း မီးခိုး | `ci/sdk_sorafs_orchestrator.sh` | ရင်းမြစ်ပေါင်းစုံ Sorafs သံစုံတီးဝိုင်းကို လက်ရှိ SDK နှင့် ပေါင်းစပ်မှုကို လေ့ကျင့်သည်။ အဆင့်လိုက် လက်ရာများကို မတင်မီ လိုအပ်ပါသည်။ |
| သက်သေခံအတည်ပြုချက် | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | `artifacts/android/attestation/**` အောက်တွင် StrongBox/TEE သက်သေခံချက်အစုအဝေးများကို စုစည်းသည်။ အနှစ်ချုပ်ကို GA packets တွင် ပူးတွဲပါ။ |
| စက်ပစ္စည်း-ဓာတ်ခွဲခန်းအထိုင် အတည်ပြုခြင်း | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | ထုပ်ပိုးမှုများကို ထုတ်လွှတ်ရန်အတွက် အထောက်အထားများ မတွဲမီ ကိရိယာတန်ဆာပလာအစုအဝေးများကို မှန်ကန်ကြောင်း စစ်ဆေးပါ။ CI သည် `fixtures/android/device_lab/slot-sample` (telemetry/attestation/queue/logs + `sha256sum.txt`) ရှိ နမူနာအပေါက်နှင့် ဆန့်ကျင်ဘက်ဖြစ်သည်။ |

> ** အကြံပြုချက်-** ဤအလုပ်များကို `android-release` Buildkite ပိုက်လိုင်းတွင် ထည့်သွင်းရန်၊
> လွှတ်တင်ရေးဌာနခွဲထိပ်ဖျားဖြင့် ဂိတ်တိုင်းကို အလိုအလျောက် ပြန်လည်လည်ပတ်စေသော ရက်သတ္တပတ်များ။

စုစည်းထားသော `.github/workflows/android-and6.yml` အလုပ်သည် ပျဉ်းမနား၊
test-suite၊ attestation-summary နှင့် device-lab slot သည် PR/push တိုင်းတွင် စစ်ဆေးသည်။
Android ရင်းမြစ်များကို ထိခြင်း၊ `artifacts/android/{lint,tests,attestation,device_lab}/` အောက်တွင် အထောက်အထားများ အပ်လုဒ်လုပ်ခြင်း။

## 3. Lint & Dependency Scans

repo root မှ `scripts/android_lint_checks.sh --version <semver>` ကို run ပါ။ ဟိ
ဇာတ်ညွှန်းကို လုပ်ဆောင်သည်-

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- အစီရင်ခံစာများနှင့် မှီခိုမှု-ကာကွယ်မှုဆိုင်ရာ ရလဒ်များကို အောက်တွင် သိမ်းဆည်းထားသည်။
  ထုတ်ဝေမှုအတွက် `artifacts/android/lint/<label>/` နှင့် `latest/` သင်္ကေတ
  ပိုက်လိုင်းများ။
- ပိုးမွှားတွေ့ရှိချက်များကို ပြုပြင်မွမ်းမံခြင်း သို့မဟုတ် ထုတ်ဝေမှုတွင် ထည့်သွင်းမှု လိုအပ်သည်။
  RFC သည် လက်ခံထားသော အန္တရာယ်ကို မှတ်တမ်းတင်ခြင်း (ဖြန့်ချိရေးအင်ဂျင်နီယာ + ပရိုဂရမ်မှ အတည်ပြုသည်။
  ခဲ)။
- `dependencyGuardBaseline` သည် မှီခိုမှုလော့ခ်ကို ပြန်လည်ထုတ်ပေးသည်။ ကွဲပြားမှုကို ပူးတွဲပါ။
  go/no-go packet သို့။

## 4. Device Lab & StrongBox လွှမ်းခြုံမှု

1. ဖော်ပြထားသော စွမ်းရည်ခြေရာခံကိရိယာကို အသုံးပြု၍ Pixel + Galaxy စက်ပစ္စည်းများကို သိမ်းဆည်းပါ။
   `docs/source/compliance/android/device_lab_contingency.md`။ ပိတ်ဆို့ခြင်းများ
   ရရှိနိုင်မှု `။
3. ကိရိယာတန်ဆာပလာ matrix ကိုဖွင့်ပါ (စက်ထဲတွင် suite/ABI စာရင်းကို မှတ်တမ်းတင်ပါ။
   ခြေရာခံသူ)။ ထပ်ခါတလဲလဲ မအောင်မြင်သော်လည်း အဖြစ်အပျက်မှတ်တမ်းရှိ မအောင်မြင်မှုများကို ဖမ်းယူပါ။
4. Firebase Test Lab သို့ ပြန်လည်ရောက်ရှိရန် လိုအပ်ပါက လက်မှတ်တစ်စောင် တင်သွင်းပါ။ လက်မှတ်လင့်ခ်
   အောက်ဖော်ပြပါ စစ်ဆေးစာရင်းတွင်။

## 5. Compliance & Telemetry Artefacts- EU အတွက် `docs/source/compliance/android/and6_compliance_checklist.md` ကို လိုက်နာပါ။
  နှင့် JP တင်ပြချက်များ။ `docs/source/compliance/android/evidence_log.csv` ကို အပ်ဒိတ်လုပ်ပါ။
  hashes + Buildkite အလုပ် URL များ။
- telemetry redaction အထောက်အထားများမှတစ်ဆင့် ပြန်လည်စတင်ပါ။
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`။
  ရလာတဲ့ JSON ကို အောက်မှာ သိမ်းဆည်းပါ။
  `artifacts/android/telemetry/<version>/status.json`။
- schema diff output ကို မှတ်တမ်းတင်ပါ။
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  Rust တင်ပို့သူများနှင့် တူညီကြောင်း သက်သေပြရန်။

## 6. Provenance, SBOM, and Publishing

1. ထုတ်ဝေသည့် ပိုက်လိုင်းကို ခြောက်သွေ့အောင် လုပ်ဆောင်ပါ-

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore သက်သေကို ထုတ်ပါ-

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` ကို ပူးတွဲ၍ လက်မှတ်ရေးထိုးပါ။
   `checksums.sha256` မှ ထွက်ရှိသော RFC ။
4. အစစ်အမှန် Maven repository သို့ အရောင်းမြှင့်တင်သောအခါ၊ ပြန်ဖွင့်ပါ။
   `scripts/publish_android_sdk.sh` မပါဘဲ `--dry-run`၊ ကွန်ဆိုးလ်ကို ဖမ်းယူပါ
   မှတ်တမ်းယူပြီး ရလဒ်ထွက်ပစ္စည်းများကို `artifacts/android/maven/<semver>` သို့ အပ်လုဒ်လုပ်ပါ။

## 7. Submission Packet Template

GA/LTS/hotfix ထုတ်ဝေမှုတိုင်းတွင် အောက်ပါတို့ ပါဝင်သင့်သည်-

1. **ပြီးပြည့်စုံသော စစ်ဆေးစာရင်း** — ဤဖိုင်၏ဇယားကို ကူးယူပါ၊ အကြောင်းအရာတစ်ခုစီကို အမှန်ခြစ်ပြီး လင့်ခ်ကို ကူးပါ။
   ပစ္စည်းများကို ပံ့ပိုးပေးခြင်း (Buildkite run၊ မှတ်တမ်းများ၊ doc diffs)။
2. **စက်ပစ္စည်းဓာတ်ခွဲခန်းအထောက်အထား** — သက်သေခံချက်အစီရင်ခံစာအကျဉ်းချုပ်၊ ကြိုတင်မှာယူမှုမှတ်တမ်း နှင့်
   မည်သည့်အရေးပေါ်လှုပ်ရှားမှုများ။
3. **Telemetry packet** — ပြန်လည်ဖြေရှင်းမှုအခြေအနေ JSON၊ schema diff၊ လင့်ခ်သို့
   `docs/source/sdk/android/telemetry_redaction.md` အပ်ဒိတ်များ (ရှိပါက)။
4. **လိုက်နာမှုဆိုင်ရာ အထောက်အထားများ** — လိုက်နာမှုဖိုင်တွဲတွင် ထည့်သွင်းမှုများ/မွမ်းမံမှုများ၊
   အသစ်ပြန်လည်ပြင်ဆင်ထားသော အထောက်အထားမှတ်တမ်း CSV။
5. **Provenance bundle** — SBOM၊ Sigstore လက်မှတ် နှင့် `checksums.sha256`။
6. **ထုတ်ဝေမှုအကျဉ်းချုပ်** — `status.md` အကျဉ်းချုပ်နှင့် ပူးတွဲပါရှိသော တစ်မျက်နှာ ခြုံငုံသုံးသပ်ချက်
   အထက်ဖော်ပြပါ (ရက်စွဲ၊ ဗားရှင်း၊ မည်သည့်သက်ညှာသောဂိတ်များ၏ အသားပေးဖော်ပြချက်)။

ပက်ကတ်ကို `artifacts/android/releases/<version>/` အောက်တွင် သိမ်းဆည်းပြီး ၎င်းကို ကိုးကားပါ။
`status.md` နှင့် RFC ထွက်ရှိသည်။

- `scripts/run_release_pipeline.py --publish-android-sdk ...` အလိုအလျောက်
  နောက်ဆုံးပေါ် သံမဏိမှတ်တမ်း (`artifacts/android/lint/latest`) နှင့် မိတ္တူ
  လိုက်နာမှုအထောက်အထား `artifacts/android/releases/<version>/` သို့ဝင်ရောက်ပါ။
  တင်သွင်းမှု ပက်ကေ့ချ်တွင် အမြဲတမ်း canonical တည်နေရာတစ်ခုရှိသည်။

---

**သတိပေးချက်-** CI အလုပ်အသစ်များ၊ လိုက်နာမှုဆိုင်ရာ ပစ္စည်းများ၊
သို့မဟုတ် တယ်လီမီတာ လိုအပ်ချက်များကို ထည့်သွင်းထားသည်။ လမ်းပြမြေပုံပါ အကြောင်းအရာ AND6 သည် ဖွင့်သည့်အချိန်အထိ ဆက်လက်တည်ရှိနေပါသည်။
စစ်ဆေးချက်စာရင်းနှင့် ဆက်စပ်သော အလိုအလျောက်စနစ်သည် နှစ်ဆက်ဆက်တိုက် ထွက်ရှိမှုအတွက် တည်ငြိမ်နေပါသည်။
ရထားများ
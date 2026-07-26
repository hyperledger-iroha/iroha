---
lang: my
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK Binding & Fixture အုပ်ချုပ်မှု

လမ်းပြမြေပုံပေါ်ရှိ WP1-E သည် “docs/bindings” ကို ထိန်းသိမ်းရန် canonical place အဖြစ်၊
ဘာသာစကား ချိတ်ဆက်မှု အခြေအနေ။ ဤစာတမ်းသည် စည်းနှောင်ထားသောစာရင်းကို မှတ်တမ်းတင်သည်၊
အသစ်ပြန်လည်ထုတ်လုပ်သည့်အမိန့်များ၊ ပျံကျအစောင့်များနှင့် အထောက်အထားတည်နေရာများ ထို့ကြောင့် GPU တူညီမှု
ဂိတ်များ (WP1-E/F/G) နှင့် cross-SDK cadence council တွင် တစ်ခုတည်းသော ရည်ညွှန်းချက်ရှိသည်။

## မျှဝေထားသော အကာအရံများ
- ** Canonical playbook:** `docs/source/norito_binding_regen_playbook.md` စာလုံးပေါင်းထွက်သည်။
  လည်ပတ်မှုမူဝါဒ၊ မျှော်လင့်ထားသည့် အထောက်အထားများနှင့် Android အတွက် တိုးမြှင့်လုပ်ဆောင်မှုအသွားအလာ၊
  Swift၊ Python နှင့် အနာဂတ်နှောင်ကြိုးများ။
- **Norito schema တူညီမှု-** `scripts/check_norito_bindings_sync.py` (ဆင့်ခေါ်သည်
  `scripts/check_norito_bindings_sync.sh` ဖြင့် CI ဖြင့် ဂိတ်ပေါက်ထားသည်။
  `ci/check_norito_bindings_sync.sh`) သည် Rust၊ Java သို့မဟုတ် Python ကိုတည်ဆောက်သောအခါတွင်ပိတ်ဆို့သည်
  schema artefacts ပျံ့။
- **Cadence watchdog:** `scripts/check_fixture_cadence.py` က ဖတ်ပါတယ်။
  `artifacts/*_fixture_regen_state.json` ကို ဖိုင်များနှင့် အင်္ဂါနေ့/သောကြာ (Android၊
  Python) နှင့် Wed (Swift) ပြတင်းပေါက်များ ဖြစ်သောကြောင့် လမ်းပြမြေပုံဂိတ်များတွင် စစ်ဆေးနိုင်သော အချိန်တံဆိပ်များ ရှိသည်။

## Binding matrix

| စည်းနှောင်ခြင်း | ဝင်ခွင့်အမှတ် | Fixture / regen command | ရေစုန်မျော | အထောက်အထား |
|--------|----------------------------------------------------------------|-----------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (ရွေးချယ်နိုင်သည် `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| စပါးအုံး | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## စည်းနှောင်မှုအသေးစိတ်

### Android (Java)
Android SDK သည် `java/iroha_android/` အောက်တွင်နေထိုင်ပြီး canonical Norito ကိုစားသုံးသည်
`scripts/android_fixture_regen.sh` မှ ထုတ်လုပ်သော ပစ္စည်းများ။ အဲဒါက ပို့ကုန်အထောက် အကူပေါ့။
Rust toolchain မှ လတ်ဆတ်သော `.norito` blobs၊ အပ်ဒိတ်များ
`artifacts/android_fixture_regen_state.json` နှင့် cadence metadata တို့ကို မှတ်တမ်းတင်သည်။
`scripts/check_fixture_cadence.py` နှင့် အုပ်ချုပ်မှု ဒက်ရှ်ဘုတ်များ စားသုံးသည်။ ရေစုန်မျောခြင်းသည်
`scripts/check_android_fixtures.py` (သို့လည်း ကြိုးတပ်၍ စစ်ဆေးတွေ့ရှိခဲ့သည်။
`ci/check_android_fixtures.sh`) နှင့် `java/iroha_android/run_tests.sh` ၊
JNI စည်းနှောင်မှုများ၊ WorkManager တန်းစီခြင်းကို ပြန်လည်ပြသခြင်းနှင့် StrongBox တုံ့ပြန်မှုများကို လေ့ကျင့်ခန်းလုပ်သည်။
အလှည့်ကျအထောက်အထားများ၊ ပျက်ကွက်မှတ်စုများနှင့် ပြန်လည်လုပ်ဆောင်သည့် မှတ်တမ်းများအောက်တွင် နေထိုင်ပါသည်။
`artifacts/android/fixture_runs/`။

### Swift (macOS/iOS)
`IrohaSwift/` သည် တူညီသော Norito ကို `scripts/swift_fixture_regen.sh` မှတဆင့် မှန်ကြည့်သည်။
script သည် လည်ပတ်မှုပိုင်ရှင်၊ cadence အညွှန်းနှင့် အရင်းအမြစ် (`live` vs `archive`) ကို မှတ်တမ်းတင်သည်
`artifacts/swift_fixture_regen_state.json` အတွင်းရှိ မက်တာဒေတာကို ဖိုင်ထဲသို့ ထည့်ပေးသည်။
cadence checker ။ `scripts/swift_fixture_archive.py` သည် ထိန်းသိမ်းသူများအား စားသုံးနိုင်စေပါသည်။
သံချေးထုတ်ပေးသော မှတ်တမ်းများ၊ `scripts/check_swift_fixtures.py` နှင့်
`ci/check_swift_fixtures.sh` သည် byte-level parity နှင့် SLA အသက်ကန့်သတ်ချက်များကို တွန်းအားပေးနေစဉ်၊
`scripts/swift_fixture_regen.sh` သည် လူကိုယ်တိုင်အတွက် `SWIFT_FIXTURE_EVENT_TRIGGER` ကို ပံ့ပိုးသည်
လည်ပတ်မှုများ။ တိုးမြှင့်လုပ်ဆောင်မှုအသွားအလာ၊ KPI နှင့် ဒက်ရှ်ဘုတ်များကို မှတ်တမ်းတင်ထားသည်။
`docs/source/swift_parity_triage.md` နှင့် cadence briefs များအောက်တွင်
`docs/source/sdk/swift/`။

### Python
Python client (`python/iroha_python/`) သည် Android တပ်ဆင်မှုများကို မျှဝေသည်။ ပြေးသည်။
`scripts/python_fixture_regen.sh` သည် နောက်ဆုံးထွက် `.norito` ကို ဆွဲထုတ်ပြီး ပြန်လည်ဆန်းသစ်သည်
`python/iroha_python/tests/fixtures/` နှင့် cadence မက်တာဒေတာကို ထုတ်လွှတ်ပါမည်။
`artifacts/python_fixture_regen_state.json` သည် ပထမဆုံး post-roadmap လည်ပတ်မှု တစ်ကြိမ်ဖြစ်သည်။
ဖမ်းထားသည်။ `scripts/check_python_fixtures.py` နှင့်
`python/iroha_python/scripts/run_checks.sh` ဂိတ် pytest၊ mypy၊ ruff နှင့် fixture
ဒေသအလိုက်နှင့် CI တွင် တန်းတူညီမျှမှု။ end-to-end docs (`docs/source/sdk/python/…`) နှင့်
binding regen playbook သည် Android နှင့် လည်ပတ်မှုများကို မည်သို့ညှိနှိုင်းရမည်ကို ဖော်ပြသည်။
ပိုင်ရှင်များ။

### JavaScript
`javascript/iroha_js/` သည် ဒေသတွင်း `.norito` ဖိုင်များကို အားမကိုးဘဲ WP1-E သီချင်းများ
၎င်း၏ထုတ်ပြန်ချက်အထောက်အထားကြောင့် GPU CI လမ်းကြောများသည် ပြည့်စုံသောသက်သေကို အမွေဆက်ခံသည်။ လွှတ်တိုင်း
`npm run release:provenance` (ပါဝါဖြင့် ဖမ်းယူပါသည်။
`javascript/iroha_js/scripts/record-release-provenance.mjs`) ထုတ်ပေးပြီး ဆိုင်းဘုတ်များ
`scripts/js_sbom_provenance.sh` ပါသော SBOM အစုအဝေးများသည် လက်မှတ်ရေးထိုးထားသော အဆင့်ခြောက်ခြားခြင်းကို လုပ်ဆောင်သည်
(`scripts/js_signed_staging.sh`) နှင့် registry artefact ကို စစ်ဆေးသည်
`javascript/iroha_js/scripts/verify-release-tarball.mjs`။ ရလဒ် metadata ကို
`artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/` လက်အောက်ရှိ မြေများ၊
`artifacts/js/sbom/` နှင့် `artifacts/js/verification/`၊ အဆုံးအဖြတ်ပေးသော၊
လမ်းပြမြေပုံ JS5/JS6 နှင့် WP1-F စံနှုန်းများ လုပ်ဆောင်ခြင်းအတွက် အထောက်အထား။ ကစားစာအုပ် ထုတ်ဝေသည်။
`docs/source/sdk/js/` သည် အလိုအလျောက်စနစ်နှင့် ချိတ်ဆက်ထားသည်။
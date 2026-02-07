---
lang: my
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# အရေးပေါ် Canon & TTL မူဝါဒ (MINFO-6a)

လမ်းပြမြေပုံရည်ညွှန်း- **MINFO-6a — အရေးပေါ်ကျမ်းဂန်နှင့် TTL မူဝါဒ**။

ဤစာရွက်စာတမ်းသည် Torii နှင့် CLI တွင် ပို့ဆောင်ပေးသည့် TTL လိုက်နာမှု နှင့် အုပ်ချုပ်မှုဆိုင်ရာ တာဝန်များကို ငြင်းဆိုထားသည်။ အော်ပရေတာများသည် ထည့်သွင်းမှုအသစ်များကို မထုတ်ပြန်မီ သို့မဟုတ် အရေးပေါ်ကျမ်းဂန်များကို မခေါ်ဆိုမီ ဤစည်းမျဉ်းများကို လိုက်နာရမည်ဖြစ်သည်။

## အဆင့်သတ်မှတ်ချက်များ

| ဆင့် | မူရင်း TTL | ပြန်လည်သုံးသပ် Window | လိုအပ်ချက်များ |
|--------|----------------|----------------|----------------|
| စံ | ရက်ပေါင်း 180 (`torii.sorafs_gateway.denylist.standard_ttl`) | n/a | `issued_at` ပေးရပါမည်။ `expires_at` ကို ချန်လှပ်ထားနိုင်သည်၊ Torii သည် `issued_at + standard_ttl` သို့ ပုံသေသတ်မှတ်ထားပြီး ပိုရှည်သောဝင်းဒိုးများကို ငြင်းပယ်သည်။ |
| အရေးပေါ် | ရက် 30 (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 ရက် (`torii.sorafs_gateway.denylist.emergency_review_window`) | ကြိုတင်အတည်ပြုထားသော canon ကိုရည်ညွှန်းသော အချည်းနှီးမဟုတ်သော `emergency_canon` အညွှန်းတစ်ခု လိုအပ်သည် (ဥပမာ၊ `csam-hotline`)။ `issued_at` + `expires_at` သည် ရက် 30-ကြာဝင်းဒိုးအတွင်း ဝင်ရောက်ရမည်ဖြစ်ပြီး ပြန်လည်သုံးသပ်သည့်အထောက်အထားများသည် အလိုအလျောက်ထုတ်ပေးသည့် နောက်ဆုံးရက် (`issued_at + review_window`) ကို ကိုးကားရမည်ဖြစ်သည်။ |
| အမြဲတမ်း | သက်တမ်းကုန်ဆုံးခြင်း | n/a | လူများစု အုပ်ချုပ်မှု ဆုံးဖြတ်ချက်များအတွက် သီးသန့်ထားသည်။ ထည့်သွင်းသူများသည် အချည်းနှီးမဟုတ်သော `governance_reference` (vote id၊ manifesto hash စသည်ဖြင့်) ကိုကိုးကားရပါမည်။ `expires_at` ကို ငြင်းပယ်သည်။ |

ပုံသေများသည် `torii.sorafs_gateway.denylist.*` နှင့် `iroha_cli` မှတစ်ဆင့် Torii ဖိုင်ကို ပြန်လည်စတင်ခြင်းမပြုမီ မမှန်ကန်သောထည့်သွင်းမှုများကိုဖမ်းမိရန် ကန့်သတ်ချက်များကို ထင်ဟပ်စေသည်။

## အလုပ်အသွားအလာ

1. **မက်တာဒေတာကို ပြင်ဆင်ပါ-** တွင် `policy_tier`၊ `issued_at`၊ `expires_at` (ဖြစ်နိုင်လျှင်) နှင့် `emergency_canon`/`governance_reference` တို့ပါဝင်ပါသည်။
2. **စက်တွင်းတွင် အတည်ပြုပါ-** `iroha app sorafs gateway lint-denylist --path <denylist.json>` ကို run ထားသောကြောင့် CLI သည် ဖိုင်ကို မကျူးလွန်မီ သို့မဟုတ် ပင်မထိုးမီ အဆင့်အလိုက် သတ်မှတ်ထားသော TTL များနှင့် လိုအပ်သော အကွက်များကို တွန်းအားပေးပါသည်။
3. ** သက်သေခံအထောက်အထားများထုတ်ဝေခြင်း-** GAR အမှုတွဲ (အစီအစဉ်ထုပ်ပိုးမှု၊ လူထုဆန္ဒခံယူပွဲမိနစ်စသည်ဖြင့်) သို့ဝင်ရောက်မှုတွင်ဖော်ပြထားသော canon ID သို့မဟုတ် အုပ်ချုပ်မှုအကိုးအကားကို ပူးတွဲပါ ပူးတွဲပါရှိသောကြောင့် စာရင်းစစ်များသည် ဆုံးဖြတ်ချက်ကိုခြေရာခံနိုင်သည်။
4. **အရေးပေါ်ထည့်သွင်းမှုများကို ပြန်လည်သုံးသပ်ပါ-** အရေးပေါ် canons များသည် ရက် 30 အတွင်း အလိုအလျောက် သက်တမ်းကုန်ပါသည်။ အော်ပရေတာများသည် 7-ရက်ကြာဝင်းဒိုးအတွင်း အချက်အလက်ပြန်လည်သုံးသပ်ခြင်းကို အပြီးသတ်ပြီး ရလဒ်ကို ဝန်ကြီးဌာနခြေရာခံကိရိယာ/SoraFS အထောက်အထားစတိုးတွင် မှတ်တမ်းတင်ရပါမည်။
5. **Torii ကို ပြန်လည်စတင်ပါ-** အတည်ပြုပြီးသည်နှင့်၊ `torii.sorafs_gateway.denylist.path` မှတစ်ဆင့် ငြင်းပယ်စာရင်းလမ်းကြောင်းကို ဖြန့်ကျက်ပြီး Torii ကို ပြန်လည်စတင်/ပြန်ဖွင့်ပါ။ ထည့်သွင်းမှုများကို လက်ခံခြင်းမပြုမီ runtime သည် တူညီသောကန့်သတ်ချက်များကို တွန်းအားပေးသည်။

## Tooling & References

- Runtime policy enforcement သည် `sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) တွင်နေထိုင်ပြီး `torii.sorafs_gateway.denylist.*` သွင်းအားများကိုခွဲခြမ်းစိတ်ဖြာသောအခါ loader သည် ယခုအခါ အဆင့် metadata ကိုအသုံးပြုပါသည်။
- CLI validation သည် `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) အတွင်းရှိ runtime semantics ကို ထင်ဟပ်စေသည်။ TTL များသည် configured window ထက်ကျော်လွန်သည့်အခါ သို့မဟုတ် မဖြစ်မနေ canon/governance ကိုးကားချက်များ ပျောက်ဆုံးနေချိန်တွင် linter သည် ပျက်ကွက်ပါသည်။
- Config knob များကို `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) အောက်တွင် သတ်မှတ်ထားသောကြောင့် အော်ပရေတာများသည် အုပ်ချုပ်မှုမှ ကွဲပြားခြားနားသော နယ်နိမိတ်များကို အတည်ပြုပါက TTLs/review နောက်ဆုံးရက်များကို ပြင်ဆင်နိုင်သည်။
- အများသူငှာနမူနာငြင်းပယ်စာရင်း (`docs/source/sorafs_gateway_denylist_sample.json`) သည် ယခုအခါ အဆင့်သုံးဆင့်စလုံးကို သရုပ်ဖော်ထားပြီး အသစ်ထည့်သွင်းမှုများအတွက် canonical template အဖြစ် အသုံးပြုသင့်သည်။ဤအကာအရံများသည် လမ်းပြမြေပုံပါအကြောင်းအရာ **MINFO-6a** ကို အရေးပေါ် canon စာရင်းကို ကုဒ်လုပ်ခြင်း၊ အကန့်အသတ်မရှိသော TTLs များကို တားဆီးခြင်းနှင့် အမြဲတမ်းလုပ်ကွက်များအတွက် တိကျပြတ်သားသော အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထားများကို အတင်းအကြပ်ပြုလုပ်ခြင်းဖြင့် ကျေနပ်စေသည်။

## Registry Automation & Evidence Exports

အရေးပေါ် canon အတည်ပြုချက်များသည် အဆုံးအဖြတ်ပေးသော မှတ်ပုံတင်ခြင်းလျှပ်တစ်ပြက်နှင့် a
Torii သည် ငြင်းပယ်စာရင်းကို မဖွင့်မီ diff အတွဲလိုက်။ အောက်မှာ tooling ပါ။
`xtask/src/sorafs.rs` နှင့် CI ကြိုးကြိုး `ci/check_sorafs_gateway_denylist.sh`
အလုပ်အသွားအလာတစ်ခုလုံးကို လွှမ်းခြုံထားသည်။

### Canonical bundle မျိုးဆက်

1. လုပ်ဆောင်နေသည့် လုပ်ငန်းစဉ်တစ်ခုတွင် ကုန်ကြမ်းထည့်သွင်းမှုများ (ပုံမှန်အားဖြင့် အုပ်ချုပ်မှုမှ ပြန်လည်သုံးသပ်ထားသော ဖိုင်)
   လမ်းညွှန်။
2. အောက်ပါမှတစ်ဆင့် JSON ကို Canonicalise လုပ်ပြီးတံဆိပ်ခတ်ပါ။
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   ညွှန်ကြားချက်သည် လက်မှတ်ရေးထိုးထားသော အဆင်ပြေသော `.json` အတွဲ၊ Norito `.to` ကို ထုတ်လွှတ်သည်
   စာအိတ်၊ နှင့် Merkle-root စာသားဖိုင်ကို အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများ မျှော်လင့်ထားသည်။
   `artifacts/ministry/denylist_registry/` (သို့မဟုတ် သင့်
   ရွေးချယ်ထားသောအထောက်အထားပုံး) ထို့ကြောင့် `scripts/ministry/transparency_release.py` လုပ်နိုင်သည်
   `--artifact denylist_bundle=<path>` ဖြင့် နောက်ပိုင်းတွင် ကောက်ယူပါ။
3. ထုတ်လုပ်ထားသော `checksums.sha256` ကို မတွန်းမီ အစုအဝေးနှင့်အတူ ထားပါ
   SoraFS/GAR သို့ CI ၏ `ci/check_sorafs_gateway_denylist.sh` သည် အလားတူ လေ့ကျင့်ခန်းဖြစ်သည်။
   Tooling တွင်အလုပ်လုပ်ကြောင်းအာမခံရန်နမူနာငြင်းဆိုစာရင်းမှကူညီသူ `pack`
   ထုတ်ဝေမှုတိုင်း။

### Diff + စာရင်းစစ်အတွဲ

1. အစုအဝေးအသစ်ကို အသုံးပြု၍ ယခင်ထုတ်လုပ်မှုလျှပ်တစ်ပြက်နှင့် နှိုင်းယှဉ်ပါ။
   xtask diff helper-
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON အစီရင်ခံစာသည် ထပ်တိုး/ဖယ်ရှားမှုများအားလုံးကို စာရင်းပြုစုပြီး အထောက်အထားကို ရောင်ပြန်ဟပ်သည်။
   `MinistryDenylistChangeV1` (ကိုးကားသည်။
   `docs/source/sorafs_gateway_self_cert.md` နှင့် လိုက်နာမှု အစီအစဉ်)။
2. Canon တောင်းဆိုမှုတိုင်းတွင် `denylist_diff.json` ကို ပူးတွဲပါ (မည်မျှရှိသည်ကို သက်သေပြသည်
   ထည့်သွင်းမှုများကို ထိတွေ့ခဲ့ပြီး၊ မည်သည့်အဆင့်သို့ ပြောင်းလဲခဲ့ကြောင်းနှင့် မည်သည့်သက်သေအထောက်အထား hash မြေပုံများကို ဖော်ပြခဲ့သည်။
   canonical အစုအဝေး)။
3. ကွဲပြားမှုများကို အလိုအလျောက်ထုတ်ပေးသောအခါ (CI သို့မဟုတ် ပိုက်လိုင်းများထုတ်လွှတ်ခြင်း) ကို တင်ပို့ပါ။
   `denylist_diff.json` လမ်းကြောင်း `--artifact denylist_diff=<path>` မှတဆင့်၊
   ပွင့်လင်းမြင်သာမှု ဖော်ပြချက်သည် ၎င်းကို သန့်စင်သော တိုင်းတာမှုများနှင့်အတူ မှတ်တမ်းတင်သည်။ CI အတူတူပါပဲ။
   ကူညီသူသည် CLI အနှစ်ချုပ်အဆင့်ကိုလုပ်ဆောင်သည့် `--evidence-out <path>` ကိုလက်ခံပြီး
   ထွက်ပေါ်လာသော JSON ကို နောက်ပိုင်းထုတ်ဝေမှုအတွက် တောင်းဆိုထားသော တည်နေရာသို့ ကူးယူသည်။

### ထုတ်ဝေမှုနှင့် ပွင့်လင်းမြင်သာမှု1. အထုပ် + ကွဲပြားသည့် အနုပညာပစ္စည်းများကို သုံးလတစ်ကြိမ် ပွင့်လင်းမြင်သာမှုလမ်းညွှန်တွင် ချလိုက်ပါ။
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`)။ ပွင့်လင်းမြင်သာမှု
   လွတ်မြောက်ရေးကူညီသူသည် ၎င်းတို့ကို ထည့်သွင်းနိုင်သည်-
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. သုံးလပတ်အစီရင်ခံစာတွင် ထုတ်ပေးထားသောအတွဲ/ကွဲပြားမှုကို ကိုးကားပါ။
   (`docs/source/ministry/reports/<YYYY-Q>.md`) နှင့် တူညီသောလမ်းကြောင်းများကို ပူးတွဲပါ။
   စာရင်းစစ်များသည် ဝင်ရောက်ကြည့်ရှုခြင်းမရှိဘဲ အထောက်အထားလမ်းကြောင်းကို ပြန်လည်ပြသနိုင်စေရန် GAR မဲပက်ကတ်
   အတွင်း CI ။ `ci/check_sorafs_gateway_denylist.sh --evidence-out \
   artifacts/ministry/denylist_registry//denylist_evidence.json` ယခု
   pack/diff/proof dry-run ကိုလုပ်ဆောင်သည် (`iroha_cli app sorafs gateway ကိုခေါ်ဆိုသည်
   အထောက်အထား` ဘောင်အောက်တွင်) ထို့ကြောင့် အလိုအလျောက်စနစ်သည် အကျဉ်းချုပ်နှင့် ယှဉ်တွဲတည်ရှိနိုင်သည်။
   canonical အစုအဝေးများ။
3. ထုတ်ဝေပြီးနောက်၊ အုပ်ချုပ်မှုဆိုင်ရာ ဝန်ထုပ်ဝန်ပိုးကို ကျောက်ချပါ။
   `cargo xtask ministry-transparency anchor` (အလိုအလျောက်ခေါ်ဆိုသည်။
   `--governance-dir` ကိုပေးသောအခါ `transparency_release.py`) ထို့ကြောင့်၊
   denylist registry digest သည် ပွင့်လင်းမြင်သာမှုအဖြစ် DAG သစ်ပင်တွင် ပေါ်လာသည်။
   လွှတ်ပေးရန်။

ဤလုပ်ငန်းစဉ်ကို လိုက်နာခြင်းဖြင့် "registry automation and evidence export" ကို ပိတ်ပါမည်။
`roadmap.md:450` တွင် gap ခေါ်ပြီး အရေးပေါ် canonတိုင်းကို သေချာစေပါသည်။
ပြန်လည်ထုတ်လုပ်နိုင်သော ပစ္စည်းများ၊ JSON ကွဲပြားမှုများနှင့် ပွင့်လင်းမြင်သာမှုမှတ်တမ်းတို့နှင့်အတူ ဆုံးဖြတ်ချက်ကို ရောက်ရှိလာပါသည်။
ထည့်သွင်းမှုများ။

### TTL & Canon Evidence Helper

အစုအစည်း/ကွဲပြားသည့်အတွဲကို ထုတ်လုပ်ပြီးနောက်၊ ဖမ်းယူရန် CLI အထောက်အထားအကူအညီကို ဖွင့်ပါ။
အုပ်ချုပ်မှုလိုအပ်သော TTL အနှစ်ချုပ်များနှင့် အရေးပေါ်သုံးသပ်ချက် နောက်ဆုံးရက်များ-

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

ညွှန်ကြားချက်သည် ရင်းမြစ် JSON ကို ဆိုင်းငံ့ထားကာ ဝင်ရောက်မှုတိုင်းကို အတည်ပြုပြီး ကျစ်လစ်သိပ်သည်းစွာ ထုတ်လွှတ်သည်။
ပါဝင်သော အနှစ်ချုပ်-

- `kind` နှင့် အစောဆုံး/နောက်ဆုံးပေါ် မူဝါဒအဆင့်အလိုက် စုစုပေါင်းထည့်သွင်းမှုများ
  အချိန်တံဆိပ်တုံးများကို ကြည့်ရှုခဲ့သည်။
- `emergency_reviews[]` စာရင်းတစ်ခုစီသည် အရေးပေါ် canon တစ်ခုစီကို ၎င်း၏နှင့်အတူ ရေတွက်ပါသည်။
  ဖော်ပြချက်၊ ထိရောက်သော သက်တမ်းကုန်ဆုံးမှု၊ အများဆုံးခွင့်ပြုထားသော TTL နှင့် တွက်ချက်ထားသည်။
  `review_due_by` နောက်ဆုံးရက်။

စာရင်းစစ်များအတွက် `denylist_evidence.json` ကို ထုပ်ပိုးထားသောအတွဲ/ကွဲပြားမှုနှင့်အတူ ပူးတွဲပါ
CLI ကို ပြန်လည်မလုပ်ဆောင်ဘဲ TTL လိုက်နာမှုကို အတည်ပြုပါ။ CI ထုတ်ပေးပြီးသား အလုပ်များ
အစုအဝေးများသည် အကူအညီပေးသူကို ဖိတ်ခေါ်နိုင်ပြီး အထောက်အထားလက်ရာများကို ထုတ်ဝေနိုင်သည် (ဥပမာအားဖြင့်
`ci/check_sorafs_gateway_denylist.sh --evidence-out <path>`) ကို ခေါ်ဆိုခြင်း)
Canon တိုင်းသည် တသမတ်တည်း အကျဉ်းချုပ်ဖြင့် မြေများကို တောင်းဆိုသည်။

### Merkle မှတ်ပုံတင်ခြင်းအထောက်အထား

MINFO-6 တွင် မိတ်ဆက်ထားသော Merkle မှတ်ပုံတင်ခြင်းအား ထုတ်ဝေရန် အော်ပရေတာများ လိုအပ်သည်။
TTL အနှစ်ချုပ်နှင့်အတူ root နှင့် per-entry အထောက်အထားများ။ ပြေးပြီးပြီးချင်း
အထောက်အထားအကူအညီပေးသူ Merkle ၏အနုပညာပစ္စည်းများကိုဖမ်းပါ။

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```လျှပ်တစ်ပြက်ရိုက်ချက် JSON သည် BLAKE3 Merkle အမြစ်၊ အရွက်အရေအတွက်နှင့် တစ်ခုစီကို မှတ်တမ်းတင်သည်။
descriptor/hash pair သည် GAR votes သည် hashed the tree အတိအကျကို ကိုးကားနိုင်စေရန်။
`--norito-out` သည် JSON နှင့်အတူ `.to` လက်ရာတစ်ခုကို သိုလှောင်ထားသည်၊
gateways များသည် မခြစ်ဘဲ Norito မှတဆင့် registry entry များကို တိုက်ရိုက်ထည့်သွင်းသည်
stdout။ `merkle proof` သည် မည်သည့်အတွက်မဆို ဦးတည်ချက် bits နှင့် sibling hashes ကို ထုတ်လွှတ်သည်
သုညအခြေခံထည့်သွင်းမှုအညွှန်းကိန်းတစ်ခုစီအတွက် ပါဝင်မှုအထောက်အထားတစ်ခုကို ပူးတွဲတင်ပြရန် ရိုးရှင်းစေသည်။
GAR မှတ်စုတိုတွင် ဖော်ပြထားသော အရေးပေါ် Canon—ရွေးချယ်နိုင်သော Norito မိတ္တူသည် အထောက်အထားကို သိမ်းဆည်းသည်
စာရင်းစာအုပ်တွင် ဖြန့်ချီရန် အသင့်ဖြစ်နေပါပြီ။ JSON နှင့် Norito တို့ကို နောက်တွင် သိမ်းဆည်းပါ။
TTL အကျဉ်းချုပ်နှင့် ကွဲပြားသောအစုအဝေးသို့ ပွင့်လင်းမြင်သာစွာ ထုတ်ပြန်ခြင်းနှင့် အုပ်ချုပ်မှု
anchors သည် တူညီသော root ကိုရည်ညွှန်းသည်။
<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

#Nexus Cross-Dataspace Localnet Proof

ဤ runbook သည် Nexus ပေါင်းစည်းမှု အထောက်အထားကို လုပ်ဆောင်သည်-

- ကန့်သတ်ထားသော သီးသန့်ဒေတာနေရာနှစ်ခု (`ds1`, `ds2`) ဖြင့် 4-peer localnet ကို စတင်ပါ။
- dataspace တစ်ခုစီသို့ အကောင့်လမ်းကြောင်းများ လမ်းကြောင်းများ၊
- dataspace တစ်ခုစီတွင် ပိုင်ဆိုင်မှုတစ်ခုကို ဖန်တီးသည်၊
- လမ်းကြောင်းနှစ်ခုစလုံးတွင် dataspaces များတစ်လျှောက် အနုမြူဗုံးလဲလှယ်ဖြေရှင်းမှုကို လုပ်ဆောင်သည်၊
- ငွေကြေးမလုံလောက်သောခြေထောက်ကိုတင်ပြခြင်းဖြင့် rollback semantics သက်သေပြပြီး ချိန်ခွင်လျှာစစ်ဆေးခြင်းကို မပြောင်းလဲပါ။

Canonical စမ်းသပ်မှုမှာ-
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`။

## အမြန်ပြေးပါ။

repository root မှ wrapper script ကိုသုံးပါ

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

မူရင်းအပြုအမူ-

- cross-dataspace proof test ကိုသာ လုပ်ဆောင်သည်၊
- `NORITO_SKIP_BINDINGS_SYNC=1` အစုံ၊
- `IROHA_TEST_SKIP_BUILD=1` အစုံ၊
- `--test-threads=1` ကိုအသုံးပြုသည်၊
- `--nocapture` အောင်သည်။

## အသုံးဝင်သောရွေးချယ်မှုများ

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` သည် မှုခင်းဆေးပညာအတွက် ယာယီသက်တူရွယ်တူ လမ်းညွှန်များ (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) ကို သိမ်းဆည်းထားသည်။
- `--all-nexus` သည် `mod nexus::` (အပြည့်အဝ Nexus ပေါင်းစည်းမှုအပိုင်း) ကို သက်သေပြစမ်းသပ်ရုံမျှမက လုပ်ဆောင်သည်။

## CI ဂိတ်

CI အကူအညီပေးသူ-

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

ပစ်မှတ်လုပ်ပါ

```bash
make check-nexus-cross-dataspace
```

ဤဂိတ်သည် အဆုံးအဖြတ်ပေးသော သက်သေထုပ်ပိုးမှုကို လုပ်ဆောင်ပြီး ဒေတာအာကာသ အက်တမ်ဖြတ်ကျော်ပါက အလုပ်ပျက်သွားသည်
လဲလှယ်မှုအခြေအနေသည် နောက်ပြန်ဆုတ်သွားသည်။

## Manual Equivalent Commands များ

ပစ်မှတ်ထား သက်သေစမ်းသပ်မှု-

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Nexus အပြည့်အစုံ-

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## မျှော်လင့်ထားသော သက်သေအချက်များ- စာမေးပွဲအောင်မြင်သည်။
- ငွေကြေးမလုံလောက်သော ဖြေရှင်းမှုခြေထောက်အတွက် ရည်ရွယ်ချက်ရှိရှိ ပျက်ကွက်မှုအတွက် မျှော်လင့်ထားသည့် သတိပေးချက်တစ်ခု ပေါ်လာသည်-
  `settlement leg requires 10000 but only ... is available`။
- နောက်ဆုံးလက်ကျန်ငွေတောင်းခံမှုများ အောင်မြင်ပြီးနောက်-
  - အောင်မြင်သော forward swap၊
  - အောင်မြင်သော ပြောင်းပြန်လဲလှယ်မှု၊
  - ငွေကြေးမလုံလောက်သော လဲလှယ်မှု မအောင်မြင်ပါ (မပြောင်းလဲသော လက်ကျန်များ)။

## လက်ရှိ အတည်ပြုခြင်း လျှပ်တစ်ပြက်

**ဖေဖော်ဝါရီ 19 ရက်၊ 2026** တွင် ဤလုပ်ငန်းအသွားအလာသည်-

- ပစ်မှတ်စမ်းသပ်မှု- `1 passed; 0 failed`၊
- Nexus အပြည့်အစုံ- `24 passed; 0 failed`။
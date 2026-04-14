<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

#Sumeragi တရားဝင်မော်ဒယ် (TLA+ / Apalache)

ဤလမ်းညွှန်တွင် Sumeragi commit-path safety and liveness အတွက် ကန့်သတ်ထားသော တရားဝင်မော်ဒယ်တစ်ခုပါရှိသည်။

## နယ်ပယ်

မော်ဒယ်က ရိုက်ကူးသည်-
- အဆင့်တိုးတက်မှု (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`)၊
- ဆန္ဒမဲနှင့် အထမြောက်မှု သတ်မှတ်ချက်များ (`CommitQuorum`, `ViewQuorum`)၊
- NPoS စတိုင် ကျူးလွန်အစောင့်များအတွက်၊
- ခေါင်းစီး/အချေအတင် အထောက်အထားများဖြင့် RBC အကြောင်းရင်း (`Init -> Chunk -> Ready -> Deliver`)၊
- GST နှင့် ရိုးသားသော တိုးတက်မှုလုပ်ဆောင်ချက်များအပေါ် အားနည်းသော တရားမျှတမှုဆိုင်ရာ ယူဆချက်များ။

၎င်းသည် ဝါယာကြိုးဖော်မတ်များ၊ လက်မှတ်များနှင့် ကွန်ရက်ချိတ်ဆက်မှုအသေးစိတ်များကို ရည်ရွယ်ချက်ရှိရှိ ဖယ်ထုတ်သည်။

## ဖိုင်များ

- `Sumeragi.tla`- ပရိုတိုကော မော်ဒယ်နှင့် ဂုဏ်သတ္တိများ။
- `Sumeragi_fast.cfg`- သေးငယ်သော CI-ဖော်ရွေသော ကန့်သတ်ဘောင်။
- `Sumeragi_deep.cfg`- ပိုကြီးသော ဖိစီးမှု ကန့်သတ်ဘောင်။

## သတ္တိ

ပုံစံကွဲများ-
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

ယာယီပိုင်ဆိုင်မှု-
- GST လွန်မျှတမှု ကုဒ်ဖြင့် ပြုလုပ်ထားသော `EventuallyCommit` (`[] (gst => <> committed)`)
  `Next` တွင် လည်ပတ်လုပ်ဆောင်နိုင်သည် (အချိန်လွန်/အမှားပြင်ဆင်မှု အစောင့်များကို ဖွင့်ထားသည်
  တိုးတက်မှုလုပ်ဆောင်ချက်များ)။ ၎င်းသည် မော်ဒယ်ကို Apalache 0.52.x ဖြင့် စစ်ဆေးနိုင်စေပါသည်။
  စစ်ဆေးထားသော ယာယီဂုဏ်သတ္တိများအတွင်းရှိ `WF_` တရားမျှတမှု အော်ပရေတာများကို မပံ့ပိုးပါ။

## ပြေးသည်။

repository root မှ

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### ပြန်လည်ထုတ်လုပ်နိုင်သော စက်တွင်းထည့်သွင်းမှု (Docker မလိုအပ်ပါ)ဤသိုလှောင်ခန်းမှအသုံးပြုသော ပင်ထိုးထားသော ဒေသတွင်း Apalache toolchain ကို ထည့်သွင်းပါ-

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

အပြေးသမားသည် ဤထည့်သွင်းမှုကို အလိုအလျောက် သိရှိသည်-
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`။
ထည့်သွင်းပြီးနောက်၊ `ci/check_sumeragi_formal.sh` သည် အပို env vars မပါဘဲ အလုပ်လုပ်သင့်သည်-

```bash
bash ci/check_sumeragi_formal.sh
```

Apalache သည် `PATH` တွင်မဟုတ်ပါက၊ သင်သည်-

- `APALACHE_BIN` ကို executable path သို့ သတ်မှတ်ပါ။
- Docker ကိုသုံးပါ (`docker` ကိုရရှိနိုင်သောအခါ မူရင်းအတိုင်းဖွင့်ထားသည်)
  ပုံ- `APALACHE_DOCKER_IMAGE` (မူရင်း `ghcr.io/apalache-mc/apalache:latest`)
  - လည်ပတ်နေသော Docker daemon လိုအပ်သည်။
  - `APALACHE_ALLOW_DOCKER=0` ဖြင့် လှည့်ပြန်ခြင်းကို ပိတ်ပါ။

ဥပမာများ-

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## မှတ်ချက်

- ဤမော်ဒယ်သည် လည်ပတ်နိုင်သော Rust မော်ဒယ်စမ်းသပ်မှုများကို ဖြည့်စွက်ပေးပါသည်။
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  နှင့်
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`။
- စစ်ဆေးမှုများသည် `.cfg` ဖိုင်များတွင် အဆက်မပြတ်တန်ဖိုးများဖြင့် ကန့်သတ်ထားသည်။
- PR CI သည် ဤစစ်ဆေးမှုများကို `.github/workflows/pr.yml` မှတစ်ဆင့် လုပ်ဆောင်သည်။
  `ci/check_sumeragi_formal.sh`။
---
lang: my
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#Iroha VM + Kotodama Docs အညွှန်း

ဤအညွှန်းသည် IVM၊ Kotodama နှင့် IVM-ပထမပိုက်လိုင်းအတွက် ပင်မဒီဇိုင်းနှင့် ရည်ညွှန်းစာရွက်စာတမ်းများကို ချိတ်ဆက်ထားသည်။ 日本語訳は [`README.ja.md`](./README.ja.md) を参照してください။

- IVM ဗိသုကာပညာနှင့် ဘာသာစကားမြေပုံ- `../../ivm.md`
- IVM syscall ABI: `ivm_syscalls.md`
- ထုတ်လုပ်ထားသော syscall ကိန်းသေများ- `ivm_syscalls_generated.md` (ပြန်လည်စတင်ရန် `make docs-syscalls` ကို run)
- IVM bytecode ခေါင်းစီး- `ivm_header.md`
- Kotodama သဒ္ဒါနှင့် ဝေါဟာရ- `kotodama_grammar.md`
- Kotodama နမူနာများနှင့် syscall မြေပုံများ- `kotodama_examples.md`
- ငွေပေးငွေယူ ပိုက်လိုင်း (IVM-ပထမ): `../../new_pipeline.md`
- Torii စာချုပ်များ API (ဖော်ပြချက်များ- `torii_contracts_api.md`
- Universal အကောင့်/UAID လုပ်ငန်းဆောင်ရွက်မှုလမ်းညွှန်- `universal_accounts_guide.md`
- JSON စုံစမ်းမေးမြန်းချက် စာအိတ် (CLI / tooling): `query_json.md`
- Norito တိုက်ရိုက်ထုတ်လွှင့်မှု မော်ဂျူး ရည်ညွှန်း- `norito_streaming.md`
- Runtime ABI နမူနာများ- `samples/runtime_abi_active.md`၊ `samples/runtime_abi_hash.md`၊ `samples/find_active_abi_versions.md`
- ZK App API ( ပူးတွဲပါဖိုင်များ၊ သက်သေပြချက် ၊ မဲစာရင်း ) : `zk_app_api.md`
- Torii ZK attachments/prover runbook- `zk/prover_runbook.md`
- Torii ZK အက်ပ် API အော်ပရေတာလမ်းညွှန် ( ပူးတွဲပါဖိုင်/သက်သေ; crate doc )- `../../crates/iroha_torii/docs/zk_app_api.md`
- VK/အထောက်အထားသက်တမ်း (မှတ်ပုံတင်ခြင်း၊ အတည်ပြုခြင်း၊ တယ်လီမီတာ) - `zk/lifecycle.md`
- Torii အော်ပရေတာ အကူအညီများ (မြင်နိုင်မှုအတွက် အဆုံးမှတ်များ)- `references/operator_aids.md`
- Nexus မူရင်း-လမ်းသွား အမြန်စတင်ခြင်း- `quickstart/default_lane.md`
- MOCHI ကြီးကြပ်ရေးမှူး အမြန်စတင် & ဗိသုကာ- `mochi/index.md`
- JavaScript SDK လမ်းညွှန်များ (အမြန်စတင်ရန်၊ ဖွဲ့စည်းမှုပုံစံ၊ ထုတ်ဝေခြင်း)- `sdk/js/index.md`
- Swift SDK parity/CI ဒက်ရှ်ဘုတ်များ- `references/ios_metrics.md`
- အုပ်ချုပ်မှု- `../../gov.md`
- ဒိုမိန်းထောက်ခံချက်များ (ကော်မတီများ၊ မူဝါဒများ၊ အတည်ပြုချက်)- `domain_endorsements.md`
- JDG အထောက်အထားများ (အော့ဖ်လိုင်းအတည်ပြုခြင်းကိရိယာ)- `jdg_attestations.md`
- ရှင်းလင်းချက်ညှိနှိုင်းမှု အချက်ပြချက်များ- `coordination_llm_prompts.md`
- လမ်းပြမြေပုံ- `../../roadmap.md`
- Docker တည်ဆောက်သူ ရုပ်ပုံအသုံးပြုမှု- `docker_build.md`

အသုံးပြုမှုအကြံပြုချက်များ
- ပြင်ပကိရိယာများ (`koto_compile`, `ivm_run`) ကို အသုံးပြု၍ `examples/` တွင် နမူနာများကို တည်ဆောက်ပြီး လုပ်ဆောင်ပါ။
  - `make examples-run` (နှင့် `ivm_tool` ရှိလျှင် `make examples-inspect`)
- နမူနာများနှင့် ခေါင်းစီးစစ်ဆေးမှုများအတွက် `integration_tests/tests/` တွင် ရွေးချယ်နိုင်သော ပေါင်းစပ်စစ်ဆေးမှုများ (ပုံမှန်အားဖြင့် လျစ်လျူရှုထားသည်)။ပိုက်လိုင်းဖွဲ့စည်းမှု
- runtime အပြုအမူအားလုံးကို `iroha_config` ဖိုင်များမှတစ်ဆင့် ပြင်ဆင်သတ်မှတ်ထားပါသည်။ Environment variable များကို အော်ပရေတာများအတွက် အသုံးမပြုပါ။
- အာရုံခံစားနိုင်သော ပုံသေများကို ပေးထားသည်။ ဖြန့်ကျက်မှုအများစုသည် အပြောင်းအလဲများ မလိုအပ်ပါ။
- `[pipeline]` အောက်တွင် သက်ဆိုင်ရာသော့များ
  - `dynamic_prepass`- ဝင်သုံးခွင့်အစုံများရယူရန် IVM ကို ဖတ်ရန်-သီးသန့်ကြိုတင်ကိုဖွင့်ပါ (မူလ-အမှန်)။
  - `access_set_cache_enabled`- `(code_hash, entrypoint)` အလိုက် ကက်ရှ်မှရရှိသော access sets အမှားရှာခြင်း အရိပ်အမြွက်များကို ပိတ်ရန် (မူလ-အမှန်)။
  - `parallel_overlay`- မျဉ်းပြိုင်တွင် ထပ်ဆင့်များတည်ဆောက်ပါ။ commit သည် အဆုံးအဖြတ်အတိုင်း ကျန်ရှိနေသည် (မူလ-အမှန်)။
  - `gpu_key_bucket`- `(key, tx_idx, rw_flag)` ပေါ်ရှိ တည်ငြိမ်သောအစွန်းကိုသုံး၍ အချိန်ဇယားဆွဲကြိုတင်ပေးပို့မှုအတွက် စိတ်ကြိုက်ရွေးချယ်နိုင်သောသော့ပုံးထည့်ခြင်း။ အဆုံးအဖြတ်ပေးသော CPU သည် အမြဲတမ်းတက်ကြွနေသည် (မူလ- false)။
  - `cache_size`- ကမ္ဘာလုံးဆိုင်ရာ IVM ကြိုတင်-ကုဒ် ကုဒ်ကက်ရှ် ၏ စွမ်းရည် ပုံသေ- 128။ တိုးမြှင့်ခြင်းသည် ထပ်ခါတလဲလဲ လုပ်ဆောင်မှုအတွက် ကုဒ်ကုဒ်အချိန်ကို လျှော့ချနိုင်သည်။

Docs စင့်ခ်လုပ်ခြင်းများ
- Syscall ကိန်းသေများ (docs/source/ivm_syscalls_generated.md)
  - ပြန်ထုတ်ခြင်း- `make docs-syscalls`
  - စစ်ဆေးရန်- `bash scripts/check_syscalls_doc.sh`
- Syscall ABI ဇယား (crates/ivm/docs/syscalls.md)
  - စစ်ဆေးရန်- `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - ထုတ်ပေးသည့်အပိုင်း (နှင့် ကုဒ်ဒေါ့စ်ဇယား) ကို အပ်ဒိတ်လုပ်ပါ- `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Pointer-ABI ဇယားများ (crates/ivm/docs/pointer_abi.md နှင့် ivm.md)
  - စစ်ဆေးရန်- `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - အပ်ဒိတ်အပိုင်းများ- `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM ခေါင်းစီးမူဝါဒနှင့် ABI hashs (docs/source/ivm_header.md)
  - စစ်ဆေးရန်သာ- `cargo run -p ivm --bin gen_header_doc -- --check` နှင့် `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - အပ်ဒိတ်အပိုင်းများ- `cargo run -p ivm --bin gen_header_doc -- --write` နှင့် `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Actions အလုပ်အသွားအလာ `.github/workflows/check-docs.yml` သည် ဤစစ်ဆေးမှုများကို push/PR တိုင်းတွင် လုပ်ဆောင်ပြီး ထုတ်လုပ်လိုက်သော docs အကောင်အထည်ဖော်မှုမှ ရုန်းထွက်ပါက ကျရှုံးလိမ့်မည်။
- [Governance Playbook](governance_playbook.md)
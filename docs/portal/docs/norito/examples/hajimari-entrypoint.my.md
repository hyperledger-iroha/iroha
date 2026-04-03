<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8367687fcf43fcb50ab43940a4ebeb8b8ba22a3ab8a6c3ed5088c52b1fdd7baf
source_last_modified: "2026-01-22T15:38:30.521640+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/hajimari-entrypoint
title: Hajimari ဝင်ပေါက်အမှတ် အရိုးစု
description: Kotodama အများသူငှာ ဝင်ပေါက်အမှတ်နှင့် ပြည်နယ်လက်ကိုင်တစ်ခုဖြင့် အနည်းဆုံး Kotodama စာချုပ်။
source: crates/ivm/docs/examples/01_hajimari.ko
---

Kotodama အများသူငှာ ဝင်ပေါက်အမှတ်နှင့် ပြည်နယ်လက်ကိုင်တစ်ခုဖြင့် အနည်းဆုံး Kotodama စာချုပ်။

## လယ်ဂျာရှင်းလင်းချက်

- [Norito စတင်ခြင်း](/norito/getting-started#1-compile-a-kotodama-contract) သို့မဟုတ် `cargo test -p ivm developer_portal_norito_snippets_compile` တွင် ပြထားသည့်အတိုင်း `koto_compile --abi 1` နှင့် စာချုပ်ကို စုစည်းပါ။
- node တစ်ခုကိုမထိမီ `info!` မှတ်တမ်းနှင့် ကနဦး syscall ကိုစစ်ဆေးရန် `ivm_run` / `developer_portal_norito_snippets_run` ဖြင့် Smoke-test ။
- `iroha_cli app contracts deploy` မှတစ်ဆင့် ရှေးဟောင်းပစ္စည်းကို ဖြန့်ကျက်ပြီး [Norito စတင်ခြင်း](/norito/getting-started#4-deploy-via-iroha_cli) ရှိ အဆင့်များကို အသုံးပြု၍ မန်နီးဖက်စ်ကို အတည်ပြုပါ။

## သက်ဆိုင်ရာ SDK လမ်းညွှန်များ

- [Rrust SDK အမြန်စတင်ခြင်း](/sdks/rust)
- [Python SDK အမြန်စတင်ခြင်း](/sdks/python)
- [JavaScript SDK အမြန်စတင်ခြင်း](/sdks/javascript)

[Kotodama အရင်းအမြစ်ကို ဒေါင်းလုဒ်လုပ်ပါ](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
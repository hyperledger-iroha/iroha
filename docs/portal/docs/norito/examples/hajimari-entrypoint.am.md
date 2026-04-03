<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
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
title: የሃጂማሪ መግቢያ ነጥብ አጽም
description: አነስተኛ Kotodama የኮንትራት ስካፎል ከአንድ የህዝብ መግቢያ ነጥብ እና የግዛት እጀታ ጋር።
source: crates/ivm/docs/examples/01_hajimari.ko
---

ዝቅተኛው Kotodama የኮንትራት ስካፎል ከአንድ የህዝብ መግቢያ ነጥብ እና የግዛት እጀታ ጋር።

## የመመዝገቢያ መመሪያ

- በ[Norito መጀመር](/norito/getting-started#1-compile-a-kotodama-contract) ወይም በ`cargo test -p ivm developer_portal_norito_snippets_compile` ላይ እንደሚታየው ከ`koto_compile --abi 1` ጋር ውሉን ያጠናክሩ።
- መስቀለኛ መንገድን ከመንካትዎ በፊት የ `info!` ሎግ እና የመነሻ syscall ለማረጋገጥ የባይቴኮዱን በአገር ውስጥ በ`ivm_run`/`developer_portal_norito_snippets_run` ይሞክሩት።
- ቅርሶቹን በ`iroha_cli app contracts deploy` በኩል ያሰማሩ እና በ[Norito መጀመር](/norito/getting-started#4-deploy-via-iroha_cli) ያሉትን ደረጃዎች በመጠቀም አንጸባራቂውን ያረጋግጡ።

## ተዛማጅ የኤስዲኬ መመሪያዎች

- [ዝገት ኤስዲኬ ፈጣን ጅምር](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[የKotodama ምንጭ አውርድ](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
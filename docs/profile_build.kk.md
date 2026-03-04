---
lang: kk
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Профильдеу `iroha_data_model` құрастыру

`iroha_data_model` ішіндегі баяу құрастыру қадамдарын табу үшін көмекші сценарийді іске қосыңыз:

```sh
./scripts/profile_build.sh
```

Бұл `cargo build -p iroha_data_model --timings` іске қосады және уақыт есептерін `target/cargo-timings/` жүйесіне жазады.
Шолғышта `cargo-timing.html` ашыңыз және қандай жәшіктер немесе құрастыру қадамдары көп уақыт алатынын көру үшін тапсырмаларды ұзақтығы бойынша сұрыптаңыз.

Оңтайландыру әрекеттерін ең баяу тапсырмаларға бағыттау үшін уақыттарды пайдаланыңыз.
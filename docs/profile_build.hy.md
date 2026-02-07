---
lang: hy
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Profiling `iroha_data_model` Կառուց

`iroha_data_model`-ում դանդաղ կառուցման քայլերը գտնելու համար գործարկեք օգնական սկրիպտը.

```sh
./scripts/profile_build.sh
```

Սա աշխատում է `cargo build -p iroha_data_model --timings` և գրում է ժամանակային հաշվետվություններ `target/cargo-timings/`-ին:
Բացեք `cargo-timing.html`-ը զննարկիչում և դասավորեք առաջադրանքները ըստ տևողության՝ տեսնելու, թե որ տուփերը կամ շինարարական քայլերն են ավելի շատ ժամանակ պահանջում:

Օգտագործեք ժամանակացույցը՝ օպտիմալացման ջանքերը կենտրոնացնելու ամենադանդաղ առաջադրանքների վրա:
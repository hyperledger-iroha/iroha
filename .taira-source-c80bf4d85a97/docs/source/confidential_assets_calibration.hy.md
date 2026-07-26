---
lang: hy
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Գազի չափորոշման գաղտնի ելակետեր

Այս մատյանը հետևում է գազի գաղտնի տրամաչափման վավերացված արդյունքներին
հենանիշներ. Յուրաքանչյուր տող փաստաթղթավորում է թողարկման որակի չափման հավաքածու, որը նկարահանվել է
ընթացակարգը, որը նկարագրված է `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`-ում:

| Ամսաթիվ (UTC) | Պարտավորել | Անձնագիր | `ns/op` | `gas/op` | `ns/gas` | Ծանոթագրություններ |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | բազային-նեոնային | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | բազային-նեոն-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darwin 25.0.0 arm64 (`rustc 1.91.0`): Հրաման՝ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; մուտք գործել `docs/source/confidential_assets_calibration_neon_20260428.log`: x86_64 հավասարաչափ գործարկումներ (SIMD-չեզոք + AVX2) նախատեսված են 2026-03-19 Ցյուրիխի լաբորատորիայի համար; Արտեֆակտները կհայտնվեն `artifacts/confidential_assets_calibration/2026-03-x86/`-ի տակ՝ համապատասխան հրամաններով և կմիավորվեն բազային աղյուսակում, երբ նկարվեն: |
| 2026-04-28 | — | բազային-simd-չեզոք | — | — | — | **Հրաժարվել է** Apple Silicon-ում. `ring`-ը պարտադրում է NEON-ը ABI պլատֆորմի համար, ուստի `RUSTFLAGS="-C target-feature=-neon"`-ը ձախողվում է մինչև նստարանն աշխատի (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`): Չեզոք տվյալները պահպանվում են CI հոսթի `bench-x86-neon0`-ում: |
| 2026-04-28 | — | բազային-avx2 | — | — | — | **Հետաձգվում** մինչև x86_64 վազող հասանելի լինի: `arch -x86_64`-ը չի կարող երկուականներ ստեղծել այս մեքենայի վրա («Վատ պրոցեսորի տեսակը գործարկվող է», տես `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`): CI հոսթ `bench-x86-avx2a` մնում է ռեկորդային աղբյուր: |

`ns/op`-ը միավորում է պատի ժամացույցի մեդիանը՝ չափանիշով չափված յուրաքանչյուր հրահանգի համար.
`gas/op`-ը համապատասխան ժամանակացույցի ծախսերի թվաբանական միջինն է
`iroha_core::gas::meter_instruction`; `ns/gas`-ը բաժանում է ամփոփված նանովայրկյանները
ամփոփված գազը ինը հրահանգներից բաղկացած նմուշի հավաքածուում:

*Նշում։* Ներկայիս arm64 հոսթը չի թողարկում `raw.csv` չափանիշի ամփոփագրերը
տուփ; կրկնել `CRITERION_OUTPUT_TO=csv`-ով կամ վերընթաց ուղղումով, նախքան a-ն նշելը
թողարկել, որպեսզի ընդունման ստուգաթերթով պահանջվող արտեֆակտները կցվեն:
Եթե `target/criterion/`-ը դեռ բացակայում է `--save-baseline`-ից հետո, հավաքեք վազքը
Linux հոսթի վրա կամ սերիականացնել կոնսոլի ելքը թողարկման փաթեթի մեջ որպես ա
ժամանակավոր կանգառ: Տեղեկատվության համար, arm64 կոնսոլի գրանցամատյանը վերջին գործարկումից
ապրում է `docs/source/confidential_assets_calibration_neon_20251018.log`-ում:

Մեկ հրահանգի միջինները նույն գործարկումից (`cargo bench -p iroha_core --bench isi_gas_calibration`).

| Հրահանգ | միջին `ns/op` | ժամանակացույց `gas` | `ns/gas` |
| --- | --- | --- | --- |
| Գրանցվել Դոմեն | 3.46e5 | 200 | 1.73e3 |
| ԳրանցվելՀաշիվ | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

### 2026-04-28 (Apple Silicon, NEON միացված)

Միջին ուշացումները 2026-04-28 թարմացման համար (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Հրահանգ | միջին `ns/op` | ժամանակացույց `gas` | `ns/gas` |
| --- | --- | --- | --- |
| Գրանցվել Դոմեն | 8.58e6 | 200 | 4.29e4 |
| Գրանցվել Հաշիվ | 4.40e6 | 200 | 2.20e4 |
| RegisterAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

Վերոնշյալ աղյուսակում `ns/op` և `ns/gas` ագրեգատները ստացվում են գումարից.
այս մեդիանները (ընդհանուր `3.85717e7`ns ինը հրահանգների հավաքածուում և 1,413
գազի ագրեգատներ):

Ժամանակացույցի սյունակը պարտադրված է `gas::tests::calibration_bench_gas_snapshot`-ով
(ընդհանուր 1,413 գազ ամբողջ ինը հրահանգների հավաքածուում) և կկանգնեցվի, եթե ապագա կարկատվեն
փոխել հաշվառումը` առանց տրամաչափման սարքերը թարմացնելու:

## Պարտավորությունների ծառի հեռաչափության ապացույց (M2.2)

Ըստ ճանապարհային քարտեզի առաջադրանքի **M2.2**, յուրաքանչյուր չափաբերման առաջադրանք պետք է պարունակի նորը
պարտավորությունների ծառաչափեր և վտարման հաշվիչներ՝ ապացուցելու Մերկլի սահմանի պահպանումը
կազմաձևված սահմաններում.

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

Գրանցեք արժեքները ստուգաչափման ծանրաբեռնվածությունից անմիջապես առաջ և հետո: Ա
մեկ ակտիվի մեկ հրամանը բավարար է. օրինակ `4cuvDVPuLBKJyN6dPbRQhmLh68sU`-ի համար.

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

Կցեք չմշակված ելքը (կամ Prometheus լուսանկարը) տրամաչափման տոմսին, որպեսզի
Կառավարման վերանայողը կարող է հաստատել արմատային պատմության գլխարկները և անցակետերի միջակայքերը
մեծարված. Հեռաչափության ուղեցույցը `docs/source/telemetry.md#confidential-tree-telemetry-m22`-ում
ընդլայնում է ահազանգման ակնկալիքները և հարակից Grafana վահանակները:

Ներառեք ստուգիչի քեշի հաշվիչները նույն քերծվածքում, որպեսզի վերանայողները կարողանան հաստատել
բաց թողնված հարաբերակցությունը մնացել է 40% նախազգուշացման շեմից ցածր.

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Փաստաթղթավորեք ստացված հարաբերակցությունը (`miss / (hit + miss)`) տրամաչափման գրության ներսում
ցուցադրելու համար SIMD-ի չեզոք արժեքի մոդելավորման վարժությունները, փոխարենը կրկին օգտագործված տաք պահոցներ
thrashing Halo2 ստուգիչ ռեեստրի.

## Չեզոք և AVX2 հրաժարում

SDK խորհուրդը ժամանակավոր հրաժարում է տվել PhaseC դարպասի համար, որը պահանջում է
`baseline-simd-neutral` և `baseline-avx2` չափումներ.

- **SIMD-չեզոք:** Apple Silicon-ի վրա `ring` կրիպտո ֆոնդը պարտադրում է NEON-ը
  ABI կոռեկտություն. Գործառույթի անջատում (`RUSTFLAGS="-C target-feature=-neon"`)
  ընդհատում է կառուցումը նախքան նստարանի երկուականի արտադրությունը (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`):
- **AVX2:** Տեղական գործիքների շղթան չի կարող ստեղծել x86_64 երկուականներ (`arch -x86_64 rustc -V`
  → «Վատ պրոցեսորի տեսակը գործարկվող է»; տես
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`):

Քանի դեռ CI հոսթեր `bench-x86-neon0` և `bench-x86-avx2a` առցանց չեն, NEON-ը գործարկվում է
վերևում գումարած հեռաչափության ապացույցները բավարարում են PhaseC-ի ընդունման չափանիշները:
Հրաժարումը գրանցված է `status.md`-ում և կվերանայվի, երբ x86 սարքավորումը միանա
հասանելի.
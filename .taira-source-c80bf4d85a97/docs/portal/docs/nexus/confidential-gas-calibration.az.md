---
lang: az
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: db113723dbedefdb89333e2e8fb1483aed9b5603cd2e9bb93fb46f7206533518
source_last_modified: "2025-12-29T18:16:35.121357+00:00"
translation_last_reviewed: 2026-02-07
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
slug: /nexus/confidential-gas-calibration
translator: machine-google-reviewed
---

# Məxfi Qaz Kalibrləmə Əsasları

Bu kitab məxfi qaz kalibrləməsinin təsdiq edilmiş nəticələrini izləyir
meyarlar. Hər bir sıra ilə çəkilmiş buraxılış keyfiyyəti ölçmə dəstini sənədləşdirir
[Məxfi Aktivlər və ZK Köçürmələri](./confidential-assets#calibration-baselines--acceptance-gates) bölməsində təsvir edilən prosedur.

| Tarix (UTC) | Etmək | Profil | `ns/op` | `gas/op` | `ns/gas` | Qeydlər |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baza-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | gözləyən | baza-simd-neytral | — | — | — | CI host `bench-x86-neon0`-də planlaşdırılan x86_64 neytral işləmə; GAS-214 biletinə baxın. Nəticələr dəzgah pəncərəsi tamamlandıqdan sonra əlavə olunacaq (birləşmədən əvvəl yoxlama siyahısı hədəfləri buraxılışı 2.1). |
| 2026-04-13 | gözləyən | baza-avx2 | — | — | — | Neytral qaçışla eyni icra/quraşdırmadan istifadə edərək AVX2 kalibrləməsini izləyin; `bench-x86-avx2a` host tələb olunur. GAS-214, `baseline-neon` ilə delta müqayisəsi ilə hər iki qaçışı əhatə edir. |

`ns/op` Kriteriya ilə ölçülən hər təlimata görə median divar saatını cəmləşdirir;
`gas/op` müvafiq cədvəl xərclərinin arifmetik ortasıdır.
`iroha_core::gas::meter_instruction`; `ns/gas` cəmlənmiş nanosaniyələri bölür
doqquz təlimatlı nümunə dəsti üzrə cəmlənmiş qaz.

*Qeyd.* Cari arm64 hostu `raw.csv` kriteriyasının xülasəsini yaymır.
qutu; a. işarələmədən əvvəl `CRITERION_OUTPUT_TO=csv` və ya yuxarıya doğru düzəliş ilə təkrar işə salın
buraxın ki, qəbul yoxlama siyahısında tələb olunan artefaktlar əlavə olunsun.
`target/criterion/`, `--save-baseline`-dən sonra hələ də yoxdursa, qaçışı toplayın.
Linux hostunda və ya konsol çıxışını buraxılış paketinə seriyalaşdırın
müvəqqəti dayanma boşluğu. İstinad üçün, arm64 konsolunun ən son buraxılışından qeydi
`docs/source/confidential_assets_calibration_neon_20251018.log`-də yaşayır.

Eyni əməliyyatdan hər təlimat üçün medianlar (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Təlimat | median `ns/op` | cədvəli `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| Qeydiyyat Hesabı | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

Cədvəl sütunu `gas::tests::calibration_bench_gas_snapshot` tərəfindən tətbiq edilir
(doqquz təlimat dəsti üzrə cəmi 1,413 qaz) və gələcək yamalar olarsa, işə düşəcək
kalibrləmə qurğularını yeniləmədən ölçməni dəyişdirin.
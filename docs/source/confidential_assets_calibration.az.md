---
lang: az
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Məxfi Qaz Kalibrləmə Əsasları

Bu kitab məxfi qaz kalibrləməsinin təsdiq edilmiş nəticələrini izləyir
meyarlar. Hər bir sıra ilə çəkilmiş buraxılış keyfiyyəti ölçmə dəstini sənədləşdirir
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`-də təsvir edilən prosedur.

| Tarix (UTC) | Etmək | Profil | `ns/op` | `gas/op` | `ns/gas` | Qeydlər |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baza-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 28-04-2026 | 8ea9b2a7 | baza-neon-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darwin 25.0.0 arm64 (`rustc 1.91.0`). Komanda: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; `docs/source/confidential_assets_calibration_neon_20260428.log` ünvanına daxil olun. x86_64 paritet qaçışları (SIMD-neytral + AVX2) 2026-03-19 Sürix laboratoriya yuvası üçün planlaşdırılır; artefaktlar uyğun əmrlərlə `artifacts/confidential_assets_calibration/2026-03-x86/` altında enəcək və çəkildikdən sonra əsas cədvələ birləşdiriləcək. |
| 28-04-2026 | — | baza-simd-neytral | — | — | — | **Apple Silicon-da imtina edildi**—`ring` ABI platforması üçün NEON-u tətbiq edir, beləliklə, `RUSTFLAGS="-C target-feature=-neon"` dəzgah işə düşməzdən əvvəl uğursuz olur (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Neytral məlumatlar CI host `bench-x86-neon0`-də qapalı qalır. |
| 28-04-2026 | — | baza-avx2 | — | — | — | **X86_64 qaçışı əlçatan olana qədər təxirə salındı**. `arch -x86_64` bu maşında ikili faylları yarada bilməz (“İcra edilə biləndə pis CPU növü”; bax `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). CI host `bench-x86-avx2a` rekordun mənbəyi olaraq qalır. |

`ns/op` Kriteriya ilə ölçülən hər təlimata görə median divar saatını cəmləşdirir;
`gas/op` müvafiq cədvəl xərclərinin arifmetik ortasıdır.
`iroha_core::gas::meter_instruction`; `ns/gas` cəmlənmiş nanosaniyələri bölür
doqquz təlimatlı nümunə dəsti üzrə cəmlənmiş qaz.

*Qeyd.* Cari arm64 hostu `raw.csv` kriteriyasının xülasəsini yaymır.
qutu; a. işarələmədən əvvəl `CRITERION_OUTPUT_TO=csv` və ya yuxarı axın düzəlişi ilə təkrar işə salın
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

### 28-04-2026 (Apple Silicon, NEON aktivdir)

28-04-2026 yeniləməsi üçün orta gecikmələr (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Təlimat | median `ns/op` | cədvəli `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 8.58e6 | 200 | 4.29e4 |
| Qeydiyyat Hesabı | 4.40e6 | 200 | 2.20e4 |
| RegisterAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

Yuxarıdakı cədvəldəki `ns/op` və `ns/gas` aqreqatları cəmindən əldə edilmişdir.
bu medianlar (doqquz təlimat dəsti üzrə cəmi `3.85717e7`ns və 1,413
qaz aqreqatları).

Cədvəl sütunu `gas::tests::calibration_bench_gas_snapshot` tərəfindən tətbiq edilir
(doqquz təlimat dəsti üzrə cəmi 1,413 qaz) və gələcək yamalar olarsa, işə düşəcək
kalibrləmə qurğularını yeniləmədən ölçməni dəyişdirin.

## Öhdəlik Ağacı Telemetriya Sübutları (M2.2)

Yol xəritəsi tapşırığına görə **M2.2**, hər bir kalibrləmə işi yenisini tutmalıdır
Merkle sərhədinin qaldığını sübut etmək üçün öhdəlik ağacı ölçüləri və evakuasiya sayğacları
konfiqurasiya edilmiş sərhədlər daxilində:

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

Kalibrləmə iş yükündən dərhal əvvəl və sonra dəyərləri qeyd edin. A
aktiv üçün tək əmr kifayətdir; `xor#wonderland` üçün nümunə:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Xam çıxışı (və ya Prometheus snapshot) kalibrləmə biletinə əlavə edin ki,
İdarəetmə rəyçisi kök tarixçələrinin hədlərini və yoxlama nöqtələri intervallarını təsdiqləyə bilər
şərəfləndirilmişdir. `docs/source/telemetry.md#confidential-tree-telemetry-m22`-də telemetriya bələdçisi
xəbərdarlıq gözləntiləri və əlaqəli Grafana panellərini genişləndirir.

Doğrulayıcı keş sayğaclarını eyni qırıntıya daxil edin ki, rəyçilər təsdiq edə bilsin
qaçırma nisbəti 40% xəbərdarlıq həddinin altında qaldı:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Alınan nisbəti (`miss / (hit + miss)`) kalibrləmə qeydində sənədləşdirin
SIMD-neytral dəyəri göstərmək üçün modelləşdirmə məşqləri yerinə isti önbellekleri yenidən istifadə etdi
Halo2 doğrulayıcı reyestrini döyür.

## Neytral və AVX2 İmtina

SDK Şurası tələb olunan PhaseC qapısı üçün müvəqqəti imtina verdi
`baseline-simd-neutral` və `baseline-avx2` ölçmələri:

- **SIMD-neytral:** Apple Silicon-da `ring` kriptovalyutası NEON-u tətbiq edir.
  ABI düzgünlüyü. Xüsusiyyətin deaktiv edilməsi (`RUSTFLAGS="-C target-feature=-neon"`)
  dəzgah ikilisi istehsal edilməzdən əvvəl quruluşu dayandırır (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** Yerli alətlər silsiləsi x86_64 ikili faylları yarada bilməz (`arch -x86_64 rustc -V`
  → “İcra edilə biləndə pis CPU növü”; bax
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

CI hostları `bench-x86-neon0` və `bench-x86-avx2a` onlayn olana qədər NEON işləyir
yuxarıda üstəgəl telemetriya sübutları PhaseC qəbul meyarlarına cavab verir.
İmtina `status.md`-də qeydə alınıb və x86 avadanlığına yenidən baxılacaq.
mövcuddur.
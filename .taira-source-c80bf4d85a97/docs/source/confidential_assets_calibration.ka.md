---
lang: ka
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# კონფიდენციალური გაზის კალიბრაციის საბაზისო ხაზები

ეს წიგნი აკონტროლებს კონფიდენციალური გაზის კალიბრაციის დადასტურებულ შედეგებს
ეტალონები. თითოეული მწკრივი ადასტურებს გამოშვების ხარისხის გაზომვის კომპლექტს, რომელიც აღბეჭდილია
პროცედურა აღწერილია `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`-ში.

| თარიღი (UTC) | ვალდებულება | პროფილი | `ns/op` | `gas/op` | `ns/gas` | შენიშვნები |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | საბაზისო-ნეონი | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | საბაზისო-ნეონი-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darwin 25.0.0 arm64 (`rustc 1.91.0`). ბრძანება: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; შესვლა `docs/source/confidential_assets_calibration_neon_20260428.log`-ზე. x86_64 პარიტეტული გაშვებები (SIMD-ნეიტრალური + AVX2) დაგეგმილია 2026-03-19 ციურიხის ლაბორატორიის სლოტზე; არტეფაქტები დაეშვება `artifacts/confidential_assets_calibration/2026-03-x86/`-ის ქვეშ შესაბამისი ბრძანებებით და გაერთიანდება საბაზისო ცხრილში, როგორც კი აღიბეჭდება. |
| 2026-04-28 | — | საბაზისო-simd-ნეიტრალური | — | — | — | **მოხსნილია** Apple Silicon-ზე — `ring` აიძულებს NEON-ს ABI პლატფორმისთვის, ასე რომ, `RUSTFLAGS="-C target-feature=-neon"` მარცხდება მანამ, სანამ სკამი გაუშვებს (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). ნეიტრალური მონაცემები რჩება CI ჰოსტზე `bench-x86-neon0`. |
| 2026-04-28 | — | საბაზისო-avx2 | — | — | — | **გადაიდო**, სანამ x86_64 მორბენალი იქნება ხელმისაწვდომი. `arch -x86_64` არ შეუძლია ორობითი ფაილების შექმნას ამ მანქანაზე („CPU ცუდი ტიპი შესრულებადში“; იხილეთ `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). CI ჰოსტი `bench-x86-avx2a` რჩება ჩანაწერის წყაროდ. |

`ns/op` აგროვებს მედიანურ კედლის საათს კრიტერიუმით გაზომილი ინსტრუქციის მიხედვით;
`gas/op` არის შესაბამისი გრაფიკის ხარჯების არითმეტიკული საშუალო
`iroha_core::gas::meter_instruction`; `ns/gas` ყოფს შეჯამებულ ნანოწამებს
შეჯამებული გაზი ცხრა ინსტრუქციის ნიმუშის კომპლექტში.

*შენიშვნა.* ამჟამინდელი arm64 ჰოსტი არ გამოსცემს `raw.csv` კრიტერიუმის შეჯამებებს
ყუთი; ხელახლა გაუშვით `CRITERION_OUTPUT_TO=csv` ან ზედა ნაკადის შესწორებით a-ს მონიშვნამდე
გაათავისუფლეთ ისე, რომ არტეფაქტები, რომლებიც მოთხოვნილი იყო მიღების სიით, თან ერთვის.
თუ `target/criterion/` კვლავ აკლია `--save-baseline`-ის შემდეგ, შეაგროვეთ გაშვება
Linux ჰოსტზე ან კონსოლის გამომავალი სერიულირება გამოშვების პაკეტში, როგორც a
დროებითი გაჩერება. ცნობისთვის, arm64 კონსოლის ჟურნალი უახლესი გაშვებიდან
ცხოვრობს `docs/source/confidential_assets_calibration_neon_20251018.log`-ზე.

თითო ინსტრუქციის მედიანა ერთი და იგივე გაშვებიდან (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| ინსტრუქცია | მედიანა `ns/op` | განრიგი `gas` | `ns/gas` |
| --- | --- | --- | --- |
| რეგისტრაცია დომენი | 3.46e5 | 200 | 1.73e3 |
| რეგისტრაცია ანგარიში | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

### 2026-04-28 (Apple Silicon, NEON ჩართულია)

მედიანური შეყოვნება 2026-04-28 განახლებისთვის (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| ინსტრუქცია | მედიანა `ns/op` | განრიგი `gas` | `ns/gas` |
| --- | --- | --- | --- |
| რეგისტრაცია დომენი | 8.58e6 | 200 | 4.29e4 |
| რეგისტრაცია ანგარიში | 4.40e6 | 200 | 2.20e4 |
| RegisterAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

ზემოთ მოცემულ ცხრილში `ns/op` და `ns/gas` აგრეგატები მიღებულია ჯამიდან
ეს მედიანები (სულ `3.85717e7`ns ცხრა ინსტრუქციის კომპლექტში და 1,413
გაზის ერთეულები).

განრიგის სვეტი აღსრულებულია `gas::tests::calibration_bench_gas_snapshot`-ით
(სულ 1,413 გაზი ცხრა ინსტრუქციის კომპლექტში) და გაქრება, თუ მომავალში დაყენდება
გაზომვის შეცვლა კალიბრაციის მოწყობილობების განახლების გარეშე.

## ვალდებულების ხის ტელემეტრიის მტკიცებულება (M2.2)

საგზაო რუკის ამოცანისთვის **M2.2**, ყოველი კალიბრაციის გაშვება უნდა ასახავდეს ახალს
ვალდებულებების ხის ლიანდაგები და გამოსახლების მრიცხველები, რათა დაამტკიცონ, რომ მერკლის საზღვარი რჩება
კონფიგურირებულ საზღვრებში:

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

ჩაწერეთ მნიშვნელობები უშუალოდ კალიბრაციის დატვირთვის წინ და შემდეგ. ა
საკმარისია თითო აქტივზე ერთი ბრძანება; მაგალითი `4cuvDVPuLBKJyN6dPbRQhmLh68sU`-ისთვის:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

მიამაგრეთ ნედლეული გამოსავალი (ან Prometheus სნეპშოტი) კალიბრაციის ბილეთს, რათა
მმართველობის მიმომხილველს შეუძლია დაადასტუროს root-ისტორიის ქუდები და საგუშაგოს ინტერვალები
პატივს სცემდა. ტელემეტრიის სახელმძღვანელო `docs/source/telemetry.md#confidential-tree-telemetry-m22`-ში
აფართოებს გაფრთხილების მოლოდინებს და მათთან დაკავშირებულ Grafana პანელებს.

ჩართეთ დამადასტურებელი ქეშის მრიცხველები იმავე ნაკაწრში, რათა მიმომხილველებმა დაადასტურონ
გამოტოვების კოეფიციენტი დარჩა 40%-იანი გაფრთხილების ზღურბლზე ქვემოთ:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

დაასაბუთეთ მიღებული თანაფარდობა (`miss / (hit + miss)`) კალიბრაციის ჩანაწერში
SIMD-ნეიტრალური ღირებულების მოდელირების სავარჯიშოების საჩვენებლად ხელახლა გამოყენებული თბილი ქეშები ნაცვლად
Halo2 ვერიფიკატორის რეესტრის გატეხვა.

## ნეიტრალური და AVX2 უარის თქმა

SDK-ის საბჭომ მიიღო დროებითი შეღავათი PhaseC კარიბჭისთვის, რომელიც მოითხოვს
`baseline-simd-neutral` და `baseline-avx2` გაზომვები:

- **SIMD-ნეიტრალური:** Apple Silicon-ზე, `ring` კრიპტო სარეზერვო სისტემა აიძულებს NEON-ს
  ABI სისწორე. ფუნქციის გამორთვა (`RUSTFLAGS="-C target-feature=-neon"`)
  წყვეტს კონსტრუქციას სკამზე ორობითი წარმოების წინ (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** ადგილობრივი ხელსაწყოების ჯაჭვი ვერ აწარმოებს x86_64 ბინარებს (`arch -x86_64 rustc -V`
  → „ცუდი CPU ტიპი შესრულებადში“; იხილეთ
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

სანამ CI მასპინძლები `bench-x86-neon0` და `bench-x86-avx2a` ონლაინ იქნებიან, NEON მუშაობს
ზემოთ პლუს ტელემეტრიული მტკიცებულება აკმაყოფილებს PhaseC მიღების კრიტერიუმებს.
უარის თქმა დაფიქსირდა `status.md`-ში და ხელახლა განიხილება x86 ტექნიკის დაყენების შემდეგ
ხელმისაწვდომი.
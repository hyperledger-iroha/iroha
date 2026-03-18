---
lang: mn
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Нууц хийн шалгалт тохируулгын суурь

Энэхүү дэвтэр нь хийн нууц тохируулгын баталгаажуулсан гаралтыг хянадаг
жишиг үзүүлэлтүүд. Мөр бүр нь авсан хувилбарын чанарын хэмжилтийн багцыг баримтжуулна
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`-д тодорхойлсон процедур.

| Огноо (UTC) | Амлах | Профайл | `ns/op` | `gas/op` | `ns/gas` | Тэмдэглэл |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | суурь-неон | 2.93e5 | 1.57e2 | 1.87e3 | Дарвин 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | суурь-неон-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Дарвин 25.0.0 arm64 (`rustc 1.91.0`). Тушаал: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; `docs/source/confidential_assets_calibration_neon_20260428.log` хаягаар нэвтэрнэ үү. x86_64 паритын гүйлтүүд (SIMD-саармаг + AVX2) нь 2026-03-19 Цюрих лабораторид зориулагдсан; олдворууд тохирох командуудын хамт `artifacts/confidential_assets_calibration/2026-03-x86/`-ийн доор буух ба баригдсаны дараа үндсэн хүснэгтэд нэгтгэгдэнэ. |
| 2026-04-28 | — | baseline-simd-neutral | — | — | — | **Apple Silicon дээр **Татгалзсан**—`ring` нь ABI платформд NEON-ийг хэрэгжүүлдэг тул `RUSTFLAGS="-C target-feature=-neon"` вандан сандал ажиллахаас өмнө бүтэлгүйтдэг (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Төвийг сахисан өгөгдөл нь CI хост `bench-x86-neon0` дээр хаалттай хэвээр байна. |
| 2026-04-28 | — | baseline-avx2 | — | — | — | x86_64 гүйгч бэлэн болтол **хойшлогдсон**. `arch -x86_64` энэ машин дээр хоёртын файлуудыг үүсгэх боломжгүй ("Гүйцэтгэх боломжтой CPU-ийн төрөл муу"; `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`-г үзнэ үү). CI хост `bench-x86-avx2a` бичлэгийн эх сурвалж хэвээр байна. |

`ns/op` шалгуур үзүүлэлтээр хэмжсэн заавар бүрийн дундаж ханын цагийг нэгтгэдэг;
`gas/op` нь харгалзах хуваарийн зардлын арифметик дундаж юм.
`iroha_core::gas::meter_instruction`; `ns/gas` нь нийлбэр наносекундыг хуваана
есөн заавар бүхий дээжийн багц дахь нийлбэр хий.

*Тэмдэглэл.* Одоогийн arm64 хост нь `raw.csv` шалгуурын хураангуйг гаргадаггүй.
хайрцаг; a. шошголохоос өмнө `CRITERION_OUTPUT_TO=csv` эсвэл дээд талын засварыг ашиглан дахин ажиллуулна уу
суллах тул хүлээн авах хяналтын хуудсанд шаардлагатай олдворуудыг хавсаргана.
Хэрэв `target/criterion/` `--save-baseline` дараа байхгүй хэвээр байвал гүйлтийг цуглуул.
Линукс хост дээр эсвэл консолын гаралтыг хувилбарын багц болгон цуваа болгож a
түр зуурын завсарлага. Лавлагааны хувьд arm64 консолын хамгийн сүүлийн үеийн лог
`docs/source/confidential_assets_calibration_neon_20251018.log`-д амьдардаг.

Нэг зааварчилгааны медианууд (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Заавар | медиан `ns/op` | хуваарь `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| Бүртгүүлэх Данс | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

### 2026-04-28 (Apple Silicon, NEON идэвхжсэн)

2026-04-28-ны өдрийн шинэчлэлтийн дундаж саатал (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Заавар | медиан `ns/op` | хуваарь `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 8.58e6 | 200 | 4.29e4 |
| Бүртгүүлэх Данс | 4.40e6 | 200 | 2.20e4 |
| RegisterAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

Дээрх хүснэгтэд байгаа `ns/op` ба `ns/gas` агрегатуудыг нийлбэрээс гаргаж авсан болно.
эдгээр медианууд (есөн зааврын багц дахь нийт `3.85717e7`ns ба 1,413
хийн нэгж).

Хуваарийн баганыг `gas::tests::calibration_bench_gas_snapshot` мөрддөг
(есөн заавар бүхий багцад нийт 1,413 хий) бөгөөд ирээдүйд засварууд хийгдвэл хаах болно
тохируулгын төхөөрөмжийг шинэчлэхгүйгээр тоолуурыг өөрчлөх.

## Амлалт модны телеметрийн нотолгоо (M2.2)

Замын зургийн даалгаврын дагуу **M2.2**, шалгалт тохируулгын ажил бүр шинийг авах ёстой
Мерклийн хил хэвээр байгааг нотлох амлалт-мод хэмжигч ба нүүлгэх тоолуур
тохируулсан хязгаар дотор:

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

Шалгалт тохируулгын ажлын ачааллын өмнө болон дараа шууд утгуудыг тэмдэглэ. А
хөрөнгийн нэг команд хангалттай; жишээ нь `xor#wonderland`:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Түүхий гаралтыг (эсвэл Prometheus агшин зуур) шалгалт тохируулгын тасалбарт хавсаргана.
засаглалын хянагч эх түүхийн дээд хязгаар болон хяналтын цэгийн интервалыг баталгаажуулах боломжтой
гавьяат. `docs/source/telemetry.md#confidential-tree-telemetry-m22` дахь телеметрийн гарын авлага
анхааруулах хүлээлт болон холбогдох Grafana самбаруудыг өргөжүүлдэг.

Шүүгчид баталгаажуулахын тулд баталгаажуулагчийн кэш тоолуурыг ижил хусахад оруулаарай
алдах харьцаа 40%-ийн анхааруулах босгоос доогуур хэвээр байна:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Шалгалт тохируулгын тэмдэглэл дотор үүссэн харьцааг (`miss / (hit + miss)`) баримтжуулах
SIMD-саармаг зардлын загварчлалын дасгалуудыг харуулахын тулд дулаан кэшийг дахин ашигласан
Halo2 баталгаажуулагчийн бүртгэлийг устгаж байна.

## Төвийг сахисан ба AVX2-аас татгалзах

SDK зөвлөл нь шаардлагатай PhaseC хаалганы түр чөлөөлөлтийг олгосон
`baseline-simd-neutral` ба `baseline-avx2` хэмжилтүүд:

- **SIMD төвийг сахисан:** Apple Silicon дээр `ring` крипто арын хэсэг нь NEON-ийг ашигладаг.
  ABI-ийн зөв байдал. Энэ функцийг идэвхгүй болгож байна (`RUSTFLAGS="-C target-feature=-neon"`)
  вандан хоёртын файлыг үүсгэхээс өмнө угсралтыг зогсооно (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** Орон нутгийн хэрэгслийн гинж нь x86_64 хоёртын файлыг үүсгэх боломжгүй (`arch -x86_64 rustc -V`
  → "Гүйцэтгэх боломжтой CPU-ийн төрөл муу"; үзнэ үү
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

CI хостууд `bench-x86-neon0` болон `bench-x86-avx2a` онлайн байх хүртэл NEON ажилладаг.
Дээр дурдсанаас гадна телеметрийн нотолгоо нь PhaseC-ийн хүлээн авах шалгуурыг хангаж байна.
Татгалзалтыг `status.md`-д тэмдэглэсэн бөгөөд x86 тоног төхөөрөмж дуусмагц дахин хянан үзэх болно.
боломжтой.
---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: سجل معايرة الغاز السري
описание: قياسات بجودة الاصدار تدعم جدول الغاز السري.
слизняк: /nexus/confidential-gas-калибровка
---

# خطوط اساس لمعايرة الغاز السري

Он был создан для того, чтобы провести время с пользой для здоровья. В 2017 году он был избран президентом США в 1999 году, когда его пригласили на работу в Вашингтоне. [Конфиденциальные активы и передача ZK](./confidential-assets#calibration-baselines--acceptance-gates).

| التاريخ (UTC) | Зафиксировать | الملف التعريفي | `ns/op` | `gas/op` | `ns/gas` | الملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | базовый-неон | 2.93e5 | 1.57е2 | 1.87e3 | Дарвин 25.0.0 Arm64e (информация о хосте); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 12 апреля 2026 г. | в ожидании | базовый SIMD-нейтральный | - | - | - | Создан файл x86_64 с кодом CI `bench-x86-neon0`; Автомобиль ГАЗ-214. Скамейка запасных частей (для версии 2.1 перед слиянием). |
| 13 апреля 2026 г. | в ожидании | базовый уровень-avx2 | - | - | - | Создание файла AVX2 для фиксации/сборки файла. تتطلب المضيف `bench-x86-avx2a`. Автомобиль ГАЗ-214 установлен на заводе `baseline-neon`. |

`ns/op` может быть использован для определения критерия выбора; `gas/op` в приложении к `iroha_core::gas::meter_instruction`; و`ns/gas` для получения дополнительной информации о том, как сделать это. تسع.

*ملاحظة.* المضيف Arm64 لا يصدر ملخصات Criterion `raw.csv` بشكل افتراضي؛ اعد التشغيل مع `CRITERION_OUTPUT_TO=csv` او اصلاح upstream قبل وسم الاصدار لكي يتم ارفاق artefacts المطلوبة В действительности. Для `target/criterion/` используется Linux `--save-baseline`. Он был убит в фильме "Старый мир" в Нью-Йорке. Сделал это с помощью Arm64, чтобы получить доступ к `docs/source/confidential_assets_calibration_neon_20251018.log`.

Установите флажок, чтобы установить флажок (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| تعليمة | Информация `ns/op` | Дата `gas` | `ns/gas` |
| --- | --- | --- | --- |
| Зарегистрировать домен | 3.46e5 | 200 | 1.73е3 |
| РегистрацияАккаунт | 3.15e5 | 200 | 1.58e3 |
| РегистрацияАссетДеф | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| ГрантАккаунтРоль | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33е2 |
| МинтАссет | 1.56e5 | 150 | 1.04e3 |
| ТрансферАссет | 3.68e5 | 180 | 2.04e3 |

عمود الجدول مفروض بواسطة `gas::tests::calibration_bench_gas_snapshot` (1,413 раз) В конце концов, он провёл в столичном клубе Дэйва Пьера, зафиксировав его.
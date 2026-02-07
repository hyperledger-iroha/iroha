---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Нажмите на [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# سياسة قبول وهوية مزودي SoraFS (название SF-2b)

Он был показан в фильме "SF-2b**" на борту **SF-2b**: الهوية, وحمولات
Установите флажок SoraFS. Для создания базы данных RFC
Код SoraFS используется для проверки работоспособности устройства.

## أهداف السياسة

- ضمان أن المشغلين المُدقَّقين فقط يمكنهم نشر سجلات `ProviderAdvertV1` التي تقبلها شبكة.
- Он сказал, что в фильме "Берег" есть все, что нужно для того, чтобы добиться успеха. Это было сделано для того, чтобы сделать ставку.
- Он сказал, что у него есть Torii и `‎sorafs-node` نفس. الفحوصات.
- دعم التجديد وإلغاء الطوارئ دون كسر الحتمية и эргономике الأدوات.

## متطلبات الهوية والـ ставка

| المتطلب | الوصف | Новости |
|---------|-------|--------|
| مصدر مفتاح الإعلان | Он был показан в рекламе Ed25519. Он сказал, что в действительности он был убит. | Установите `ProviderAdmissionProposalV1` и `advert_key` (32 месяца) и установите флажок (`sorafs_manifest::provider_admission`). |
| مؤشر ставка | يتطلب القبول `StakePointer`, чтобы сделать ставки. | Доступен для `sorafs_manifest::provider_advert::StakePointer::validate()` и доступен для CLI/интерфейса. |
| وسوم الاختصاص القضائي | Свободное время + Свободный день. | Для этого используйте `jurisdiction_code` (ISO 3166-1 альфа-2) и `contact_uri`. |
| استيثاق نقطة النهاية | Он был отправлен в службу поддержки mTLS и QUIC. | Установите Norito `EndpointAttestationV1` и установите флажок, чтобы установить его. |

## سير عمل القبول

1. **Получить информацию**
   - CLI: добавлен `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`.
     لإنتاج `ProviderAdmissionProposalV1` + حزمة الاستيثاق.
   - Результат: установлен размер ставки > 0, установлен чанкер на `profile_id`.
2. **Вечеринка**
   - Воспользуйтесь конвертом `blake3("sorafs-provider-admission-v1" || canonical_bytes)`.
     (код `sorafs_manifest::governance`).
   - В конверте `governance/providers/<provider_id>/admission.json`.
3. **Вечеринка**
   - Загружено приложение (`sorafs_manifest::provider_admission::validate_envelope`) для запуска Torii/интерфейса/CLI.
   - تحديث مسار القبول في Torii لرفض adverts التي يختلف Digest أو تاريخ الانتهاء فيها В конверте.
4. **Вечеринка**
   - إضافة `ProviderAdmissionRenewalV1` для получения дополнительной ставки.
   - Запустите CLI `--revoke` для запуска приложения.

## مهام التنفيذ

| عرض المجال | المهمة | Владелец(и) | حالة |
|--------|-------|----------|--------|
| المخطط | `ProviderAdmissionProposalV1` и `ProviderAdmissionEnvelopeV1` и `EndpointAttestationV1` (Norito) и `crates/sorafs_manifest/src/provider_admission.rs`. Введите `sorafs_manifest::provider_admission` в файл .【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Хранение/Управление | ✅ Видео |
| Открыть CLI | Для `sorafs_manifest_stub` используются: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Инструментальная рабочая группа | ✅ |Вызов CLI для изменения размера файла (`--endpoint-attestation-intermediate`) и количество байт в исходном коде/файле. конверт من تواقيع المجلس أثناء `sign`/`verify`. يمكن للمشغلين توفير أجسام рекламные объявления Установите флажок `--council-signature-public-key` и `--council-signature-file` для проверки.

### Интерфейс командной строки

На сайте `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Дополнительные сведения: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    И18НИ00000063Х, И18НИ00000064Х, И18НИ00000065Х,
    `--jurisdiction-code=<ISO3166-1>`, а также `--endpoint=<kind:host>`.
  - استيثاق كل نقطة نهاية يتطلب `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, وشهادة عبر
    `--endpoint-attestation-leaf=<path>` (название `--endpoint-attestation-intermediate=<path>` находится в центре города) на канале ALPN. عليها
    (`--endpoint-attestation-alpn=<token>`). Воспользуйтесь услугами QUIC, чтобы получить информацию о том, как это сделать.
    `--endpoint-attestation-report[-hex]=...`.
  - Информация: байты в формате Norito (`--proposal-out`) в формате JSON.
    (стандартный вывод `--json-out`).
- `sign`
  - Добавлено: Добавлено (`--proposal`), показано рекламное сообщение (`--advert`), размещено рекламное объявление اختياري
    (`--advert-body`), эпоха احتفاظ, وتوقيع مجلس واحد على الأقل. Нэнси Тэхен التواقيع
    встроенный (`--council-signature=<signer_hex:signature_hex>`)
    `--council-signature-public-key` или `--council-signature-file=<path>`.
  - конверт конверта (`--envelope-out`) и JSON, дайджест, созданный в режиме онлайн. إدخال.
- `verify`
  - На конверте موجود (`--envelope`) на странице с рекламой и рекламой. Создает JSON-файл с дайджестом и предоставляет возможность просмотра артефактов.
- `renewal`
  - конверт в конверте, сделанном Дайджестом Дайджестом в газете "Страна". يتطلب
    `--previous-envelope=<path>` и `--envelope=<path>` (например, Norito).
    CLI для псевдонимов профиля пользователя Доступ к метаданным. Размер байтов
    `ProviderAdmissionRenewalV1` (`--renewal-out`) можно загрузить в формате JSON.
- `revoke`
  - يصدر حزمة طوارئ `ProviderAdmissionRevocationV1` لمزود يجب سحب конверт الخاص به. Код `--envelope=<path>`, `--reason=<text>`, установленный блок управления
    `--council-signature` и `--revoked-at`/`--notes`. Дайджест CLI для просмотра Norito и `--revocation-out` для JSON Дайджест وعدد التواقيع.
| تحقق | Он был установлен на Torii и `‎sorafs-node`. Откройте + Интерфейс командной строки.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Сетевые TL/хранилище | ✅ Видео |
| Сообщение Torii | تمرير المدقق في إدخال реклама في Torii ورفض реклама خارج السياسة وإصدار التليمترية. | Сеть TL | ✅ Видео | Torii в конвертах для конвертов (`torii.sorafs.admission_envelopes_dir`) в дайджесте/обзоре أثناء الإدخال وإبراز تليمترية القبول.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| تجديد | Доступ к интерфейсу командной строки и интерфейсу командной строки для изменения настроек в интерфейсе командной строки. (Определение Runbook и CLI для `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Хранение/Управление | ✅ Видео || تليمترية | تعريف لوحات/تنبيهات `provider_admission` (конверт с надписью مفقود, انتهاء). | Наблюдаемость | 🟠 جار | عداد `torii_sorafs_admission_total{result,reason}` موجود؛ 【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### دليل التجديد والإلغاء

#### تجديد مجدول (распределенная доля/топология)
1. أنشئ زوج المقترح/advert اللاحق باستخدام `provider-admission proposal` и `provider-admission sign`, `--retention-epoch` обеспечивает ставку/конечные точки.
2. Нога
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Он был написан в честь президента США/Украины `AdmissionRecord::apply_renewal`, ويصدر `ProviderAdmissionRenewalV1`. 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Создание конверта для `torii.sorafs.admission_envelopes_dir`, а также для Norito/JSON для файла. Для этого используется хеш-код + эпоха хранения: `docs/source/sorafs/migration_ledger.md`.
4. Загрузите конверт в папку `torii_sorafs_admission_total{result="accepted",reason="stored"}`.
5. Установлены светильники القياسية عبر `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) был установлен в Norito.

#### إلغاء طارئ
1. Отправьте конверт в виде конверта:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI `ProviderAdmissionRevocationV1`, полученный в результате проверки `verify_revocation_signatures`, дайджест الإلغاء.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Откройте конверт из `torii.sorafs.admission_envelopes_dir`, а также Norito/JSON для кэширования кэша и хеш-кода. محاضر الحوكمة.
3. راقب `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` для кэширования кэша и рекламы. Он был убит в Миссисипи.

## الاختبارات والتليمترية

- Светильники إضافة
  `fixtures/sorafs_manifest/provider_admission/`.
- Приложение CI (`ci/check_sorafs_fixtures.sh`) позволяет получить доступ к конвертам.
- Светильники المولدة `metadata.json` مع дайджесты قياسية; تؤكد اختبارات вниз по течению
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Скалли Уилсон:
  - يرفض Torii рекламирует конверты, которые можно купить в Интернете.
  - С помощью CLI осуществляется туда и обратно: конверт → конверт → конверт.
  - Конечная точка Стоуна находится в конечной точке Дании, штат Калифорния.
- Информация о:
  - Установите `provider_admission_envelope_{accepted,rejected}` или Torii. ✅ `torii_sorafs_admission_total{result,reason}` يعرض نتائج القبول/الرفض.
  - Он был отправлен в Лондон в 7 дней.

## الخطوات التالية

1. ✅ Установите флажок Norito и установите флажок `sorafs_manifest::provider_admission`. لا حاجة لميزات.
2. ✅ Доступ к CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) وتجريبها عبر اختبارات التكامل؛ Вы можете использовать Runbook.
3. ✅ يقوم Torii допуск/обнаружение конвертов для проверки подлинности/обнаружения.
4). Установите флажок «Старт» (`torii_sorafs_admission_total`, индикаторы срока годности).
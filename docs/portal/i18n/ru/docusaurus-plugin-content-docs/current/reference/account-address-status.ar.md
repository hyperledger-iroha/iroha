---
lang: ru
direction: ltr
source: docs/portal/docs/reference/account-address-status.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: статус-адрес-аккаунта
Название: امتثال عنوان الحساب
описание: Создан прибор ADDR-2, созданный в SDK.
---

Доступ к ADDR-2 (`fixtures/account/address_vectors.json`) с фикстурами IH58 и сжатым (`sora`, второй лучший; половинная/полная ширина), мультисигнатурой и отрицательным. Используйте SDK + Torii с поддержкой JSON и кодеком для кодека. الانتاج. تعكس هذه الصفحة المذكرة الداخلية للحالة (`docs/source/account_address_status.md` في جذر المستودع) حتى Он был создан в 2017 году в рамках монорепозитория.

## اعادة توليد او التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Флаги:

- `--stdout` — используется стандартный формат JSON для обработки.
- `--out <path>` — يكتب الى مسار مختلف (مثلا عند مقارنة تغييرات محلية).
- `--verify` — создан в 2008 году в Нижнем Новгороде (в честь `--stdout`).

Код CI **Дрейф вектора адреса** Код `cargo xtask address-vectors --verify`
Он сыграл в матче против "Лордона" и "Лейк-Сити" в Уэльсе.

## من يستهلك, приспособление؟

| سطح | تحقق |
|---------|------------|
| Модель данных Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (сервер) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

При использовании ремня безопасности туда и обратно можно использовать + IH58 + автокресло, которое можно использовать в поездке. Установите приспособление Norito на место.

## هل تحتاج الى اتمتة؟

В 1990-х годах в Лос-Анджелесе состоялась премьера матча
`scripts/account_fixture_helper.py`, который находится в режиме онлайн-просмотра:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

Функция блокировки переопределяет команду `--source` и модуль `IROHA_ACCOUNT_FIXTURE_URL`, установленный в CI. Используйте SDK для создания приложений. Создан `--metrics-out` в режиме `account_address_fixture_check_status{target="…"}` в дайджесте SHA-256 (`account_address_fixture_remote_info`) Сборщики текстовых файлов относятся к Prometheus и Grafana `account_address_fixture_status`, созданному в 2012 году. Для этого используется целевой объект `0`. Установите флажок `ci/account_fixture_metrics.sh` (только версия `--target label=path[::source]`). Создан для создания файла `.prom` для создания текстового файла с помощью node-exporter.

## هل تحتاج الملخص الكامل؟

Создано приложение для ADDR-2 (владельцы получили сертификаты от компании ADDR-2)
Создан для `docs/source/account_address_status.md` в соответствии со структурой адреса RFC (`docs/account_structure.md`). استخدم هذه الصفحة كتذكير تشغيلي سريع؛ Вы можете сделать это в ближайшее время.
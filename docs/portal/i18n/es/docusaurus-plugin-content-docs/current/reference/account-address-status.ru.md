---
lang: es
direction: ltr
source: docs/portal/docs/reference/account-address-status.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: dirección-cuenta-estado
título: Соответствие адресов аккаунтов
descripción: Сводка рабочего процесса ADDR-2 aparato y sincronización del comando SDK.
---

Канонический пакет ADDR-2 (`fixtures/account/address_vectors.json`) incluye accesorios I105 (preferido), comprimido (`sora`, segundo mejor; ancho medio/completo), firma múltiple y negativo. Si utiliza SDK + Torii para operar en Odín y en JSON, podrá instalar un códec en el producto. Esta página contiene un bloque de estado actual (`docs/source/account_address_status.md` en el repositorio principal), cuáles son los principales portales de citas обратиться к flujo de trabajo без необходимости копаться в mono-repo.

## Перегенерация или проверка пакета

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Banderas:

- `--stdout`: incluye JSON en la salida estándar para archivos ad-hoc.
- `--out <path>` — записывает в другой путь (por ejemplo, при локальном сравнении изменений).
- `--verify` — сравнивает рабочую копию со свежесгенерированным содержимым (нельзя совмещать с `--stdout`).

Flujo de trabajo de CI **Deriva del vector de dirección** запускает `cargo xtask address-vectors --verify`
каждый раз, когда меняется luminarias, generadores o documentos, чтобы немедленно предупредить ревьюеров.

## ¿Qué estás usando?

| Superficie | Validación |
|---------|------------|
| Modelo de datos de Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK de JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK de Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |Каждый arnés выполняет канонических байт + I105 + сжатых кодировок and проверяет, что коды ошибок в стиле Norito совпадают с accesorio для negativo кейсов.

## ¿No hay automatización?

La herramienta de liberación puede automatizar la instalación del accesorio de ayuda
`scripts/account_fixture_helper.py`, который получает или проверяет канонический пакет без копирования/вставки:

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

El asistente anula las anulaciones de `--source` o la configuración permanente de `IROHA_ACCOUNT_FIXTURE_URL` y el SDK de CI que se puede utilizar предпочтительное зеркало. El asistente `--metrics-out` contiene `account_address_fixture_check_status{target="…"}` junto con el resumen canónico SHA-256 (`account_address_fixture_remote_info`), los recopiladores de archivos de texto Prometheus y El tablero de instrumentos Grafana `account_address_fixture_status` puede sincronizar el cable de alimentación. Tenga en cuenta que el objetivo es `0`. Para la automatización de múltiples superficies se utiliza el envoltorio `ci/account_fixture_metrics.sh` (principalmente `--target label=path[::source]`), los comandos públicos de guardia Archivo original `.prom` para el exportador de nodo del recopilador de archivos de texto.

## ¿No hay nada mejor que eso?

Полный статус соответствия ADDR-2 (propietarios, plan de monitorización, открытые задачи)
хранится в `docs/source/account_address_status.md` внутри репозитория вместе с Address Structure RFC (`docs/account_structure.md`). Ignore esta página como operación operativa; для глубокой справки обращайтесь к docs репозитория.
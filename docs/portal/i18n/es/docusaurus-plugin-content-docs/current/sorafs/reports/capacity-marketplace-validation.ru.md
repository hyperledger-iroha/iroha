---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Валидация маркетплейса емкости SoraFS
Etiquetas: [SF-2c, aceptación, lista de verificación]
resumen: Чеклист приемки, покрывающий онбординг proveedores, потоки споров и сверку казначейства, которые закрывают готовность маркетплейса емкости SoraFS к GA.
---

# Чеклист валидации маркетплейса емкости SoraFS

**Окно проверки:** 2026-03-18 -> 2026-03-24  
**Владельцы программы:** Equipo de almacenamiento (`@storage-wg`), Consejo de gobierno (`@council`), Gremio de tesorería (`@treasury`)  
**Objetivo:** Proveedores de paquetes integrados, dispositivos de almacenamiento y procesos de conexión, no necesarios para GA SF-2c.

Asegúrese de que no haya productos en el mercado para operadores de máquinas de coser. Каждая строка ссылается на детерминированные доказательства (pruebas, accesorios y documentos), которые аудиторы pueden воспроизвести.

## Чеклист приемки

### Proveedores de integración| Proverka | Validación | Доказательство |
|-------|------------|----------|
| Registro de códigos de registro canónicos | La prueba integrada incluye `/v2/sorafs/capacity/declare` con la API de la aplicación, la comprobación de datos, la actualización de metadatos y la actualización de los nuevos registros. | `crates/iroha_torii/src/routing.rs:7654` |
| Contrato inteligente que protege las cargas útiles | La garantía de la prueba unitaria, los proveedores de ID y los GiB comprometidos cumplen con la declaración oficial previa. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI выпускает канонические artefactos integrados | El arnés CLI contiene datos de ida y vuelta válidos Norito/JSON/Base64 y los operadores pueden activar declaraciones fuera de línea. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Руководство оператора описывает flujo de trabajo primero y guardarrailes управления | La documentación sobre las declaraciones, la política de incumplimientos y las revisiones del consejo. | `../storage-capacity-marketplace.md` |

### Разрешение споров| Proverka | Validación | Доказательство |
|-------|------------|----------|
| Записи споров сохраняются с каноническим resumen de carga útil | Registro de pruebas unitarias, carga útil decodificada y estados de estado pendientes de garantía del libro mayor de determinación. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Generador de CLI para aplicaciones canónicas | La CLI prueba archivos Base64/Norito y archivos JSON para `CapacityDisputeV1`, garantizando paquetes de evidencia hash determinados. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Replay-test доказывает детерминизм споров/пенализаций | Fallo de prueba de telemetría, registros de fallas, registros de instantáneas idénticas, créditos y disputas, barras diagonales de pares determinados. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook documentación sobre escalas y revocaciones | Consejos de flujo de trabajo de funciones operativas, ajustes de configuración y reversión de procedimientos. | `../dispute-revocation-runbook.md` |

### Сверка казначейства| Proverka | Validación | Доказательство |
|-------|------------|----------|
| Libro de contabilidad de devengo actualizado con un período de remojo de 30 días | Soak тест охватывает пять пять proveedores за 30 окон liquidación, сравнивая записи libro mayor с ожидаемой референсной выплатой. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Сверка экспорта libro mayor записывается каждую ночь | `capacity_reconcile.py` registra el libro mayor de tarifas con la exportación XOR, publica las métricas Prometheus y la puerta de enlace de Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Paneles de facturación que permiten acumular sanciones y telemetría | La importación Grafana incluye acumulación de horas GiB, huelgas y garantías garantizadas para llamadas de guardia. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Metodología de remojo y repetición de comandos públicos | Aquí se describen cómo remojar, los comandos de seguimiento y los ganchos de observabilidad para los auditores. | `./sf2c-capacity-soak.md` |

## Примечания по выполнению

Перезапустите набор проверок перед aprobación:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Los operadores que utilizan la tecnología de generación de cargas útiles incluyen la integración/descarga de datos `sorafs_manifest_stub capacity {declaration,dispute}` y la configuración de la información JSON/Norito bytes incluidos en el ticket de gobernanza.

## Aprobación de Артефакты| Artefacto | Camino | blake2b-256 |
|----------|------|-------------|
| Proveedores de paquetes de integración | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Пакет одобрения разрешения споров | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquetes de artículos de seguridad | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarde copias de estos artefactos en el paquete de lanzamiento y coloque los archivos en el paquete de lanzamiento.

## Подписи

- Líder del equipo de almacenamiento: @storage-tl (24/03/2026)  
- Secretario del Consejo de Gobernación — @council-sec (24/03/2026)  
- Líder de operaciones de tesorería — @treasury-ops (24/03/2026)
---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Carta de migración SoraFS"
---

> Adaptado a [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Дорожная карта миграции SoraFS (SF-1)

Este documento de operación recomienda recomendaciones para las migraciones y la seguridad en
`docs/source/sorafs_architecture_rfc.md`. Он разворачивает entregables SF-1 en
готовые к исполнению вехи, критерии гейтов и чек-листы владельцев, чтобы команды
Artefactos de publicaciones en la base SoraFS.

Дорожная карта намеренно детерминирована: каждая веха называет необходимые
Artefactos, comandos y otras certificaciones, qué programas de pago posteriores
идентичные выходы, а gobernancia сохраняла аудируемый след.

## Обзор вех

| Веха | Ok | Основные цели | Должно быть доставлено | Владельцы |
|------|------|---------------|-------------------------|-----------|
| **M1 - Aplicación determinista** | Números 7-12 | Pruebe los accesorios y las pruebas de alias, coloque los indicadores de expectativa. | Ночная верификация accesorios, манифесты с подписью совета, puesta en escena записи в alias registro. | Almacenamiento, Gobernanza, SDK |

El estado está modificado en `docs/source/sorafs/migration_ledger.md`. Все изменения
En este cuadro de diálogo adicional se incluyen el libro mayor, la gobernanza y la ingeniería de lanzamiento.
оставались синхронизированы.

## Потоки работ

### 2. Принятие детерминированного fijación| Шаг | Веха | Descripción | Propietario(s) | Выход |
|-----|------|----------|----------|-------|
| Repeticiones de partidos | M0 | Además de los ensayos en seco, se pueden utilizar resúmenes de fragmentos locales con `fixtures/sorafs_chunker`. Publicar en `docs/source/sorafs/reports/`. | Proveedores de almacenamiento | `determinism-<date>.md` con matriz pasa/falla. |
| Принудить подписи | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` son compatibles con la deriva o los manifiestos. El desarrollador anula la exención de gobernanza de требуют en PR. | Grupo de Trabajo sobre Herramientas | Лог CI, ссылка на waiver ticket (если применимо). |
| Banderas de expectativa | M1 | Пайплайны вызывают `sorafs_manifest_stub` с явными expectativas для фиксации salida: | Documentos CI | Обновленные скрипты со ссылкой на expectation flags (см. блок команды ниже). |
| Fijación de registro primero | M2 | `sorafs pin propose` и `sorafs pin approve` оборачивают отправку manifiesto; CLI para el usuario instalado `--require-registry`. | Operaciones de gobernanza | Registro de auditoría de CLI del Registro, телеметрия неудачных предложений. |
| Paridad de observabilidad | M3 | Los paneles Prometheus/Grafana proporcionan información sobre el inventario de fragmentos y los manifiestos de registro; alert'ы подключены к operaciones de guardia. | Observabilidad | Ссылка на panel, IDs правил алертов, результаты GameDay. |

#### Каноническая команда публикации

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Замените значения digest, размера и CID на ожидаемые ссылки, указанные в записи
libro mayor de migración для артефакта.

### 3. Переход на alias и коммуникации| Шаг | Веха | Descripción | Propietario(s) | Выход |
|-----|------|----------|----------|-------|
| Pruebas de alias en puesta en escena | M1 | Registre los reclamos de alias en la preparación del Registro de Pin y descargue las pruebas de Merkle y los manifiestos (`--alias`). | Gobernanza, Documentos | Paquete de prueba рядом с manifest + комментарий libro mayor с именем alias. |
| Ejecución de pruebas | M2 | Las puertas de enlace se configuran según los encabezados `Sora-Proof`; CI получает шаг `sorafs alias verify` для извлечения pruebas. | Redes | La configuración de parches de puerta de enlace + salida CI se realiza mediante un proveedor exclusivo. |

### 4. Comunicaciones y auditorías

- **Libro mayor:** каждое изменение состояния (derivación de accesorios, envío de registro,
  activación de alias) должно добавлять датированную заметку в
  `docs/source/sorafs/migration_ledger.md`.
- **Actas de gobierno:** заседания совета, утверждающие изменения pin registro или
  alias политик, должны ссылаться на эту дорожную карту и ledger.
- **Comunicados de comunicación:** DevRel publica el estado de actualización en cada etapa (blog +
  extracto del registro de cambios), подчеркивая детерминированные гарантии и таймлайны alias.

## Зависимости и риски| Зависимость | Влияние | Mitigación |
|------------|---------|-----------|
| Доступность контракта Registro de PIN | Блокирует M2 pin-first rollout. | Puede contratar el M2 con pruebas de repetición; поддерживать sobre de reserva para отсутствия регрессий. |
| Ключи подписи совета | Требуются для sobres de manifiesto y aprobaciones de registro. | Ceremonia de firma descripción en `docs/source/sorafs/signing_ceremony.md`; ротировать ключи с перекрытием и записью в ledger. |
| Cadence релизов SDK | Los clientes utilizan pruebas de alias para M3. | Sincronización de las versiones del SDK con puertas de hitos; Agregar listas de verificación de migración en plantillas de lanzamiento. |

Riesgos y mitigaciones relacionados con `docs/source/sorafs_architecture_rfc.md`
и должны кросс-референситься при изменениях.

## Чеклист критериев выхода

| Веха | Criterios |
|------|----------|
| M1 | - Trabajo nocturno en horarios fijos зеленый семь дней подряд.  - Pruebas de alias de puesta en escena проверены в CI.  - Banderas de expectativas políticas de gobernanza. |

## Управление изменениями

1. Antes de la configuración de PR, обновляя этот файл **и**
   `docs/source/sorafs/migration_ledger.md`.
2. Ссылайтесь на actas de gobernanza y evidencia de CI en descripciones de relaciones públicas.
3. Después de fusionar el almacenamiento + la lista de correo de DevRel con listas y actualizaciones
   действиями операторов.

Siga este procedimiento de garantía, ya que el lanzamiento SoraFS está determinado por el fabricante,
Аудируемым и прозрачным между командами, участвующими в запуске Nexus.
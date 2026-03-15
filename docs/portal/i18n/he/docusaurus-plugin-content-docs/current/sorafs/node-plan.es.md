---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: תוכנית צמתים
כותרת: Plan de implementación del nodo SoraFS
sidebar_label: Plan de implementación del nodo
תיאור: Convierte la hoja de ruta de almacenamiento SF-3 en trabajo de ingeniería accionable con hitos, tareas y cobertura de pruebas.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/sorafs_node_plan.md`. Mantén ambas copias sincronizadas hasta que la documentación heredada de Sphinx se retired.
:::

SF-3 entrega el primer ארגז ניתן להוצאת `sorafs-node` que convierte un processo Iroha/Torii en un proveedor de almacenamiento SoraFS. Usa este plan junto con la [guía de almacenamiento del nodo](node-storage.md), la [politica de admisión de proveedores](provider-admission-policy.md) y la [hoja de ruta del market de capacidad de almacenamiento de almacenamiento](0100205) נראים.

## Alcance objetivo (Hito M1)

1. ** אינטגרציה של נתחים.** Envuelve `sorafs_car::ChunkStore` עם קצה אחורי מתמשך עם שומר בתים של נתח, מתבטא ב-PoR ב-Directorio de datas configurado.
2. **נקודות קצה של שער.** הצגת נקודות קצה HTTP Norito עבור סיכות סיכות, אחזור של נתחים, מוסטראו PoR וטלמטריה של אלמסנמיינטו דנטרו של תהליך Torii.
3. **Plomería de configuración.** Agrega una estructura de config `SoraFsStorage` (דגל, capacidad, directorios, limites de concurrencia) cableada a través de `iroha_config`, I100NI3400 `iroha_torii`.
4. **Cuotas/planificación.** Impone límites de disco/paralelismo definidos por el operador y encola solicitudes con-back-pressure.
5. **Telemetría.** Emite métricas/logs para éxito de pins, latencia de fetch de chunks, utilización de capacidad and resultados de muestreo PoR.

## Desglose del trabajo

### A. Estructura de crate y módulos

| טארא | אחראי(ים) | Notas |
|------|---------------|------|
| Crear `crates/sorafs_node` עם שיטות: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Equipo de Storage | Reexporta tipos reutlizables para integración con Torii. |
| Implementar `StorageConfig` mapeado desde `SoraFsStorage` (שימוש → אמיתי → ברירות מחדל). | Equipo de Storage / Config WG | Asegura que las capas Norito/`iroha_config` קבוע דטרמיניסטים. |
| בדוק את `NodeHandle` כדי להשתמש ב-Torii בסיכות/שליפות. | Equipo de Storage | Encapsula internos de almacenamiento y plomería async. |

### B. Almacén de chunks מתמיד| טארא | אחראי(ים) | Notas |
|------|---------------|------|
| Construir un backend en disco que envuelva `sorafs_car::ChunkStore` con un indice de manifests en disco (`sled`/`sqlite`). | Equipo de Storage | דטרמיניסטת פריסה: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Mantener metadatos PoR (ארboles de 64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | Equipo de Storage | שידור חוזר של Soporta tras reinicios; falla rápido ante corrupción. |
| שידור חוזר מיושם de integridad al inicio (rehash de manifests, podar pins incompletos). | Equipo de Storage | Bloquea el arranque de Torii יש להשלים את השידור החוזר. |

### ג. נקודות קצה של שער

| נקודת קצה | Comportamiento | טאראס |
|--------|----------------|--------|
| `POST /sorafs/pin` | Acepta `PinProposalV1`, מניפסט valida, encola la ingestión, response con el CID del Manifest. | Valida el perfil de chunker, impone cuotas, streamea datas vía chunk store. |
| `GET /sorafs/chunks/{cid}` + consulta por rango | Sirve bytes de chunk con headers `Content-Chunker`; respeta la especificación de capacidad de rango. | ארה"ב מתזמן + preupuestos de stream (vincular a capacidad de rango SF-2d). |
| `POST /sorafs/por/sample` | Ejecuta muestreo PoR para un manifest y devuelve bundle de pruebas. | Reusa el muestreo del chunk store, תגובה עם מטענים Norito JSON. |
| `GET /sorafs/telemetry` | רזומה: capacidad, éxito de PoR, conteos de errores de fech. | מידע על לוחות מחוונים/מפעילים. |

La plomería en runtime enlaza las interacciones PoR a través de `sorafs_node::por`: el tracker registra cada `PorChallengeV1`, `PorProofV1` y `AuditVerdictV1` para que las00000X para que las0 de I1060X veredictos de gobernanza sin lógica Torii personalizada.【crates/sorafs_node/src/scheduler.rs#L147】

הערות ליישום:

- Usa el stack Axum de Torii עם מטענים `norito::json`.
- Arega esquemas Norito para respuestas (`PinResultV1`, `FetchErrorV1`, estructuras de telemetría).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` ahora expone la profundidad del backlog más la época/límite más antiguos y los timestamps de éxito/fallo más recientes por proveedor, impulsado por `sorafs_node::NodeHandle::por_ingestion_status`, y `sorafs_node::NodeHandle::por_ingestion_status`, y I00000180x registred gauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para לוחות מחוונים.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:18 83】【crates/iroha_torii/src/routing.rs:7244】【ארגזים/iroha_telemetry/src/metrics.rs:5390】

### D. מתזמן y cumplimiento de cuotas

| טארא | פרטים |
|------|--------|
| Cuota de disco | Seguimiento de bytes en disco; rechaza nuevos pins al superar `max_capacity_bytes`. הוכחת ווים של גירוש לפוליטיקה עתידית. |
| Concurrencia de fetch | Semáforo Global (`max_parallel_fetches`) עוד מוקדמים על ידי הוכחה ספירת גבולות SF-2d. |
| קולה דה סיכות | Limita los trabajos de ingestión pendientes; חשיפה של נקודות קצה Norito de estado para profundidad de cola. |
| Cadencia PoR | Worker en segundo plano impulsado por `por_sample_interval_secs`. |

### E. טלמטריה ורישום

Métricas (Prometheus):- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (היסטוגרמה עם כללי התנהגות `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

יומנים / אירועים:

- Telemetría Norito estructurada para ingestión de gobernanza (`StorageTelemetryV1`).
- התראות cuando la utilización > 90% o la racha de fallos PoR supera el umbral.

### F. Estrategia de pruebas

1. **Pruebas unitarias.** Persistencia del chunk store, cálculos de cuota, invariantes del scheduler (ver `crates/sorafs_node/src/scheduler.rs`).
2. **Pruebas de integración** (`crates/sorafs_node/tests`). סיכה → אחזר הלוך ושוב, recuperación tras reinicio, rechazo por cuota, verificación de pruebas de muestreo PoR.
3. **Pruebas de integración de Torii.** Ejecuta Torii עם תקינות, נקודות קצה בעלות קצה HTTP דרך `assert_cmd`.
4. **Hoja de ruta de caos.** Futuros מתרגלת סימולאן אגוטמיינטו דה דיסקו, IO lento, retiro de proveedores.

## Dependencias

- Politica de admisión SF-2b - אסיגורר que los nodos verifiquen envelopes de admisión antes de ununciarse.
- Marketplace de capacidad SF-2c - vincular la telemetría de vuelta a las declaraciones de capacidad.
- הרחבות פרסומת SF-2d - צריכת כמות גדולה של רנגו + הרשאות זרם זמינות.

## קריטריונים דה סלידה דל היטו

- `cargo run -p sorafs_node --example pin_fetch` מקומיים של מכשירי funciona contra.
- Torii קומפילה עם `--features sorafs-storage` y pasa pruebas de integración.
- Documentación ([guía de almacenamiento del nodo](node-storage.md)) בפועל עם ברירות המחדל של הגדרות + eemplos de CLI; רונבוק דה מפעיל זמין.
- Telemetría גלוי על לוחות המחוונים של הבמה; הגדרות התראה ל-Seturación de capacidad y fallos PoR.

## פריטי תיעוד ומבצעים

- Actualizar la [referencia de almacenamiento del nodo](node-storage.md) עם ברירות המחדל של הגדרות, שימוש ב-CLI ופתרון בעיות.
- Mantener el [runbook de operations de nodo](node-operations.md) alineado con la implementación conforme evoluciona SF-3.
- הפניות פומביות ל-API עבור נקודות קצה `/sorafs/*` דנטרו של הפורטל דה-sarrolladores y conectarlas al manifiesto OpenAPI זה לא מטפלים ב-Torii יש רשימות.
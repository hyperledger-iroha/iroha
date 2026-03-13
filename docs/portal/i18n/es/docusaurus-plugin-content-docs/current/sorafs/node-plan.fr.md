---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de nodo
título: Plan de implementación del nuevo SoraFS
sidebar_label: Plan de implementación del nuevo
descripción: Transformer la feuille de route de stockage SF-3 en travail d'ingénierie actionnable avec jalons, tâches et couverture de tests.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/sorafs_node_plan.md`. Guarde las dos copias sincronizadas justo al retrato de la documentación histórica de la Esfinge.
:::

SF-3 contiene el primer archivo ejecutable `sorafs-node` que transforma un proceso Iroha/Torii en un proveedor de almacenamiento SoraFS. Utilice este plan con la [guía de almacenamiento del nuevo](node-storage.md), la [política de admisión de proveedores](provider-admission-policy.md) y la [hoja de ruta del mercado de capacidad de almacenamiento](storage-capacity-marketplace.md) para secuenciar los hogares.

## Portée cible (Jalón M1)1. **Integración del almacén de fragmentos.** Envelopper `sorafs_car::ChunkStore` con un backend persistente que almacena los bytes de fragmentos, los manifiestos y los árboles PoR en el repertorio de datos configurados.
2. **Puerta de enlace de puntos finales.** Expositor de puntos finales HTTP Norito para la transmisión de pin, la recuperación de fragmentos, el acceso PoR y la télémétrie de almacenamiento en el proceso Torii.
3. **Plomería de configuración.** Agregue una estructura de configuración `SoraFsStorage` (bandera de activación, capacidad, repertorios, límites de concurrencia) confiada a `iroha_config`, `iroha_core` e `iroha_torii`.
4. **Cuota/ordenanza.** Aplicación de límites de disco/parallélisme definidos por el operador y mettre en archivo las solicitudes con contrapresión.
5. **Télémétrie.** Emmettre des métriques/logs pour les succès de pin, la latencia de fetch de chunks, la utilización de capacidad y los resultados de échantillonnage PoR.

## Descomposición del trabajo

### A. Estructura de caja y módulos.| tache | Responsable(s) | Notas |
|------|----------------|------|
| Cree `crates/sorafs_node` con los módulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Almacenamiento equipado | Exporte los tipos reutilizables para la integración Torii. |
| Implementador `StorageConfig` asignado después de `SoraFsStorage` (usuario → actual → valores predeterminados). | Equipo de almacenamiento / configuración WG | Garantía de que los sofás Norito/`iroha_config` permanecen determinados. |
| Fournir una fachada `NodeHandle` que Torii utiliza para soumettre pins/fetches. | Almacenamiento equipado | Encapsular los internos de almacenamiento y la fontanería async. |

### B. Almacenamiento de fragmentos persistente

| tache | Responsable(s) | Notas |
|------|----------------|------|
| Construya un sobre de disco backend `sorafs_car::ChunkStore` con un índice de manifiesto en disco (`sled`/`sqlite`). | Almacenamiento equipado | Diseño determinado: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Maintenir les métadonnées PoR (árboles 64 KiB/4 KiB) a través de `ChunkStore::sample_leaves`. | Almacenamiento equipado | Admite la repetición después de volver a casar; Falla rápido en casos de corrupción. |
| Implementar la repetición de integridad en el inicio (refrito de manifiestos, purga de pines incompletos). | Almacenamiento equipado | Bloquee el inicio de Torii justo al final de la reproducción. |

### C. Puerta de enlace de puntos finales| Punto final | Comportamiento | Tâches |
|----------|--------------|-------|
| `POST /sorafs/pin` | Acepte `PinProposalV1`, valide los manifiestos, cumpla con la ingestión en el archivo, responda con el CID del manifiesto. | Validar el perfil de fragmentador, aplicar cuotas, transmitir los donados a través de la tienda de fragmentos. |
| `GET /sorafs/chunks/{cid}` + solicitud de rango | Coloque los bytes del fragmento con los encabezados `Content-Chunker`; Respete la especificación de capacidad de alcance. | Utilice el programador + presupuestos de flujo (está en la capacidad del rango SF-2d). |
| `POST /sorafs/por/sample` | Lanza un échantillonnage PoR para un manifiesto y envía un paquete previo. | Al utilizar el canto del almacén de fragmentos, responda con las cargas útiles Norito JSON. |
| `GET /sorafs/telemetry` | Currículums: capacidad, éxito PoR, contadores de errores de recuperación. | Fournir les données pour Dashboards/Opérateurs. |

El tiempo de ejecución de la bomba depende de las interacciones PoR a través de `sorafs_node::por`: el rastreador se registra cada vez que `PorChallengeV1`, `PorProofV1` y `AuditVerdictV1` para que las métricas `CapacityMeter` reflejen los veredictos de gobierno sin lógica. Torii específicamente.【crates/sorafs_node/src/scheduler.rs#L147】

Notas de implementación:

- Utilice la pila Axum de Torii con las cargas útiles `norito::json`.
- Agregar esquemas Norito para respuestas (`PinResultV1`, `FetchErrorV1`, estructuras de televisión).- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` expone el mantenimiento de la profundidad del trabajo pendiente además de la época/échéance la más antigua y las marcas de tiempo de éxito/échec les plus récents por el proveedor, a través de `sorafs_node::NodeHandle::por_ingestion_status`, y Torii registra los calibres `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para archivos paneles.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Programador y aplicación de cuotas

| tache | Detalles |
|------|---------|
| Disco de cuota | Suivre les bytes en el disco; Rejeter les nouveaux pins au-delà de `max_capacity_bytes`. Fournir des ganchos de desalojo para los futuros políticos. |
| Concurrencia de recuperación | Sémaphore global (`max_parallel_fetches`) más presupuestos del proveedor issus des caps de range SF-2d. |
| Archivo de pines | Limitar las tareas de ingestión attente; Exponente de puntos finales de estado Norito para el procesador de archivos. |
| Cadencia PoR | Trabajador de fondo pilotado por `por_sample_interval_secs`. |

### E. Télémétrie y registro

Métricas (Prometheus) :

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histograma con etiquetas `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Registros / eventos :- Telemétrie Norito estructurada para la gobernanza de la ingestión (`StorageTelemetryV1`).
- Alertas cuando el uso es > 90 % o si la serie de controles PoR ha superado el segundo.

### F. Estrategia de pruebas

1. **Pruebas unitarias.** Persistencia del almacén de fragmentos, cálculos de cuota, invariantes del programador (ver `crates/sorafs_node/src/scheduler.rs`).
2. **Pruebas de integración** (`crates/sorafs_node/tests`). Pin → buscar aller-retour, reprise après redémarrage, rejet de cuota, vérification de preuve d'échantillonnage PoR.
3. **Pruebas de integración Torii.** Ejecutador Torii con el almacenamiento activado, ejerza los puntos finales HTTP a través de `assert_cmd`.
4. **Caos en la hoja de ruta.** Des Drills futurs simulent l'épuisement du disque, l'IO lente, la supresión de proveedores.

## Dependencias

- Política de admisión SF-2b: garantiza que los nuevos datos verifiquen los sobres de admisión antes de la publicación de anuncios.
- Marketplace de capacidad SF-2c: conecte la televisión a las declaraciones de capacidad.
- Extensiones de anuncio SF-2d: consuma la capacidad de alcance + presupuestos de flujo de disponibilidad.

## Criterios de salida del jalón- `cargo run -p sorafs_node --example pin_fetch` funciona en dispositivos locales.
- Torii compila con `--features sorafs-storage` y pasa las pruebas de integración.
- Documentación ([guide de stockage du nœud](node-storage.md)) puesta al día con valores predeterminados de configuración + ejemplos CLI; Operador de runbook disponible.
- Telemetría visible en los paneles de puesta en escena; Alertas configuradas para saturación de capacidad y control PoR.

## Documentación y operaciones de Livrables

- Introduzca hoy la [référence de stockage du nœud](node-storage.md) con los valores predeterminados de configuración, el uso de CLI y las etapas de solución de problemas.
- Garder le [runbook d'opérations du nœud](node-operations.md) alineado con la implementación au fur y à mesure de l'evolution SF-3.
- Publique las referencias API para los puntos finales `/sorafs/*` en el portal desarrollador y confíe en el manifiesto OpenAPI uno de los controladores Torii en su lugar.
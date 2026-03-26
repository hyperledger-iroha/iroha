---
lang: es
direction: ltr
source: docs/fraud_playbook.md
status: complete
translator: manual
source_hash: b3253ff47a513529c1dba6ef44faf38087ee0e5f5520f8c3fd770ab8d36c7786
source_last_modified: "2025-11-02T04:40:28.812006+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/fraud_playbook.md (Fraud Governance Playbook) -->

# Manual de Gobernanza de Fraude

Este documento resume el andamiaje necesario para la pila de fraude PSP mientras
los microservicios completos y los SDKs están en desarrollo activo. Recoge las
expectativas para analítica, flujos de trabajo de auditoría y procedimientos de
fallback, para que las implementaciones futuras se puedan acoplar al ledger de
forma segura.

## Resumen de Servicios

1. **API Gateway**: recibe payloads síncronos `RiskQuery`, los reenvía a la
   agregación de features y devuelve respuestas `FraudAssessment` a los flujos
   del ledger. Requiere alta disponibilidad (activo‑activo); usa pares
   regionales con hashing determinista para evitar sesgos de carga.
2. **Agregación de Features**: compone vectores de características para scoring.
   Emite únicamente hashes `FeatureInput`; los payloads sensibles permanecen
   off‑chain. La observabilidad debe exponer histogramas de latencia, gauges de
   profundidad de cola y contadores de replays por tenant.
3. **Motor de Riesgo**: evalúa reglas/modelos y produce salidas deterministas
   `FraudAssessment`. Asegura que el orden de ejecución de reglas sea estable y
   captura logs de auditoría por ID de evaluación.

## Analítica y Promoción de Modelos

- **Detección de anomalías**: mantiene un job de streaming que marque
  desviaciones en las tasas de decisión por tenant. Envía las alertas al
  panel de gobernanza y guarda resúmenes para revisiones trimestrales.
- **Análisis de grafos**: ejecuta recorridos nocturnos sobre exports
  relacionales para identificar clusters de colusión. Exporta hallazgos al
  portal de gobernanza mediante `GovernanceExport` con referencias a la
  evidencia de soporte.
- **Ingesta de feedback**: curar resultados de revisión manual e informes de
  chargeback. Convertirlos en deltas de features e incorporarlos a los
  datasets de entrenamiento. Publicar métricas de estado de ingesta para que
  el equipo de riesgo pueda detectar feeds bloqueados.
- **Pipeline de promoción de modelos**: automatiza la evaluación de candidatos
  (métricas offline, scoring canario, preparación para rollback). Las
  promociones deben emitir un conjunto de muestras `FraudAssessment` firmado y
  actualizar el campo `model_version` en `GovernanceExport`.

## Flujo de Trabajo de Auditoría

1. Tomar un snapshot del último `GovernanceExport` y verificar que el
   `policy_digest` coincida con el manifest proporcionado por el equipo de
   riesgo.
2. Validar que los agregados de reglas cuadren con los totales de decisiones
   del lado del ledger para la ventana muestreada.
3. Revisar los informes de detección de anomalías y análisis de grafos en busca
   de issues pendientes. Documentar las escaladas y las personas responsables
   de la remediación esperada.
4. Firmar y archivar la checklist de revisión. Almacenar los artefactos
   codificados con Norito en el portal de gobernanza para garantizar la
   reproducibilidad.

## Playbooks de Fallback

- **Caída del motor**: si el motor de riesgo está indisponible durante más de
  60 segundos, el gateway debe pasar a modo solo revisión, emitiendo
  `AssessmentDecision::Review` para todas las peticiones y alertando a los
  operadores.
- **Brecha de telemetría**: cuando las métricas o los traces se retrasen (más
  de 5 minutos sin datos), detener las promociones automáticas de modelos y
  notificar al ingeniero de guardia.
- **Regresión del modelo**: si el feedback posterior al despliegue indica un
  aumento de pérdidas por fraude, hacer rollback al bundle de modelo firmado
  anterior y actualizar el roadmap con acciones correctivas.

## Acuerdos de Intercambio de Datos

- Mantener anexos específicos por jurisdicción que cubran retención, cifrado y
  SLAs de notificación de brechas. Los partners deben firmar el anexo antes de
  recibir exports de `FraudAssessment`.
- Documentar prácticas de minimización de datos para cada integración (por
  ejemplo, hashing de identificadores de cuenta, truncado de números de
  tarjeta).
- Renovar los acuerdos anualmente o cuando cambien los requisitos
  regulatorios.

## Ejercicios de Red‑Team

- Los ejercicios se ejecutan trimestralmente. La próxima sesión está
  programada para **2026‑01‑15**, con escenarios que cubren envenenamiento de
  features, amplificación de replays e intentos de falsificación de firmas.
- Capturar los hallazgos en el modelo de amenazas de fraude y añadir las tareas
  resultantes a `roadmap.md` bajo el workstream
  «Fraud & Telemetry Governance Loop».

## Esquemas de API

El gateway ahora expone envelopes JSON concretos que mapean uno‑a‑uno a los
tipos Norito implementados en `crates/iroha_data_model::fraud`:

- **Entrada de riesgo** – `POST /v1/fraud/query` acepta el esquema
  `RiskQuery`:
  - `query_id` (`[u8; 32]`, codificado en hex)
  - `subject` (`AccountId`, `domainless encoded literal; canonical I105 only (non-canonical I105 literals rejected)`)
  - `operation` (enum etiquetado que coincide con `RiskOperation`; el
    discriminante JSON `type` refleja la variante del enum)
  - `related_asset` (`AssetId`, opcional)
  - `features` (array de `{ key: String, value_hash: hex32 }` mapeado desde
    `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext`; incluye `tenant_id`, `session_id` opcional y
    `reason` opcional)
- **Decisión de riesgo** – `POST /v1/fraud/assessment` consume el payload
  `FraudAssessment` (también reflejado en los exports de gobernanza):
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`,
    `decision` (enum `AssessmentDecision`), `rule_outcomes`
    (array de `{ rule_id, score_delta_bps, rationale }`)
  - `policy_digest` (hash Norito de la política activa)
  - `model_version` (string versionado)

La implementación en Rust usa Norito como codec de referencia para estas
estructuras; las representaciones JSON aquí descritas deben permanecer en
paridad con los tipos en `crates/iroha_data_model/src/fraud.rs` y con los
tests asociados.

---
lang: ru
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: es
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00cf1d37cf06d24b6eb7b2acba6b5c2ec3c3fae249b5cb6055384ca19ceaefac
source_last_modified: "2025-11-10T16:27:31.384538+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Especificación del runner de moderación de IA
summary: Diseño determinista del comité de moderación para el entregable del Ministerio de Información (MINFO-1).
---

# Especificación del runner de moderación de IA

Esta especificación cumple la parte de documentación de **MINFO-1 — Establecer la línea base de moderación de IA**. Define el contrato de ejecución determinista del servicio de moderación del Ministerio de Información para que cada gateway ejecute canalizaciones idénticas antes de los flujos de apelaciones y transparencia (SFM-4/SFM-4b). Todo el comportamiento descrito aquí es normativo salvo que se marque explícitamente como informativo.

## 1. Objetivos y alcance
- Proveer un comité de moderación reproducible que evalúe el contenido del gateway (objetos, manifiestos, metadatos, audio) usando modelos heterogéneos.
- Garantizar ejecución determinista entre operadores: opset fijo, tokenización con semilla, precisión acotada y artefactos versionados.
- Producir artefactos listos para auditoría: manifiestos, scorecards, evidencias de calibración y digests de transparencia aptos para publicar en el DAG de gobernanza.
- Exponer telemetría para que SRE pueda detectar deriva, falsos positivos y caídas sin recolectar datos crudos de usuarios.

## 2. Contrato de ejecución determinista
- **Runtime:** ONNX Runtime 1.19.x (backend CPU) compilado con AVX2 deshabilitado y `--enable-extended-minimal-build` para mantener fijo el conjunto de opcodes. Los runtimes CUDA/Metal están explícitamente prohibidos en producción.
- **Opset:** `opset=17`. Los modelos que apunten a opsets más nuevos deben convertirse hacia abajo y validarse antes de su admisión.
- **Derivación de la semilla:** cada evaluación deriva una semilla RNG de `BLAKE3(content_digest || manifest_id || run_nonce)` donde `run_nonce` proviene del manifiesto aprobado por gobernanza. Las semillas alimentan todos los componentes estocásticos (beam search, toggles de dropout) para que los resultados sean reproducibles bit a bit.
- **Hilos:** un worker por modelo. La concurrencia la coordina el orquestador del runner para evitar condiciones de carrera en estado compartido. Las librerías BLAS operan en modo de un solo hilo.
- **Numéricos:** se prohíbe la acumulación FP16. Usa intermedios FP32 y limita las salidas a cuatro decimales antes de agregarlas.

## 3. Composición del comité
El comité base contiene tres familias de modelos. La gobernanza puede añadir modelos, pero el quórum mínimo debe seguir satisfecho.

| Familia | Modelo base | Propósito |
|--------|----------------|---------|
| Visión | OpenCLIP ViT-H/14 (ajustado para seguridad) | Detecta contrabando visual, violencia, indicadores de CSAM. |
| Multimodal | LLaVA-1.6 34B Safety | Capta interacciones de texto + imagen, pistas contextuales, acoso. |
| Perceptual | Ensamble pHash + aHash + NeuralHash-lite | Detección rápida de casi duplicados y recuperación de material malicioso conocido. |

Cada entrada del modelo especifica:
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 de la imagen OCI)
- `weights_digest` (BLAKE3-256 del ONNX o blob safetensors combinado)
- `opset` (debe ser `17`)
- `weight` (peso del comité, por defecto `1.0`)
- `critical_labels` (conjunto de etiquetas que disparan `Escalate` de inmediato)
- `max_eval_ms` (guardarraíl para watchdogs deterministas)

## 4. Manifiestos y resultados Norito

### 4.1 Manifiesto del comité
```norito
struct AiModerationManifestV1 {
    manifest_id: Uuid,
    issued_at: Timestamp,
    runner_hash: Digest32,
    runtime_version: String,
    models: Vec<AiModerationModelV1>,
    calibration_dataset: DatasetReferenceV1,
    calibration_hash: Digest32,
    thresholds: AiModerationThresholdsV1,
    run_nonce: Digest32,
    governance_signature: Signature,
}

struct AiModerationModelV1 {
    model_id: Uuid,
    family: AiModerationFamilyV1, // vision | multimodal | perceptual | audio
    artifact_digest: Digest32,
    weights_digest: Digest32,
    opset: u8,
    weight: f32,
    critical_labels: Vec<String>,
    max_eval_ms: u32,
}
```

### 4.2 Resultado de evaluación
```norito
struct AiModerationResultV1 {
    manifest_id: Uuid,
    request_id: Uuid,
    content_digest: Digest32,
    content_uri: String,
    content_class: ModerationContentClassV1, // manifest | chunk | metadata | audio
    model_scores: Vec<AiModerationModelScoreV1>,
    combined_score: f32,
    verdict: ModerationVerdictV1, // pass | quarantine | escalate
    executed_at: Timestamp,
    execution_ms: u32,
    runner_hash: Digest32,
    annotations: Option<Vec<String>>,
}

struct AiModerationModelScoreV1 {
    model_id: Uuid,
    score: f32,
    threshold: f32,
    confidence: f32,
    label: Option<String>,
}
```

El runner DEBE emitir un `AiModerationDigestV1` determinista (BLAKE3 sobre el resultado serializado) para los logs de transparencia y anexar resultados al ledger de moderación cuando el veredicto no sea `pass`.

### 4.3 Manifiesto del corpus adversarial

Los operadores de gateway ahora ingieren un manifiesto compañero que enumera “familias” de hashes/embeddings perceptuales derivadas de las ejecuciones de calibración:

```norito
struct AdversarialCorpusManifestV1 {
    schema_version: u16,                // must equal 1
    issued_at_unix: u64,
    cohort_label: Option<String>,       // e.g. "2026-Q1"
    families: Vec<AdversarialPerceptualFamilyV1>,
}

struct AdversarialPerceptualFamilyV1 {
    family_id: Uuid,
    description: String,
    variants: Vec<AdversarialPerceptualVariantV1>,
}

struct AdversarialPerceptualVariantV1 {
    variant_id: Uuid,
    attack_vector: String,
    reference_cid_b64: Option<String>,
    perceptual_hash: Option<Digest32>,   // Goldilocks hash, BLAKE3 domain separated
    hamming_radius: u8,                  // ≤ 32
    embedding_digest: Option<Digest32>,  // BLAKE3 of quantised embedding vector
    notes: Option<String>,
}
```

El esquema vive en `crates/iroha_data_model/src/sorafs/moderation.rs` y se valida mediante `AdversarialCorpusManifestV1::validate()`. El manifiesto permite que el cargador de denylists del gateway pueble entradas `perceptual_family` que bloquean clusters de casi duplicados completos en lugar de bytes individuales. Un fixture ejecutable (`docs/examples/ai_moderation_perceptual_registry_202602.json`) demuestra el layout esperado y alimenta directamente la denylist de gateway de ejemplo.

## 5. Pipeline de ejecución
1. Cargar `AiModerationManifestV1` desde el DAG de gobernanza. Rechazar si `runner_hash` o `runtime_version` no coinciden con el binario desplegado.
2. Obtener artefactos de modelo vía digest OCI, verificando digests antes de cargar.
3. Construir lotes de evaluación por tipo de contenido; el orden debe ordenar por `(content_digest, manifest_id)` para asegurar agregación determinista.
4. Ejecutar cada modelo con la semilla derivada. Para hashes perceptuales, combinar el ensamble por voto mayoritario → score en `[0,1]`.
5. Agregar puntajes en `combined_score` usando la razón recortada ponderada:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. Producir `ModerationVerdictV1`:
   - `escalate` si dispara alguna `critical_labels` o `combined ≥ thresholds.escalate`.
   - `quarantine` si supera `thresholds.quarantine` pero está por debajo de `escalate`.
   - `pass` en caso contrario.
7. Persistir `AiModerationResultV1` y encolar procesos downstream:
   - Servicio de cuarentena (si el veredicto escala o cuarentena)
   - Escritor del log de transparencia (`ModerationLedgerV1`)
   - Exportador de telemetría

## 6. Calibración y evaluación
- **Datasets:** la calibración base usa el corpus mixto curado con aprobación del equipo de políticas. Referencia registrada en `calibration_dataset`.
- **Métricas:** calcular Brier score, Expected Calibration Error (ECE) y AUROC por modelo y veredicto combinado. La recalibración mensual DEBE mantener `Brier ≤ 0.18` y `ECE ≤ 0.05`. Los resultados se almacenan en el árbol de reportes SoraFS (p. ej., [calibración febrero 2026](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Calendario:** recalibración mensual (primer lunes). Se permite recalibración de emergencia si se disparan alertas de deriva.
- **Proceso:** ejecutar la canalización de evaluación determinista sobre el set de calibración, regenerar `thresholds`, actualizar el manifiesto y preparar cambios para voto de gobernanza.

## 7. Empaquetado y despliegue
- Construir imágenes OCI vía `docker buildx bake -f docker/ai_moderation.hcl`.
- Las imágenes incluyen:
  - Entorno Python bloqueado (`poetry.lock`) o binario Rust `Cargo.lock`.
  - Directorio `models/` con pesos ONNX hasheados.
  - Punto de entrada `run_moderation.py` (o equivalente en Rust) que expone API HTTP/gRPC.
- Publicar artefactos en `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`.
- El binario del runner se envía como parte del crate `sorafs_ai_runner`. El pipeline de build incrusta el hash del manifiesto en el binario (expuesto vía `/v1/info`).

## 8. Telemetría y observabilidad
- Métricas Prometheus:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Logs: JSON lines con `request_id`, `manifest_id`, `verdict` y el digest del resultado almacenado. Los puntajes crudos se redaccionan a dos decimales en los logs.
- Dashboards almacenados en `dashboards/grafana/ministry_moderation_overview.json` (publicados junto al primer reporte de calibración).
- Umbrales de alerta:
  - Ingesta faltante (`moderation_requests_total` detenido por 10 minutos).
  - Detección de deriva (delta de score promedio de modelo >20% vs media móvil de 7 días).
  - Backlog de falsos positivos (cola de cuarentena > 50 ítems por >30 minutos).

## 9. Gobernanza y control de cambios
- Los manifiestos requieren doble firma: miembro del consejo del Ministerio + líder SRE de moderación. Las firmas se registran en `AiModerationManifestV1.governance_signature`.
- Los cambios siguen `ModerationManifestChangeProposalV1` a través de Torii. Los hashes se ingresan en el DAG de gobernanza; el despliegue queda bloqueado hasta que la propuesta se promulgue.
- Los binarios del runner incrustan `runner_hash`; CI rechaza el despliegue si los hashes divergen.
- Transparencia: `ModerationScorecardV1` semanal que resume volumen, mix de veredictos y resultados de apelaciones. Publicado en el portal del Parlamento de Sora.

## 10. Seguridad y privacidad
- Los digests de contenido usan BLAKE3. Los payloads crudos nunca persisten fuera de cuarentena.
- El acceso a cuarentena requiere aprobaciones Just-In-Time; todos los accesos se registran.
- El runner sandboxea contenido no confiable, imponiendo límites de memoria de 512 MiB y guardarraíles de 120 s de wall-clock.
- La privacidad diferencial NO se aplica aquí; los gateways confían en cuarentena + flujos de auditoría. Las políticas de redacción siguen el plan de cumplimiento del gateway (`docs/source/sorafs_gateway_compliance_plan.md`; copia en portal pendiente).

## 11. Publicación de la calibración (2026-02)
- **Manifiesto:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  registra el `AiModerationManifestV1` firmado por gobernanza (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), referencia de dataset
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, hash del runner
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` y los
  umbrales de calibración 2026-02 (`quarantine = 0.42`, `escalate = 0.78`).
- **Scoreboard:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  más el reporte legible en
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  capturan Brier, ECE, AUROC y el mix de veredictos para cada modelo. Las métricas combinadas cumplieron los objetivos (`Brier = 0.126`, `ECE = 0.034`).
- **Dashboards y alertas:** `dashboards/grafana/ministry_moderation_overview.json`
  y `dashboards/alerts/ministry_moderation_rules.yml` (con pruebas de regresión en
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) proporcionan la historia de monitoreo de ingestión/latencia/deriva requerida para el rollout.

## 12. Esquema de reproducibilidad y validador (MINFO-1b)
- Los tipos Norito canónicos ahora viven junto al resto del esquema SoraFS en
  `crates/iroha_data_model/src/sorafs/moderation.rs`. Las estructuras
  `ModerationReproManifestV1`/`ModerationReproBodyV1` capturan el UUID del manifiesto, el hash del runner, los digests de modelos, el conjunto de umbrales y el material de semilla.
  `ModerationReproManifestV1::validate` aplica la versión de esquema
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), asegura que cada manifiesto lleve al menos un modelo y firmante, y verifica cada `SignatureOf<ModerationReproBodyV1>` antes de devolver un resumen legible por máquina.
- Los operadores pueden invocar el validador compartido vía
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (implementado en `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`). La CLI
  acepta tanto los artefactos JSON publicados en
  `docs/examples/ai_moderation_calibration_manifest_202602.json` como la codificación Norito cruda e imprime los conteos de modelos/firmas junto con el timestamp del manifiesto al completar la validación.
- Gateways y automatización se conectan al mismo helper para que los manifiestos de reproducibilidad puedan rechazarse de forma determinista cuando el esquema deriva, faltan digests o fallan las firmas.
- Los bundles de corpus adversarial siguen el mismo patrón:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  analiza `AdversarialCorpusManifestV1`, aplica la versión de esquema y rechaza manifiestos que omitan familias, variantes o metadatos de huella. Las ejecuciones exitosas emiten el timestamp de emisión, la etiqueta de cohorte y los conteos de familias/variantes para que los operadores puedan fijar la evidencia antes de actualizar las entradas de denylist del gateway descritas en la Sección 4.3.

## 13. Pendientes abiertos
- Las ventanas de recalibración mensual después del 2026-03-02 continúan siguiendo el procedimiento de la Sección 6; publica `ai-moderation-calibration-<YYYYMM>.md` junto con bundles actualizados de manifiesto/scorecard bajo el árbol de reportes SoraFS.
- MINFO-1b y MINFO-1c (validadores de manifiestos de reproducibilidad más registro de corpus adversarial) permanecen rastreados por separado en el roadmap.

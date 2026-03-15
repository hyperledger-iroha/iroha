---
lang: es
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: pt
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00cf1d37cf06d24b6eb7b2acba6b5c2ec3c3fae249b5cb6055384ca19ceaefac
source_last_modified: "2025-11-10T16:27:31.384538+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Especificação do runner de moderação de IA
summary: Design determinístico do comitê de moderação para o entregável do Ministério da Informação (MINFO-1).
---

# Especificação do runner de moderação de IA

Esta especificação cumpre a parte de documentação de **MINFO-1 — Estabelecer a linha de base de moderação de IA**. Ela define o contrato de execução determinístico do serviço de moderação do Ministério da Informação para que cada gateway execute pipelines idênticos antes dos fluxos de apelação e transparência (SFM-4/SFM-4b). Todo o comportamento descrito aqui é normativo, salvo quando explicitamente marcado como informativo.

## 1. Objetivos e escopo
- Fornecer um comitê de moderação reproduzível que avalia o conteúdo do gateway (objetos, manifests, metadados, áudio) usando modelos heterogêneos.
- Garantir execução determinística entre operadores: opset fixo, tokenização com semente, precisão limitada e artefatos versionados.
- Produzir artefatos prontos para auditoria: manifests, scorecards, evidências de calibração e digests de transparência adequados para publicação no DAG de governança.
- Expor telemetria para que SREs detectem deriva, falsos positivos e indisponibilidade sem coletar dados brutos de usuários.

## 2. Contrato de execução determinístico
- **Runtime:** ONNX Runtime 1.19.x (backend CPU) compilado com AVX2 desabilitado e `--enable-extended-minimal-build` para manter o conjunto de opcodes fixo. Runtimes CUDA/Metal são explicitamente proibidos em produção.
- **Opset:** `opset=17`. Modelos que visam opsets mais novos devem ser rebaixados e validados antes da admissão.
- **Derivação da semente:** cada avaliação deriva uma semente RNG de `BLAKE3(content_digest || manifest_id || run_nonce)` onde `run_nonce` vem do manifesto aprovado pela governança. As sementes alimentam todos os componentes estocásticos (beam search, toggles de dropout) para que os resultados sejam reproduzíveis bit a bit.
- **Threading:** um worker por modelo. A concorrência é coordenada pelo orquestrador do runner para evitar condições de corrida de estado compartilhado. Bibliotecas BLAS operam em modo single-thread.
- **Numéricos:** acumulação FP16 é proibida. Use intermediários FP32 e limite as saídas a quatro casas decimais antes da agregação.

## 3. Composição do comitê
O comitê base contém três famílias de modelos. A governança pode adicionar modelos, mas o quórum mínimo deve permanecer satisfeito.

| Família | Modelo base | Propósito |
|--------|----------------|---------|
| Visão | OpenCLIP ViT-H/14 (ajustado para segurança) | Detecta contrabando visual, violência, indicadores de CSAM. |
| Multimodal | LLaVA-1.6 34B Safety | Capta interações texto + imagem, pistas contextuais, assédio. |
| Perceptual | Ensemble pHash + aHash + NeuralHash-lite | Detecção rápida de quase duplicados e recall de material malicioso conhecido. |

Cada entrada de modelo especifica:
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 da imagem OCI)
- `weights_digest` (BLAKE3-256 do ONNX ou blob safetensors combinado)
- `opset` (deve ser `17`)
- `weight` (peso do comitê, padrão `1.0`)
- `critical_labels` (conjunto de labels que acionam `Escalate` imediatamente)
- `max_eval_ms` (guard-rail para watchdogs determinísticos)

## 4. Manifests e resultados Norito

### 4.1 Manifesto do comitê
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

### 4.2 Resultado da avaliação
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

O runner DEVE emitir um `AiModerationDigestV1` determinístico (BLAKE3 sobre o resultado serializado) para os logs de transparência e anexar resultados ao ledger de moderação quando o veredito não for `pass`.

### 4.3 Manifesto do corpus adversarial

Operadores de gateway agora ingerem um manifesto complementar que enumera “famílias” de hashes/embeddings perceptuais derivados das execuções de calibração:

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

O esquema vive em `crates/iroha_data_model/src/sorafs/moderation.rs` e é validado via `AdversarialCorpusManifestV1::validate()`. O manifesto permite que o carregador de denylist do gateway preencha entradas `perceptual_family` que bloqueiam clusters inteiros de quase duplicados em vez de bytes individuais. Um fixture executável (`docs/examples/ai_moderation_perceptual_registry_202602.json`) demonstra o layout esperado e alimenta diretamente a denylist de gateway de exemplo.

## 5. Pipeline de execução
1. Carregar `AiModerationManifestV1` do DAG de governança. Rejeitar se `runner_hash` ou `runtime_version` não corresponderem ao binário implantado.
2. Buscar artefatos de modelo via digest OCI, verificando digests antes de carregar.
3. Construir lotes de avaliação por tipo de conteúdo; a ordenação deve classificar por `(content_digest, manifest_id)` para garantir agregação determinística.
4. Executar cada modelo com a semente derivada. Para hashes perceptuais, combinar o ensemble via voto majoritário → score em `[0,1]`.
5. Agregar scores em `combined_score` usando a razão recortada ponderada:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. Produzir `ModerationVerdictV1`:
   - `escalate` se qualquer `critical_labels` acionar ou `combined ≥ thresholds.escalate`.
   - `quarantine` se acima de `thresholds.quarantine` mas abaixo de `escalate`.
   - `pass` caso contrário.
7. Persistir `AiModerationResultV1` e enfileirar processos downstream:
   - Serviço de quarentena (se o veredito escalar/quarentenar)
   - Escritor do log de transparência (`ModerationLedgerV1`)
   - Exportador de telemetria

## 6. Calibração e avaliação
- **Datasets:** a calibração base usa o corpus misto curado com aprovação da equipe de políticas. Referência registrada em `calibration_dataset`.
- **Métricas:** calcular Brier, Expected Calibration Error (ECE) e AUROC por modelo e veredito combinado. A recalibração mensal DEVE manter `Brier ≤ 0.18` e `ECE ≤ 0.05`. Resultados armazenados na árvore de relatórios SoraFS (ex.: [calibração de fevereiro de 2026](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Agenda:** recalibração mensal (primeira segunda-feira). Recalibração emergencial permitida se alertas de deriva dispararem.
- **Processo:** executar o pipeline determinístico de avaliação no conjunto de calibração, regenerar `thresholds`, atualizar o manifesto e preparar mudanças para voto de governança.

## 7. Empacotamento e implantação
- Construir imagens OCI via `docker buildx bake -f docker/ai_moderation.hcl`.
- As imagens incluem:
  - Ambiente Python travado (`poetry.lock`) ou binário Rust `Cargo.lock`.
  - Diretório `models/` com pesos ONNX hashados.
  - Ponto de entrada `run_moderation.py` (ou equivalente Rust) expondo API HTTP/gRPC.
- Publicar artefatos em `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`.
- O binário do runner é entregue como parte do crate `sorafs_ai_runner`. O pipeline de build embute o hash do manifesto no binário (exposto via `/v2/info`).

## 8. Telemetria e observabilidade
- Métricas Prometheus:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Logs: linhas JSON com `request_id`, `manifest_id`, `verdict` e o digest do resultado armazenado. Os scores brutos são redigidos para duas casas decimais nos logs.
- Dashboards armazenados em `dashboards/grafana/ministry_moderation_overview.json` (publicados junto com o primeiro relatório de calibração).
- Limiares de alerta:
  - Ingestão ausente (`moderation_requests_total` parado por 10 minutos).
  - Detecção de deriva (delta médio de score do modelo >20% versus média móvel de 7 dias).
  - Backlog de falso positivo (fila de quarentena > 50 itens por >30 minutos).

## 9. Governança e controle de mudanças
- Manifests exigem dupla assinatura: membro do conselho do Ministério + líder SRE de moderação. As assinaturas são registradas em `AiModerationManifestV1.governance_signature`.
- Mudanças seguem `ModerationManifestChangeProposalV1` via Torii. Hashes são inseridos no DAG de governança; a implantação fica bloqueada até a proposta ser aprovada.
- Binários do runner embutem `runner_hash`; a CI recusa a implantação se os hashes divergirem.
- Transparência: `ModerationScorecardV1` semanal resumindo volume, mix de veredictos e resultados de apelação. Publicado no portal do Parlamento Sora.

## 10. Segurança e privacidade
- Digests de conteúdo usam BLAKE3. Payloads brutos nunca persistem fora da quarentena.
- Acesso à quarentena requer aprovações Just-In-Time; todos os acessos são registrados.
- O runner sandboxa conteúdo não confiável, impondo limites de memória de 512 MiB e guard-rails de 120 s de wall-clock.
- Privacidade diferencial NÃO é aplicada aqui; gateways confiam em quarentena + fluxos de auditoria. Políticas de redaction seguem o plano de conformidade do gateway (`docs/source/sorafs_gateway_compliance_plan.md`; cópia do portal pendente).

## 11. Publicação da calibração (2026-02)
- **Manifesto:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  registra o `AiModerationManifestV1` assinado pela governança (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), referência de dataset
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, hash do runner
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` e os
  thresholds de calibração 2026-02 (`quarantine = 0.42`, `escalate = 0.78`).
- **Scoreboard:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  mais o relatório legível em
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  capturam Brier, ECE, AUROC e o mix de veredictos para cada modelo. As métricas combinadas atenderam aos alvos (`Brier = 0.126`, `ECE = 0.034`).
- **Dashboards e alertas:** `dashboards/grafana/ministry_moderation_overview.json`
  e `dashboards/alerts/ministry_moderation_rules.yml` (com testes de regressão em
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) fornecem o monitoramento de ingestão/latência/deriva requerido para o rollout.

## 12. Esquema de reprodutibilidade e validador (MINFO-1b)
- Tipos Norito canônicos agora vivem junto ao restante do esquema SoraFS em
  `crates/iroha_data_model/src/sorafs/moderation.rs`. As estruturas
  `ModerationReproManifestV1`/`ModerationReproBodyV1` capturam o UUID do manifesto, o hash do runner, os digests dos modelos, o conjunto de thresholds e o material de semente.
  `ModerationReproManifestV1::validate` aplica a versão do esquema
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), garante que cada manifesto carregue ao menos um modelo e assinante, e verifica cada `SignatureOf<ModerationReproBodyV1>` antes de retornar um resumo legível por máquina.
- Operadores podem invocar o validador compartilhado via
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (implementado em `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`). A CLI
  aceita tanto os artefatos JSON publicados em
  `docs/examples/ai_moderation_calibration_manifest_202602.json` quanto a codificação Norito bruta e imprime as contagens de modelos/assinaturas junto com o timestamp do manifesto após validação.
- Gateways e automação se conectam ao mesmo helper para que manifests de reprodutibilidade possam ser rejeitados deterministicamente quando o esquema diverge, faltam digests ou as assinaturas falham.
- Bundles de corpus adversarial seguem o mesmo padrão:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  analisa `AdversarialCorpusManifestV1`, aplica a versão do esquema e recusa manifests que omitem famílias, variantes ou metadados de impressão digital. Execuções bem-sucedidas emitem o timestamp de emissão, o rótulo de coorte e as contagens de família/variante para que operadores possam fixar a evidência antes de atualizar as entradas de denylist do gateway descritas na Seção 4.3.

## 13. Pendências abertas
- As janelas mensais de recalibração após 2026-03-02 continuam seguindo o procedimento da Seção 6; publicar `ai-moderation-calibration-<YYYYMM>.md` junto com bundles atualizados de manifesto/scorecard sob a árvore de relatórios SoraFS.
- MINFO-1b e MINFO-1c (validadores de manifestos de reprodutibilidade mais registro do corpus adversarial) permanecem acompanhados separadamente no roadmap.

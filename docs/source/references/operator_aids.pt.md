---
lang: pt
direction: ltr
source: docs/source/references/operator_aids.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf412f2645cea9d5468f4541ff48d4ace67bc6f3d60a97e68561dde4949ff9be
source_last_modified: "2025-12-13T10:25:50.323533+00:00"
translation_last_reviewed: 2026-01-01
---

# Endpoints do Torii — Auxílio ao operador (Referência rápida)

Esta página lista endpoints não consensuais, voltados a operadores, que ajudam com visibilidade e solução de problemas. As respostas são JSON salvo indicação.

Consenso (Sumeragi)
- Métricas: os gauges `sumeragi_new_view_receipts_by_hv{height,view}` refletem as contagens.
- GET `/v1/sumeragi/status`
  - Instantâneo do índice do líder, Highest/Locked QCs (`highest_qc`/`locked_qc`, alturas, views, hashes de subject), contadores de coletores/VRF, adiamentos do pacemaker, profundidade da fila de transações e saúde do armazenamento RBC (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`).
- GET `/v1/sumeragi/status/sse`
  - Fluxo SSE (≈1 s) do mesmo payload que `/v1/sumeragi/status` para dashboards ao vivo.
- GET `/v1/sumeragi/qc`
  - Instantâneo de highest/locked QCs; inclui `subject_block_hash` para o highest QC quando conhecido.
- GET `/v1/sumeragi/pacemaker`
  - Temporizadores/configuração do pacemaker: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v1/sumeragi/leader`
  - Instantâneo do índice do líder. Em modo NPoS, inclui contexto PRF: `{ height, view, epoch_seed }`.
- GET `/v1/sumeragi/telemetry`
  - Aggregated consensus telemetry: `availability.collectors` contains observed collector indices, peer IDs, and ingested-vote counts; `rbc_backlog` contains missing-chunk totals; `rbc_pending` contains bounded pre-session queue totals, drops, and limits. This is not a deterministic collector plan or a per-session RBC contract.
- GET `/v1/sumeragi/params`
  - Instantâneo dos parâmetros Sumeragi on-chain `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - When `da_enabled` is true, availability evidence is tracked but does not gate commit; local payload is required and can be satisfied via RBC `DELIVER` or block sync. Use the aggregated telemetry endpoint, Prometheus counters, status snapshots, and logs to diagnose payload transport.

Evidência (auditoria; sem consenso)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Inclui campos básicos (p. ex., DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) para inspeção.
  - Exemplos:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- POST `/v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - Auxílios de CLI:
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (ou `--evidence-hex-file <path>`)

Autenticação de operador (WebAuthn/mTLS)
- POST `/v1/operator/auth/registration/options`
  - Retorna opções de registro WebAuthn (`publicKey`) para o cadastro inicial de credenciais.
- POST `/v1/operator/auth/registration/verify`
  - Verifica o payload de attestation WebAuthn e persiste a credencial do operador.
- POST `/v1/operator/auth/login/options`
  - Retorna opções de autenticação WebAuthn (`publicKey`) para o login do operador.
- POST `/v1/operator/auth/login/verify`
  - Verifica o payload de assertion WebAuthn e retorna um token de sessão do operador.
- Cabeçalhos:
  - `x-iroha-operator-session`: token de sessão para endpoints de operador (emitido por login verify).
  - `x-iroha-operator-token`: token bootstrap (permitido quando `torii.operator_auth.token_fallback` permite).
  - `x-api-token`: exigido quando `torii.require_api_token = true` ou `torii.operator_auth.token_source = "api"`.
  - `x-forwarded-client-cert`: exigido quando `torii.operator_auth.require_mtls = true` (definido pelo proxy de entrada).
- Fluxo de cadastro:
  1. Chame registration options com um token bootstrap (permitido apenas antes do primeiro cadastro quando `token_fallback = "bootstrap"`).
  2. Execute `navigator.credentials.create` na UI do operador e envie a attestation para registration verify.
  3. Chame login options e login verify para obter `x-iroha-operator-session`.
  4. Envie `x-iroha-operator-session` nos endpoints de operador.

Notas
- Esses endpoints são visões locais do nó (em memória quando indicado) e não afetam o consenso nem a persistência.
- O acesso pode ser protegido por tokens de API, autenticação de operador (WebAuthn/mTLS) e limites de taxa conforme sua configuração do Torii.

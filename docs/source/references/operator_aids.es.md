---
lang: es
direction: ltr
source: docs/source/references/operator_aids.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf412f2645cea9d5468f4541ff48d4ace67bc6f3d60a97e68561dde4949ff9be
source_last_modified: "2025-12-13T10:25:50.323533+00:00"
translation_last_reviewed: 2026-01-01
---

# Endpoints de Torii — Ayudas para operadores (Referencia rápida)

Esta página enumera endpoints no consensuales, orientados a operadores, que ayudan con visibilidad y resolución de problemas. Las respuestas son JSON salvo que se indique.

Consenso (Sumeragi)
- Métricas: los gauges `sumeragi_new_view_receipts_by_hv{height,view}` reflejan los conteos.
- GET `/v1/sumeragi/status`
  - Instantánea del índice de líder, Highest/Locked QCs (`highest_qc`/`locked_qc`, alturas, vistas, hashes de sujeto), contadores de colectores/VRF, aplazamientos del pacemaker, profundidad de la cola de transacciones y salud del almacén RBC (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`).
- GET `/v1/sumeragi/status/sse`
  - Flujo SSE (≈1 s) del mismo payload que `/v1/sumeragi/status` para paneles en vivo.
- GET `/v1/sumeragi/qc`
  - Instantánea de highest/locked QCs; incluye `subject_block_hash` para el highest QC cuando se conoce.
- GET `/v1/sumeragi/pacemaker`
  - Temporizadores/configuración del pacemaker: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v1/sumeragi/leader`
  - Instantánea del índice del líder. En modo NPoS, incluye contexto PRF: `{ height, view, epoch_seed }`.
- GET `/v1/sumeragi/telemetry`
  - Aggregated consensus telemetry: `availability.collectors` contains observed collector indices, peer IDs, and ingested-vote counts; `rbc_backlog` contains missing-chunk totals; `rbc_pending` contains bounded pre-session queue totals, drops, and limits. This is not a deterministic collector plan or a per-session RBC contract.
- GET `/v1/sumeragi/params`
  - Instantánea de los parámetros de Sumeragi en cadena `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - When `da_enabled` is true, availability evidence is tracked but does not gate commit; local payload is required and can be satisfied via RBC `DELIVER` or block sync. Use the aggregated telemetry endpoint, Prometheus counters, status snapshots, and logs to diagnose payload transport.

Evidencia (auditoría; no consenso)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Incluye campos básicos (p. ej., DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) para inspección.
  - Ejemplos:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- POST `/v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - Ayudantes de CLI:
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (o `--evidence-hex-file <path>`)

Autenticación de operador (WebAuthn/mTLS)
- POST `/v1/operator/auth/registration/options`
  - Devuelve opciones de registro WebAuthn (`publicKey`) para la inscripción inicial de credenciales.
- POST `/v1/operator/auth/registration/verify`
  - Verifica el payload de attestation WebAuthn y persiste la credencial del operador.
- POST `/v1/operator/auth/login/options`
  - Devuelve opciones de autenticación WebAuthn (`publicKey`) para el inicio de sesión del operador.
- POST `/v1/operator/auth/login/verify`
  - Verifica el payload de assertion WebAuthn y devuelve un token de sesión del operador.
- Encabezados:
  - `x-iroha-operator-session`: token de sesión para endpoints de operador (emitido por login verify).
  - `x-iroha-operator-token`: token bootstrap (permitido cuando `torii.operator_auth.token_fallback` lo permite).
  - `x-api-token`: requerido cuando `torii.require_api_token = true` o `torii.operator_auth.token_source = "api"`.
  - `x-forwarded-client-cert`: requerido cuando `torii.operator_auth.require_mtls = true` (establecido por el proxy de ingreso).
- Flujo de inscripción:
  1. Llama a registration options con un token bootstrap (solo permitido antes de que se registre la primera credencial cuando `token_fallback = "bootstrap"`).
  2. Ejecuta `navigator.credentials.create` en la UI del operador y envía la attestation a registration verify.
  3. Llama a login options y login verify para obtener `x-iroha-operator-session`.
  4. Envía `x-iroha-operator-session` en los endpoints de operador.

Notas
- Estos endpoints son vistas locales del nodo (en memoria cuando se indica) y no afectan al consenso ni a la persistencia.
- El acceso puede estar protegido por tokens API, autenticación de operador (WebAuthn/mTLS) y límites de tasa según la configuración de Torii.

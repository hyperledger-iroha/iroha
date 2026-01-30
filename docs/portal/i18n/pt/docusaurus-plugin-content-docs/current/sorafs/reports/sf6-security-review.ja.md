---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/reports/sf6-security-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa087505ba58b14f4f0dd378fe78bff33f9c9e8d486d8b75ffd816638230a4f2
source_last_modified: "2025-11-14T04:43:22.277118+00:00"
translation_last_reviewed: 2026-01-30
---

# Revisao de seguranca SF-6

**Janela de avaliacao:** 2026-02-10 -> 2026-02-18  
**Lideres da revisao:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Escopo:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), proof streaming APIs, Torii manifest handling, integracao Sigstore/OIDC, CI release hooks.  
**Artefactos:**  
- Fonte do CLI e tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii manifest/proof handlers (`crates/iroha_torii/src/sorafs/api.rs`)  
- Release automation (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Deterministic parity harness (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA Parity Report](./orchestrator-ga-parity.md))

## Metodologia

1. **Threat modelling workshops** mapearam capacidades de ataque para workstations de developers, sistemas CI e nodes Torii.  
2. **Code review** focou em credential surfaces (OIDC token exchange, keyless signing), validacao de Norito manifests e back-pressure em proof streaming.  
3. **Dynamic testing** reexecutou fixture manifests e simulou failure modes (token replay, manifest tampering, proof streams truncados) usando parity harness e fuzz drives sob medida.  
4. **Configuration inspection** validou defaults `iroha_config`, CLI flag handling e release scripts para garantir runs deterministicas e auditaveis.  
5. **Process interview** confirmou remediation flow, escalation paths e captura de audit evidence com os release owners do Tooling WG.

## Findings Summary

| ID | Severity | Area | Finding | Resolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | High | Keyless signing | OIDC token audience defaults estavam implicitos em CI templates, com risco de cross-tenant replay. | Foi adicionada enforcement explicita de `--identity-token-audience` em release hooks e CI templates ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`). O CI agora falha quando a audience e omitida. |
| SF6-SR-02 | Medium | Proof streaming | Back-pressure paths aceitavam buffers de subscribers sem limite, permitindo memory exhaustion. | `sorafs_cli proof stream` aplica channel sizes limitados com truncation deterministico, registra Norito summaries e aborta o stream; o Torii mirror foi atualizado para limitar response chunks (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medium | Manifest submission | O CLI aceitava manifests sem verificar embedded chunk plans quando `--plan` estava ausente. | `sorafs_cli manifest submit` agora recomputa e compara CAR digests a menos que `--expect-plan-digest` seja fornecido, rejeitando mismatches e exibindo remediation hints. Tests cobrem casos de sucesso/falha (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Low | Audit trail | O release checklist nao tinha um signed approval log para a revisao de seguranca. | Foi adicionada uma secao em [release process](../developer-releases.md) exigindo anexar hashes do review memo e URL do ticket de sign-off antes de GA. |

Todos os achados high/medium foram corrigidos durante a janela de revisao e validados pelo parity harness existente. Nenhum issue critico latente permanece.

## Control Validation

- **Credential scope:** CI templates agora exigem audience e issuer explicitos; o CLI e o release helper falham rapido a menos que `--identity-token-audience` acompanhe `--identity-token-provider`.  
- **Deterministic replay:** Tests atualizados cobrem fluxos positivos/negativos de manifest submission, garantindo que mismatched digests continuem sendo falhas nao deterministicas e sejam expostas antes de tocar a rede.  
- **Proof streaming back-pressure:** Torii agora faz stream de itens PoR/PoTR em canais limitados, e o CLI reten apenas latency samples truncados + cinco failure exemplars, prevenindo crescimento sem limite e mantendo summaries deterministicas.  
- **Observability:** Proof streaming counters (`torii_sorafs_proof_stream_*`) e CLI summaries capturam abort reasons, fornecendo audit breadcrumbs aos operadores.  
- **Documentation:** Developer guides ([developer index](../developer-index.md), [CLI reference](../developer-cli.md)) destacam flags sensiveis a seguranca e escalation workflows.

## Release Checklist Additions

Release managers **devem** anexar a seguinte evidencia ao promover um GA candidate:

1. Hash do security review memo mais recente (este documento).  
2. Link para o remediation ticket rastreado (ex.: `governance/tickets/SF6-SR-2026.md`).  
3. Output de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` mostrando argumentos audience/issuer explicitos.  
4. Logs capturados do parity harness (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmacao de que as release notes do Torii incluem bounded proof streaming telemetry counters.

Nao coletar os artefactos acima bloqueia o sign-off de GA.

**Reference artefact hashes (sign-off 2026-02-20):**

- `sf6_security_review.md` - `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Outstanding Follow-ups

- **Threat model refresh:** Repita esta revisao trimestralmente ou antes de grandes adicoes de CLI flags.  
- **Fuzzing coverage:** Proof streaming transport encodings sao fuzzed via `fuzz/proof_stream_transport`, cobrindo payloads identity, gzip, deflate e zstd.  
- **Incident rehearsal:** Agende um exercicio de operadores simulando token compromise e manifest rollback, garantindo que a documentacao reflita procedimentos praticados.

## Approval

- Security Engineering Guild representative: @sec-eng (2026-02-20)  
- Tooling Working Group representative: @tooling-wg (2026-02-20)

Guarde approvals assinados junto ao release artefact bundle.

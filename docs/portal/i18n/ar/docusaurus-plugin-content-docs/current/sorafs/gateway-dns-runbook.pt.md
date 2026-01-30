---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Runbook de kickoff de Gateway e DNS da SoraFS

Esta copia do portal espelha o runbook canonico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Ela captura os guardrails operacionais do workstream de DNS descentralizado e Gateway
para que lideres de networking, ops e documentacao possam ensaiar a pilha de
automatizacao antes do kickoff 2025-03.

## Escopo e entregaveis

- Conectar os marcos de DNS (SF-4) e gateway (SF-5) ensaiando derivacao
  deterministica de hosts, releases de diretorio de resolvers, automatizacao TLS/GAR
  e captura de evidencias.
- Manter os insumos do kickoff (agenda, convite, tracker de presenca, snapshot de
  telemetria GAR) sincronizados com as ultimas atribuicoes de owners.
- Produzir um bundle de artefatos auditavel para revisores de governanca: notas de
  release do diretorio de resolvers, logs de probe do gateway, saida do harness de
  conformidade e o resumo de Docs/DevRel.

## Papeis e responsabilidades

| Workstream | Responsabilidades | Artefatos requeridos |
|------------|-------------------|----------------------|
| Networking TL (stack DNS) | Manter o plano deterministico de hosts, executar releases de diretorio RAD, publicar inputs de telemetria de resolvers. | `artifacts/soradns_directory/<ts>/`, diffs de `docs/source/soradns/deterministic_hosts.md`, metadata RAD. |
| Ops Automation Lead (gateway) | Executar drills de automatizacao TLS/ECH/GAR, rodar `sorafs-gateway-probe`, atualizar hooks do PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, probe JSON, entradas `ops/drill-log.md`. |
| QA Guild & Tooling WG | Rodar `ci/check_sorafs_gateway_conformance.sh`, curar fixtures, arquivar bundles de self-cert Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | Capturar minutos, atualizar o pre-read de design + apendices, e publicar o resumo de evidencias neste portal. | Arquivos `docs/source/sorafs_gateway_dns_design_*.md` atualizados e notas de rollout. |

## Entradas e pre-requisitos

- Especificacao de hosts deterministas (`docs/source/soradns/deterministic_hosts.md`) e
  o scaffolding de atestacao de resolvers (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefatos de gateway: manual do operador, helpers de automatizacao TLS/ECH,
  guidance de direct-mode e workflow de self-cert em `docs/source/sorafs_gateway_*`.
- Tooling: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, e helpers de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Segredos: chave de release GAR, credenciais ACME DNS/TLS, routing key do PagerDuty,
  token de auth do Torii para fetches de resolvers.

## Checklist pre-flight

1. Confirme participantes e agenda atualizando
   `docs/source/sorafs_gateway_dns_design_attendance.md` e circulando a agenda
   atual (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Prepare raizes de artefatos como
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` e
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Atualize fixtures (manifests GAR, provas RAD, bundles de conformidade do gateway) e
   garanta que o estado de `git submodule` esteja alinhado ao ultimo tag de ensaio.
4. Verifique segredos (chave de release Ed25519, arquivo de conta ACME, token do PagerDuty)
   e se batem com checksums do vault.
5. Faça smoke-test nos targets de telemetria (endpoint Pushgateway, board GAR Grafana)
   antes do drill.

## Etapas de ensaio de automatizacao

### Mapa deterministico de hosts e release do diretorio RAD

1. Rode o helper de derivacao deterministica de hosts contra o set de manifests
   proposto e confirme que nao ha drift em relacao a
   `docs/source/soradns/deterministic_hosts.md`.
2. Gere um bundle de diretorio de resolvers:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Registre o ID do diretorio, o SHA-256 e os caminhos de saida impressos dentro de
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` e nas minutos do kickoff.

### Captura de telemetria DNS

- Faça tail dos logs de transparencia de resolvers por ≥10 minutos usando
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporte metricas do Pushgateway e arquive os snapshots NDJSON ao lado do
  diretorio do run ID.

### Drills de automatizacao do gateway

1. Execute o probe TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Rode o harness de conformidade (`ci/check_sorafs_gateway_conformance.sh`) e
   o helper de self-cert (`scripts/sorafs_gateway_self_cert.sh`) para atualizar
   o bundle de atestacoes Norito.
3. Capture eventos de PagerDuty/Webhook para provar que o caminho de automatizacao
   funciona de ponta a ponta.

### Empacotamento de evidencias

- Atualize `ops/drill-log.md` com timestamps, participantes e hashes de probes.
- Armazene artefatos nos diretorios de run ID e publique um resumo executivo
  nas minutos do Docs/DevRel.
- Linke o bundle de evidencias no ticket de governanca antes da revisao do kickoff.

## Facilitacao de sessao e hand-off de evidencias

- **Linha do tempo do moderador:**
  - T-24 h — Program Management posta o lembrete + snapshot de agenda/presenca em `#nexus-steering`.
  - T-2 h — Networking TL atualiza o snapshot de telemetria GAR e registra os deltas em `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation verifica a prontidao de probes e escreve o run ID ativo em `artifacts/sorafs_gateway_dns/current`.
  - Durante a chamada — O moderador compartilha este runbook e designa um escriba ao vivo; Docs/DevRel capturam itens de acao inline.
- **Template de minutos:** Copie o esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (tambem espelhado no bundle
  do portal) e comite uma instancia preenchida por sessao. Inclua lista de
  participantes, decisoes, itens de acao, hashes de evidencias e riscos pendentes.
- **Upload de evidencias:** Zip o diretorio `runbook_bundle/` do ensaio,
  anexe o PDF de minutos renderizado, registre hashes SHA-256 nas minutos +
  agenda, e depois avise o alias de reviewers de governanca quando os uploads
  chegarem em `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Snapshot de evidencias (kickoff de marco 2025)

Os ultimos artefatos de ensaio/live referenciados no roadmap e nas minutos
ficam no bucket `s3://sora-governance/sorafs/gateway_dns/`. Os hashes abaixo
espelham o manifest canonico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Dry run — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball do bundle: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF das minutos: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Workshop ao vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Upload pendente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel anexara o SHA-256 quando o PDF renderizado chegar ao bundle.)_

## Material relacionado

- [Operations playbook do gateway](./operations-playbook.md)
- [Plano de observabilidade da SoraFS](./observability-plan.md)
- [Tracker de DNS descentralizado e gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)

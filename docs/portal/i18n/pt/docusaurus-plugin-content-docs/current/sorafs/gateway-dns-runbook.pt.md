---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de kickoff do Gateway e DNS da SoraFS

Esta cópia do portal espelha o runbook canonico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Ela captura os guardrails operacionais do fluxo de trabalho de DNS descentralizado e Gateway
para que líderes de networking, operações e documentação possam ensaiar a pilha de
automatização antes do pontapé inicial 2025-03.

## Escopo e entregas

- Conectar os marcos de DNS (SF-4) e gateway (SF-5) ensaiando derivação
  determinística de hosts, releases de diretório de resolvedores, automatização TLS/GAR
  e captura de evidências.
- Manter os insumos do kickoff (agenda, convite, tracker de presença, snapshot de
  telemetria GAR) sincronizados com as últimas atribuições de proprietários.
- Produzir um pacote de artistas auditáveis para revisores de governança: notas de
  release do diretório de resolvedores, logs de probe do gateway, saida do chicote de
  conformidade e resumo de Docs/DevRel.

## Papéis e responsabilidades

| Fluxo de trabalho | Responsabilidades | Artefatos necessários |
|------------|-------------------|----------------------|
| TL de rede (DNS de pilha) | Manter o plano determinístico de hosts, executar releases do diretório RAD, publicar entradas de telemetria de resolvedores. | `artifacts/soradns_directory/<ts>/`, diferenças de `docs/source/soradns/deterministic_hosts.md`, metadados RAD. |
| Líder de Automação de Operações (gateway) | Executar drills de automatização TLS/ECH/GAR, rodar `sorafs-gateway-probe`, atualizar hooks do PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, sonda JSON, entradas `ops/drill-log.md`. |
| Grupo de controle de qualidade e ferramentas | Rodar `ci/check_sorafs_gateway_conformance.sh`, curar fixtures, arquivar pacotes de auto-certificação Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | Capturar minutos, atualizar ou pré-ler o design + apêndices, e publicar o resumo de evidências neste portal. | Arquivos `docs/source/sorafs_gateway_dns_design_*.md` atualizados e notas de implementação. |

## Entradas e pré-requisitos

- Especificação de hosts deterministas (`docs/source/soradns/deterministic_hosts.md`) e
  o andaime de atestação de resolvedores (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefatos de gateway: manual do operador, ajudantes de automatização TLS/ECH,
  orientação de modo direto e fluxo de trabalho de autocertificação em `docs/source/sorafs_gateway_*`.
- Ferramental: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, e ajudantes de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Segredos: chave de liberação GAR, credenciais ACME DNS/TLS, chave de roteamento do PagerDuty,
  token de autenticação do Torii para buscar resolvedores.

## Checklist pré-voo

1. Confirme participantes e agenda atualizando
   `docs/source/sorafs_gateway_dns_design_attendance.md` e circulando pela agenda
   atual (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Prepare raízes de artefactos como
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` e
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Atualizar fixtures (manifestos GAR, provas RAD, pacotes de conformidade do gateway) e
   certifique-se de que o estado de `git submodule` esteja alinhado ao último tag de ensaio.
4. Verifique segredos (chave de lançamento Ed25519, arquivo de conta ACME, token do PagerDuty)
   e se batem com checksums do vault.
5. Faça smoke-test nos alvos de telemetria (endpoint Pushgateway, placa GAR Grafana)
   antes de perfurar.## Etapas de ensaio de automatização

### Mapa determinístico de hosts e release do diretorio RAD

1. Rode o auxiliar de derivação determinística de hosts contra o conjunto de manifestos
   Proponho e confirmo que não há deriva em relação a
   `docs/source/soradns/deterministic_hosts.md`.
2. Gere um pacote de diretório de resolvedores:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Cadastre o ID do diretório, o SHA-256 e os caminhos de saída impressos dentro de
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` e nos minutos do pontapé inicial.

### Captura de telemetria DNS

- Faça os logs de transparência dos resolvedores por ≥10 minutos usando
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporte métricas do Pushgateway e arquive os snapshots NDJSON ao lado do
  diretório do run ID.

### Drills de automatização do gateway

1. Execute o teste TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Rode o chicote de conformidade (`ci/check_sorafs_gateway_conformance.sh`) e
   o auxiliar de autocertificação (`scripts/sorafs_gateway_self_cert.sh`) para atualização
   ou pacote de atestados Norito.
3. Capture eventos do PagerDuty/Webhook para provar que o caminho de automatização
   funciona de ponta a ponta.

### Empacotamento de evidências

- Atualizar `ops/drill-log.md` com timestamps, participantes e hashes de probes.
- Armazene artistas nos diretórios de run ID e publique um resumo executivo
  nos minutos do Docs/DevRel.
- Linke o pacote de evidências no ticket de governança antes da revisão do kickoff.

## Facilitação de sessão e entrega de evidências

- **Linha do tempo do moderador:**
  - T-24 h — Gestão do Programa posta o lembrete + snapshot de agenda/presença em `#nexus-steering`.
  - T-2 h — Networking TL atualiza o snapshot de telemetria GAR e registra os deltas em `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation verifica a prontidão de probes e escreve o run ID ativo em `artifacts/sorafs_gateway_dns/current`.
  - Durante a chamada — O moderador compartilha este runbook e designa um escrito ao vivo; Docs/DevRel capturam itens de ação inline.
- **Modelo de minutos:** Copiar o esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (tambem espelhado no bundle
  do portal) e comite uma instância preenchida por sessão. Inclui lista de
  participantes, decisões, itens de ação, hashes de evidências e riscos pendentes.
- **Upload de evidências:** CEP do diretório `runbook_bundle/` do ensaio,
  anexo o PDF de minutos renderizado, registre hashes SHA-256 nos minutos +
  agenda, e depois avisar o alias de revisores de governança quando os uploads
  chegarem em `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Instantâneo de evidências (início de março de 2025)

Os últimos artefactos de ensaio/live referenciados no roadmap e nos minutos
fique no balde `s3://sora-governance/sorafs/gateway_dns/`. Os hashes abaixo
espelham o manifest canonico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Teste — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball do pacote: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF dos minutos: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Workshop ao vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Upload pendente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel fixaçãoa o SHA-256 quando o PDF renderizado chegar ao pacote.)_

## Material relacionado- [Manual de operações do gateway](./operations-playbook.md)
- [Plano de observabilidade da SoraFS](./observability-plan.md)
- [Rastreador de DNS descentralizado e gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
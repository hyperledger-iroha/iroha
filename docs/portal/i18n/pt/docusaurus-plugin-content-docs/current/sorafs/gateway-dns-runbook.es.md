---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de lançamento do Gateway e DNS de SoraFS

Esta cópia do portal reflete o runbook canônico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Reconhecer as salvaguardas operacionais do fluxo de trabalho de DNS e Gateway descentralizado
para que leads de rede, operações e documentação possam ensaiar a pilha de
automação antes do início do jogo 2025-03.

## Alcance e entregas

- Vincular os sucessos de DNS (SF-4) e gateway (SF-5) testando a derivação
  determinista de hosts, lançamentos de diretório de resolvedores, automatização TLS/GAR
  e captura de evidências.
- Manter os insumos do kickoff (agenda, convite, rastreador de assistência, instantâneo
  de telemetria GAR) sincronizados com as últimas atribuições de proprietários.
- Produzir um pacote de artefatos auditáveis para revisores de governança: notas de
  liberação do diretório de resolvedores, logs de sondas do gateway, saída do chicote
  de conformidade e o resumo do Docs/DevRel.

## Funções e responsabilidades

| Fluxo de trabalho | Responsabilidades | Artefatos necessários |
|------------|-------------------|------------|
| TL de rede (DNS de pilha) | Manter o plano determinista de hosts, executar liberações do diretório RAD, publicar entradas de telemetria de resolvedores. | `artifacts/soradns_directory/<ts>/`, diferenças de `docs/source/soradns/deterministic_hosts.md`, metadados RAD. |
| Líder de Automação de Operações (gateway) | Executar exercícios de automação TLS/ECH/GAR, executar `sorafs-gateway-probe`, atualizar ganchos de PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON do probe, entradas em `ops/drill-log.md`. |
| Grupo de controle de qualidade e ferramentas | Executar `ci/check_sorafs_gateway_conformance.sh`, curar luminárias, arquivar pacotes de auto-certificação Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | Registrar minutas, atualizar a pré-leitura do projeto + apêndices e publicar o resumo das evidências neste portal. | Arquivos atualizados `docs/source/sorafs_gateway_dns_design_*.md` e notas de implementação. |

## Entradas e pré-requisitos

- Especificação de hosts deterministas (`docs/source/soradns/deterministic_hosts.md`) e
  o pedido de atestado de resolvedores (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefatos do gateway: manual do operador, ajudantes de automatização TLS/ECH,
  guia de modo direto e fluxo de autocertificação abaixo de `docs/source/sorafs_gateway_*`.
- Ferramental: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, e ajudantes de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Segredos: chave de liberação GAR, credenciais ACME DNS/TLS, chave de roteamento do PagerDuty,
  token de autenticação de Torii para obter resolvedores.

## Checklist pré-vuelo1. Confirme os assistentes e atualize a agenda
   `docs/source/sorafs_gateway_dns_design_attendance.md` e circulando a agenda
   vigente (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Prepare receitas de artefatos como
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` e
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Dispositivos Refresca (manifestos GAR, testes RAD, pacotes de conformidade do gateway) e
   Certifique-se de que o estado de `git submodule` coincida com o último tag de ensaio.
4. Verifique os segredos (clave de lançamento Ed25519, arquivo de conta ACME, token de PagerDuty)
   e que coincide com as somas de verificação do cofre.
5. Teste de fumaça perigoso dos alvos de telemetria (endpoint do Pushgateway, tabela GAR Grafana)
   antes da broca.

## Passos de ensaio de automação

### Mapa determinista de hosts e lançamento do diretório RAD

1. Execute o auxiliar de derivação determinista de hosts contra o conjunto de manifestos
   propuesto e confirma que não haya drift a respeito de
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

3. Registre o ID do diretório, o SHA-256 e as rotas de saída impressas dentro do
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` e nos minutos iniciais.

### Captura de DNS de telemetria

- Perigou os logs de transparência dos resolvedores durante ≥10 minutos usando
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporte métricas de Pushgateway e arquive os snapshots NDJSON junto com
  diretório do ID de execução.

### Exercícios de automação do gateway

1. Execute a sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Execute o chicote de conformidade (`ci/check_sorafs_gateway_conformance.sh`) e
   o auxiliar de autocertificação (`scripts/sorafs_gateway_self_cert.sh`) para atualizar
   o pacote de atestados Norito.
3. Capturar eventos do PagerDuty/Webhook para demonstrar a automatização
   funciona de extremo a extremo.

### Empaquetado de evidências

- Atualize `ops/drill-log.md` com carimbos de data/hora, participantes e hashes de testes.
- Guarda os artefatos abaixo dos diretórios de ID de execução e publica um currículo executivo
  nos minutos do Docs/DevRel.
- Coloque o pacote de evidências no bilhete de governo antes da revisão
  do pontapé inicial.

## Facilitação de sessão e entrega de evidências- **Linha de tempo do moderador:**
  - T-24 h — Gestão de Programa publica el recordatorio + snapshot de agenda/asistencia en `#nexus-steering`.
  - T-2 h — Networking TL atualiza o snapshot de telemetria GAR e registra os deltas em `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation verifica a preparação das sondas e escreve o ID de execução ativo em `artifacts/sorafs_gateway_dns/current`.
  - Durante a chamada — O moderador compara este runbook e atribui um registro ao vivo; Docs/DevRel captura itens de ação online.
- **Plantilla de minutas:** Copiar o esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (também espiado no pacote
  del portal) e comite uma instância completada por sessão. Inclui lista de
  assistentes, decisões, itens de ação, hashes de evidência e riscos pendentes.
- **Carga de evidências:** Comprima o diretório `runbook_bundle/` do ensaio,
  adicione o PDF de minutos renderizado, registre hashes SHA-256 nos minutos +
  agenda e depois notificar al alias de revisores de governo quando as cargas
  aterrissado em `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Instantâneo de evidências (início de março de 2025)

Os últimos artefatos de ensaio/produção referenciados no roteiro e nas minutas
viven no balde `s3://sora-governance/sorafs/gateway_dns/`. Os hashes abaixo
reflita sobre o manifesto canônico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Teste — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball do pacote: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF de minutos: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Workshop ao vivo — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Carga pendente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel prende o SHA-256 quando o PDF renderizado sai do pacote.)_

## Material relacionado

- [Manual de operações do gateway](./operations-playbook.md)
- [Plano de observação de SoraFS](./observability-plan.md)
- [Rastreador de DNS descentralizado e gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
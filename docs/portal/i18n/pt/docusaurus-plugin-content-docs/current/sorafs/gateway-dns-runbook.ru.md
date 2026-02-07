---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбук запуска Gateway e DNS SoraFS

Esta cópia no portal foi cancelada em um banco de dados
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Para proteger a operação de guardrails para o desenvolvimento de DNS e gateway descentralizados,
dicas sobre redes, operações e documentação podem ser exploradas
автоматизации перед pontapé inicial 2025-03.

## Entregas e entregas

- Alterar seus DNS (SF-4) e gateway (SF-5) para determinar a repetição
  conjuntos de derivativos, resolvedores de catálogo de pacotes, TLS/GAR automatizados e
  сбора доказательств.
- Держать kickoff-артефакты (agenda, convite, rastreador de presença, instantâneo
  телеметрии GAR) синхронизированными с последними назначениями proprietários.
- Подготовить аудируемый bundle артефактов для revisores de governança: notas de lançamento
  resolvedores de catálogo, sondas de gateway de log, chicote de conformidade de segurança e soluções
  Documentos/DevRel.

## Роли e ответственности

| Fluxo de trabalho | Exibição | Artefactos de trabalho |
|------------|-----------------|---------------------|
| TL de rede (pilha DNS) | Você pode determinar o plano de instalação, liberar versões de diretório RAD, publicar seus resolvedores de telemetria. | `artifacts/soradns_directory/<ts>/`, diferenças para `docs/source/soradns/deterministic_hosts.md`, metadados RAD. |
| Líder de Automação de Operações (gateway) | Выполнять brocas автоматизации TLS/ECH/GAR, запускать `sorafs-gateway-probe`, обновлять ganchos PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, teste JSON, escrito em `ops/drill-log.md`. |
| Grupo de controle de qualidade e ferramentas | Запускать `ci/check_sorafs_gateway_conformance.sh`, курировать fixtures, архивировать Norito pacotes de autocertificação. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | Фиксировать minutos, обновлять design pré-leitura + apêndices, публиковать resumo de evidências no portal. | Revisão `docs/source/sorafs_gateway_dns_design_*.md` e notas de implementação. |

## Requisitos e pré-requisitos

- Especificação de dados de localização (`docs/source/soradns/deterministic_hosts.md`) e
  atestado de cartão para resolvedores (`docs/source/soradns/resolver_attestation_directory.md`).
- Gateway Артефакты: manual do operador, auxiliares de automação TLS/ECH,
  orientação sobre modo direto e fluxo de trabalho de autocertificação em `docs/source/sorafs_gateway_*`.
- Ferramental: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` e ajudantes de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Segredos: ключ релиза GAR, credenciais DNS/TLS ACME, chave de roteamento PagerDuty,
  Token de autenticação Torii para buscar resolvedores.

## Lista de verificação pré-voo

1. Organize suas tarefas e agenda, обновив
   `docs/source/sorafs_gateway_dns_design_attendance.md` e tecnologia de resolução
   ordem do dia (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Use artefactos de cor, por exemplo
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` e
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Atualizar fixtures (manifestos GAR, provas RAD, gateway de conformidade de pacotes) e
   убедитесь, что состояние `git submodule` соответствует последнему etiqueta de ensaio.
4. Prove segredos (chave de liberação Ed25519, arquivo de conta ACME, token PagerDuty) e
   adicionar somas de verificação no vault.
5. Prover alvos de telemetria de teste de fumaça (endpoint Pushgateway, placa GAR Grafana)
   primeiro broca.

## Шаги репетиции автоматизации

### Determinar o mapa e liberar o catálogo RAD1. Clique no ajudante para determinar a derivação do local de trabalho
   manifesta-se e pode evitar desvios
   `docs/source/soradns/deterministic_hosts.md`.
2. Gerencie resolvedores de catálogo de pacotes:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Registre seu catálogo de ID, SHA-256 e insira seu nome
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` e em minutos de início.

### Abrir a rede DNS

- Logs de transparência do resolvedor de cauda em течение ≥10 minutos por semana
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exiba métricas do Pushgateway e forneça snapshots NDJSON remotamente
  ID de execução do diretório.

### Brocas автоматизации gateway

1. Verifique a sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Instale o chicote de conformidade (`ci/check_sorafs_gateway_conformance.sh`) e
   auxiliar de autocertificação (`scripts/sorafs_gateway_self_cert.sh`) para atualização
   Pacote de atestado Norito.
3. Verifique a segurança do PagerDuty/Webhook para que você possa trabalhar de ponta a ponta
   пути автоматизации.

### Упаковка доказательств

- Verifique `ops/drill-log.md` com carimbos de data e hora, testes de verificação e hashes.
- Сохраните артефакты директориях run ID e опубликуйте resumo executivo
  em minutos Docs/DevRel.
- Solicite um pacote de evidências para a governança para a revisão inicial.

## Moderação de sessão e transferência de documentos

- **Moderador da linha do tempo:**
  - T-24 h — Gerenciamento de programa публикует напоминание + instantâneo agenda/presença em `#nexus-steering`.
  - T-2 h — Networking TL obtém instantâneos de telemetria GAR e фиксирует deltas em `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation fornece testes de teste e identifica o ID de execução ativo em `artifacts/sorafs_gateway_dns/current`.
  - Во время звонка — Модератор делится этим ранбуком и назначает escriba ao vivo; Docs/DevRel фиксируют itens de ação para o caso.
- **Minutos extras:** Скопируйте скелет из
  `docs/source/sorafs_gateway_dns_design_minutes.md` (também disponível no pacote do portal)
  e comprometa-se a configurar o exemplo na sessão anterior. Включите список участников,
  решения, itens de ação, evidências de hashes e riscos de descoberta.
- **Загрузка доказательств:** Заархивируйте `runbook_bundle/` do ensaio,
  usar minutos em PDF, inserir hashes SHA-256 em minutos + agenda,
  затем уведомите alias de revisor de governança после загрузки в
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Снимок доказательств (início em março de 2025)

Possibilidade de ensaio/artefatos ao vivo, definição de roteiro e minutos, permanência no balde
`s3://sora-governance/sorafs/gateway_dns/`. Não há nada canônico
manifesto (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Teste — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Pacote Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Minutos PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Workshop ao vivo — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Upload pendente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel добавит SHA-256 после попадания PDF no pacote.)_

## Materiais úteis

- [Manual de operações para gateway](./operations-playbook.md)
- [SoraFS](./observability-plan.md)
- [Gerenciador de DNS e gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
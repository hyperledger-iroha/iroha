<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: migration-ledger
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> Adaptado de [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Razao de migracao SoraFS

Este razao espelha o changelog de migracao capturado no RFC de Arquitetura SoraFS.
As entradas sao agrupadas por marco e listam a janela efetiva, os times impactados e
as acoes requeridas. Atualizacoes no plano de migracao DEVEM modificar esta pagina e o RFC
(`docs/source/sorafs_architecture_rfc.md`) para manter os consumidores downstream alinhados.

| Marco | Janela efetiva | Resumo da mudanca | Times impactados | Itens de acao | Status |
|------|----------------|------------------|-----------------|--------------|--------|
| M0 | Semanas 1-6 | Fixtures de chunker publicados; pipelines emitem bundles CAR + manifest junto com artefatos legados; entradas do razao de migracao criadas. | Docs, DevRel, SDKs | Adotar `sorafs_manifest_stub` com expectation flags, registrar entradas neste razao, manter o CDN legado. | Ativo |
| M1 | Semanas 7-12 | CI impoe fixtures deterministicas; provas de alias disponiveis em staging; tooling expoe expectation flags explicitas. | Docs, Storage, Governance | Garantir que fixtures continuem assinados, registrar aliases no registry de staging, atualizar checklists de release com exigencia `--car-digest/--root-cid`. | Pendente |
| M2 | Semanas 13-20 | Pinning com registry vira caminho principal; artefatos legados viram somente leitura; gateways priorizam provas de registry. | Storage, Ops, Governance | Roteie pinning via registry, congele hosts legados, publique avisos de migracao para operadores. | Pendente |
| M3 | Semana 21+ | Acesso somente por alias imposto; observabilidade alerta sobre paridade do registry; CDN legado desativado. | Ops, Networking, SDKs | Remova DNS legado, rotacione URLs em cache, monitore dashboards de paridade, atualize defaults do SDK. | Pendente |
| R0-R3 | 2025-03-31 -> 2025-07-01 | Fases de enforcement de provider advert: R0 observar, R1 alertar, R2 impor handles/capabilities canonicos, R3 limpar payloads legados. | Observability, Ops, SDKs, DevRel | Importar `grafana_sorafs_admission.json`, seguir o checklist do operador em `provider_advert_rollout.md`, agendar renovacoes de advert 30+ dias antes do gate R2. | Pendente |

As atas do plano de controle de governanca que referenciam esses marcos vivem em
`docs/source/sorafs/`. Os times devem adicionar bullets com data abaixo de cada linha
quando eventos notaveis ocorrerem (por exemplo, novos registros de alias, retrospectivas
sobre incidentes de registry) para fornecer um trilho de auditoria.

## Atualizacoes recentes

- 2025-11-01 - Circulou `migration_roadmap.md` para o conselho de governanca e listas de
  operadores para revisao; aguardando aprovacao na proxima sessao do conselho
  (ref: `docs/source/sorafs/council_minutes_2025-10-29.md` follow-up).
- 2025-11-02 - O ISI de registro do Pin Registry agora aplica validacao compartilhada
  de chunker/politica via helpers `sorafs_manifest`, mantendo os caminhos on-chain
  alinhados com os checks de Torii.
- 2026-02-13 - Adicionadas fases de rollout de provider advert (R0-R3) ao razao e
  publicados os dashboards e a orientacao de operadores associada
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).

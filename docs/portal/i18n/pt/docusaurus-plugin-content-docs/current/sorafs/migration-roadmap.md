<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> Adaptado de [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Roadmap de migracao SoraFS (SF-1)

Este documento operacionaliza a orientacao de migracao capturada em
`docs/source/sorafs_architecture_rfc.md`. Ele expande os entregaveis SF-1 em
marcos prontos para execucao, criterios de gate e checklists de donos para que
as equipes de storage, governanca, DevRel e SDK coordenem a transicao do hosting
de artefatos legados para publicacao apoiada por SoraFS.

O roadmap e intencionalmente deterministico: cada marco nomeia os artefatos
necessarios, as invocacoes de comando e os passos de atestacao para que os
pipelines downstream produzam outputs identicos e a governanca mantenha uma
trilha auditavel.

## Visao geral dos marcos

| Marco | Janela | Objetivos principais | Deve entregar | Donos |
|-------|--------|----------------------|---------------|-------|
| **M0 - Bootstrap** | Semanas 1-6 | Publicar fixtures deterministicas do chunker e fazer publicacao dupla (legado + SoraFS). | Fixtures `sorafs_chunker`, integracao CLI `sorafs_manifest_stub`, entradas do ledger de migracao. | Docs, DevRel, Storage |
| **M1 - Enforcement deterministico** | Semanas 7-12 | Exigir fixtures assinadas e preparar provas de alias enquanto os pipelines adotam expectation flags. | Verificacao noturna de fixtures, manifests assinados pelo conselho, entradas de staging no registry de alias. | Storage, Governance, SDKs |
| **M2 - Registry primeiro** | Semanas 13-20 | Roteiar pins pelo registry, congelar bundles legados e expor telemetria de paridade. | Contrato Pin Registry + CLI (`sorafs pin propose/approve`), dashboards de observabilidade, runbooks de operadores. | Governance, Ops, Observability |
| **M3 - Somente alias** | Semana 21+ | Desativar o hosting legado e exigir provas de alias para recuperacao. | Gateways alias-only, alertas de paridade, defaults do SDK atualizados, aviso de desligamento legacy. | Ops, Networking, SDKs |

O status dos marcos e acompanhado em `docs/source/sorafs/migration_ledger.md`. Todas
as mudancas neste roadmap DEVEM atualizar o ledger para manter governanca e release
engineering em sincronia.

## Frentes de trabalho

### 1. Reempacotamento de dados legados

| Passo | Marco | Descricao | Dono(s) | Saida |
|-------|-------|-----------|---------|-------|
| Inventario e tagging | M0 | Exportar digests SHA3-256 de bundles legados e registrar no ledger de migracao (append-only). | Docs, DevRel | Entradas do ledger com `source_path`, `sha3_digest`, `owner`, `planned_manifest_cid`. |
| Reconstrucao deterministica | M0-M1 | Invocar `sorafs_manifest_stub` para cada artefato de release e persistir CAR, manifest, envelope de assinatura e plano de fetch em `artifacts/<team>/<alias>/<timestamp>/`. | Docs, CI | Bundles CAR + manifest reproduziveis por release. |
| Loop de validacao | M1 | Reexecutar `sorafs_fetch` contra gateways de staging para confirmar que limites/digests de chunk batem com fixtures. Registrar pass/fail nos comentarios do ledger. | Governance QA | Relatorio de verificacao de staging + issue GitHub por drift. |
| Corte de registry | M2 | Trocar status do ledger para `Pinned` quando o digest do manifest existir on-chain; bundle legado vira read-only (servir mas nao modificar). | Governance, Ops | Hash da transacao no registry, ticket read-only para storage legado. |
| Decommission | M3 | Remover entradas do CDN legado apos periodo de graca de 30 dias, arquivar aprovacoes de DNS e publicar post-mortem. | Ops | Checklist de decommission, registro de mudanca DNS, encerramento de ticket de incidente. |

### 2. Adocao de pinning deterministico

| Passo | Marco | Descricao | Dono(s) | Saida |
|-------|-------|-----------|---------|-------|
| Ensaios de fixtures | M0 | Dry-runs semanais comparando digests locais de chunk com `fixtures/sorafs_chunker`. Publicar relatorio em `docs/source/sorafs/reports/`. | Storage Providers | `determinism-<date>.md` com matriz de pass/fail. |
| Exigir assinaturas | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` falham se assinaturas ou manifests derivarem. Overrides de dev exigem waiver de governanca anexado ao PR. | Tooling WG | Log de CI, link do ticket de waiver (se aplicavel). |
| Expectation flags | M1 | Pipelines chamam `sorafs_manifest_stub` com expectations explicitas para fixar outputs: | Docs CI | Scripts atualizados referenciando expectation flags (ver bloco de comando abaixo). |
| Pinning registry-first | M2 | `sorafs pin propose` e `sorafs pin approve` envolvem envios de manifest; CLI padrao usa `--require-registry`. | Governance Ops | Log de auditoria do CLI do registry, telemetria de propostas falhadas. |
| Paridade de observabilidade | M3 | Dashboards Prometheus/Grafana alertam quando inventarios de chunks divergem dos manifests do registry; alertas ligados ao on-call de ops. | Observability | Link do dashboard, IDs de regra de alerta, resultados de GameDay. |

#### Comando canonico de publicacao

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Substitua os valores de digest, tamanho e CID pelas referencias esperadas
registradas na entrada do ledger de migracao para o artefato.

### 3. Transicao de alias e comunicacoes

| Passo | Marco | Descricao | Dono(s) | Saida |
|-------|-------|-----------|---------|-------|
| Provas de alias em staging | M1 | Registrar alias claims no Pin Registry de staging e anexar provas Merkle aos manifests (`--alias`). | Governance, Docs | Bundle de provas armazenado ao lado do manifest + comentario no ledger com o nome do alias. |
| DNS duplo + notificacao | M1-M2 | Operar DNS legado e Torii/SoraDNS em paralelo; publicar avisos de migracao para operadores e canais de SDK. | Networking, DevRel | Post de anuncio + ticket de mudanca DNS. |
| Enforcement de provas | M2 | Gateways rejeitam manifests sem headers `Sora-Proof` recentes; CI ganha etapa `sorafs alias verify` para buscar provas. | Networking | Patch de config do gateway + output de CI capturando verificacao bem-sucedida. |
| Rollout alias-only | M3 | Remover DNS legado, atualizar defaults do SDK para usar Torii/SoraDNS + provas de alias, documentar janela de rollback. | SDK Maintainers, Ops | Notas de release do SDK, atualizacao de runbook de ops, plano de rollback. |

### 4. Comunicacao e auditoria

- **Disciplina do ledger:** cada mudanca de estado (drift de fixtures, envio ao registry,
  ativacao de alias) deve anexar uma nota datada em
  `docs/source/sorafs/migration_ledger.md`.
- **Minutas de governanca:** sessoes do conselho aprovando mudancas no pin registry ou
  politicas de alias devem referenciar este roadmap e o ledger.
- **Comunicacao externa:** DevRel publica atualizacoes de status em cada marco (blog +
  trecho de changelog) destacando garantias deterministicas e cronogramas de alias.

## Dependencias e riscos

| Dependencia | Impacto | Mitigacao |
|-------------|---------|-----------|
| Disponibilidade do contrato Pin Registry | Bloqueia o rollout M2 pin-first. | Preparar o contrato antes de M2 com testes de replay; manter fallback de envelope ate ficar sem regressao. |
| Chaves de assinatura do conselho | Necessarias para envelopes de manifest e aprovacoes do registry. | Cerimonia de assinatura documentada em `docs/source/sorafs/signing_ceremony.md`; rotacionar chaves com sobreposicao e nota no ledger. |
| Tooling de paridade do gateway | Necessario para enforcement de provas de alias e paridade de chunks. | Entregar updates do gateway em M1, manter comportamento legado atras de feature flag ate criterios M2. |
| Cadencia de release do SDK | Clientes devem respeitar provas de alias antes do M3. | Alinhar janelas de release do SDK com gates dos marcos; adicionar checklists de migracao nos templates de release. |

Riscos residuais e mitigacoes estao espelhados em `docs/source/sorafs_architecture_rfc.md`
e devem ser cruzados quando ajustes forem feitos.

## Checklist de criterios de saida

| Marco | Criterios |
|-------|-----------|
| M0 | - Todos os artefatos alvo reconstruidos via `sorafs_manifest_stub` com expectation flags. <br /> - Ledger de migracao preenchido para cada familia de artefatos. <br /> - Publicacao dupla (legado + SoraFS) ativa. |
| M1 | - Job noturno de fixtures verde por sete dias consecutivos. <br /> - Provas de alias de staging verificadas no CI. <br /> - Governanca ratifica a politica de expectation flags. |
| M2 | - 100% dos novos manifests roteados pelo Pin Registry. <br /> - Storage legado marcado como read-only; playbook de incidente aprovado. <br /> - Dashboards de observabilidade online com thresholds de alerta. |
| M3 | - Gateways alias-only em producao. <br /> - DNS legado removido e refletido em tickets de mudanca. <br /> - Defaults do SDK atualizados e liberados. <br /> - Status final anexado ao ledger de migracao. |

## Gestao de mudancas

1. Propor ajustes via PR atualizando este arquivo **e**
   `docs/source/sorafs/migration_ledger.md`.
2. Vincular minutas de governanca e evidencias de CI na descricao do PR.
3. No merge, notificar a lista de storage + DevRel com resumo e acoes esperadas
   para operadores.

Seguir este procedimento garante que o rollout SoraFS permanece deterministico,
auditable e transparente entre as equipes que participam do lancamento Nexus.

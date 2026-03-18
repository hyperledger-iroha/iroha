---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Folheto de rota de migração SoraFS"
---

> Adaptado de [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Folha de rota de migração SoraFS (SF-1)

Este documento operacionaliza as diretivas de migração capturadas em
`docs/source/sorafs_architecture_rfc.md`. Desenvolver os livrables SF-1 en
responsabilidades prets um executor, critérios de passagem e listas de verificação de responsabilidades afin
que as equipes de armazenamento, governança, DevRel e SDK coordenam a transição do

A folha de rota é voluntariamente determinada: cada jalon nomme les
artefatos necessários, as invocações de comandos e as etapas de atestado para
que os pipelines a jusante produzem saídas idênticas e que la
governança conserva um traço auditável.

## Vista do conjunto de jalons

| Jalão | Fenetre | Objectivos principais | Doit livrer | Proprietários |
|---|--------|-----------|-------------|--------|
| **M1 - Determinação da execução** | Semanas 7-12 | Exigir luminárias assinadas e preparar as preferências do alias para que os pipelines adotem os sinalizadores de expectativa. | Verificação noturna de luminárias, manifestos sinais par le conseil, entradas encenação du registro d'alias. | Armazenamento, governança, SDKs |

O status dos jalons é suivi em `docs/source/sorafs/migration_ledger.md`. Tudo
as modificações desta folha de rota DOIVENT mettre a jour le registre afin
que governança e engenharia de liberação permanecem sincronizadas.

## Pistas de trabalho

### 2. Adoção da fixação determinada

| Étape | Jalão | Descrição | Proprietário(s) | Sorteio |
|-------|-------|-------------|----------|--------|
| Repetições de jogos | M0 | Hebdomadaires comparam resumos locais de pedaços com `fixtures/sorafs_chunker`. Publique um relatório sob `docs/source/sorafs/reports/`. | Provedores de armazenamento | `determinism-<date>.md` com matriz aprovada/reprovada. |
| Exigir assinaturas | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` ecoam assinaturas ou manifestos derivados. As substituições dev exigem um adido de governança de renúncia ao PR. | GT Ferramentaria | Log CI, garantia versus ticket de waiver (se aplicável). |
| Sinalizadores de expectativa | M1 | Os pipelines recorrentes `sorafs_manifest_stub` com expectativas explícitas para definir as saídas: | Documentos CI | Os scripts agora se referem aos sinalizadores de expectativa (veja o bloco de comando ci-dessous). |
| Fixando o registro primeiro | M2 | `sorafs pin propose` e `sorafs pin approve` envolvem as mensagens do manifesto; A CLI por padrão utiliza `--require-registry`. | Operações de Governança | Registro de auditoria do registro CLI, taxas de telemetria de propostas. |
| Paridade observabilidade | M3 | Os painéis Prometheus/Grafana alertam quando os inventários de pedaços divergentes do registro de manifestos; alertes branchees sur l'astreinte ops. | Observabilidade | Painel de garantia, IDs das regras de alerta, resultados do GameDay. |

#### Comando canônico de publicação

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

Substitua os valores de resumo, cauda e CID pelas referências presentes
recenseados na entrada do registro de migração para o artefato.

### 3. Transição de alias e comunicações| Étape | Jalão | Descrição | Proprietário(s) | Sorteio |
|-------|-------|-------------|----------|--------|
| Testes de apelidos e encenação | M1 | Registre as reivindicações de alias no teste Pin Registry e anexe os preuves Merkle aux manifests (`--alias`). | Governança, Documentos | Pacote de testes armazenado na parte do manifesto + comentário do registro com o nome de pseudônimo. |
| Aplicação das medidas preventivas | M2 | Os gateways rejeitam os manifestos sem cabeçalhos `Sora-Proof` recentes; Adicione a fita `sorafs alias verify` para recuperar os danos. | Rede | Patch de configuração do gateway + sortie CI capturando a verificação repetida. |

### 4. Comunicação e auditoria

- **Discipline du registre:** cada mudança de estado (drift de fixtures, registro de soumission,
  ativação do alias) adicione uma nota datada em
  `docs/source/sorafs/migration_ledger.md`.
- **Ata de governo:** as sessões de aconselhamento aprovam as alterações no registro do PIN ou
  les politiques d'alias devem referenciar esta folha de rota e o registro.
- **Comunicações externas:** DevRel publie des mises a jour a chaque jalon (blog + extrait de changelog)
  definimos antes as garantias determinísticas e os calendários de alias.

## Dependências e riscos

| Dependência | Impacto | Mitigação |
|------------|--------|-----------|
| Disponibilidade do contrato Pin Registry | Bloqueie o lançamento do pino M2 primeiro. | Prepare o contrato anterior ao M2 com testes de repetição; manter um envelope substituto apenas estabilizado. |
| Clés de assinatura do conselho | Requisitos para os envelopes de manifesto e registro de aprovação. | Cerimônia de assinatura documentada em `docs/source/sorafs/signing_ceremony.md`; rotação com chevauchement e nota no registro. |
| Cadência de lançamento do SDK | Os clientes devem honrar as preferências do alias anterior ao M3. | Alinhe as janelas de lançamento do SDK nas portas dos bloqueios; adiciona listas de verificação de migração a modelos de lançamento. |

Riscos residuais e mitigações são representados em `docs/source/sorafs_architecture_rfc.md`
e deve ser recuperado durante os ajustes.

## Checklist dos critérios de surtida

| Jalão | Critérios |
|-------|----------|
| M1 | - Trabalho noturno de luminárias vert pendente setembro dias consecutivos.  - Preuves d'alias staging verificaes en CI.  - A governança ratifica a política de expectativa. |

## Gerenciamento de alteração

1. Proponente de ajustes via PR enviado para este arquivo **e**
   `docs/source/sorafs/migration_ledger.md`.
2. Lier les minutos de governo et les preuves CI na descrição do PR.
3. Após a mesclagem, notifique a lista de armazenamento + DevRel com um currículo e ações
   atende aos operadores.

Siga este procedimento para garantir que a implementação SoraFS seja determinada,
auditável e transparente entre as equipes participantes no lançamento Nexus.
---
lang: pt
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6609b9628186b58c0441d1ca1594a3030b7f7d81bb2df9c36af3a9d65cdd963d
source_last_modified: "2025-11-15T20:04:53.140162+00:00"
translation_last_reviewed: 2026-01-01
---

# Template de acknowledgement do custodiante de repo

Use este template quando um repo (bilateral ou tri-party) referencia um custodiante via `RepoAgreement::custodian`. O objetivo e registrar o SLA de custody, contas de roteamento e contatos de drill antes da movimentacao de ativos. Copie o template para seu diretorio de evidencia (por exemplo
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), preencha os placeholders e calcule o hash do arquivo como parte do pacote de governance descrito em
`docs/source/finance/repo_ops.md` sec 2.8.

## 1. Metadados

| Campo | Valor |
|-------|-------|
| Identificador do acordo | `<repo-yyMMdd-XX>` |
| ID da conta do custodiante | `<ih58...>` |
| Preparado por / data | `<custodian ops lead>` |
| Contatos de desk reconhecidos | `<desk lead + counterparty>` |
| Diretorio de evidencia | ``artifacts/finance/repo/<slug>/`` |

## 2. Escopo de custody

- **Definicoes de collateral recebidas:** `<list of asset definition ids>`
- **Moeda do cash leg / settlement rail:** `<xor#sora / other>`
- **Janela de custody:** `<start/end timestamps or SLA summary>`
- **Standing instructions:** `<hash + path to standing instruction document>`
- **Prerequisitos de automacao:** `<scripts, configs, or runbooks custodian will invoke>`

## 3. Roteamento e monitoring

| Item | Valor |
|------|-------|
| Wallet de custody / conta de ledger | `<asset ids or ledger path>` |
| Canal de monitoring | `<Slack/phone/on-call rotation>` |
| Contato de drill | `<primary + backup>` |
| Alertas requeridos | `<PagerDuty service, Grafana board, etc.>` |

## 4. Declaracoes

1. *Custody readiness:* "Revisamos o payload `repo initiate` staged com os
   identificadores acima e estamos prontos para aceitar collateral sob o SLA listado
   na sec 2."
2. *Rollback commitment:* "Executaremos o playbook de rollback acima se o incident commander
   determinar, e forneceremos logs de CLI mais hashes em
   `governance/drills/<timestamp>.log`."
3. *Evidence retention:* "Manteremos o acknowledgement, standing instructions e
   logs de CLI por pelo menos `<duration>` e os forneceremos ao conselho de financas sob
   solicitacao."

Assine abaixo (assinaturas eletronicas aceitas quando roteadas pelo tracker de governance).

| Nome | Papel | Assinatura / data |
|------|------|------------------|
| `<custodian ops lead>` | Operador do custodiante | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> Depois de assinado, calcule o hash do arquivo (exemplo: `sha256sum custodian_ack_<cust>.md`) e registre o digest na tabela do pacote de governance para que os reviewers possam verificar os bytes do acknowledgement referenciados durante a votacao.

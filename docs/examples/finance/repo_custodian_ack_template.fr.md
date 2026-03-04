---
lang: fr
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6609b9628186b58c0441d1ca1594a3030b7f7d81bb2df9c36af3a9d65cdd963d
source_last_modified: "2025-11-15T20:04:53.140162+00:00"
translation_last_reviewed: 2026-01-01
---

# Modele d'accuse du custodian de repo

Utilisez ce modele lorsqu'un repo (bilateral ou tri-party) reference un custodian via `RepoAgreement::custodian`. L'objectif est d'enregistrer le SLA de custody, les comptes de routage et les contacts de drill avant le mouvement des actifs. Copiez le modele dans votre repertoire de preuves (par exemple
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), remplissez les placeholders et calculez le hash du fichier dans le paquet governance decrit dans
`docs/source/finance/repo_ops.md` sec 2.8.

## 1. Metadonnees

| Champ | Valeur |
|-------|-------|
| Identifiant d'accord | `<repo-yyMMdd-XX>` |
| ID de compte custodian | `<ih58...>` |
| Prepare par / date | `<custodian ops lead>` |
| Contacts desk reconnus | `<desk lead + counterparty>` |
| Repertoire de preuves | ``artifacts/finance/repo/<slug>/`` |

## 2. Portee de custody

- **Definitions de collateral recues:** `<list of asset definition ids>`
- **Devise cash leg / rail de settlement:** `<xor#sora / other>`
- **Fenetre de custody:** `<start/end timestamps or SLA summary>`
- **Instructions permanentes:** `<hash + path to standing instruction document>`
- **Prerequis d'automatisation:** `<scripts, configs, or runbooks custodian will invoke>`

## 3. Routage et monitoring

| Item | Valeur |
|------|-------|
| Wallet custody / compte ledger | `<asset ids or ledger path>` |
| Canal de monitoring | `<Slack/phone/on-call rotation>` |
| Contact drill | `<primary + backup>` |
| Alertes requises | `<PagerDuty service, Grafana board, etc.>` |

## 4. Declarations

1. *Custody readiness:* "Nous avons revu le payload `repo initiate` stage avec les
   identifiants ci-dessus et sommes prets a accepter le collateral sous le SLA liste
   en sec 2."
2. *Engagement rollback:* "Nous executerons le playbook rollback cite ci-dessus si
   l'incident commander le demande, et fournirons les logs CLI plus les hashes dans
   `governance/drills/<timestamp>.log`."
3. *Retention de preuves:* "Nous conserverons l'accuse, les instructions permanentes et les
   logs CLI pour au moins `<duration>` et les fournirons au conseil finance sur demande."

Signez ci-dessous (signatures electroniques acceptables lorsqu'elles passent par le tracker governance).

| Nom | Role | Signature / date |
|------|------|------------------|
| `<custodian ops lead>` | Operateur custodian | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> Une fois signe, hashez le fichier (exemple: `sha256sum custodian_ack_<cust>.md`) et enregistrez le digest dans la table du paquet governance afin que les reviewers puissent verifier les bytes d'accuse references pendant le vote.

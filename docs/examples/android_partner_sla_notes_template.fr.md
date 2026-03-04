---
lang: fr
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1d3e5d2b42f7e6f9c2de4f2be56b6994b4b88b109f70edc7e6f04ec0f3465ac
source_last_modified: "2025-11-12T08:32:28.349523+00:00"
translation_last_reviewed: 2026-01-01
---

# Notes de decouverte SLA pour partenaire Android - Modele

Utilisez ce modele pour chaque session de decouverte SLA AND8. Stockez la copie completee
sous `docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` et joignez les
artefacts de support (reponses au questionnaire, confirmations, pieces jointes) dans le meme
repertoire.

```
Partner: <Nom>                      Date: <YYYY-MM-DD>  Heure: <UTC>
Contacts principaux: <noms, roles, email>
Participants Android: <Program Lead / Partner Eng / Support Eng / Compliance>
Lien de reunion / ticket: <URL ou ID>
```

## 1. Agenda et contexte

- Objet de la session (perimetre pilote, fenetre de release, attentes de telemetrie).
- Docs de reference partages avant l'appel (support playbook, calendrier de release,
  dashboards de telemetrie).

## 2. Vue d'ensemble des charges

| Sujet | Notes |
|------|-------|
| Charges cibles / chaines | |
| Volume de transactions attendu | |
| Fenetres business critiques / periodes de blackout | |
| Regimes regulatoires (GDPR, MAS, FISC, etc.) | |
| Langues requises / localisation | |

## 3. Discussion SLA

| Classe SLA | Attente du partenaire | Ecart vs baseline? | Action requise |
|------------|------------------------|--------------------|----------------|
| Correctif critique (48 h) | | Oui/Non | |
| Haute severite (5 business days) | | Oui/Non | |
| Maintenance (30 days) | | Oui/Non | |
| Avis de cutover (60 days) | | Oui/Non | |
| Cadence de communication d'incident | | Oui/Non | |

Documentez toute clause SLA supplementaire demandee par le partenaire (ex. pont telephonique


dedie, exports telemetrie supplementaires).

## 4. Exigences de telemetrie et d'acces

- Besoins d'acces Grafana / Prometheus:
- Exigences d'export de logs/traces:
- Attentes d'evidence offline ou dossier:

## 5. Notes compliance et legales

- Exigences de notification juridictionnelle (statut + timing).
- Contacts legaux requis pour les updates d'incident.
- Contraintes de residence des donnees / exigences de stockage.

## 6. Decisions et actions

| Item | Owner | Date | Notes |
|------|-------|------|-------|
| | | | |

## 7. Acknowledgement

- Le partenaire a reconnu le SLA de base? (O/N)
- Methode de confirmation de suivi (email / ticket / signature):
- Joignez l'email de confirmation ou le compte rendu de reunion a ce repertoire avant de cloturer.

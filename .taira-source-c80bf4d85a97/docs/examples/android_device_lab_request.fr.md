---
lang: fr
direction: ltr
source: docs/examples/android_device_lab_request.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a8e6a4981a11faac56d9b04432773e94fd59f8e2524fa4c552be459291c7c39
source_last_modified: "2025-11-12T08:31:44.643013+00:00"
translation_last_reviewed: 2026-01-01
---

# Modele de demande de reservation du laboratoire de devices Android

Copiez ce modele dans la file Jira `_android-device-lab` lors de la reservation du materiel.
Joignez des liens vers les pipelines Buildkite, les artefacts de compliance et tout ticket
partenaire dependant du run.

```
Resume: <Jalon / charge> - <lane(s)> - <date/heure UTC>

Jalon / Suivi:
- Item de roadmap: AND6 / AND7 / AND8 (choisir)
- Ticket(s) associes: <lien vers issue ANDx>, <reference partner-sla si applicable>

Demandeur / Contact:
- Ingenieur principal:
- Ingenieur de secours:
- Canal Slack / escalade pager:

Details de reservation:
- Lanes requises: <pixel8pro-strongbox-a / pixel8a-ci-b / pixel7-fallback / firebase-burst / strongbox-external>
- Creneau souhaite: <YYYY-MM-DD HH:MM UTC> pour <duree>
- Type de charge: <CI smoke / attestation sweep / chaos rehearsal / partner demo>
- Tooling a executer: <noms de jobs/scripts Buildkite>
- Artefacts produits: <logs, bundles attestation, dashboards>

Dependances:
- Reference snapshot capacite: lien vers `android_strongbox_capture_status.md`
- Lignes de matrice readiness touchees: lien vers `android_strongbox_device_matrix.md`
- Lien compliance (si applicable): ligne checklist AND6, ID log evidence

Plan de fallback:
- Si le creneau primaire est indisponible, le creneau alternatif est:
- Besoin d'un pool fallback / Firebase? (oui/non)
- Retainer StrongBox externe requis? (oui/non - inclure lead time)

Approbations:
- Responsable Hardware Lab:
- TL Android Foundations (si lanes CI impactees):
- Responsable programme (si StrongBox retainer invoque):

Checklist post-run:
- Joindre URL(s) Buildkite:
- Mettre a jour la ligne du log evidence: <ID/date>
- Noter les deviations/surcouts:
```

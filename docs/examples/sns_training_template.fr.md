---
lang: fr
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-11-15T09:17:21.371048+00:00"
translation_last_reviewed: 2026-01-01
---

# Modele de diapositives de formation SNS

Ce plan Markdown reflete les diapositives que les facilitateurs doivent adapter pour leurs cohortes linguistiques. Copiez ces sections dans Keynote/PowerPoint/Google Slides et localisez les puces, captures et diagrammes selon les besoins.

## Diapositive de titre
- Programme: "Sora Name Service onboarding"
- Sous-titre: preciser suffixe + cycle (ex., `.sora - 2026-03`)
- Presentateurs + affiliations

## Orientation KPI
- Capture ou embed de `docs/portal/docs/sns/kpi-dashboard.md`
- Liste de puces expliquant les filtres de suffixe, table ARPU, suivi des gels
- Encarts pour exporter PDF/CSV

## Cycle de vie du manifest
- Diagramme: registrar -> Torii -> governance -> DNS/gateway
- Etapes referencant `docs/source/sns/registry_schema.md`
- Extrait de manifest annote en exemple

## Exercices de litige et gel
- Diagramme de flux pour intervention du guardian
- Checklist referencant `docs/source/sns/governance_playbook.md`
- Chronologie de ticket de gel en exemple

## Capture des annexes
- Extrait de commande montrant `cargo xtask sns-annex ... --portal-entry ...`
- Rappel d'archiver le JSON Grafana sous `artifacts/sns/regulatory/<suffix>/<cycle>/`
- Lien vers `docs/source/sns/reports/.<suffix>/<cycle>.md`

## Prochaines etapes
- Lien de feedback de formation (voir `docs/examples/sns_training_eval_template.md`)
- Handles de canaux Slack/Matrix
- Dates des prochains jalons

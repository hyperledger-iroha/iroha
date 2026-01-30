---
lang: fr
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
translator: manual
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-11-10T19:22:20.036140+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Traduction française de docs/examples/docs_preview_feedback_form.md (Docs preview feedback form) -->

# Formulaire de feedback pour le preview des docs (vague W1)

Utilisez ce template pour collecter le feedback des relecteurs de la vague W1. Dupliquez‑le
pour chaque partenaire, renseignez les métadonnées et stockez la copie complétée sous
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.

## Métadonnées du relecteur

- **ID partenaire :** `partner-w1-XX`
- **Ticket de demande :** `DOCS-SORA-Preview-REQ-PXX`
- **Invitation envoyée (UTC) :** `YYYY-MM-DD hh:mm`
- **Checksum reconnu (UTC) :** `YYYY-MM-DD hh:mm`
- **Domaines principaux examinés :** (par ex. _docs de l’orchestrateur SoraFS_,
  _flux ISO Torii_)

## Confirmations de télémétrie et d’artefacts

| Élément de checklist | Résultat | Évidence |
| --- | --- | --- |
| Vérification des checksums | ✅ / ⚠️ | Chemin vers le log (ex. `build/checksums.sha256`) |
| Smoke test du proxy Try it | ✅ / ⚠️ | Extrait de `npm run manage:tryit-proxy …` |
| Revue de dashboard Grafana | ✅ / ⚠️ | Chemin(s) vers des captures d’écran |
| Revue du rapport de probe du portail | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

Ajoutez des lignes pour tout SLO supplémentaire inspecté par le relecteur.

## Journal de feedback

| Zone | Sévérité (info/minor/major/blocker) | Description | Correctif suggéré ou question | Ticket de suivi |
| --- | --- | --- | --- | --- |
| | | | | |

Faites référence au ticket GitHub ou interne dans la dernière colonne pour que le tracker
de preview puisse rattacher les actions correctives à ce formulaire.

## Résumé du questionnaire

1. **Quel niveau de confiance avez‑vous dans les consignes de checksum et le processus
   d’invitation ?** (1–5)
2. **Quels docs ont été les plus/moins utiles ?** (réponse courte)
3. **Avez‑vous rencontré des blocages pour accéder au proxy Try it ou aux dashboards de
   télémétrie ?**
4. **Faut‑il ajouter du contenu de localisation ou d’accessibilité ?**
5. **Autres commentaires avant le GA ?**

Capturez des réponses courtes et joignez les exports bruts du questionnaire si vous
utilisez un formulaire externe.

## Vérification des connaissances

- Score : `__/10`
- Questions incorrectes (le cas échéant) : `[#1, #4, …]`
- Actions de suivi (si score < 9/10) : call de remédiation planifié ? o/n

## Validation

- Nom du relecteur et horodatage :
- Nom du relecteur Docs/DevRel et horodatage :

Conservez la copie signée avec les artefacts associés afin que les auditeurs puissent
rejouer la vague sans contexte supplémentaire.

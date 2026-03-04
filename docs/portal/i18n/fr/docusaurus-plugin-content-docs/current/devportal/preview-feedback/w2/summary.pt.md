---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-summary
titre : Résumé des commentaires et statut W2
sidebar_label : CV W2
description : Résumé ao vivo para a onda de preview comunitaria (W2).
---

| Article | Détails |
| --- | --- |
| Onde | W2 - Réviseurs communautaires |
| Janela de convite | 2025-06-15 -> 2025-06-29 |
| Étiquette d'artefato | `preview-2025-06-15` |
| Problème de suivi | `DOCS-SORA-Preview-W2` |
| Participants | comm-vol-01...comm-vol-08 |

## Destaques

1. **Gouvernance et outillage** - La politique communautaire d'admission a été approuvée à l'unanimité le 20/05/2025 ; Le modèle de sollicitation actualisé avec les champs de motivation/fuso horario est sur `docs/examples/docs_preview_request_template.md`.
2. **Preflight de preflight** - A mudanca do proxy Try it `OPS-TRYIT-188` rodou em 2025-06-09, les tableaux de bord sont capturés par Grafana, et les sorties du descripteur/checksum/probe de `preview-2025-06-15` sont archivées dans `artifacts/docs_preview/W2/`.
3. **Onda des convites** - Voici les critiques communautaires convidados le 2025-06-15, avec remerciements enregistrés dans le tableau des convites du tracker ; toutes les vérifications de la somme de contrôle avant la navigation.
4. **Commentaires** - `docs-preview/w2 #1` (libellé de l'info-bulle) et `#2` (ordre dans la barre latérale de localisation) enregistrés le 2025-06-18 et résolus le 2025-06-21 (Docs-core-04/05) ; nenhum incidente durant une onda.

## Ingrédients de cacao| ID | Description | Responsavel | Statut |
| --- | --- | --- | --- |
| W2-A1 | Tratar `docs-preview/w2 #1` (formulation de l'info-bulle). | Docs-core-04 | Concluido 2025-06-21 |
| W2-A2 | Tratar `docs-preview/w2 #2` (barre latérale de localisation). | Docs-core-05 | Concluido 2025-06-21 |
| W2-A3 | Arquivar preuve de la dite + mise à jour de la feuille de route/du statut. | Responsable Docs/DevRel | Concluido 2025-06-29 |

## Résumé de l'encerclement (2025-06-29)

- Tous nos évaluateurs communautaires confirment la conclusion et l'accès à l'aperçu révisé ; remerciements enregistrés no log de convites do tracker.
- Les instantanés finaux de télémétrie (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) sont verts ; les journaux et les transcriptions font du proxy Essayez-le anexados a `DOCS-SORA-Preview-W2`.
- Bundle de preuves (descripteur, journal de somme de contrôle, sortie de sonde, rapport de lien, captures d'écran de Grafana, accusés de réception) archivé dans `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Le journal des points de contrôle W2 du tracker a été mis à jour lors de l'encerclement, garanti un enregistrement audité avant le début de l'avion W3.
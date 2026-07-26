---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-summary
titre : Résumé de feedback et d'encerramento W1
sidebar_label : CV W1
description : Achados, acoes et évidence de encerramento para a onda de preview de parceiros/integradores Torii.
---

| Article | Détails |
| --- | --- |
| Onde | W1 - Parciers et intégrateurs Torii |
| Janela de convite | 2025-04-12 -> 2025-04-26 |
| Étiquette d'artefato | `preview-2025-04-12` |
| Problème de suivi | `DOCS-SORA-Preview-W1` |
| Participants | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Destaques

1. **Flux de somme de contrôle** - Tous les réviseurs valident le descripteur/archive via `scripts/preview_verify.sh` ; enregistre les armazenados avec les accusés de réception des invités.
2. **Télémétrie** - Tableaux de bord `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` sont verts pour tout le monde ; nenhum incidente ou page d’alerte.
3. **Documents de commentaires (`docs-preview/w1`)** - Dois nits menores registrados :
   - `docs-preview/w1 #1` : esclarecer wording de navegacao na secao Essayez-le (resolvido).
   - `docs-preview/w1 #2` : capture d'écran actualisée de Try it (résolu).
4. **Parité des runbooks** - Les opérateurs de SoraFS confirment que les nouveaux liens croisés entre `orchestrator-ops` et `multi-source-rollout` résolvent en fonction des préoccupations de W0.

## Ingrédients de cacao| ID | Description | Responsavel | Statut |
| --- | --- | --- | --- |
| W1-A1 | Actualiser la formulation de navegacao do Try it conforme `docs-preview/w1 #1`. | Docs-core-02 | Concluido (2025-04-18). |
| W1-A2 | Actualiser la capture d'écran de Try it conforme `docs-preview/w1 #2`. | Docs-core-03 | Concluido (2025-04-19). |
| W1-A3 | Résumer les achats de colis et les preuves de télémétrie sur la feuille de route/le statut. | Responsable Docs/DevRel | Concludo (version tracker + status.md). |

## Résumé de l'encerclement (2025-04-26)

- Tous nos évaluateurs confirment la conclusion pendant les heures de bureau finales, limparam artefatos locais e tiveram o acesso revogado.
- Une télémétrie ficou verde a mangé une Saida ; instantanés finais anexados a `DOCS-SORA-Preview-W1`.
- Le journal des convites a été actualisé avec les remerciements de la dite ; Le tracker Marcou W1 comme conclu et ajouté les points de contrôle.
- Bundle de preuves (descripteur, journal de somme de contrôle, sortie de sonde, transcription du proxy Try it, captures d'écran de télémétrie, résumé de commentaires) archivé dans `artifacts/docs_preview/W1/`.

## Passons à proximité

- Préparer le plan d'admission communautaire W2 (approbation de gouvernance + ajustement sans modèle de sollicitation).
- Actualiser la balise de l'artéfact de prévisualisation pour l'ensemble W2 et réexécuter le script de contrôle en amont lorsque les données sont finalisées.
- Levar achados aplicaveis de W1 para roadmap/status para que a onda comunitaria tenha a orientacao plus recente.
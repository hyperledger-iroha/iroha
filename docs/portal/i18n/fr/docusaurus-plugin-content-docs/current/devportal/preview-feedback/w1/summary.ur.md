---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-summary
titre : W1 فيڈبیک اور اختتامی خلاصہ
sidebar_label : W1 ici
description : vague de prévisualisation des intégrateurs Torii disponible ici
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W1 - Intégrateurs Torii |
| دعوتی ونڈو | 2025-04-12 -> 2025-04-26 |
| آرٹیفیکٹ ٹیگ | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |
| شرکا | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## نمایاں نکات

1. **Checksum ورک فلو** - Les réviseurs de ce groupe sont `scripts/preview_verify.sh` pour le descripteur/l'archive en question. journaux et accusés de réception
2. **ٹیلیمیٹری** - `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` tableaux de bord en noir et blanc Incidents et pages d'alerte
3. **Docs فيڈبیک (`docs-preview/w1`)** - دو معمولی نٹس ریکارڈ ہوئیں :
   - `docs-preview/w1 #1` : Essayez-le avec le libellé de navigation et le texte (حل ہو گیا)۔
   - `docs-preview/w1 #2` : Essayez-le ici. (حل ہو گیا)۔
4. **Parité Runbook** - Opérateurs SoraFS pour les liens croisés `orchestrator-ops` et `multi-source-rollout` pour les liens croisés et W0 خدشات حل کیے۔

## ایکشن آئٹمز| ID | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W1-A1 | `docs-preview/w1 #1` کے مطابق Essayez-le libellé de navigation اپ ڈیٹ کرنا۔ | Docs-core-02 | ✅مکمل (2025-04-18). |
| W1-A2 | `docs-preview/w1 #2` کے مطابق Essayez-le اسکرین شاٹ اپ ڈیٹ کرنا۔ | Docs-core-03 | ✅مکمل (2025-04-19). |
| W1-A3 | Résultats des recherches et preuves télémétriques et feuille de route/état des lieux | Responsable Docs/DevRel | ✅ مکمل (tracker + status.md دیکھیں). |

## اختتامی خلاصہ (2025-04-26)

- Il y a des évaluateurs pendant les heures de bureau et des artefacts et des artefacts. لی گئی۔
- ٹیلیمیٹری اختتام تک vert رہی؛ Captures d'écran `DOCS-SORA-Preview-W1` pour photos instantanées
- Le journal des accusés de sortie et les accusés de réception sont également disponibles. tracker نے W1 کو 🈴 پر سیٹ کیا اور points de contrôle شامل کیے۔
- ensemble de preuves (descripteur, journal de somme de contrôle, sortie de sonde, transcription du proxy Try it, captures d'écran de télémétrie, résumé des commentaires) `artifacts/docs_preview/W1/` میں archive ہوا۔

## اگلے اقدامات

- Plan d'admission communautaire W2 تیار کریں (approbation de la gouvernance + ajustements du modèle de demande).
- W2 wave présente la balise d'artefact d'aperçu et le contrôle en amont du vol en amont.
- Résultats du W1 et feuille de route/statut ainsi que la vague communautaire et les conseils d'orientation.
---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-plan
titre : W2 کمیونٹی admission پلان
sidebar_label : W2 ici
description : Aperçu de la cohorte, admission, approbations, liste de contrôle des preuves
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W2 - critiques de کمیونٹی |
| ہدف ونڈو | T3 2025 1 (عارضی) |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-06-15` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W2` |

## مقاصد

1. Critères d'admission et flux de travail de vérification
2. Liste des listes et addendum d'utilisation acceptable pour l'approbation de la gouvernance et approbation de la gouvernance.
3. somme de contrôle pour vérifier l'aperçu de l'artefact et l'ensemble de télémétrie pour les détails et les détails
4. Voici comment procéder Essayez-le proxy et les tableaux de bord et les scènes

## ٹاسک بریک ڈاؤن| ID | ٹاسک | مالک | مقررہ تاریخ | اسٹیٹس | نوٹس |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Critères d'admission (éligibilité, emplacements maximum, exigences CoC) et gouvernance et gouvernance | Responsable Docs/DevRel | 2025-05-15 | ✅ مکمل | admission پالیسی `DOCS-SORA-Preview-W2` میں fusion ہوئی اور 2025-05-20 کے conseil میٹنگ میں approuver ہوئی۔ |
| W2-P2 | modèle de demande pour les demandes de paiement (motivation, disponibilité, besoins de localisation) | Docs-core-01 | 2025-05-18 | ✅ مکمل | `docs/examples/docs_preview_request_template.md` میں اب Community سیکشن شامل ہے، جو admission فارم میں حوالہ ہے۔ |
| W2-P3 | admission پلان کے لئے gouvernance approbation حاصل کرنا (vote de la réunion + procès-verbal enregistré) | Liaison gouvernance | 2025-05-22 | ✅ مکمل | le 20/05/2025 dans la journée minutes + appel nominal `DOCS-SORA-Preview-W2` میں لنک ہیں۔ |
| W2-P4 | W2 ونڈو کے لئے Essayez-le proxy staging + télémétrie capture شیڈول کرنا (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | ✅ مکمل | changer le ticket `OPS-TRYIT-188` منظور اور 2025-06-09 02:00-04:00 UTC میں exécuter ہوا؛ Grafana captures d'écran dans les archives ہیں۔ |
| W2-P5 | Voici un aperçu de la balise d'artefact (`preview-2025-06-15`) construire/vérifier le descripteur/somme de contrôle/archives des journaux de sonde ici | Portail TL | 2025-06-07 | ✅ مکمل | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 2025-06-10 en anglais sorties `artifacts/docs_preview/W2/preview-2025-06-15/` میں محفوظ ہیں۔ || W2-P6 | کمیونٹی liste d'invitations تیار کرنا (<=25 évaluateurs, lots par étapes) informations de contact approuvées par la gouvernance کے ساتھ | Gestionnaire de communauté | 2025-06-10 | ✅ مکمل | پہلے cohorte کے 8 évaluateurs de la communauté منظور ہوئے؛ ID de demande `DOCS-SORA-Preview-REQ-C01...C08` tracker en ligne |

## Liste de contrôle des preuves

- [x] dossier d'approbation de la gouvernance (notes de réunion + lien de vote) `DOCS-SORA-Preview-W2` کے ساتھ منسلک ہے۔
- [x] Modèle de demande mis à jour `docs/examples/` pour commit ہے۔
- [x] Descripteur `preview-2025-06-15`, journal de somme de contrôle, sortie de sonde, rapport de lien, et Essayez-le transcription proxy `artifacts/docs_preview/W2/` میں محفوظ ہیں۔
- [x] Captures d'écran Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) Fenêtre de contrôle en amont W2
- [x] Table de liste d'invitations, identifiants de réviseur, demandes de tickets, et envoi des horodatages d'approbation (tracker de W2 pour les clients)

یہ پلان اپ ڈیٹ رکھیں؛ tracker اسے ریفرنس کرتا ہے تاکہ DOCS-SORA roadmap واضح طور پر دیکھ سکے کہ W2 invitations سے پہلے کیا باقی ہے۔
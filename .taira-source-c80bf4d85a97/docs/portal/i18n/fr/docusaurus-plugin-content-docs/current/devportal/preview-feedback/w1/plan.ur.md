---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-plan
titre : W1 شراکت داروں کے لئے پری فلائٹ پلان
sidebar_label : W1 ici
description: aperçu aperçu کوہوٹ کے لئے ٹاسکس، مالکان، اور ثبوت چیک لسٹ۔
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W1 - Intégrateurs Torii |
| ہدف ونڈو | T2 2025 partie 3 |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |

## مقاصد

1. Aperçu de l'aperçu شرائط لئے قانونی اور گورننس منظوری حاصل کرنا۔
2. Essayez-le proxy et instantanés de télémétrie par exemple.
3. somme de contrôle pour vérifier l'aperçu de l'artefact et la sonde
4. Liste des modèles de demande de liste et modèles de demande

## ٹاسک بریک ڈاؤن| ID | ٹاسک | مالک | مقررہ تاریخ | اسٹیٹس | نوٹس |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | aperçu des termes addendum کے لئے قانونی منظوری حاصل کرنا | Responsable Docs/DevRel -> Juridique | 2025-04-05 | ✅ مکمل | قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` 2025-04-05 کو منظور ہوا؛ PDF ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P2 | Essayez-le proxy et staging et (2025-04-10) محفوظ کرنا اور proxy health et تصدیق | Docs/DevRel + Ops | 2025-04-06 | ✅ مکمل | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 2025-04-06 کو چلایا گیا؛ Transcription CLI اور `.env.tryit-proxy.bak` محفوظ کر دیے گئے۔ |
| W1-P3 | aperçu de l'artefact (`preview-2025-04-12`) pour `scripts/preview_verify.sh` + `npm run probe:portal` descripteur/sommes de contrôle | Portail TL | 2025-04-08 | ✅ مکمل | artefact et journaux de vérification `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ ہیں؛ sortie de la sonde ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P4 | Formulaires d'admission (`DOCS-SORA-Preview-REQ-P01...P08`) Contacts et NDA | Liaison gouvernance | 2025-04-07 | ✅ مکمل | تمام آٹھ درخواستیں منظور ہوئیں (آخری دو 2025-04-11 کو منظور ہوئیں)؛ approbations ٹریکر میں لنک ہیں۔ |
| W1-P5 | دعوتی متن تیار کرنا (`docs/examples/docs_preview_invite_template.md` for مبنی)، ہر پارٹنر کے لئے `<preview_tag>` اور `<request_ticket>` سیٹ کرنا | Responsable Docs/DevRel | 2025-04-08 | ✅ مکمل | Il s'agit du 2025-04-12 15:00 UTC et l'artefact est en cours de réalisation. |

## Contrôle en amont> Modèle : `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` Mise à jour 1-5 Mise à jour de la mise à jour (construction, vérification de la somme de contrôle, sonde de portail, vérificateur de liens et essayez la mise à jour du proxy). JSON est utilisé pour créer un lien vers une application JSON.

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` en haut) et `build/checksums.sha256` et `build/release.json` en haut
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` et `build/link-report.json` et descripteur pour les archives
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (cible `--tryit-target` pour la cible)؛ Il s'agit d'un `.env.tryit-proxy` pour valider une restauration et d'une restauration pour `.bak`.
6. W1 contient les chemins de journalisation et les fichiers de connexion (somme de contrôle du descripteur, sortie de la sonde, essayez-le proxy et instantanés Grafana)

## ثبوت چیک لسٹ

- [x] دستخط شدہ قانونی منظوری (PDF en ligne) `DOCS-SORA-Preview-W1` کے ساتھ منسلک ہے۔
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` et captures d'écran Grafana۔
- [x] Descripteur `preview-2025-04-12` et journal de somme de contrôle `artifacts/docs_preview/W1/` pour le journal de contrôle
- [x] Tableau de liste des horodatages `invite_sent_at` en anglais (journal de journal W1)
- [x] artefacts de rétroaction [`preview-feedback/w1/log.md`](./log.md) میں نظر آتے ہیں، ہر پارٹنر کے لئے ایک row (2025-04-26 کو liste/télémétrie/problèmes ڈیٹا کے ساتھ اپ ڈیٹ)۔

جوں جوں کام آگے بڑھے یہ پلان اپ ڈیٹ کریں؛ Feuille de route pour l'auditabilité

## فیڈبیک ورک فلو1. ہر critique کے لئے
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) Modèle de modèle
   میٹا ڈیٹا بھریں اور مکمل کاپی
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` میں رکھیں۔
2. invitations, points de contrôle de télémétrie et problèmes ouverts
   [`preview-feedback/w1/log.md`](./log.md) Un journal en direct est disponible pour les réviseurs de gouvernance et les réviseurs
   référentiel pour replay et replay
3. Effectuer la vérification des connaissances et les exportations d'enquêtes, ainsi que le journal de bord, le chemin de l'artefact et l'attachement de l'objet.
   Problème de tracker et lien ici
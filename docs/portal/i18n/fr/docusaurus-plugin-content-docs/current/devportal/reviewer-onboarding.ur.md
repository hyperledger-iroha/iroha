---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو ریویور آن بورڈنگ

## جائزہ

DOCS-SORA ڈویلپر پورٹل کے مرحلہ وار لانچ کو ٹریک کرتا ہے۔ constructions contrôlées par somme de contrôle
(`npm run serve`) اور مضبوط Essayez-le si vous avez un problème avec votre appareil : 
کھلنے سے پہلے ویری فائیڈ ریویورز کی آن بورڈنگ۔ یہ گائیڈ بیان کرتا ہے کہ درخواستیں کیسے جمع کی جائیں،
اہلیت کیسے چیک کی جائے، رسائی کیسے دی جائے، اور شرکاء کو محفوظ طریقے سے آف بورڈ کیسے کیا جائے۔
کوهورٹ پلاننگ، دعوتی cadence، اور ٹیلی میٹری exportations کے لئے
[prévisualiser le flux d'invitation](./preview-invite-flow.md) دیکھیں؛ نیچے کے مراحل اس پر فوکس کرتے ہیں
کہ ریویور منتخب ہونے کے بعد کون سی کارروائیاں کرنی ہیں۔

- **اسکوپ:** et la version GA de GA pour l'aperçu de la documentation (`docs-preview.sora`, builds de pages GitHub, ou bundles SoraFS) درکار ہے۔
- **Modalités :** Torii et SoraFS Kit d'intégration (kits d'intégration pour les déploiements) et déploiements (دیکھیں [`devportal/deploy-guide`](./deploy-guide.md))۔

## رولز اور پری ریکویزٹس| رول | عام اہداف | artefacts مطلوبہ | نوٹس |
| --- | --- | --- | --- |
| Responsable du noyau | Plus de tests de fumée et de tests de fumée | GitHub handle, Matrix contact, signé CLA فائل پر۔ | Je suis en ligne avec `docs-preview` GitHub en ligne. پھر بھی درخواست درج کریں تاکہ رسائی auditable رہے۔ |
| Réviseur partenaire | Il existe des extraits de code SDK et du contenu de gouvernance ainsi que des extraits de code SDK. | E-mail d'entreprise, POC légal, conditions d'aperçu signées۔ | ٹیلی میٹری + exigences en matière de traitement des données کی منظوری لازم ہے۔ |
| Bénévole communautaire | Commentaires sur la convivialité ici | Identifiant GitHub, contact préféré, fuseau horaire, CoC | کوہورٹس چھوٹے رکھیں؛ accord de contribution |

تمام ریویور اقسام کو لازمی ہے کہ:

1. aperçu des artefacts à usage acceptable پالیسی تسلیم کریں۔
2. annexes de sécurité/observabilité
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Le service d'instantané est disponible pour `docs/portal/scripts/preview_verify.sh` pour le service de sauvegarde.

## Admission ورک فلو1. درخواست دینے والے سے
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   فارم بھرنے کو کہیں (یا issue میں copier/coller کریں)۔ کم از کم یہ ریکارڈ کریں: شناخت، رابطے کا طریقہ،
   GitHub handle, مجوزہ dates de révision, اور security docs پڑھنے کی تصدیق۔
2. Le tracker `docs-preview` (problème GitHub et ticket de gouvernance) est associé à l'attribution d'un approbateur.
3. پری ریکویزٹس چیک کریں:
   - CLA / accord de contributeur فائل پر موجود ہو (یا référence du contrat partenaire)۔
   - accusé de réception d'utilisation acceptable درخواست میں محفوظ ہو۔
   - évaluation des risques کمل ہو (مثال: partenaires examinateurs کو Legal نے approuver کیا ہو)۔
4. Approbateur pour l'approbation et le problème de suivi et pour l'entrée de gestion du changement
   (Porté : `DOCS-SORA-Preview-####`)۔

## Provisioning اور ٹولنگ

1. **Artefacts en cours** — Flux de travail CI avec broche SoraFS et descripteur d'aperçu + archive en cours
   (Artefact `docs-portal-preview`)۔ ریویورز کو یاد دلائیں کہ چلائیں:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. ** L'application de la somme de contrôle est utilisée pour servir ** — L'application de la somme de contrôle est la clé du service :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   `scripts/serve-verified-preview.mjs` pour la réutilisation et la construction non vérifiée pour une version non vérifiée

3. **GitHub رسائی دیں (اختیاری)** — اگر ریویورز کو branches non publiées چاہییں تو انہیں review مدت کے لئے
   `docs-preview` GitHub est disponible pour les membres et l'adhésion pour les membres du groupe4. **Canaux d'assistance en ligne** — contact de garde (Matrix/Slack) et procédure d'incident
   [`incident-runbooks`](./incident-runbooks.md) سے شیئر کریں۔

5. **Télémétrie + feedback** — Analyses anonymisées et analyses anonymisées
   (دیکھیں [`observability`](./observability.md))۔ Vous avez besoin d'un formulaire de commentaires et d'un modèle de problème.
   اور event کو [`preview-feedback-log`](./preview-feedback-log) helper سے لاگ کریں تاکہ wave summary اپ ٹو ڈیٹ رہے۔

## ریویور چیک لسٹ

پریویو تک رسائی سے پہلے ریویورز کو یہ مکمل کرنا ہوگا:

1. les artefacts téléchargés vérifient کریں (`preview_verify.sh`)۔
2. `npm run serve` (یا `serve:verified`) pour la protection de la somme de contrôle pour l'argent
3. Sécurité et observabilité
4. Console OAuth/Try it et connexion par code de périphérique pour les jetons de production (applicables et les jetons de production)
5. les résultats du tracker convenu sont ici (numéro, document partagé ici) et la balise de version préliminaire et la balise de sortie

## Mainteneur ذمہ داریاں اور offboarding| مرحلہ | اقدامات |
| --- | --- |
| Coup d'envoi | Liste de contrôle d'admission pour les artefacts + artefacts + détails sur la liste de contrôle d'admission [`preview-feedback-log`](./preview-feedback-log) Voici votre avis `invite-sent` انٹری شامل کریں، اور اگر review ایک ہفتے سے Il s'agit d'une synchronisation à mi-parcours. |
| Surveillance | aperçu de la télémétrie (vous pouvez l'essayer en cas d'échec de la sonde) et le runbook d'incident est également disponible résultats et `feedback-submitted`/`issue-opened` événements et métriques d'onde |
| Débarquement | GitHub est en ligne avec SoraFS et `access-revoked` contient les archives des archives (résumé des commentaires + actions en suspens کریں)، اور registre des évaluateurs اپ ڈیٹ کریں۔ ریویور سے مقامی builds صاف کرنے کو کہیں اور [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) سے بنا digest منسلک کریں۔ |

Les vagues sont en rotation et les vagues tournent et les vagues tournent. repo میں trace papier
(numéro + modèles) Document de référence DOCS-SORA vérifiable en matière de gouvernance et de gouvernance
Aperçu des contrôles documentés d'accès

## Modèles d'invitation et suivi- ہر sensibilisation کی شروعات
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  فائل سے کریں۔ Il s'agit d'un aperçu de la somme de contrôle et d'un aperçu de la somme de contrôle.
  ریویورز acceptable-use پالیسی تسلیم کریں۔
- modèle pour les espaces réservés des canaux de contact `<preview_tag>`, `<request_ticket>` et les espaces réservés des canaux de contact
  message final pour le ticket d'admission pour les approbateurs et les auditeurs
  بھیجے گئے الفاظ کا حوالہ دے سکیں۔
- Il s'agit d'une feuille de calcul de suivi et d'un problème avec l'horodatage `invite_sent_at` et la date de fin.
  اپ ڈیٹ کریں تاکہ [aperçu du flux d'invitation] (./preview-invite-flow.md) رپورٹ خودکار طور پر cohorte اٹھا سکے۔
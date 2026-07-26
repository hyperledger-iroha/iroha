---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو دعوتی فلو

## مقصد

روڈ میپ آئٹم **DOCS-SORA** ریویور آن بورڈنگ اور پبلک پریویو دعوتی پروگرام کو وہ آخری رکاوٹیں قرار دیتا ہے جن کے بعد پورٹل بیٹا سے باہر جا سکتا ہے۔ یہ صفحہ بیان کرتا ہے کہ ہر دعوتی ویو کیسے کھولی جائے، کون سے artefacts دعوتیں بھیجنے Il s'agit d'une question d'auditabilité. Voici la liste des choses à faire :

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) ہر ریویور کی ہینڈلنگ کے لئے۔
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) somme de contrôle ضمانتوں کے لئے۔
- [`devportal/observability`](./observability.md) permet d'exporter des crochets d'alerte et de les utiliser.

## ویو پلان| et | سامعین | انٹری معیار | ایگزٹ معیار | نوٹس |
| --- | --- | --- | --- | --- |
| **W0 - Mainteneurs principaux** | Les responsables de Docs/SDK dès le premier jour valident les tâches | GitHub utilise `docs-portal-preview` pour `npm run serve` checksum gate et Alertmanager 7 aujourd'hui | Il s'agit d'un arriéré de documents P0 et d'un incident de blocage. | فلو valider کرنے کے لئے؛ دعوتی ای میل نہیں، صرف aperçu des artefacts شیئر کریں۔ |
| **W1 - Partenaires** | SoraFS pour les intégrateurs Torii et NDA et les réviseurs de gouvernance | W0 ختم، قانونی شرائط منظور، Try-it proxy staging پر۔ | Il y a une signature de signature (problème sous forme signée) et il y a =2 versions de documentation du pipeline d'aperçu et de restauration et de restauration | invitations simultanées محدود (<=25) اور ہفتہ وار بیچ۔ |

`status.md` Aperçu des demandes de suivi des demandes et des informations sur la gouvernance et la gouvernance des informations sur la gouvernance

## Liste de contrôle en amont

ان اقدامات کو **دعوتیں شیڈول کرنے سے پہلے** مکمل کریں:1. **Artefacts CI ajoutés**
   - تازہ ترین `docs-portal-preview` + descripteur `.github/workflows/docs-portal-preview.yml` کے ذریعے اپ لوڈ ہو۔
   - Broche SoraFS `docs/portal/docs/devportal/deploy-guide.md` en haut (descripteur de basculement en haut).
2. **Application de la somme de contrôle**
   - `docs/portal/scripts/serve-verified-preview.mjs` `npm run serve` کے ذریعے invoquer ہو۔
   - `scripts/preview_verify.sh` pour macOS + Linux pour tous les utilisateurs
3. **Référence de télémétrie**
   - `dashboards/grafana/docs_portal.json` صحت مند Essayez-le maintenant et `docs.preview.integrity` الرٹ سبز ہو۔
   - `docs/portal/docs/devportal/observability.md` dans l'annexe Grafana pour le produit
4. **Artefacts de gouvernance**
   - problème de suivi des invitations تیار ہو (problème ہر ویو کے لئے ایک). 
   - modèle de registre des réviseurs کاپی ہو (دیکھیں [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Problème d'approbation SRE en ce qui concerne la question des approbations SRE

Le suivi des invitations est disponible pour le suivi des invitations et le contrôle en amont.

## فلو کے مراحل1. **امیدوار منتخب کریں**
   - ویٹ لسٹ شیٹ یا پارٹنر کیو سے نکالیں۔
   - Modèle de demande de modèle de demande de téléchargement gratuit
2. **رسائی کی منظوری**
   - problème de suivi des invitations par l'approbateur اسائن کریں۔
   - prérequis چیک کریں (CLA/contrat, utilisation acceptable, brief de sécurité).
3. **دعوتیں ارسال کریں**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) et espaces réservés (`<preview_tag>`, `<request_ticket>`, contacts)
   - descripteur + hachage d'archive, essayez-le, URL de préparation et canaux de support
   - Problème de فائنل ای میل (transcription Matrix/Slack) میں محفوظ کریں۔
4. **Intégration ici**
   - suivi des invitations comme `invite_sent_at`, `expected_exit_at`, et statut (`pending`, `active`, `complete`, `revoked`) ڈیٹ کریں۔
   - auditabilité et demande d'admission des examinateurs
5. **Télémétrie مانیٹر کریں**
   - Alertes `docs.preview.session_active` et `TryItProxyErrors` pour les alertes de sécurité
   - Il s'agit d'une ligne de base et d'un incident et d'une entrée d'invitation pour un incident.
6. **فیڈبیک جمع کریں اور خارج ہوں**
   - فیڈبیک آنے پر یا `expected_exit_at` گزرنے پر دعوتیں بند کریں۔
   - اگلی cohorte پر جانے سے پہلے ویو question میں مختصر خلاصہ (résultats, incidents, prochaines actions) اپ ڈیٹ کریں۔

## Preuves et rapports| Artefact | کہاں محفوظ کریں | اپ ڈیٹ cadence |
| --- | --- | --- |
| problème de suivi des invitations | GitHub version `docs-portal-preview` | ہر دعوت کے بعد اپ ڈیٹ کریں۔ |
| exportation de la liste des évaluateurs | `docs/portal/docs/devportal/reviewer-onboarding.md` Registre lié | ہفتہ وار۔ |
| instantanés de télémétrie | `docs/source/sdk/android/readiness/dashboards/<date>/` (réutilisation du paquet de télémétrie par exemple) | ہر ویو + incidents کے بعد۔ |
| résumé des commentaires | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (en anglais seulement) | et sortie à 5 heures du matin |
| note de réunion de gouvernance | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Synchronisation de la gouvernance DOCS-SORA |

Il s'agit d'un `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
چلائیں تاکہ مشین ریڈایبل digest بنے۔ Il s'agit d'un problème JSON et d'un problème de gouvernance que les réviseurs de gouvernance ont mis en œuvre pour résoudre le problème. کی تصدیق کر سکیں۔

ہر ویو ختم ہونے پر preuves کی فہرست `status.md` کے ساتھ منسلک کریں تاکہ روڈ میپ انٹری جلدی اپ ڈیٹ ہو سکے۔

## Rollback et pause

جب درج ذیل میں سے کوئی ہو تو دعوتی فلو روک دیں (اور gouvernance کو مطلع کریں):

- Essayez-le lors d'un incident de proxy en cas de restauration (`npm run manage:tryit-proxy`).
- Fatigue des alertes : 7 points de terminaison en aperçu uniquement et >3 pages d'alerte.
- Écart de conformité : conditions signées et modèle de demande pour les termes et les conditions signés
- Risque d'intégrité : `scripts/preview_verify.sh` en cas de non-concordance de la somme de contrôle.

invite tracker میں remediation دستاویز کرنے اور کم از کم 48 گھنٹے تک ٹیلی میٹری ڈیش بورڈ مستحکم ہونے کی تصدیق کے بعد ہی دوبارہ شروع کریں۔
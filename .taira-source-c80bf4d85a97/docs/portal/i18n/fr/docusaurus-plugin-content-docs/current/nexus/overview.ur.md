---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu du lien
titre : Sora Nexus ici
description : Iroha 3 (Sora Nexus) دستاویزات کے حوالے۔
---

Nexus (Iroha 3) Iroha 2 voies multi-voies pour camions et camions Le SDK est un outil de développement rapide pour les utilisateurs یہ صفحہ مونو ریپو میں نئے خلاصے `docs/source/nexus_overview.md` کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین جلد سمجھ سکیں کہ معماری کے اجزا کیسے جڑتے ہیں۔

## ریلیز لائنز

- **Iroha 2** - Il s'agit d'un produit qui n'est pas disponible.
- **Iroha 3 / Sora Nexus** - Système multivoies à plusieurs voies (DS) ہیں اور مشترکہ گورننس، سیٹلمنٹ، اور آبزرویبیلٹی ٹولنگ وراثت میں لیتے ہیں۔
- Un espace de travail complet (chaîne d'outils IVM + Kotodama) et un SDK complet. ABI اپڈیٹس اور Norito فکسچرز نقل رہتے ہیں۔ آپریٹرز Nexus میں شامل ہونے کے لئے `iroha3-<version>-<os>.tar.zst` بَنڈل ڈاؤن لوڈ کرتے ہیں؛ فل اسکرین چیک لسٹ کے لئے `docs/source/sora_nexus_operator_onboarding.md` دیکھیں۔

## بنیادی اجزا| جزو | خلاصہ | پورٹل لنکس |
|-----------|---------|--------------|
| ڈیٹا اسپیس (DS) | گورننس سے متعین اجرا/اسٹوریج ڈومین جو ایک یا زیادہ voies رکھتا ہے، ویلیڈیٹر سیٹس، پرائیویسی کلاس، اور فیس + DA پالیسی کا اعلان کرتا ہے۔ | مینفسٹ اسکیمہ کے لئے [Nexus spec](./nexus-spec) دیکھیں۔ |
| Voie | اجرا کا ڈیٹرمنسٹک شَارڈ؛ ایسے engagements جاری کرتا ہے جنہیں عالمی NPoS رنگ ترتیب دیتا ہے۔ Lane کلاسز میں `default_public`, `public_custom`, `private_permissioned`, et `hybrid_confidential` ہیں۔ | [Modèle de voie](./nexus-lane-model) جیومیٹری، اسٹوریج پری فکسز، اور ریٹینشن کو بیان کرتا ہے۔ |
| ٹرانزیشن پلان | Identificateurs d'espace réservé, phases de routage, pour emballage à double profil, pour les emballages à une seule voie, Nexus کرتے ہیں۔ | [Notes de transition](./nexus-transition-notes) ہر مائیگریشن مرحلہ دستاویز کرتی ہیں۔ |
| Répertoire spatial | رجسٹری کنٹریکٹ جو DS مینفسٹس + ورژنز اسٹور کرتا ہے۔ آپریٹرز شامل ہونے سے پہلے کیٹلاگ انٹریز کو اس ڈائریکٹری کے ساتھ ملاتے ہیں۔ | Il s'agit d'un `docs/source/project_tracker/nexus_config_deltas/` qui est en vente libre. |
| Catalogue des voies | `[nexus]` comprend les identifiants de voie et les alias, le routage et les seuils DA et les seuils de sécurité `irohad --sora --config … --trace-config` est disponible pour le catalogue résolu pour plus de détails | Procédure pas à pas CLI pour `docs/source/sora_nexus_operator_onboarding.md`. || Routeur de règlement | XOR propose des voies CBDC et des voies de circulation en ligne. | `docs/source/cbdc_lane_playbook.md` boutons de commande pour portails et portes en acier inoxydable |
| Télémétrie/SLO | `dashboards/grafana/nexus_*.json` correspond à la hauteur de la file d'attente + hauteur de voie, à l'arriéré DA, à la latence de règlement et à la profondeur de la file d'attente de gouvernance et à la profondeur de la file d'attente de gouvernance. | [Plan de remédiation de télémétrie](./nexus-telemetry-remediation) ڈیش بورڈز، الرٹس اور آڈٹ ثبوت کی وضاحت کرتا ہے۔ |

## رول آؤٹ اسنیپ شاٹ

| مرحلہ | فوکس | اخراجی معیار |
|-------|-------|--------------------|
| N0 - بند بیٹا | Bureau d'enregistrement (`.sora`) et magasin de catalogue de voies | دستخط شدہ DS مینفسٹس + گورننس ہینڈ آفز کی مشق۔ |
| N1 - عوامی لانچ | `.nexus` Bureau d'enregistrement en libre-service Câblage de règlement XOR pour les bureaux d'enregistrement | synchronisation du résolveur/de la passerelle pour la réconciliation et les exercices de table de litige |
| N2 - توسیع | `.dao`, API de revendeur, analyses, portail de litige, cartes de pointage de steward متعارف کراتا ہے۔ | artefacts de conformité et boîte à outils du jury politique et rapports sur la transparence du Trésor |
| Porte NX-12/13/14 | moteur de conformité, tableaux de bord de télémétrie et documentation pour le client | [Présentation Nexus](./nexus-overview) + [Opérations Nexus](./nexus-operations) شائع، tableaux de bord câblés, moteur de stratégie fusionné۔ |

## آپریٹر کی ذمہ داریاں1. ** کنفیگ حفظانِ صحت** - `config/config.toml` کو شائع شدہ voie et espace de données کیٹلاگ کے ساتھ ہم آہنگ رکھیں؛ ہر ریلیز ٹکٹ کے ساتھ `--trace-config` آؤٹ پٹ محفوظ کریں۔
2. **مینفسٹ ٹریکنگ** - شامل ہونے یا نوڈ اپ گریڈ سے پہلے کیٹلاگ انٹریز کو تازہ ترین Space Directory بَنڈل سے ہم آہنگ کریں۔
3. **ٹیلیمیٹری کوریج** - `nexus_lanes.json`, `nexus_settlement.json` pour le SDK qui expose les fichiers. L'application PagerDuty propose une solution de remédiation pour les personnes handicapées.
4. **حادثہ رپورٹنگ** - [Opérations Nexus](./nexus-operations) میں سیوریٹی میٹرکس کی پیروی کریں اور La prise en charge de la prise en charge RCA par RCA
5. ** گورننس تیاری** - اپنی lanes پر اثر انداز ہونے والی Nexus کونسل وووٹس میں حصہ لیں اور سہ ماہی rollback ہدایات کی مشق کریں (ٹریکنگ `docs/source/project_tracker/nexus_config_deltas/` کے ذریعے ہوتی ہے)۔

## مزید دیکھیں

- Nom du produit : `docs/source/nexus_overview.md`
- Nom du produit : [./nexus-spec](./nexus-spec)
- Voie جیومیٹری : [./nexus-lane-model](./nexus-lane-model)
- ٹرانزیشن پلان : [./nexus-transition-notes](./nexus-transition-notes)
- ٹیلیمیٹری remédiation پلان : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook disponible : [./nexus-operations](./nexus-operations)
- Nom d'intégration : `docs/source/sora_nexus_operator_onboarding.md`
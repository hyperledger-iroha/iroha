---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de connexion
titre : Nexus آپریشنز رن بُک
description: Nexus آپریٹر ورک فلو کا فیلڈ کے لئے تیار خلاصہ، `docs/source/nexus_operations.md` کی عکاسی کرتا ہے۔
---

Il s'agit d'un `docs/source/nexus_operations.md` qui est en train de se connecter à votre compte bancaire. Il s'agit d'un crochet pour crochets et d'un crochet pour crochets. سمیٹتا ہے جن پر Nexus آپریٹرز کو عمل کرنا ہوتا ہے۔

## لائف سائیکل چیک لسٹ

| مرحلہ | اقدامات | ثبوت |
|-------|--------|--------------|
| پری فلائٹ | ریلیز ہیشز/سگنیچرز کی تصدیق کریں، `profile = "iroha3"` کنفرم کریں، اور کنفیگ ٹیمپلیٹس تیار کریں۔ | `scripts/select_release_profile.py` La somme de contrôle est en cours de mise à jour |
| کیٹلاگ الائنمنٹ | `[nexus]` کیٹلاگ، روٹنگ پالیسی اور DA تھریش ہولڈز کو کونسل کے جاری کردہ مینفسٹ کے مطابق اپ ڈیٹ کریں، پھر `--trace-config` کیپچر کریں۔ | `irohad --sora --config ... --trace-config` آؤٹ پٹ جو onboarding ٹکٹ کے ساتھ محفوظ ہے۔ |
| اسموک اور کٹ اوور | `irohad --sora --config ... --trace-config` Câble CLI (`FindNetworkStatus`) Câble de commande CLI کریں، اور ایڈمیشن کی درخواست دیں۔ | اسموک ٹیسٹ لاگ + Alertmanager کنفرمیشن۔ |
| اسٹیڈی اسٹیٹ | tableaux de bord/alertes configuration de la cadence et de la cadence et configurations/runbooks ہم آہنگ کریں۔ | Vous avez besoin de plus d'informations sur les IDs۔ |

Intégration des tâches (embarquement des utilisateurs) `docs/source/sora_nexus_operator_onboarding.md` میں موجود ہے۔

## تبدیلی کا انتظام1. **ریلیز اپ ڈیٹس** - `status.md`/`roadmap.md` میں اعلانات ٹریک کریں؛ ہر ریلیز PR کے ساتھ onboarding چیک لسٹ منسلک کریں۔
2. **Lane مینفسٹ تبدیلیاں** - Space Directory est disponible en ligne `docs/source/project_tracker/nexus_config_deltas/` کے تحت محفوظ کریں۔
3. ** کنفیگریشن ڈیلٹاز** - `config/config.toml` میں ہر تبدیلی کے لئے voie/espace de données کا حوالہ دینے والا ٹکٹ ضروری ہے۔ جب نوڈز شامل ہوں یا اپ گریڈ ہوں تو موثر کنفیگ کی ریڈیکٹڈ کاپی محفوظ کریں۔
4. **Exercices de restauration** – pour arrêter/restaurer/fumer. نتائج `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` میں لاگ کریں۔
5. **Approbations de conformité** - voies privées/CBDC `docs/source/cbdc_lane_playbook.md`).

## ٹیلیمیٹری اور SLO- Tableaux de bord : `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, et SDK مخصوص ویوز (مثلاً `android_operator_console.json`).
- Alertes : `dashboards/alerts/nexus_audit_rules.yml` et Torii/Norito transport قواعد (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- دیکھنے کے لئے میٹرکس:
  - `nexus_lane_height{lane_id}` - Les emplacements pour machines à sous sont disponibles en ligne
  - `nexus_da_backlog_chunks{lane_id}` - voie مخصوص تھریش ہولڈز سے اوپر الرٹ کریں (ڈیفالٹ 64 publiques / 8 privées).
  - `nexus_settlement_latency_seconds{lane_id}` - Pour P99 900 ms (public) et 1200 ms (privé)
  - `torii_request_failures_total{scheme="norito_rpc"}` - Taux d'erreur de 5 mois > 2 % et taux d'erreur de 5 mois
  - `telemetry_redaction_override_total` - 2 septembre L'obligation de conformité remplace la conformité par rapport à la conformité.
- [Plan de remédiation de télémétrie Nexus] (./nexus-telemetry-remediation) میں دیا گیا چیک لسٹ کم از کم سہ ماہی چلائیں اور بھرا ہوا فارم آپریشنز ریویو نوٹس کے ساتھ منسلک کریں۔

## انسیڈنٹ میٹرکس| شدت | تعریف | ردعمل |
|--------------|------------|--------------|
| 1 septembre | isolement de l'espace de données et règlement en 15 ans de gouvernance et de gouvernance | Nexus Primaire + Ingénierie des versions + Conformité کو پیج کریں، ایڈمیشن فریز کریں، آرٹیفیکٹس جمع کریں، 30 ans de déploiement | Nexus Primaire + SRE کو پیج کریں، <=4 گھنٹے مٹیگیٹ کریں، 2 کاروباری دن کے اندر suivis فائل کریں۔ |
| 3 septembre | غیر رکاوٹی drift (docs، alertes). | tracker pour les tâches de sprint et de réparation pour les tâches de sprint |

Il existe plusieurs ID de voie/espace de données, ainsi que des identifiants de voie/espace de données. اور suivi ٹاسکس/مالکان درج ہونا ضروری ہیں۔

## ثبوت آرکائیو

- exports de bundles/manifestes/télémétrie comme `artifacts/nexus/<lane>/<date>/` pour les exportations
- ہر ریلیز کے لئے expurgé configs + `--trace-config` آؤٹ پٹ محفوظ رکھیں۔
- جب config یا مینفسٹ تبدیلیاں ہوں تو کونسل منٹس + دستخط شدہ فیصلے منسلک کریں۔
- Nexus prend en charge les instantanés Prometheus et instantanés 12 fois plus
- Modifications du runbook `docs/source/project_tracker/nexus_config_deltas/README.md` میں ریکارڈ کریں تاکہ آڈیٹرز جان سکیں ذمہ داریاں کب بدلیں۔

## متعلقہ مواد- Présentation : [Nexus présentation](./nexus-overview)
- Spécification : [Spécification Nexus] (./nexus-spec)
- Géométrie de la voie : [Modèle de voie Nexus](./nexus-lane-model)
- Cales de transition et de routage : [Notes de transition Nexus](./nexus-transition-notes)
- Intégration de l'opérateur : [intégration de l'opérateur Sora Nexus] (./nexus-operator-onboarding)
- Correction de télémétrie : [Plan de correction de télémétrie Nexus](./nexus-telemetry-remediation)
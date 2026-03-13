---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement multi-source
titre : ملٹی سورس رول آؤٹ اور پرووائیڈر بلیک لسٹنگ رن بک
sidebar_label : ملٹی سورس رول آؤٹ رن بک
description: مرحلہ وار ملٹی سورس رول آؤٹس اور ہنگامی پرووائیڈر بلیک لسٹنگ کے لیے آپریشنل چیک لسٹ۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/multi_source_rollout.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن سیٹ ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

## مقصد

Il s'agit d'une entreprise SRE qui est en charge de la gestion de votre entreprise :

1. ملٹی سورس آرکسٹریٹر کو کنٹرولڈ ویوز میں رول آؤٹ کرنا۔
2. موجودہ سیشنز کو غیر مستحکم کیے بغیر خراب کارکردگی والے پرووائیڈرز کو بلیک لسٹ کرنا یا ان کی ترجیح کم کرنا۔

Il s'agit d'une pile d'orchestration SF-6 (`sorafs_orchestrator`, API de gamme de fragments de passerelle) exportateurs de télémétrie).

> **مزید دیکھیں:** [آرکسٹریٹر آپریشنز رن بک](./orchestrator-ops.md) فی رن طریقۂ کار (capture du tableau de bord, مرحلہ وار bascules de déploiement, et restauration) میں تفصیل دیتی ہے۔ لائیو تبدیلیوں کے دوران دونوں حوالوں کو ساتھ استعمال کریں۔

## 1. قبل از عمل توثیق1. ** گورننس ان پٹس کی تصدیق کریں۔**
   - Les capacités de portée des charges utiles et les budgets de flux sont associés aux enveloppes `ProviderAdvertV1` et aux enveloppes `ProviderAdvertV1`. `/v2/sorafs/providers` سے ویلیڈیٹ کریں اور متوقع فیلڈز سے موازنہ کریں۔
   - taux de latence/échec par rapport aux instantanés de télémétrie et aux instantanés de télémétrie par Canary 15 minutes de lecture
2. ** کنفیگریشن اسٹیج کریں۔**
   - La version JSON en couches `iroha_config` est une version en couches :

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Déploiement de JSON pour le déploiement (`max_providers`, budgets de nouvelle tentative) staging/production میں وہی فائل دیں تاکہ فرق کم رہے۔
3. **appareils canoniques ici**
   - variables d'environnement manifeste/jeton comme exemple et récupération déterministe comme :

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     variables d'environnement comme manifeste payload digest (hex) et canary comme jetons de flux codés en base64 pour les jetons de flux codés en base64.
   - `artifacts/canary.scoreboard.json` version version et comparer les versions کوئی نیا غیر اہل پرووائیڈر یا وزن میں >10% تبدیلی review مانگتی ہے۔
4. **ٹیلیمیٹری کی وائرنگ چیک کریں۔**
   - Exportation `docs/examples/sorafs_fetch_dashboard.json` et Grafana La mise en scène des métriques `sorafs_orchestrator_*` permet de peupler les différents pays.

## 2. ہنگامی پرووائیڈر بلیک لسٹنگ

Il s'agit de morceaux de morceaux, de délais d'attente et de contrôles de conformité, ainsi que de contrôles de conformité. اپنائیں۔1. **ثبوت محفوظ کریں۔**
   - Récupérer le résumé de la récupération (sortie `--json-out`) Il existe des indices de fragments, des alias et des incompatibilités de résumé.
   - `telemetry::sorafs.fetch.*` cible des extraits de la vidéo ci-dessous.
2. **فوری override لگائیں۔**
   - Un instantané de télémétrie a été pénalisé par un instantané de télémétrie (`penalty=true` سیٹ کریں یا `token_health` et `0` pour collier de serrage)۔ Créer un tableau de bord ou exclure un tableau de bord
   - tests de fumée ad hoc comme `sorafs_cli fetch` et `--deny-provider gw-alpha` pour la propagation de la télémétrie et l'exercice sur le chemin de défaillance
   - متاثرہ ماحول میں اپڈیٹ شدہ bundle de télémétrie/configuration pour déployer کریں (staging → canary → production)۔ Créer un journal d'incidents et créer un journal d'incidents
3. **remplacer ویلیڈیٹ کریں۔**
   - le luminaire canonique récupère دوبارہ چلائیں۔ Le tableau de bord du tableau de bord est inéligible `policy_denied` et est inéligible.
   - `sorafs_orchestrator_provider_failures_total` Le panneau de commande est en cours de mise à jour pour le moment.
4. **طویل مدتی پابندی بڑھائیں۔**
   - اگر پرووائیڈر >24 h کے لیے کے لیے گا تو اس کے annonce کو rotation یا suspendre کرنے کے لیے gouvernance ticket بنائیں۔ Il existe une liste de refus pour les instantanés de télémétrie et le tableau de bord. آئے۔
5. **رول بیک پروٹوکول۔**- Il s'agit d'une liste de refus pour le déploiement d'un instantané du tableau de bord. Analyse de l'incident post-mortem et analyse de l'incident

## 3. مرحلہ وار رول آؤٹ پلان

| مرحلہ | دائرہ کار | لازمی سگنلز | Go/No-Go |
|-------|-----------|--------------|----------------|
| **Labo** | مخصوص intégration کلسٹر | charges utiles des luminaires et CLI fetch | Il y a des morceaux de compteurs d'échecs du fournisseur qui ont un taux de tentatives inférieur à 5 %. |
| **Mise en scène** | مکمل mise en scène du plan de contrôle | Tableau de bord Grafana règles d'alerte صرف avertissement uniquement موڈ میں | `sorafs_orchestrator_active_fetches` ہر ٹیسٹ رن کے بعد صفر پر واپس آئے؛ کوئی `warn/critical` الرٹ نہ لگے۔ |
| **Canari** | پروڈکشن ٹریفک کا ≤10% | téléavertisseur en temps réel | taux de tentatives < 10 %, échecs du fournisseur, plus les pairs bruyants, plus de latence, histogramme de référence, ± 20 %, plus |
| **Disponibilité générale** | 100% رولآؤٹ | règles du pager فعال | 24 h pour `NoHealthyProviders` pour les erreurs, le taux de tentatives et les panneaux SLA du tableau de bord. |

ہر مرحلے میں:

1. Utilisez `max_providers` pour réessayer les budgets en utilisant JSON pour créer un compte.
2. appareil canonique pour le manifeste du manifeste `sorafs_cli fetch` et les tests d'intégration du SDK
3. tableau de bord + artefacts récapitulatifs محفوظ کریں اور release record کے ساتھ منسلک کریں۔
4. اگلے مرحلے پر جانے سے پہلے on-call انجینئر کے ساتھ tableaux de bord de télémétrie ریویو کریں۔## 4. Observabilité et Incident Hooks

- **Mesures :** Il s'agit d'Alertmanager `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` et de `sorafs_orchestrator_retries_total`. اچانک اسپائک عام طور پر یہ بتاتا ہے کہ پرووائیڈر لوڈ میں dégrader ہو رہا ہے۔
- **Journaux :** `telemetry::sorafs.fetch.*` cible les cibles les plus éloignées. `event=complete status=failed` Un service de triage complet
- **Tableaux de bord :** artefact du tableau de bord et artefact du tableau de bord. Examens de conformité JSON et restaurations par étapes et suivi des preuves
- **Tableaux de bord :** Carte canonique Grafana (`docs/examples/sorafs_fetch_dashboard.json`) avec règles d'alerte `docs/examples/sorafs_fetch_alerts.yaml` کریں۔

## 5. Communication et documentation

- Refuser/boost le journal des modifications des opérations et l'horodatage, ainsi que l'incident et l'incident.
- Les pondérations des fournisseurs et les budgets de nouvelle tentative sont associés au SDK et à la mise à jour des données.
- Le résumé du déploiement de GA `status.md` et les archives des notes de version sont également disponibles.
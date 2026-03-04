---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : réglage de l'orchestrateur
titre : آرکسٹریٹر رول آؤٹ اور ٹیوننگ
sidebar_label : آرکسٹریٹر ٹیوننگ
description: ملٹی سورس آرکسٹریٹر کو GA تک لے جانے کے لیے عملی ڈیفالٹس، ٹیوننگ رہنمائی، اور آڈٹ چیک پوائنٹس۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/developer/orchestrator_tuning.md` کی عکاسی کرتا ہے۔ جب تک پرانی ڈاکیومنٹیشن مکمل طور پر ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

# آرکسٹریٹر رول آؤٹ اور ٹیوننگ گائیڈ

یہ گائیڈ [کنفیگریشن ریفرنس](orchestrator-config.md) اور
[ملٹی سورس رول آؤٹ رن بک](multi-source-rollout.md) پر مبنی ہے۔ یہ وضاحت کرتی ہے
کہ ہر رول آؤٹ مرحلے میں آرکسٹریٹر کو کیسے ٹیون کیا جائے، scoreboard
آرٹیفیکٹس کو کیسے سمجھا جائے، اور ٹریفک بڑھانے سے پہلے کون سے ٹیلیمیٹری سگنلز
لازمی ہیں۔ Les SDK CLI sont également disponibles pour tous les utilisateurs.
ہر نوڈ ایک ہی ڈٹرمنسٹک fetch پالیسی پر عمل کرے۔

## 1. بنیادی پیرامیٹر سیٹس

ایک مشترکہ کنفیگریشن ٹیمپلیٹ سے آغاز کریں اور رول آؤٹ کے ساتھ چند منتخب boutons
کو ایڈجسٹ کریں۔ نیچے جدول عام مراحل کے لیے تجویز کردہ اقدار دکھاتا ہے؛
جو اقدار درج نہیں، وہ `OrchestratorConfig::default()` et `FetchOptions::default()`
کے ڈیفالٹس پر واپس جاتی ہیں۔

| مرحلہ | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | نوٹس |
|-------|-----------------|-------------------------------|------------------------------------|-----------------------------|-----------------------------------------|-------|
| **Laboratoire / CI** | `3` | `2` | `2` | `2500` | `300` | Le plafond de latence est activé par la fenêtre de grâce et la fenêtre de grâce est également disponible. réessais کم رکھیں تاکہ غلط manifeste جلد سامنے آئیں۔ |
| **Mise en scène** | `4` | `3` | `3` | `4000` | `600` | Les pairs exploratoires sont en contact avec des pairs exploratoires. |
| **Canari** | `6` | `3` | `3` | `5000` | `900` | ڈیفالٹس کے مطابق؛ `telemetry_region` سیٹ کریں تاکہ ڈیش بورڈز کینری ٹریفک الگ دکھا سکیں۔ |
| **Disponibilité générale (GA)** | `None` (demande de paiement éligible) | `4` | `4` | `5000` | `900` | Les seuils de tentative de nouvelle tentative et les seuils d'échec sont définis comme suit. رکھیں۔ |

- `scoreboard.weight_scale` pour `10_000` pour la résolution en aval et la résolution entière pour la résolution en aval اسکیل بڑھانے سے fournisseur de commande نہیں بدلتی؛ صرف زیادہ گھنا credit distribution بنتا ہے۔
- Il s'agit d'un bundle JSON fourni avec un `--scoreboard-out` pour un paquet de fichiers JSON. میں درست پیرامیٹر سیٹ ریکارڈ ہو۔

## 2. Tableau de bord کی ہائیجین

Tableau de bord manifeste pour les annonces des fournisseurs et pour les annonces de fournisseurs
آگے بڑھنے سے پہلے:1. **ٹیلیمیٹری کی تازگی چیک کریں۔** یقینی بنائیں کہ `--telemetry-json` میں حوالہ شدہ instantanés
   مقررہ grace window کے اندر ریکارڈ ہوئے ہوں۔ `telemetry_grace_secs` سے پرانی entrées
   `TelemetryStale { last_updated }` est en cours de réalisation اسے hard stop سمجھیں اور
   ٹیلیمیٹری ایکسپورٹ اپڈیٹ کیے بغیر آگے نہ بڑھیں۔
2. **Éligibilité دیکھیں۔** `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`
   کے ذریعے آرٹیفیکٹس محفوظ کریں۔ ہر entrée میں `eligibility` بلاک ہوتا ہے جو ناکامی
   کی درست وجہ بتاتا ہے۔ inadéquation des capacités avec les annonces expirées
   charge utile en amont
3. **Poids pour le produit** `normalised_weight` pour la version de libération pour le produit
   10% سے زیادہ تبدیلیاں جان بوجھ کر publicité/télémétrie سے ہم آہنگ ہونی چاہئیں
   اور journal de déploiement میں نوٹ ہونی چاہئیں۔
4. **آرٹیفیکٹس آرکائیو کریں۔** `scoreboard.persist_path` سیٹ کریں تاکہ ہر رن میں
   فائنل instantané du tableau de bord محفوظ ہو۔ اسے release ریکارڈ کے ساتھ manifest اور
   bundle de télémétrie کے ساتھ جوڑیں۔
5. **Mélange de fournisseurs pour les métadonnées et les métadonnées** `scoreboard.json`
   `summary.json` et `provider_count`, `gateway_provider_count` et `provider_mix`
   لیبل لازماً ہو تاکہ لینے والے ثابت کر سکیں کہ رن `direct-only`, `gateway-only`
   par `mixed` La passerelle capture les éléments `provider_count=0` et `provider_mix="gateway-only"`
   ہونا چاہیے، جبکہ mixte رنز میں دونوں سورسز کے لیے غیر صفر compte ضروری ہیں۔
   `cargo xtask sorafs-adoption-check` Il y a une différence entre les deux (non-concordance entre les deux)
   Il s'agit d'un script de capture `ci/check_sorafs_orchestrator_adoption.sh`.
   Ensemble de preuves `adoption_report.json` en stock Les passerelles Torii sont disponibles
   `gateway_manifest_id`/`gateway_manifest_cid` pour l'adoption des métadonnées du tableau de bord
   enveloppe de manifeste de porte et mélange de fournisseurs capturé

فیلڈز کی تفصیلی تعریف کے لیے
`crates/sorafs_car/src/scoreboard.rs` et structure récapitulative CLI (ou `sorafs_cli fetch --json-out`
سے نکلتی ہے) دیکھیں۔

## CLI et SDK en cours de réalisation

`sorafs_cli fetch` (دیکھیں `crates/sorafs_car/src/bin/sorafs_cli.rs`)
Wrapper `iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) ici
surface de configuration de l'orchestrateur preuve de déploiement - canonique
replay des rencontres کرنے کے لیے یہ flags استعمال کریں:

Référence d'indicateur multi-source partagée (aide CLI et documentation pour la synchronisation des fichiers) :- Fournisseurs éligibles `--max-peers=<count>` et tableau de bord des fournisseurs éligibles Les fournisseurs éligibles proposent un flux de données selon `1` pour une solution de secours à source unique آزمانا ہو۔ SDK comme le bouton `maxPeers` et les boutons (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` `FetchOptions` limite de tentatives par fragment Il s'agit d'un déploiement de déploiement complet et d'un déploiement complet. preuves que les valeurs par défaut du SDK et les paramètres CLI sont également disponibles.
- `--telemetry-region=<label>` Prometheus `sorafs_orchestrator_*` Relais (relais OTLP) dans la région/env pour le laboratoire, la mise en scène, Canary اور GA ٹریفک الگ کر سکیں۔
- Tableau de bord `--telemetry-json=<path>` pour injection d'instantané référencé JSON et le tableau de bord sont configurés pour replay (`cargo xtask sorafs-adoption-check --require-telemetry` ou `cargo xtask sorafs-adoption-check --require-telemetry`). Il s'agit d'une capture de flux OTLP et d'un flux de données.
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) crochets d'observation du pont Il s'agit d'un orchestrateur pour le proxy Norito/Kaigi et d'un flux de morceaux, ainsi que de clients de navigateur, de caches de garde et de salles Kaigi, ainsi que de reçus. جو Rust émet کرتا ہے۔
- `--scoreboard-out=<path>` (اختیاری `--scoreboard-now=<unix_secs>` کے ساتھ) instantané d'éligibilité محفوظ کرتا ہے۔ Il existe un ticket de sortie JSON et un ticket de sortie, ainsi qu'une télémétrie référencée et des artefacts manifestes.
- Métadonnées publicitaires `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` et métadonnées déterministes Il s'agit de drapeaux et de répétitions. Les déclassements et les artefacts de gouvernance sont en cours de mise à niveau et l'ensemble de politiques est en cours.
- `--provider-metrics-out` / `--chunk-receipts-out` métriques de santé par fournisseur et reçus en bloc preuves d'adoption جمع کرتے وقت دونوں artefacts ضرور شامل کریں۔

مثال (شائع شدہ luminaire استعمال کرتے ہوئے):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK pour la configuration et `SorafsGatewayFetchOptions` pour le client Rust
(`crates/iroha/src/client.rs`) et liaisons JS (`javascript/iroha_js/src/sorafs.js`) ici
Swift SDK (`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`) est un outil de développement rapide
Les assistants et les valeurs par défaut de la CLI sont les étapes de verrouillage et l'automatisation de l'automatisation.
Fabrication de produits sur mesure pour fabrication de produits sur mesure

## 3. Récupérer پالیسی ٹیوننگ

`FetchOptions` tentatives, concurrence et vérification et vérification de la concurrence ٹیوننگ کے وقت:- **Nouvelles tentatives :** `per_chunk_retry_limit` et `4` sont en cours de récupération et de récupération des erreurs du fournisseur. Pour `4`, le plafond est en place et la rotation des prestataires est en place pour les artistes interprètes ou exécutants.
- **Seuil d'échec :** `provider_failure_threshold` est désactivé par le fournisseur et est désactivé. Le nombre de nouvelles tentatives est important pour les tentatives d'orchestration : le budget de nouvelle tentative est défini par l'orchestrateur pour les nouvelles tentatives. pair کو نکال دیتا ہے۔
- **Concurrence :** `global_parallel_limit` et `None` permettent de définir les plages annoncées et de saturer les plages annoncées. Il y a beaucoup de fournisseurs de budgets de flux et de famine ≤ ہو تاکہ famine
- **Activations de vérification :** `verify_lengths` et `verify_digests` پروڈکشن میں فعال رہنے چاہئیں۔ یہ flottes de fournisseurs mixtes et déterminisme کی ضمانت دیتے ہیں؛ انہیں صرف environnements fuzzing isolés میں ہی بند کریں۔

## 4. ٹرانسپورٹ اور انانیمٹی اسٹیجنگ

La posture est définie par `rollout_phase`, `anonymity_policy`, et `transport_policy`:

- `rollout_phase="snnet-5"` pour la politique d'anonymat par défaut et les jalons SNNet-5 pour la politique d'anonymat par défaut `anonymity_policy_override` صرف تب استعمال کریں جب gouvernance نے directive signée جاری کیا ہو۔
- `transport_policy="soranet-first"` et ligne de base pour SNNet-4/5/5a/5b/6a/7/8/12/13 🈺 Oui
  (دیکھیں `roadmap.md`). `transport_policy="direct-only"` Déclassements et exercices de conformité pour l'examen de la couverture PQ `transport_policy="soranet-strict"` جائیں—اس سطح پر صرف relais classiques رہیں تو فوری échec
- `write_mode="pq-only"` صرف اسی وقت نافذ کریں جب ہر chemin d'écriture (SDK, orchestrateur, outils de gouvernance) PQ تقاضے پورے کر سکے۔ déploiements pour les routes directes `write_mode="allow-downgrade"` pour les routes directes en ligne pour la rétrogradation de la télémétrie et les nouvelles versions کرے۔
- Sélection de garde et mise en scène du circuit Répertoire SoraNet pour les utilisateurs signé `relay_directory` snapshot فراہم کریں اور `guard_set` cache محفوظ کریں تاکہ guard churn طے شدہ retention window میں رہے۔ `sorafs_cli fetch` est une preuve de déploiement d'empreintes digitales en cache et est en cours de mise en cache.

## 5. Rétrograder les hooks de conformité

دو ذیلی نظام دستی مداخلت کے بغیر پالیسی نافذ کرنے میں مدد دیتے ہیں:

- **Remédiation vers le bas** (`downgrade_remediation`) : `handshake_downgrade_total` et `window_secs` et `threshold`. Il s'agit d'un proxy local `target_mode` pour un proxy local (métadonnées uniquement) ڈیفالٹس (`threshold=3`, `window=300`, `cooldown=900`) برقرار رکھیں جب تک انسیڈنٹ ریویوز مختلف پیٹرن نہ بتائیں۔ Il s'agit d'un remplacement du journal de déploiement qui s'applique également à `sorafs_proxy_downgrade_state`.
- **Politique de conformité** (`compliance`) : juridiction et exclusions manifestes, listes de désinscription gérées par la gouvernance Un bundle de remplacements ad hoc est également disponible. Il s'agit d'une mise à jour signée `governance/compliance/soranet_opt_outs.json` et d'un JSON généré pour le déploiement.Il s'agit d'un paquet de preuves pour libérer des preuves. Un déclassement déclenche un déclassement

## 6. ٹیلیمیٹری اور ڈیش بورڈز

رول آؤٹ بڑھانے سے پہلے تصدیق کریں کہ درج ذیل سگنلز target ماحول میں فعال ہیں:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  کینری مکمل ہونے کے بعد صفر ہونا چاہیے۔
- `sorafs_orchestrator_retries_total` ici
  `sorafs_orchestrator_retry_ratio` — Il y a 10 % de plus que 5 % de GA à 5 % de plus
- `sorafs_orchestrator_policy_events_total` — Étape de déploiement en cours pour les coupures de courant (étiquette `stage`) et `outcome` pour les baisses de tension کرتا ہے۔
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — Relais PQ pour relais PQ
- `telemetry::sorafs.fetch.*` pour le téléphone — مشترکہ لاگ ایگریگیٹر پر جائیں اور `status=failed` کے لیے محفوظ تلاشیں ہوں۔

`dashboards/grafana/sorafs_fetch_observability.json` et tableau de bord canonique Grafana.
(SoraFS → Fetch Observability** pour les sélecteurs de région/manifeste)
Carte thermique de nouvelle tentative du fournisseur, histogrammes de latence de fragments et compteurs de décrochage, ainsi que SRE
burn-ins Règles Alertmanager par `dashboards/alerts/sorafs_fetch_rules.yml`
Utilisez la syntaxe `scripts/telemetry/test_sorafs_fetch_alerts.sh` et Prometheus
ویلیڈیٹ کریں (helper `promtool test rules` کو لوکل یا Docker میں چلاتا ہے)۔ Transferts d’alertes
Le routage est également un outil de routage et un déploiement de déploiement
ٹکٹ سے جوڑ سکیں۔

### ٹیلیمیٹری burn-in ورک فلو

Élément de la feuille de route **SF-6e** il y a 30 ans de rodage de télémétrie pour plus de 30 ans
ملٹی سورس آرکسٹریٹر GA ڈیفالٹس پر چلا جائے۔ ریپو اسکرپٹس کے ذریعے ہر دن کے لیے
ریپروڈیوس ایبل آرٹیفیکٹس بنائیں:

1. `ci/check_sorafs_orchestrator_adoption.sh` et boutons d'environnement de rodage pour les boutons de gravure مثال:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   Helper `fixtures/sorafs_orchestrator/multi_peer_parity_v1` ری پلے کرتا ہے،
   `scoreboard.json`, `summary.json`, `provider_metrics.json`, `chunk_receipts.json`,
   اور `adoption_report.json` et `artifacts/sorafs_orchestrator/<timestamp>/` میں لکھتا ہے،
   Selon `cargo xtask sorafs-adoption-check`, les fournisseurs éligibles appliquent les règles de sécurité.
2. Le burn-in est appliqué sur l'étiquette `burn_in_note.json` sur l'étiquette.
   index du jour, identifiant du manifeste, source de télémétrie et résumés d'artefacts اسے déploiement
   log میں شامل کریں تاکہ واضح ہو کہ 30 روزہ ونڈو کا کون سا دن کس capture سے پورا ہوا۔
3. اپڈیٹ شدہ Grafana بورڈ (`dashboards/grafana/sorafs_fetch_observability.json`) pour la mise en scène/production
   espace de travail étiquette gravée étiquette gravée et étiquette gravée
   manifeste/région کے نمونے دکھاتا ہے۔
4. Pour `dashboards/alerts/sorafs_fetch_rules.yml` pour `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   (`promtool test rules …`) Il existe des métriques de burn-in de routage d'alertes et des métriques de burn-in de routage d'alerte.
5. Sortie de test d'instantané et d'alerte `telemetry::sorafs.fetch.*` pour le test d'alerte
   La gouvernance des systèmes en direct et les mesures et les preuves
   ری پلے کر سکے۔

## 7. رول آؤٹ چیک لسٹ1. CI configuration des candidats et tableaux de bord ainsi que les artefacts et le contrôle de version
2. Fonctions (laboratoire, préparation, Canary, production) pour la récupération de luminaires déterministes et le déploiement des artefacts `--scoreboard-out` et `--json-out`. منسلک کریں۔
3. de garde Il existe de nombreuses métriques et des échantillons en direct.
4. Il s'agit d'un registre de gouvernance (il s'agit d'un `iroha_config`) et d'un registre de gouvernance avec git commit et d'un git commit. annonces اور conformité کے لیے استعمال ہوا۔
5. Rollout Tracker est un outil de suivi du SDK et un outil de suivi du déploiement. انٹیگریشنز سیدھ میں رہیں۔

اس گائیڈ کی پیروی آرکسٹریٹر ڈیپلائمنٹس کو ڈٹرمنسٹک اور آڈٹ ایبل رکھتی ہے جبکہ
budgets de nouvelle tentative, capacité du fournisseur, et posture de confidentialité
فیڈبیک لوپس فراہم کرتی ہے۔
---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-rollout-plan
title: خطة اطلاق ما بعد الكم SNNet-16G
sidebar_label : خطة اطلاق PQ
description : Il s'agit d'une poignée de main pour X25519+ML-KEM pour SoraNet avec Canary par défaut, pour les relais, les clients et les SDK.
---

:::note المصدر القياسي
Il s'agit de la référence `docs/source/soranet/pq_rollout_plan.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

SNNet-16G est compatible avec SoraNet. مفاتيح `rollout_phase` تسمح للمشغلين بتنسيق ترقية حتمية من متطلب guard for Stage A الى تغطية الاغلبية في Stage B ثم Le PQ est utilisé pour l'étape C pour utiliser JSON/TOML.

يغطي هذا playbook:

- تعريفات المراحل ومفاتيح التهيئة الجديدة (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) الموصولة في codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Les indicateurs de compatibilité avec le SDK et la CLI sont également disponibles pour le client lors du déploiement.
- Planification des relais/clients et gouvernance des relais (`dashboards/grafana/soranet_pq_ratchet.json`).
- Crochets pour la restauration et l'exercice d'incendie ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## خريطة المراحل

| `rollout_phase` | مرحلة اخفاء الهوية الفعلية | الاثر الافتراضي | الاستخدام المعتاد |
|-----------------|---------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Étape A) | فرض وجود guard PQ واحد على الاقل لكل circuit بينما تسخن المنظومة. | baseline واسابيع canari الاولى. |
| `ramp` | `anon-majority-pq` (Étape B) | تحيز الاختيار نحو relais PQ لتحقيق تغطية >= ثلثين؛ Il s'agit d'un relais de repli. | canaries حسب المناطق للـ relais؛ active/désactive le SDK. |
| `default` | `anon-strict-pq` (Étape C) | فرض circuits PQ فقط وتشديد انذارات downgrade. | الترقية النهائية بعد اكتمال télémétrie et gouvernance. |

اذا قام سطح ما ايضا بتعيين `anonymity_policy` صريحة فانها تتغلب على المرحلة لذلك المكون. حذف المرحلة الصريحة يجعلها تعتمد على قيمة `rollout_phase` حتى يتمكن المشغلون من تبديل المرحلة مرة واحدة لكل بيئة وترك clients يرثونها.

## مرجع التهيئة

### Orchestrateur (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Il s'agit d'un chargeur d'orchestrateur pour une solution de secours et d'un système de secours (`crates/sorafs_orchestrator/src/lib.rs:2229`) et d'un `sorafs_orchestrator_policy_events_total` et d'un `sorafs_orchestrator_pq_ratio_*`. Utilisez `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` pour votre appareil.

### Client Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` يسجل الان المرحلة المحللة (`crates/iroha/src/client.rs:2315`) pour le لاوامر المساعدة (مثل `iroha_cli app sorafs fetch`) ان تبلغ عن المرحلة الحالية مع سياسة اخفاء الهوية الافتراضية.

## الاتمتة

ادوات `cargo xtask` اثنان تقوم باتمتة توليد الجدول والتقاط artefacts.

1. **توليد الجدول الاقليمي**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Vérifiez les paramètres `s` et `m` et `h` et `d`. Utilisez le code `artifacts/soranet_pq_rollout_plan.json` et Markdown (`artifacts/soranet_pq_rollout_plan.md`) pour le faire.

2. **التقاط artefacts للتمرين مع التوقيعات**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   Il s'agit d'un artefact `artifacts/soranet_pq_rollout/<timestamp>_<label>/` et d'un résumé de BLAKE3 pour un artefact `rollout_capture.json`. على métadonnées وتوقيع Ed25519 على الحمولة. Il s'agit d'une clé privée pour l'exercice d'incendie et la gouvernance pour la sécurité.## Exemples de flags pour le SDK et la CLI

| السطح | Canari (stade A) | Rampe (étape B) | Par défaut (étape C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` récupérer | `--anonymity-policy stage-a` pour la prise en charge | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuration de l'orchestrateur JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuration du client Rust (`iroha.toml`) | `rollout_phase = "canary"` (par défaut) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Commandes signées `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, en option `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, en option `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, en option `.ANON_STRICT_PQ` |
| Aides de l'orchestrateur JavaScript | `rolloutPhase: "canary"` et `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python`fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rapide `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Pour basculer entre le SDK et l'analyseur de scène pour l'orchestrateur (`crates/sorafs_orchestrator/src/lib.rs:365`), vous pouvez activer/désactiver l'analyseur de scène et l'orchestrateur (`crates/sorafs_orchestrator/src/lib.rs:365`). مع المرحلة المهيئة.

## قائمة planification للكاناري

1. **Vérification en amont (T moins 2 semaines)**

- La baisse de tension au niveau de l'étape A correspond à 1 % de la baisse de tension et du PQ >=70 % (`sorafs_orchestrator_pq_candidate_ratio`).
   - جدولة slot لمراجعة gouvernance يوافق نافذة canari.
   - Utiliser `sorafs.gateway.rollout_phase = "ramp"` pour la mise en scène (pour l'orchestrateur JSON et pour l'exécution à sec) et pour l'exécution à sec.

2. **Relais canari (jour T)**

   - Il s'agit d'un orchestrateur de manifestations pour les relais `rollout_phase = "ramp"`.
   - Ajout de "Événements de politique par résultat" et "Taux de baisse de tension" pour le PQ Ratchet (déploiement de la fonction de mise en œuvre) et le cache de garde TTL.
   - Captures instantanées par `sorafs_cli guard-directory fetch` pour les captures d'écran et les captures d'écran.

3. **Canari client/SDK (T plus 1 semaine)**

   - Le client `rollout_phase = "ramp"` et le client remplace `stage-b` par le SDK.
   - Télémétrie des fonctions de télémétrie (`sorafs_orchestrator_policy_events_total` avec `client_id` et `region`) et déploiement du système.

4. **Promotion par défaut (T plus 3 semaines)**

   - Pour la gouvernance, pour l'orchestrateur et le client `rollout_phase = "default"` et la liste de contrôle pour les artefacts.

## قائمة gouvernance والادلة| تغيير المرحلة | بوابة الترقية | حزمة الادلة | Tableaux de bord وAlertes |
|--------------|----------------|-----------------|-----------|
| Canaries -> Rampe *(Aperçu de l'étape B)* | معدل brownout لمرحلة Stage A اقل من 1% خلال 14 يوما، `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 لكل منطقة تمت ترقيتها، تحقق Argon2 ticket p95 < 50 ms, وحجز slot للترقية في gouvernance. | Utiliser JSON/Markdown pour `cargo xtask soranet-rollout-plan`, snapshots ou `sorafs_cli guard-directory fetch` (en option) ou `cargo xtask soranet-rollout-capture --label canary` pour Canary Voir [PQ Ratchet Runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Événements de politique + taux de baisse de tension) et `dashboards/grafana/soranet_privacy_metrics.json` (taux de déclassement SN16) et télémétrie par `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Rampe -> Par défaut *(Application de l'étape C)* | Le burn-in dure 30 ans pour SN16, et `sn16_handshake_downgrade_total` est la ligne de base, et ` sorafs_orchestrator_brownouts_total` est pour canary. وتوثيق répétition للـ bascule proxy. | Utilisez `sorafs_cli proxy set-mode --mode gateway|direct` pour `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` ou `sorafs_cli guard-directory verify` ou `cargo xtask soranet-rollout-capture --label default`. | Pour PQ Ratchet, vous pouvez rétrograder SN16 avec `docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json`. |
| Rétrogradation d'urgence/préparation à la restauration | Vous pouvez également utiliser le répertoire de garde et le tampon `/policy/proxy-toggle` pour rétrograder. | Liste de contrôle pour `docs/source/ops/soranet_transport_rollback.md`, pour `sorafs_cli guard-directory import` / `guard-cache prune`, pour `cargo xtask soranet-rollout-capture --label rollback`, pour les tâches à accomplir. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, et est également disponible (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Il s'agit d'un artefact `artifacts/soranet_pq_rollout/<timestamp>_<label>/` et d'un `rollout_capture.json` pour la gouvernance, le tableau de bord, les traces et les résumés de PromTool.
- ارفق digests SHA256 للادلة المرفوعة (minutes PDF, bundle de capture, instantanés de garde) Mise en scène.
- ارجع لخطة telemetry في تذكرة الترقية لاثبات ان `docs/source/soranet/snnet16_telemetry_plan.md` هي المصدر القياسي لمفردات downgrade وحدود التنبيه.

## تحديثات tableau de bord et télémétrie

`dashboards/grafana/soranet_pq_ratchet.json` Lire la suite "Plan de déploiement" pour lire le playbook et la gouvernance تاكيد المرحلة النشطة. Utilisez les boutons de commande.

بالنسبة للتنبيهات، تاكد من ان القواعد الحالية تستخدم تسمية `stage` حتى تثير مرحلتا canary وdefault حدود سياسة منفصلة (`dashboards/alerts/soranet_handshake_rules.yml`).

## Hooks pour la restauration

### Par défaut -> Rampe (Étape C -> Étape B)

1. Installez l'orchestrateur `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (et utilisez le SDK) pour Stage B pour votre ordinateur.
2. Configurer les clients avec `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` pour le flux de travail `/policy/proxy-toggle`.
3. Utilisez `cargo xtask soranet-rollout-capture --label rollback-default` pour comparer les différences avec guard-directory et promtool et les tableaux de bord avec `artifacts/soranet_pq_rollout/`.

### Rampe -> Canary (Étape B -> Étape A)1. Ajouter un instantané au répertoire guard-directory en utilisant le paquet `sorafs_cli guard-directory import --guard-directory guards.json` pour le paquet `sorafs_cli guard-directory verify` Hachages de qualité.
2. Utiliser `rollout_phase = "canary"` (remplacer par `anonymity_policy stage-a`) pour configurer l'orchestrateur pour le client ou utiliser la perceuse à cliquet PQ dans [runbook PQ Ratchet] (./pq-ratchet-runbook.md) لاثبات مسار déclassement.
3. ارفق لقطات PQ Ratchet et télémétrie SN16 pour la gouvernance.

### تذكيرات garde-corps

- Le `docs/source/ops/soranet_transport_rollback.md` est un outil de suivi du déploiement pour l'atténuation.
- Utilisez `dashboards/alerts/soranet_handshake_rules.yml` et `dashboards/alerts/soranet_privacy_rules.yml` pour créer un `promtool test rules` afin de restaurer la dérive en cas de dérive. حزمة capture.
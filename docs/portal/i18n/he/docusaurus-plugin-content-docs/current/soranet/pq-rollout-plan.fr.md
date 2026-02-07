---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
כותרת: Playbook deploiement post-quantique SNNet-16G
sidebar_label: Plan deploiement PQ
תיאור: מדריך תפעול לקידום לחיצת יד היברידית X25519+ML-KEM de SoraNet de canary ברירת מחדל על ממסרים, לקוחות ו-SDKs.
---

:::הערה מקור קנוניק
:::

SNNet-16G termine le deploiement post-quantique pour le transport SoraNet. Les knobs `rollout_phase` permettent aux operators de coordonner une promotion deterministe du demand guard Stage A vers la couverture majoritaire Stage B et la posture PQ stricte Stage C sans modifier du JSON/TOML brut pour chaque surface.

Ce playbook couvre:

- Definitions de phase et nouveaux knobs de configuration (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) כבלי בבסיס הקוד (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- מיפוי של SDK של דגלים ו-CLI על מנת לקבל לקוח חדש.
- Attentes de Scheduling Canary Relay/Client Plus Les Dashboards de governance qui gate la promotion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de Rollback et references au Runbook Drill Fire ([PQ Ratchet Runbook](./pq-ratchet-runbook.md)).

## Carte des phases

| `rollout_phase` | Etape d'anonymat יעיל | Effet par defaut | טיפוסי שימוש |
|----------------|------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (שלב א') | Exiger au moins un guard PQ par מעגל תליון que la flotte se rechauffe. | Baseline et premiere semaines de canary. |
| `ramp` | `anon-majority-pq` (שלב ב') | Favoriser la selection vers des relays PQ pour >= deux tiers de couverture; les relays classiques restent en fallback. | אזור ממסרים קנריים; מחליף את התצוגה המקדימה של SDK. |
| `default` | `anon-strict-pq` (שלב ג') | ייחודו של מעגלים PQ ועקבות אזעקות לשדרוג לאחור. | סיום קידום המכירות של טלמטריה וממשל החתימה הושלם. |

זה משטח definit aussi un `anonymity_policy` מפורש, elle override la phase pour ce composant. Omettre l'etape explicite deferre maintenance a la valeur `rollout_phase` afin que les operators puissent basculer une fois par environnement et laisser les clients l'heriter.

## עיון לתצורה

### תזמורת (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

הטוען של התזמורת יוצא ל-Tape Fallback בזמן ריצה (`crates/sorafs_orchestrator/src/lib.rs:2229`) וחשיפה דרך `sorafs_orchestrator_policy_events_total` et `sorafs_orchestrator_pq_ratio_*`. Voir `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` pour des snippets prett a appliquer.

### לקוח חלודה / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` הרשם לתחזוקה לפאזה פרסי (`crates/iroha/src/client.rs:2315`) אפין que les commandes helper (לדוגמה `iroha_cli app sorafs fetch`) פורץ כתב La phase actuelle aux cotes de la politique d'anonymat par defaut.

## אוטומציה

Deux helpers `cargo xtask` אוטומטית לדור לוח הזמנים וללכוד חפצי אמנות.

1. **כללי לוח הזמנים האזורי**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```Les durees acceptent les סיומות `s`, `m`, `h` או `d`. La commande emet `artifacts/soranet_pq_rollout_plan.json` ו-resume Markdown (`artifacts/soranet_pq_rollout_plan.md`) מצטרפים לדרישת שינוי.

2. **Capturer les artefacts du drill avec signatures**

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

   La commande copie les fichiers fournis dans `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcule des digests BLAKE3 pour chaque artefact et ecrit `rollout_capture.json` avec metadata plus une signature Ed25519 sur le payload. Utilisez la meme המפתח הפרטי que celle qui signe les minutes du-drill for que la governance puisse valider rapidement la capture.

## Matrice des flags SDK & CLI

| משטח | קנרי (שלב א') | רמפה (שלב ב') | ברירת מחדל (שלב C) |
|--------|----------------|----------------|------------------------|
| `sorafs_cli` אחזור | `--anonymity-policy stage-a` ou se reposer sur la phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| תצורת התזמורת JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| תצורת לקוח חלודה (`iroha.toml`) | `rollout_phase = "canary"` (ברירת מחדל) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` פקודות חתומות | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optionnel `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optionnel `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optionnel `.ANON_STRICT_PQ` |
| עוזרי מתזמר JavaScript | `rolloutPhase: "canary"` או `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

אם אתה מחליף את ה-SDK, נעשה שימוש במנתח זיכרון על הבמה עבור מתזמר (`crates/sorafs_orchestrator/src/lib.rs:365`).

## רשימת תזמון קנרית

1. **טיסה מוקדמת (T פחות שבועיים)**

- Confirmer que le brownout rate Stage A est <1% sur les deux semaines precedentes et que la couverture PQ est >=70% par region (`sorafs_orchestrator_pq_candidate_ratio`).
   - סקירת Planifier le slot de governance qui approuve la fenetre canary.
   - Mettre a jour `sorafs.gateway.rollout_phase = "ramp"` in staging (עורך JSON orchestrator and redployer) ו-dry run the pipeline de promotion.

2. **קנרית ממסר (יום ט')**

   - Promouvoir une region a la fois en definissant `rollout_phase = "ramp"` sur l'orchestrator et les manifests de relays משתתפי.
   - סקר "אירועי מדיניות לפי תוצאה" ו"שיעור חום" בלוח המחוונים PQ Ratchet (כולל תחזוקה להפעלת הפאנל) תליון מטמון TTL du guard.
   - Capturer des snapshots `sorafs_cli guard-directory fetch` avant et apres pour stockage d'audit.

3. **קנרית לקוח/SDK (T ועוד שבוע)**

   - Basculer `rollout_phase = "ramp"` dans les configs client ou passer des עוקף `stage-b` pour les cohorts SDK designees.
   - Capturer les diffs de telemetrie (`sorafs_orchestrator_policy_events_total` groupe par `client_id` et `region`) et les joindre au log d'incident de rollout.4. **מבצע ברירת מחדל (T ועוד 3 שבועות)**

   - אישור הממשל, מתזמר הבסיס וקונפיגורס הלקוח לעומת `rollout_phase = "default"` וטורנר פייר ל-Checklist של המוכנות החתימה על חפצי אמנות לשחרור.

## רשימת רשימת ממשל והוכחות

| שינוי דה פאזה | שער קידום מכירות | צרור ראיות | לוחות מחוונים והתראות |
|-------------|----------------|----------------|---------------------|
| קנרי -> רמפה *(תצוגה מקדימה של שלב ב')* | שיעור חום-אאוט שלב A <1% לאחר 14 ימים, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 פר אזור פרומו, אימות כרטיס Argon2 p95 < 50 ms, ועתודה לניהול משבצת. | צמד JSON/Markdown `cargo xtask soranet-rollout-plan`, מכשירי תמונות Snapshot `sorafs_cli guard-directory fetch` (avant/apres), חתום חבילה `cargo xtask soranet-rollout-capture --label canary`, ו-Reference canary minute [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (אירועי מדיניות + שיעור תקלות), `dashboards/grafana/soranet_privacy_metrics.json` (יחס שדרוג לאחור SN16), הפניות לטלמטריה ב-`docs/source/soranet/snnet16_telemetry_plan.md`. |
| רמפה -> ברירת מחדל *(אכיפה בשלב C)* | צריבה טלמטרית SN16 של 30 יממות תשומת לב, `sn16_handshake_downgrade_total` בבסיס קו הבסיס, `sorafs_orchestrator_brownouts_total` בעל אורך אפס ללקוח הקנרי, וחזרה של יומן החלפת פרוקסי. | תמלול `sorafs_cli proxy set-mode --mode gateway|direct`, מיון `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, יומן `sorafs_cli guard-directory verify`, וחתימה חבילה `cargo xtask soranet-rollout-capture --label default`. | Meme tableau PQ Ratchet פלוס les panneaux SN16 מסמכים לאחור ב-`docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json`. |
| הורדה בחירום / מוכנות לחזרה לאחור | Declenche quand les compteurs לשדרג לאחור את montent, la verification guard-directory echoue, ou le buffer `/policy/proxy-toggle` לרשום את אירועי השדרוג לאחור. | רשימת רשימות `docs/source/ops/soranet_transport_rollback.md`, יומנים `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, כרטיסים לאירוע ותבניות הודעה. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` et les deux packs d'alertes (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Stockez chaque artefact sous `artifacts/soranet_pq_rollout/<timestamp>_<label>/` avec le `rollout_capture.json` genere afin que les paquets governance contiennent le board, les traces promtool et les digests.
- Attachez les digests SHA256 des preuves chargees (דקות PDF, צרור לכידה, צילומי מצב של משמר) דקות נוספות של קידום ואישורים לפרלמנט puissent etre rejouees sans acces au cluster staging.
- Referencez le plan de telemetrie dans le ticket de promotion pour prouver que `docs/source/soranet/snnet16_telemetry_plan.md` reste la source canonique des vocabulaires de downgrade et des seuils d'alertes.

## לוחות מחוונים וטלמטריה של Mise a jour

`dashboards/grafana/soranet_pq_ratchet.json` כולל תחזוקה ופאנל ביאור "תוכנית השקה" qui revoie vers ce cebook and expos la phase actuelle afin que les revues governance confirmed quel stage est actif. Gardez la description du panel Syncee with les futures evolutions עם כפתורי התצורה.

Pour l'alerting, assurez-vous que les regles existantes utilisent le label `stage` afin que les phases canary et default declenchent des thresholds de politique separes (`dashboards/alerts/soranet_handshake_rules.yml`).

## Hooks de rollback

### ברירת מחדל -> רמפה (שלב C -> שלב ב')1. Retrogradez l'orchestrator avec `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (et faites miroiter la meme phase sur les configs SDK) pour que Stage B reprenne sur toute la flotte.
2. דחפו את הלקוחות לפרופיל התחבורה ב-`sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, ותופס את התמלול לפי זרימת העבודה של תיקון `/policy/proxy-toggle` ניתנת לביקורת.
3. Executez `cargo xtask soranet-rollout-capture --label rollback-default` pour Archiver les diffs guard-directory, la sortie promtool et les screenshots boards sos `artifacts/soranet_pq_rollout/`.

### רמפה -> קנרית (שלב ב' -> שלב א')

1. יבא את תמונת המצב של מדריך השמירה על לכידת קידום מכירות חדשני עם `sorafs_cli guard-directory import --guard-directory guards.json` ו-`sorafs_cli guard-directory verify` על מנת להוריד את החבילה בדרגה הכוללת את ה-hashes.
2. Definissez `rollout_phase = "canary"` (או עוקף avec `anonymity_policy stage-a`) sur l'orchestrator et les configs client, puis rejouez le PQ ratchet drill du [PQ ratchet runbook](./pq-ratchet-runbook.md) le pipeline down prograd.
3. צרף צילומי מסך מממשים את PQ Ratchet ו-SN16 טלמטרי שאליו יש תוצאות של התראות או יומן תקריות וניהול הודעות.

### מעקה הבטיחות של ראפלס

- Referencez `docs/source/ops/soranet_transport_rollback.md` להורדה בדרגה ורשומה כדי לצמצם את הזמן לפריט `TODO:` במעקב אחר הגלישה.
- Gardez `dashboards/alerts/soranet_handshake_rules.yml` et `dashboards/alerts/soranet_privacy_rules.yml` sous couverture `promtool test rules` avant et apres un rollback afin que toute d'alertes soit documentee avec le capture bundle.
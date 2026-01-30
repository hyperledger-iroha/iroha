---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4a7662c7edd2f4a8c0242017b0dc45f9b46477d892a1d06f2611e223aa2e6e48
source_last_modified: "2025-11-15T19:17:38.748600+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: pq-rollout-plan
title: Playbook לפריסת פוסט-קוונטום SNNet-16G
sidebar_label: תכנית פריסת PQ
description: מדריך תפעולי לקידום ה-handshake ההיברידי X25519+ML-KEM של SoraNet מ-canary ל-default על פני relays, clients ו-SDKs.
---

:::note מקור קנוני
דף זה משקף את `docs/source/soranet/pq_rollout_plan.md`. שמרו על שתי הגרסאות מסונכרנות עד שהדוקס הישנים יפרשו.
:::

SNNet-16G משלים את פריסת הפוסט-קוונטום עבור תעבורת SoraNet. ה-knobs של `rollout_phase` מאפשרים למפעילים לתאם קידום דטרמיניסטי מהדרישה הקיימת של Stage A guard אל כיסוי רוב Stage B ואל תנוחת PQ מחמירה של Stage C בלי לערוך JSON/TOML גולמי לכל משטח.

ה-playbook מכסה:

- הגדרות של שלבים וה-knobs החדשים של תצורה (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) שחוברו ל-codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- מיפוי דגלי SDK ו-CLI כדי שכל client יוכל לעקוב אחרי ה-rollout.
- ציפיות scheduling ל-canary של relay/client יחד עם dashboards של governance שמגדרים את הקידום (`dashboards/grafana/soranet_pq_ratchet.json`).
- hooks של rollback והפניות ל-runbook של fire-drill ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## מפת שלבים

| `rollout_phase` | שלב אנונימיות אפקטיבי | אפקט ברירת מחדל | שימוש טיפוסי |
|-----------------|---------------------------|----------------|---------------|
| `canary`        | `anon-guard-pq` (Stage A) | מחייב לפחות guard PQ אחד לכל circuit בזמן שהצי מתחמם. | baseline ושבועות canary מוקדמים. |
| `ramp`          | `anon-majority-pq` (Stage B) | מסיט בחירה לטובת relays PQ כדי להגיע ל- >= שני שלישים כיסוי; relays קלאסיים נשארים כ-fallback. | canaries אזוריים ל-relays; toggles ל-preview ב-SDK. |
| `default`       | `anon-strict-pq` (Stage C) | אוכף circuits של PQ בלבד ומחמיר התראות downgrade. | קידום סופי לאחר השלמת telemetry ואישור governance. |

אם משטח מגדיר גם `anonymity_policy` מפורש, הוא עוקף את ה-phase עבור אותו רכיב. השמטת השלב המפורש דוחה כעת לערך `rollout_phase` כדי שהמפעילים יוכלו להחליף phase פעם אחת בכל סביבה ולאפשר ל-clients לרשת.

## הפניות תצורה

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

ה-loader של ה-orchestrator פותר את שלב ה-fallback בזמן ריצה (`crates/sorafs_orchestrator/src/lib.rs:2229`) ומציג אותו דרך `sorafs_orchestrator_policy_events_total` ו-`sorafs_orchestrator_pq_ratio_*`. ראו `docs/examples/sorafs_rollout_stage_b.toml` ו-`docs/examples/sorafs_rollout_stage_c.toml` ל-snippets מוכנים להפעלה.

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` מתעד כעת את ה-phase המפורש (`crates/iroha/src/client.rs:2315`) כך שפקודות עזר (למשל `iroha_cli app sorafs fetch`) יכולות לדווח על ה-phase הנוכחי לצד מדיניות האנונימיות ברירת המחדל.

## אוטומציה

שני helpers של `cargo xtask` מאוטומטים יצירת schedule ולכידת artefacts.

1. **יצירת schedule אזורי**

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

   Durations מקבלות סיומות `s`, `m`, `h` או `d`. הפקודה מפיקה `artifacts/soranet_pq_rollout_plan.json` וסיכום Markdown (`artifacts/soranet_pq_rollout_plan.md`) שניתן לצרף לבקשת שינוי.

2. **לכידת artefacts של drill עם חתימות**

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

   הפקודה מעתיקה את הקבצים שסופקו אל `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, מחשבת digests של BLAKE3 לכל artefact וכותבת `rollout_capture.json` עם metadata וחתימת Ed25519 על ה-payload. השתמשו באותו private key שחותם על דקות ה-fire-drill כדי לאפשר ל-governance לאמת במהירות את הלכידה.

## מטריצת דגלי SDK ו-CLI

| משטח | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` או הסתמכות על ה-phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optional `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` או `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

כל toggles של SDK ממופים לאותו parser של stage שבו משתמש ה-orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`), כך שפריסות מרובות שפות נשארות ב-lock-step עם ה-phase המוגדר.

## Checklist לתזמון canary

1. **Preflight (T minus 2 weeks)**

- אשרו שמדד brownout של Stage A נמוך מ-1% בשבועיים האחרונים ושכיסוי PQ הוא >=70% לכל אזור (`sorafs_orchestrator_pq_candidate_ratio`).
   - קבעו slot של governance review שמאשר את חלון ה-canary.
   - עדכנו `sorafs.gateway.rollout_phase = "ramp"` ב-staging (ערכו את ה-JSON של orchestrator ופרסו מחדש) ובצעו dry-run של צינור הקידום.

2. **Relay canary (T day)**

   - קדמו אזור אחד בכל פעם על ידי הגדרת `rollout_phase = "ramp"` ב-orchestrator וב-manifests של ה-relays המשתתפים.
   - עקבו אחרי "Policy Events per Outcome" ו-"Brownout Rate" ב-dashboard של PQ Ratchet (שכולל כעת את פאנל ה-rollout) למשך פי שניים מ-TTL של guard cache.
   - צלמו snapshots של `sorafs_cli guard-directory fetch` לפני ואחרי הריצה לצורכי audit.

3. **Client/SDK canary (T plus 1 week)**

   - הפכו `rollout_phase = "ramp"` בקונפיגים של clients או העבירו overrides של `stage-b` לקוהורטים ייעודיים של SDK.
   - לכדו diffs של telemetry (`sorafs_orchestrator_policy_events_total` מקובץ לפי `client_id` ו-`region`) וצרפו אותם ליומן תקריות ה-rollout.

4. **Default promotion (T plus 3 weeks)**

   - לאחר אישור governance, החליפו גם orchestrator וגם client configs ל-`rollout_phase = "default"` וסובבו את checklist המוכנות החתום לתוך artefacts של release.

## Checklist של governance והראיות

| שינוי שלב | שער קידום | Evidence bundle | Dashboards & alerts |
|--------------|----------------|-----------------|---------------------|
| Canary -> Ramp *(Stage B preview)* | שיעור brownout Stage A <1% ב-14 הימים האחרונים, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 בכל אזור מקודם, Argon2 ticket verify p95 < 50 ms, ו-slot governance לקידום נקבע. | זוג JSON/Markdown של `cargo xtask soranet-rollout-plan`, snapshots מזווגים של `sorafs_cli guard-directory fetch` (לפני/אחרי), bundle חתום `cargo xtask soranet-rollout-capture --label canary`, ודקות canary שמפנות ל-[PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio), הפניות telemetry ב-`docs/source/soranet/snnet16_telemetry_plan.md`. |
| Ramp -> Default *(Stage C enforcement)* | burn-in של 30 יום עבור telemetry SN16 הושלם, `sn16_handshake_downgrade_total` שטוח ברמת baseline, `sorafs_orchestrator_brownouts_total` אפס במהלך canary של clients, ו-rehearsal של proxy toggle תועד. | תמלול `sorafs_cli proxy set-mode --mode gateway|direct`, פלט `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, לוג `sorafs_cli guard-directory verify`, ו-bundle חתום `cargo xtask soranet-rollout-capture --label default`. | אותו לוח PQ Ratchet יחד עם פאנלי downgrade של SN16 המתועדים ב-`docs/source/sorafs_orchestrator_rollout.md` וב-`dashboards/grafana/soranet_privacy_metrics.json`. |
| Emergency demotion / rollback readiness | מופעל כאשר מוני downgrade מזנקים, אימות guard-directory נכשל, או ה-buffer של `/policy/proxy-toggle` רושם אירועי downgrade מתמשכים. | Checklist מ-`docs/source/ops/soranet_transport_rollback.md`, לוגים של `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, כרטיסי incident ותבניות הודעה. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, ושתי חבילות ההתראות (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- שמרו כל artefact תחת `artifacts/soranet_pq_rollout/<timestamp>_<label>/` עם `rollout_capture.json` שנוצר כדי שחבילות governance יכילו scoreboard, promtool traces ו-digests.
- צרפו digests מסוג SHA256 של הראיות שהועלו (minutes PDF, capture bundle, guard snapshots) לדקות הקידום כדי שניתן יהיה לשחזר אישורי Parliament ללא גישה ל-cluster ה-staging.
- הפנו לתכנית telemetry בכרטיס הקידום כדי להוכיח ש-`docs/source/soranet/snnet16_telemetry_plan.md` נשאר המקור הקנוני ל-vocabularies של downgrade ולספי התראה.

## עדכוני dashboard ו-telemetry

`dashboards/grafana/soranet_pq_ratchet.json` כולל כעת פאנל "Rollout Plan" שמקשר חזרה ל-playbook הזה ומציג את ה-phase הנוכחי כדי שביקורות governance יאשרו איזה stage פעיל. שמרו על תיאור הפאנל מסונכרן עם שינויים עתידיים ב-knobs של התצורה.

לצורכי alerting, ודאו שהכללים הקיימים משתמשים בתווית `stage` כך ששלבי canary ו-default יפעילו ספי מדיניות נפרדים (`dashboards/alerts/soranet_handshake_rules.yml`).

## Hooks של rollback

### Default -> Ramp (Stage C -> Stage B)

1. הורידו את ה-orchestrator באמצעות `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (ומיררו את אותו phase בקונפיגים של SDK) כדי ש-Stage B יחזור לכל הצי.
2. אכפו על clients פרופיל תעבורה בטוח דרך `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, ולכדו את התמלול כך שזרימת `/policy/proxy-toggle` תישאר ניתנת לביקורת.
3. הריצו `cargo xtask soranet-rollout-capture --label rollback-default` כדי לארכב diffs של guard-directory, פלט promtool ו-screenshots של dashboards תחת `artifacts/soranet_pq_rollout/`.

### Ramp -> Canary (Stage B -> Stage A)

1. יבאו את snapshot ה-guard-directory שנלכד לפני הקידום בעזרת `sorafs_cli guard-directory import --guard-directory guards.json` והריצו שוב `sorafs_cli guard-directory verify` כדי שה-demotion packet יכלול hashes.
2. קבעו `rollout_phase = "canary"` (או override עם `anonymity_policy stage-a`) ב-orchestrator וב-configs של clients, ואז הריצו שוב את PQ ratchet drill מתוך [PQ ratchet runbook](./pq-ratchet-runbook.md) כדי להוכיח את צינור ה-downgrade.
3. צרפו screenshots מעודכנים של PQ Ratchet ו-telemetry של SN16 יחד עם תוצאות alert ללוג התקריות לפני הודעה ל-governance.

### תזכורות guardrail

- הפנו ל-`docs/source/ops/soranet_transport_rollback.md` בכל demotion ורשמו mitigations זמניים כ-item `TODO:` ב-rollout tracker להמשך טיפול.
- שמרו את `dashboards/alerts/soranet_handshake_rules.yml` ואת `dashboards/alerts/soranet_privacy_rules.yml` תחת כיסוי `promtool test rules` לפני ואחרי rollback כדי שתיעוד drift של alertים יתלווה ל-capture bundle.

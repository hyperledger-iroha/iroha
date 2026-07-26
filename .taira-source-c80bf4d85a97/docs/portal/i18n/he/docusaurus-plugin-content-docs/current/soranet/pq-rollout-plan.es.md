---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
כותרת: Plan de despliegue poscuantico SNNet-16G
sidebar_label: Plan de despliegue PQ
תיאור: מפעילה עבור מקדם או לחיצת יד היברידית X25519+ML-KEM de SoraNet עם ברירת המחדל של Canary עם ממסרים, לקוחות ו-SDKs.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/soranet/pq_rollout_plan.md`. Manten ambas copias sincronizadas.
:::

SNNet-16G השלם את ה-poscuantico despliegue para el transporte de SoraNet. Los controls `rollout_phase` לאפשר ל-Los Operatores coordinar una promocion determinista desde el requisito actual de guard de Stage.

אסטה קוביית ספרי משחק:

- הגדרות השלב והקונפיגורציה החדשות (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) עם בסיס קוד (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeo de flags de SDK y CLI עבור כל לקוח זמין עבור ההשקה.
- ציפיות לתזמון של ממסר קנרי/קליינט לאלו לוחות מחוונים לניהול que gatean la promocion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de rollback y referencias al runbook de fire-drill ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## מפה דה פאזות

| `rollout_phase` | Etapa de anonimato efectiva | Efecto por defecto | Uso tipico |
|----------------|------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (שלב א') | Requiere al menos un guard PQ por circuito mientras la flota se calienta. | קו בסיס y primeras semanas de canary. |
| `ramp` | `anon-majority-pq` (שלב ב') | Sesga la seleccion hacia relays PQ para >= dos tercios de cobertura; ממסרים קלאסיקו קבועים כמו fallback. | Canaries por region de relays; מחליף את התצוגה המקדימה ב-SDK. |
| `default` | `anon-strict-pq` (שלב ג') | אפליקציית מעגלים בודדים PQ y endurece la alarmas de downgrade. | קידום סופי ונגמר השלמה לטלמטריה וסיומה לממשל. |

אם אתה מגדיר את `anonymity_policy` באופן מפורש, הוא מחליף את השלב עבור רכיב זה. Omitir la etapa explicita ahora difiere al valor de `rollout_phase` para que los operadores puedan cambiar la fase una sola vez por entorno y dejar que los clients la hereden.

## Reference de configuracion

### תזמורת (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

הטוען של מתזמר מחדש את נקודת החזרה בזמן ריצה (`crates/sorafs_orchestrator/src/lib.rs:2229`) y la expone דרך `sorafs_orchestrator_policy_events_total` y `sorafs_orchestrator_pq_ratio_*`. Ver `docs/examples/sorafs_rollout_stage_b.toml` y `docs/examples/sorafs_rollout_stage_c.toml` para snippets listos para aplicar.

### לקוח חלודה / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` ahora registra la fase parseada (`crates/iroha/src/client.rs:2315`) para que helpers (por ejemplo `iroha_cli app sorafs fetch`) puedan reportar la fase actual junto con la politica de anonimato por defecto.

## אוטומציה

Dos helpers `cargo xtask` automatize la generacion del schema y la captura de artefactos.

1. **כללי לוח זמנים אזורי**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```Las duraciones aceptan sufijos `s`, `m`, `h` או `d`. El comando emite `artifacts/soranet_pq_rollout_plan.json` y un resumen en Markdown (`artifacts/soranet_pq_rollout_plan.md`) que puede enviarse con la solicitud de cambio.

2. **Capturar artefactos del drill con firmas**

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

   El comando copia los archivos suministrados en `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula digests BLAKE3 para cada artefacto y escribe `rollout_capture.json` con metadata mas una firma Ed25519 sobre el lastload. ארה"ב מפתח פרטי que firma las minutas del-fire-drill para que governance valide la captura rapidamente.

## Matriz de flags de SDK y CLI

| Superficie | קנרי (שלב א') | רמפה (שלב ב') | ברירת מחדל (שלב C) |
|--------|----------------|----------------|------------------------|
| `sorafs_cli` אחזור | `--anonymity-policy stage-a` o confiar in la fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| תצורת התזמורת JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| תצורת לקוח חלודה (`iroha.toml`) | `rollout_phase = "canary"` (ברירת מחדל) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` פקודות חתומות | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, אופציונלי `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, אופציונלי `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, אופציונלי `.ANON_STRICT_PQ` |
| עוזרי מתזמר JavaScript | `rolloutPhase: "canary"` o `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos los toggles de SDK mapean al mismo parser de etapas usado por el orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`), por lo que despliegues multi-lenguaje se mantienen en lock-step con la fase configurada.

## רשימת תזמון קנרית

1. **טיסה מוקדמת (T פחות שבועיים)**

- Confirmar que la tasa de brownout de Stage A sea <1% en las dos semanas previas y que la cobertura PQ sea >=70% por region (`sorafs_orchestrator_pq_candidate_ratio`).
   - סקירת תוכנית הממשל ב-Que Aprueba la Ventana de Canary.
   - Actualizar `sorafs.gateway.rollout_phase = "ramp"` בהקמה (עורך ה-JSON של התזמר והפריסה מחדש) ו-dry Run של pipeline de promotion.

2. **קנרית ממסר (יום ט')**

   - Promover una region por vez configurando `rollout_phase = "ramp"` en el orchestrator y en los manifestes de relay participantes.
   - עקוב אחר "אירועי מדיניות לפי תוצאה" ו"שיעור התחממות" בלוח המחוונים PQ Ratchet (שהוא כולל פאנל הפעלה) עם מטמון TTL כפול.
   - תצלומי מצב של Cortar de `sorafs_cli guard-directory fetch` antes y despues de la ejecucion para almacenamiento de auditoria.

3. **קנרית לקוח/SDK (T ועוד שבוע)**

   - Cambiar a `rollout_phase = "ramp"` en configs de client או pasar עוקף `stage-b` עבור קבוצות SDK designados.
   - הבדלי טלמטריה תפסו (`sorafs_orchestrator_policy_events_total` אגרופו ל-`client_id` y `region`) y adjuntarlos al log de incidentes of rollout.

4. **מבצע ברירת מחדל (T ועוד 3 שבועות)**- אחד מהמשרדים הממשלתיים, מתזמר טאנטו como configs de client a `rollout_phase = "default"` y rotar el checklist de readyness firmado hacia los artefactos de release.

## רשימת מסמכים לממשל וראיות

| Cambio de fase | שער קידום מכירות | Bundle de evidencia | לוחות מחוונים והודעות |
|-------------|----------------|----------------|---------------------|
| קנרי -> רמפה *(תצוגה מקדימה של שלב ב')* | Tasa de Brownout Stage A <1% en los ultimos 14 dias, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 por region promovida, Argon2 ticket אימות p95 < 50 ms, y el slot de governance para la promocion reservado. | Par JSON/Markdown de `cargo xtask soranet-rollout-plan`, צילומי מצב emparejados de `sorafs_cli guard-directory fetch` (antes/despues), חבילה firmado `cargo xtask soranet-rollout-capture --label canary`, y minutas de canary referenciando [PQ ratchet runbook](I018NU0050X). | `dashboards/grafana/soranet_pq_ratchet.json` (אירועי מדיניות + שיעור תקלות), `dashboards/grafana/soranet_privacy_metrics.json` (יחס שדרוג לאחור של SN16), רפרנסים לטלמטריה ב-`docs/source/soranet/snnet16_telemetry_plan.md`. |
| רמפה -> ברירת מחדל *(אכיפה בשלב C)* | צריבה של טלמטריה SN16 de 30 dias cumplido, `sn16_handshake_downgrade_total` plano in baseline, `sorafs_orchestrator_brownouts_total` in cro durante el canary de client, y repetition of proxy toggle registrado. | Transcripcion de `sorafs_cli proxy set-mode --mode gateway|direct`, salida de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log de `sorafs_cli guard-directory verify`, y bundle firmado `cargo xtask soranet-rollout-capture --label default`. | Mismo tablero PQ Ratchet mas los paneles de downgrade SN16 documentados en `docs/source/sorafs_orchestrator_rollout.md` y `dashboards/grafana/soranet_privacy_metrics.json`. |
| הורדה בחירום / מוכנות לחזרה לאחור | ראה הפעלת קוואנדו לאחור של התחתית משנה, זו לא אימות ספריית המשמר או חיץ `/policy/proxy-toggle` רישום אירועים של הורדת דירוג ססטנידס. | רשימת רשימות של `docs/source/ops/soranet_transport_rollback.md`, יומנים של `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, כרטיסים לאירועים ותבניות הודעה. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` y ambos packs de alertas (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Guarda cada artefacto bajo `artifacts/soranet_pq_rollout/<timestamp>_<label>/` con el `rollout_capture.json` generado para que los paquetes de governance contengan el board score, trazas de promtool y digests.
- אדג'ונטה מעכלת את SHA256 de evidencia subida (דקות PDF, צרור לכידה, צילומי מצב של שמירה) עד כמה דקות של קידום מכירות לפרלמנט, שחזרו לאחר הצטרפות לאשכול של הבמה.
- Referencia el plan de telemetria en el ticket de promocion para probar que `docs/source/soranet/snnet16_telemetry_plan.md` segue siendo la fuente canonica de vocabulario de downgrade y umbrales de alerta.

## אקטואליזציות של לוחות מחוונים וטלמטריה

`dashboards/grafana/soranet_pq_ratchet.json` כולל פאנל של הערות "תוכנית ההפצה" המהווה ספר הפעלה ושלב ממשי עבור שינויים בממשל. תיאור של פאנל סינכרוניזדה עם קמביוס עתידיים עם כפתורי קונפיגורציה.

עבור התראה, אבטחת את כללי ההתנהגות `stage` עבור הספים של קנרי ברירת המחדל של דיספרן פוליטיקה (`dashboards/alerts/soranet_handshake_rules.yml`).

## Hooks de rollback

### ברירת מחדל -> רמפה (שלב C -> שלב ב')1. Baja el orchestrator con `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (y refleja la misma fase en configs de SDK) para que Stage B vuelva a toda la flota.
2. Fuerza a los clients al fil de transporte seguro via `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturando la trancripcion para que el workflow de remediacion `/policy/proxy-toggle` siga siendo auditable.
3. Ejecuta `cargo xtask soranet-rollout-capture --label rollback-default` עבור מדריך ארכיון הבדלים דה guard, salida de promtool וצילומי מסך של לוחות מחוונים bajo `artifacts/soranet_pq_rollout/`.

### רמפה -> קנרית (שלב ב' -> שלב א')

1. ייבוא ספריית ה-Snapshot של המשמר קבצי קידום מכירות עם `sorafs_cli guard-directory import --guard-directory guards.json` ו-`sorafs_cli guard-directory verify` להוצאת ה-hashes.
2. Ajusta `rollout_phase = "canary"` (או לעקוף עם `anonymity_policy stage-a`) en configs de orchestrator y client, y luego repite el PQ ratchet drill desde el [PQ ratchet runbook](./pq-ratchet-runbook.md down pipeline de paragrade el.
3. צילומי מסך ממשיכים ל-PQ Ratchet ו-Telemetria SN16.

### מעקה הבטיחות

- Referencia `docs/source/ops/soranet_transport_rollback.md` מה קורה עם דמוקרטיה ורישום cualquier mitigacion זמני כמו פריט `TODO:` en el rollout tracker para trabajo posterior.
- Mantener `dashboards/alerts/soranet_handshake_rules.yml` y `dashboards/alerts/soranet_privacy_rules.yml` bajo cobertura de `promtool test rules` antes y despues de un rollback para que el drift de alertas quede documentado junto al capture bundle.
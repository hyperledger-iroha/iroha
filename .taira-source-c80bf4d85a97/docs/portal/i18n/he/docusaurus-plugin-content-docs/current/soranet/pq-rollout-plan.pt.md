---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
כותרת: Playbook de rollout pos-quantico SNNet-16G
sidebar_label: Plano de rollout PQ
תיאור: הפעלה עבור מקדם או לחיצת יד היברידית X25519+ML-KEM עבור SoraNet de canary עבור ברירת מחדל לממסרים, לקוחות ו-SDKs.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/soranet/pq_rollout_plan.md`. Mantenha ambas as copias sincronizadas.
:::

סיכומי SNNet-16G או השקה לאחר קוונטי תעביר את SoraNet. ידיות הפעלה `rollout_phase` מאפשרות למפעילים קואורדנם אומה פרומוקאו דטרמיניסטיקה לעשות את דרישות המשמר Stage A עבור שלב א קוברטורה עיקרי Stage B e postura PQ estrita Stage C sem עורך JSON/TOML bruto על פני השטח.

Este Playbook cobre:

- כפתורי הגדרה מוגדרים ונוספים (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) ללא בסיס קוד (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeamento de flags SDK e CLI עבור לקוח מלווה בהשקה.
- ציפיות לתזמון של ממסר קנרי/לקוח ולוחות מחוונים לניהול que gateiam a promocao (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de Rollback e Referencias ao Runbook תרגילי אש ([PQ Ratchet Runbook](./pq-ratchet-runbook.md)).

## מפה דה פאזות

| `rollout_phase` | Estagio de anonimato efetivo | Efeito padrao | Uso tipico |
|----------------|------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (שלב א') | Exigir ao menos um guard PQ por circuit enquanto a frota aquece. | קו הבסיס וסימנאות ההתחלה של קנרי. |
| `ramp` | `anon-majority-pq` (שלב ב') | Viesar a selecao para relays PQ com >= dois tercos de coverage; ממסר classicos ficam como fallback. | Canary por regiao de relays; מחליף את התצוגה המקדימה ב-SDK. |
| `default` | `anon-strict-pq` (שלב ג') | Forcar מעגלים apenas PQ e endurecer אזעקות להורדה. | Promocao final apos telemetria e sign-off de governance. |

ראה אומה משטח טמבם definir `anonymity_policy` מפורש, ela sobrecreve a fase para aquele componente. אומיטיר או הבמה מפורשת agora faz defer ao valor de `rollout_phase` עבור מפעילי possam virar a fase uma vez por ambiente e deixar os clients herdarem.

## Referencia de configuracao

### תזמורת (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

O loader do resolve orchestrator o stage de fallback em runtime (`crates/sorafs_orchestrator/src/lib.rs:2229`) o expone via `sorafs_orchestrator_policy_events_total` e `sorafs_orchestrator_pq_ratio_*`. Veja `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para snippets prontos para aplicar.

### לקוח חלודה / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` רשום את השלב הראשון (`crates/iroha/src/client.rs:2315`) עבור פקודות עוזרות (לדוגמה `iroha_cli app sorafs fetch`) מדווחים על שלב אטואלי בשעה פוליטית של אנונימאטו פאדראו.

## אוטומקאו

Dois helpers `cargo xtask` automatizam a geracao לעשות לוח זמנים e captura de artefatos.

1. **גרר או לוח זמנים אזורי**

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

   משכי זמן aceitam sufixos `s`, `m`, `h` או `d`. O comando emite `artifacts/soranet_pq_rollout_plan.json` e um resumo Markdown (`artifacts/soranet_pq_rollout_plan.md`) que pode ser enviado com a solicitacao de change.2. **Capturar artefatos do drill com assinaturas**

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

   O comando copia os arquivos fornecidos para `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula digests BLAKE3 para cada artefato e escreve `rollout_capture.json` contendo metadata e uma assinatura Ed25519 sobre oloadload. השתמש במפתח פרטי של אסינה כדי לעשות תרגיל כיבוי במשך דקות ארוכות עבור ניהול תקף ותקף במהירות.

## Matriz de flags SDK & CLI

| משטח | קנרי (שלב א') | רמפה (שלב ב') | ברירת מחדל (שלב C) |
|--------|----------------|----------------|------------------------|
| `sorafs_cli` אחזור | `--anonymity-policy stage-a` או בדוק את השלב | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| תצורת התזמורת JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| תצורת לקוח חלודה (`iroha.toml`) | `rollout_phase = "canary"` (ברירת מחדל) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` פקודות חתומות | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, אופציונלי `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, אופציונלי `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, אופציונלי `.ANON_STRICT_PQ` |
| עוזרי מתזמר JavaScript | `rolloutPhase: "canary"` או `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos OS מחליפים את SDK mapeiam para o mesmo stage parser usado pelo orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`), entao פריסות מרובות שפות ficam em lock-step com a faconfigurada.

## רשימת תזמון קנרית

1. **טיסה מוקדמת (T פחות שבועיים)**

- אישור שיעור החום-אאוט של Stage A esta <1% לאחר התוצאות הסופיות e que a cobertura PQ e >=70% por regiao (`sorafs_orchestrator_pq_candidate_ratio`).
   - סדר היום של סקירת משילות que aprova a janela de canary.
   - Atualizar `sorafs.gateway.rollout_phase = "ramp"` בימוי (עורך או מתזמר JSON ו-Reployment) ו-Fazer dry run do pipeline de promocao.

2. **קנרית ממסר (יום ט')**

   - Promover uma regiao por vez configurando `rollout_phase = "ramp"` ללא מתזמר ואין ביטויים של משתתפי ממסר.
   - עקוב אחר "אירועי מדיניות לפי תוצאה" ו"שיעור התחממות" ללא לוח המחוונים PQ Ratchet (אגורה עם פאנל של השקה) עבור שניות או TTL לעשות שמירה על מטמון.
   - תצלומי מצב של Fazer de `sorafs_cli guard-directory fetch` אנטה ו-depois para armazenamento de auditoria.

3. **קנרית לקוח/SDK (T ועוד שבוע)**

   - Trocar para `rollout_phase = "ramp"` em configs de client ou passé overrides `stage-b` para cohorts de SDK designados.
   - הבדלי טלמטריה תפסו (`sorafs_orchestrator_policy_events_total` אגרופאדו ל-`client_id` ו-`region`) והוספה של יומן תקריות.

4. **מבצע ברירת מחדל (T ועוד 3 שבועות)**

   - Depois עושה חתימה על הממשל, משנה את הגדרות התזמורות והלקוחות עבור `rollout_phase = "default"` ורוטציה או רשימת בדיקה של מוכנות לעריכת חפצי אמנות לשחרור.

## רשימת ביקורת של ממשל וראיות| שינוי שלב | שער קידום | צרור ראיות | לוחות מחוונים והתראות |
|-------------|----------------|----------------|---------------------|
| קנרי -> רמפה *(תצוגה מקדימה של שלב ב')* | שיעור שלב א' <1% עד 14 ימים, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 עבור קידום מכירות, אימות כרטיס Argon2 < 50 אלפיות השנייה, וחריץ ממשל עבור קידום מכירות. | Par JSON/Markdown de `cargo xtask soranet-rollout-plan`, תמונת מצב של `sorafs_cli guard-directory fetch` (antes/depois), חבילה assinado `cargo xtask soranet-rollout-capture --label canary`, e minutes de canary referenciando [PQ ratchet runbook](I180005X). | `dashboards/grafana/soranet_pq_ratchet.json` (אירועי מדיניות + שיעור תקלות), `dashboards/grafana/soranet_privacy_metrics.json` (יחס שדרוג לאחור של SN16), התייחסויות לטלמטריה ב-`docs/source/soranet/snnet16_telemetry_plan.md`. |
| רמפה -> ברירת מחדל *(אכיפה בשלב C)* | Burn-in de telemetria SN16 de 30 dias concluido, `sn16_handshake_downgrade_total` flat no baseline, `sorafs_orchestrator_brownouts_total` אפס משך הלקוח, או חזרות ל-proxy toggle logado. | תמלול `sorafs_cli proxy set-mode --mode gateway|direct`, פלט של `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, יומן `sorafs_cli guard-directory verify`, e bundle assinado `cargo xtask soranet-rollout-capture --label default`. | O mesmo board PQ Ratchet e os panels degrade downgrade SN16 documentados em `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. |
| הורדה בחירום / מוכנות לחזרה לאחור | Acionado quando counters degrade down sobem, a verificacao do do guard-directory falha, ou o buffer `/policy/proxy-toggle` registra downgrade events sustentados. | רשימת רשימות של `docs/source/ops/soranet_transport_rollback.md`, יומנים של `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, כרטיסים לאירועים ותבניות הודעה. | חבילות התראה `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, e ambos (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Armazene cada artefato em `artifacts/soranet_pq_rollout/<timestamp>_<label>/` com o `rollout_capture.json` gerado para que os governance packets contenham o board score, promtool traces e digests.
- Anexe מעכל את SHA256 das evidencias carregadas (דקות PDF, צרור לכידת, צילומי שמירה) כדקות של קידום לפרלמנט כאישורים של הפרלמנט, למשל, מקבץ בימוי.
- Referencie o plano de telemetria no ticket de promocao para provar que `docs/source/soranet/snnet16_telemetry_plan.md` segue sendo a fonte canonica para vocabularios de downgrade e ספי התראה.

## לוח המחוונים וטלמטריה תקינים

`dashboards/grafana/soranet_pq_ratchet.json` אגורה כוללת את פאנל ההטמעה "תוכנית ההפצה" que linka para este playbook e mostra a phase aual para que as reviews de governance confirmem the qual stage esta ativo. Mantenha a descricao do panel sincronizada com mudancas futuras nos config knobs.

עבור התראה, תווית `stage` עבור תווית `stage` עבור הגדרות ברירת המחדל של מדיניות מדיניות ברירת מחדל (`dashboards/alerts/soranet_handshake_rules.yml`).

## ווים לאחור

### ברירת מחדל -> רמפה (שלב C -> שלב ב')1. הורד את התזמורת ב-`sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (התזמורת ב-SDK) עבור שלב ב' מחדש.
2. לאלץ לקוחות כגון פרופיל תחבורה בטוח באמצעות `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, צילום או תמלול עבור זרימת עבודה `/policy/proxy-toggle` המשך ביקורת.
3. Rode `cargo xtask soranet-rollout-capture --label rollback-default` עבור arquivar guard-directory diffs, פלט Promtool ותמונות מסך של לוח המחוונים em `artifacts/soranet_pq_rollout/`.

### רמפה -> קנרית (שלב ב' -> שלב א')

1. יבוא תמונת מצב של מדריך השומר לפני קידום מכירות com `sorafs_cli guard-directory import --guard-directory guards.json` ו-`sorafs_cli guard-directory verify` חדש עבור מנות הורדה כולל גיבוב.
2. Ajuste `rollout_phase = "canary"` (או תעקוף com `anonymity_policy stage-a`) עם מתזמר e configs לקוח, depois עבר novamente o PQ ratchet drill do [PQ ratchet runbook](./pq-ratchet-runbook.md) degrade para down prove o.
3. צילומי מסך נספחים של PQ Ratchet וטלמטריה SN16 נוספו לתוצאות התראות ויומן תקריות לפני הודעות ממשל.

### תזכורות למעקה בטיחות

- Referencie `docs/source/ops/soranet_transport_rollback.md` semper que ocorrer demotion e Registre qualquer mitigacao temporaria como פריט `TODO:` no rollout tracker para acompanhamento.
- Mantenha `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` sob cobertura de `promtool test rules` antes e depois de um rollback para que o alert drive fique documentado junto bunto capture.
---
lang: he
direction: rtl
source: docs/portal/docs/nexus/operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-operations
כותרת: Runbook de operacoes Nexus
תיאור: Resumo pronto para uso em campo do fluxo de trabalho do operator Nexus, espelhando `docs/source/nexus_operations.md`.
---

השתמש ב-esta pagina como irmao de referencia rapida de `docs/source/nexus_operations.md`. אלa destila o checklist operational, os ganchos de gestao de mudanca e os requisitos de cobertura de telemetria que os operatores Nexus devem seguir.

## Lista de ciclo de vida

| אטאפה | Acoes | Evidencia |
|-------|--------|--------|
| Pre-voo | בדוק גיבוב / מאפייני שחרור, אשר את `profile = "iroha3"` והכן תבניות להגדרה. | Saida de `scripts/select_release_profile.py`, log de checksum, bundle de manifestos assinado. |
| Alinhamento do catalogo | הפוך את הקטלוג `[nexus]`, פוליטיקה דה רוטאמנטו e os limiares de DA conforme או מניפסט emitido to conselho, e entao capture `--trace-config`. | Saida de `irohad --sora --config ... --trace-config` armazenada com o ticket de onboard. |
| עשן e cutover | בצע את `irohad --sora --config ... --trace-config`, רכב על עשן ב-CLI (`FindNetworkStatus`), תקף לייצא טלמטריה ובקש אישור. | Log de smoke-test + confirmacao do Alertmanager. |
| Estado estavel | עקוב אחר לוחות מחוונים/אזהרות, נסע ב-Rotacao de Chaves בהתאם לקדנציה דה גוברננה וסיקור תצורות/פנקסי הפעלה quando manifestos mudarem. | Minutas de Revisao Trimestral, Capturas de Dashboards, IDs de tickets de rotacao. |

O onboarding detalhado (substituicao de chaves, templates de roteamento, passos do perfil de release) permanece em `docs/source/sora_nexus_operator_onboarding.md`.

## גסטאו דה מודנקה

1. **Atualizacoes de release** - acompanhe anuncios em `status.md`/`roadmap.md`; תוספת לרשימת בדיקה לכניסה למערכת יחסי ציבור לשחרור.
2. **Mudancas de manifesto de lane** - חבילות אימות assinados do Space Directory e arquive-os em `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuracao** - toda mudanca em `config/config.toml` מבקש מפנה כרטיס לנתיב/מרחב נתונים. Guarde uma copia redigida da configuracao efetiva quando nos entram ou sao atualizados.
4. **Treinos de rollback** - ensaie trimestralmente procedimentos de stop/restore/smoke; רשם תוצאות ב-`docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprovacoes de compliance** - מסלולים פרטיים/CBDC devem obter aval de compliance antes de alterar politica de DA ou knobs de redacao de telemetria (ver `docs/source/cbdc_lane_playbook.md`).

## Telemetria e SLOs- לוחות מחוונים: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, דגמי SDK (לדוגמה, `android_operator_console.json`).
- התראות: `dashboards/alerts/nexus_audit_rules.yml` e regras de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- מדדים לצופה:
  - `nexus_lane_height{lane_id}` - התראה על אפס התקדמות לחריצים.
  - `nexus_da_backlog_chunks{lane_id}` - alerta acima dos limiares por lane (padrao 64 ציבורי / 8 פרטי).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta quando o P99 מעבר ל-900 אלפיות השנייה (ציבורי) או 1200 אלפיות השנייה (פרטי).
  - `torii_request_failures_total{scheme="norito_rpc"}` - התראה ב-5 דקות של יותר מ-2%.
  - `telemetry_redaction_override_total` - Sev 2 imediato; garanta que מבטל את התאימות של כרטיסי טנהאם.
- בצע את רשימת התיעוד של טלמטריה ללא [Nexus תוכנית תיקון טלמטריה](./nexus-telemetry-remediation) לאחר ביצוע הפעולות.

## Matriz de incidentes

| Severidade | Definicao | רפוסטה |
|--------|------------|--------|
| סב 1 | Violacao de isolamento de data-space, parada de settlement>15 דקות, ou corrupcao de voto de governanca. | Acione Nexus Primary + Release Engineering + Compliance, congele admissao, colete artefatos, publicque communicados <=60 דקות, RCA <=5 dias uteis. |
| סב' 2 | Violacao de SLA de backlog de lane, ponto cego de telemetria >30 דקות, השקת מניפסט פאלהו. | Acione Nexus Primary + SRE, הקלה <=4 שעות, רישום מעקבים עם 2 ימים. |
| סוו 3 | Deriva nao bloqueante (מסמכים, התראות). | הרשמה אין גשש e agende a correcao dentro do sprint. |

כרטיסים de incidente devem מזהי רשם של נתיב/נתונים-מרחב, מניפסטים, ציר זמן, מדדים/יומני תמיכה ומעקב/בעלים.

## Arquivo de evidencias

- חבילות ארמזן/מניפסטים/יצוא טלמטריה em `artifacts/nexus/<lane>/<date>/`.
- Mantenha configs redigidas + saida de `--trace-config` לשחרור ראשון.
- Anexe minutas do conselho + decisoes assinadas quando mudancas de config ou Manifesto ocorrerem.
- שמרו תמונות Snapshots semanais de Prometheus רלוונטיות למטרות Nexus ל-12 חודשים.
- רשם edicoes do runbook em `docs/source/project_tracker/nexus_config_deltas/README.md` para que auditores saibam quando as responsabilidades mudaram.

## relacionado חומרי

- Visao geral: [סקירה כללית Nexus](./nexus-overview)
- ספציפי: [מפרט Nexus](./nexus-spec)
- Geometria de lanes: [דגם נתיב Nexus](./nexus-lane-model)
- Transicao e shims de roteamento: [Nexus הערות מעבר](./nexus-transition-notes)
- העלאת מפעילים: [הכנסת מפעיל Sora Nexus](./nexus-operator-onboarding)
- Remediacao de telemetria: [תוכנית תיקון טלמטריה Nexus](./nexus-telemetry-remediation)
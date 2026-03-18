---
lang: he
direction: rtl
source: docs/source/torii/norito_rpc_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ddc611ab0c68b0b4c5412056d5d93b966a2a99d7657516f7e49c05753f13f331
source_last_modified: "2026-04-22T09:12:55.053896+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/torii/norito_rpc_tracker.md -->

## מעקב פעולות Norito-RPC

| מזהה | תיאור | בעלים | סטטוס | יעד | הערות |
|----|-------------|----------|--------|--------|-------|
| NRPC-2A | טיוטת תוכנית פריסה ל‑Torii ingress/auth עם dual‑stack, mTLS ו‑fallback guards | Torii Platform TL, NetOps | 🈴 הושלם | 2026-03-21 | סעיף 3 ב‑`docs/source/torii/norito_rpc_rollout_plan.md` מתעד מדיניות ingress, דרישות mTLS, יישור rate‑limit וטיפול בשגיאות. |
| NRPC-2B | הגדרת עדכוני טלמטריה + התראות (Grafana, Alertmanager, chaos scripts) | Observability liaison | 🈴 הושלם | 2026-03-19 | מפרט טלמטריה + ערכת התראות פורסמו (`docs/source/torii/norito_rpc_telemetry.md`, `dashboards/grafana/torii_norito_rpc_observability.json`, `dashboards/alerts/torii_norito_rpc_rules.yml`, `scripts/telemetry/test_torii_norito_rpc_alerts.sh`). |
| NRPC-2C | הפקת checklist לפריסה (stage, canary, GA, rollback) | Platform Ops | 🈴 הושלם | 2026-03-21 | סעיפים 5 ו‑8 ב‑`docs/source/torii/norito_rpc_rollout_plan.md` מפרסמים checklist מדורג ו‑rollback quick reference. |
| NRPC-2R | סקירת מוכנות dual‑stack (קיבוע spec + knobs לפריסה) | Torii Platform TL, SDK Program Lead, Android Networking TL | 🈴 הושלם | 2025-06-19 | סקירה התקיימה 2025-06-19; spec + fixtures הוקפאו (`nrpc_spec.md` SHA-256 `0bb9d2c225b5485fd0b7c6ef28a3ecea663afea76b09a248701d0b50c25982b1`, `fixtures/norito_rpc/schema_hashes.json` SHA-256 `343f4c7991e6bcfbda894b3b2422a0241def279fc79db7563da618d31763ba1c`). כללי canary: `transport.norito_rpc.stage=canary` דורש mTLS + `allowed_clients`, JSON נשאר זמין ל‑rollback, שערי טלמטריה דרך `dashboards/grafana/torii_norito_rpc_observability.json`. פרוטוקול: `docs/source/torii/norito_rpc_sync_notes.md`. |
| NRPC-3A | עזר SDK ב‑JavaScript + בדיקות לבקשות Norito-RPC | JS SDK Lead | 🈴 הושלם | 2026-04-15 | `NoritoRpcClient` + `NoritoRpcError` נשלחים כעת ב‑SDK עם TypeScript definitions, תיעוד ובדיקות יחידה עבור headers/params/retries/timeout (`javascript/iroha_js/src/noritoRpcClient.js`,`javascript/iroha_js/test/noritoRpcClient.test.js`,`javascript/iroha_js/index.d.ts`,`javascript/iroha_js/README.md:342`). |
| NRPC-3B | עזר Norito transport ל‑Swift SDK | Swift SDK Lead | 🈴 הושלם | 2026-04-17 | `NoritoRpcClient` נשלח כעת תחת `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift` עם כיסוי רגרסיה (`IrohaSwift/Tests/IrohaSwiftTests/NoritoRpcClientTests.swift`) ועדכוני docs/README כך ש‑Swift תואם ל‑JS helper לאימוץ NRPC. |
| NRPC-3C | הסכם cadence חוצה‑SDK ל‑fixtures | SDK Program Lead | 🈴 הושלם | 2026-04-19 | רוטציה שבועית Wed 17:00 UTC + פרוצדורה תועדה ב‑`docs/source/torii/norito_rpc_fixture_cadence.md`, עם הפניה ל‑`scripts/run_norito_rpc_fixtures.sh`, נתיבי ארטיפקט (`artifacts/norito_rpc/<stamp>/`), וציפיות רישום סטטוס. |
| NRPC-4 | פרסום לוח אימוץ חוצה‑SDK + checklist | SDK Program Lead / Swift & JS Leads | 🈴 הושלם | 2026-03-22 | `docs/source/torii/norito_rpc_adoption_schedule.md` מתעדת את ציר הזמן המדורג, deliverables של SDK, תכנית fixtures, ו‑telemetry/reporting hooks הנדרשים ליישור NRPC GA עם AND4. |
| DOC-NRPC | עדכון פורטל מפתחים ודגימות קונסולת Try‑It | Docs/DevRel | 🈴 הושלם | 2026-03-25 | מסמכי הפורטל מכסים כעת payloads של Norito מקצה לקצה (`docs/portal/docs/devportal/try-it.md`,`docs/portal/docs/norito/try-it-console.md`), תיוג proxy (`TRYIT_PROXY_CLIENT_ID` → `X-TryIt-Client`) חווט ונבדק ב‑`tryit-proxy-lib.mjs` tests, והמסלול הדפדפני “Try it” מפעיל fixtures `application/x-norito` עם guard של מניפסט חתום עדיין פעיל. |
| OPS-NRPC | עדכון FAQ למפעילים (status.md, release notes) | DevRel, Ops | 🈴 הושלם | 2026-03-29 | FAQ למפעילים מכיל כעת טקסט release note מוכן לפרסום + שלבי פרסום בסעיף 5 (`docs/source/runbooks/torii_norito_rpc_faq.md:97`–`116`), כך שבעלי כוננות יכולים להכריז על rollout בלי לנסח העתקה אד־הוק. |
| QA-NRPC | הרחבת smoke tests עבור תאימות Norito מול JSON | QA Guild | 🈴 הושלם | 2026-04-22 | `cargo xtask norito-rpc-verify` מפעיל כעת את endpoints הכינויים הן עם JSON והן עם payloads Norito כדי לזהות רגרסיות לפני rollout.【xtask/src/main.rs:492】【xtask/src/main.rs:3517】 |
| NRPC-4F1 | אוטומציית cadence + ראיות ל‑fixtures | SDK Program Lead / Android Networking TL | 🈴 הושלם | שבועי (Wed 17:00 UTC) | מעטפת cadence מפיקה כעת סיכומי רוטציה אוטומטיים (`--auto-report` → `artifacts/norito_rpc/rotation_status.{json,md}` עם רעננות 7‑ימים) לצד לוגים/JSON/xtask לכל הרצה. הבעלים עדיין מתחלפים שבועית; נתיבי ארטיפקט נשארים תחת `artifacts/norito_rpc/` עבור `status.md` וחבילות ממשל. |

### קצב עדכון
- סקירה שבועית בסינק של פלטפורמת Torii.
- בעלי המעקב מעדכנים סטטוס לפני פגישת ה‑dry run של יום שלישי.

</div>

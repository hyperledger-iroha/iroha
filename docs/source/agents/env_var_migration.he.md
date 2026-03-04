---
lang: he
direction: rtl
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 415a4317a6ce46a8272c6474f9cbf670a9c6100af8f9ec74e2be299dc5b0be1c
source_last_modified: "2025-12-18T14:07:05.705507+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/agents/env_var_migration.md -->

# מעקב הסבת Env → Config

מעקב זה מסכם מתגים של משתני סביבה הפונים לייצור, כפי שמופיעים ב‑
`docs/source/agents/env_var_inventory.{json,md}`, ואת נתיב ההסבה המתוכנן אל
`iroha_config` (או תחימה מפורשת ל‑dev/test בלבד).

הערה: `ci/check_env_config_surface.sh` נכשל כעת כאשר מופיעים shims חדשים
של env **לייצור** ביחס ל‑`AGENTS_BASE_REF` אלא אם מוגדר
`ENV_CONFIG_GUARD_ALLOW=1`; תעדו כאן תוספות מכוונות לפני שימוש ב‑override.

## הסבות שהושלמו

- **IVM ABI opt-out** — הוסר `IVM_ALLOW_NON_V1_ABI`; הקומפיילר כעת דוחה
  ABI שאינו v1 ללא תנאי עם בדיקת יחידה המגנה על נתיב השגיאה.
- **IVM debug banner env shim** — הוסר ה‑env opt‑out `IVM_SUPPRESS_BANNER`;
  השתקת הבאנר זמינה עדיין דרך setter פרוגרמטי.
- **IVM cache/sizing** — הועברה הגדרת cache/prover/GPU דרך
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`, `accel.max_gpus`)
  והוסרו shims של env בזמן ריצה. Hosts קוראים כעת ל‑
  `ivm::ivm_cache::configure_limits` ו‑`ivm::zk::set_prover_threads`, בדיקות משתמשות
  ב‑`CacheLimitsGuard` במקום דריסות env.
- **Connect queue root** — נוסף `connect.queue.root` (ברירת מחדל:
  `~/.iroha/connect`) לתצורת הלקוח והועבר דרך ה‑CLI וה‑JS diagnostics.
  עזרי JS פותרים את התצורה (או `rootDir` מפורש) ומכבדים את
  `IROHA_CONNECT_QUEUE_ROOT` רק ב‑dev/test דרך `allowEnvOverride`;
  התבניות מתעדות את ה‑knob כך שמפעילים לא צריכים עוד דריסות env.
- **Izanami network opt-in** — נוסף דגל CLI/config מפורש `allow_net` לכלי
  הכאוס Izanami; הרצות כעת דורשות `allow_net=true`/`--allow-net` ו‑
- **IVM banner beep** — הוחלף ה‑env shim `IROHA_BEEP` במתגי תצורה
  `ivm.banner.{show,beep}` (ברירת מחדל: true/true). חיווט banner/beep
  קורא כעת רק תצורה בייצור; בניות dev/test עדיין מכבדות את דריסת ה‑env
  עבור טוגלים ידניים.
- **DA spool override (tests only)** — ה‑override `IROHA_DA_SPOOL_DIR`
  תחום כעת מאחורי עוזרי `cfg(test)`; קוד ייצור תמיד שואב את נתיב ה‑spool
  מהתצורה.
- **Crypto intrinsics** — הוחלפו `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` במדיניות תצורה מונעת `crypto.sm_intrinsics`
  (`auto`/`force-enable`/`force-disable`) והוסר ה‑guard `IROHA_SM_OPENSSL_PREVIEW`.
  Hosts מיישמים את המדיניות בעת ההפעלה; benches/tests יכולים לבחור להפעיל
  דרך `CRYPTO_SM_INTRINSICS`, ו‑OpenSSL preview מכבד כעת רק את דגל התצורה.
  Izanami כבר דורש `--allow-net`/תצורה שמורה, והבדיקות נשענות כעת על knob
  זה ולא על מתגי env כלליים.
- **FastPQ GPU tuning** — נוספו knobs תצורה `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  (ברירת מחדל: `None`/`None`/`false`/`false`/`false`) והועברו דרך parsing ה‑CLI;
  שימסי `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` מתנהגים כ‑fallback ל‑dev/test בלבד
  ומתעלמים מהם לאחר טעינת תצורה (גם כאשר התצורה משאירה אותם ריקים); התיעוד והמלאי
  רועננו כדי לסמן את ההסבה.【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) מוגדרים כעת מאחורי בניות debug/test
  באמצעות helper משותף כך שבינאריים של ייצור מתעלמים מהם תוך שמירת knobs
  לאבחון מקומי. מלאי ה‑env נוצר מחדש כדי לשקף את תחום dev/test בלבד.
- **FASTPQ fixture updates** — `FASTPQ_UPDATE_FIXTURES` מופיע כעת רק בבדיקות
  אינטגרציה של FASTPQ; מקורות ייצור אינם קוראים עוד את מתג ה‑env והמלאי משקף
  את תחום test בלבד.
- **Inventory refresh + scope detection** — כלי המלאי של env מתייג כעת קבצי
  `build.rs` כ‑build scope ומעקוב אחר מודולי `#[cfg(test)]`/integration harness
  כך שמתגים ייעודיים לבדיקות (למשל, `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) ודגלי
  build של CUDA מופיעים מחוץ לספירת הייצור. המלאי חודש Dec 07, 2025
  (518 refs / 144 vars) כדי לשמור על diff ירוק של env‑config guard.
- **P2P topology env shim release guard** — `IROHA_P2P_TOPOLOGY_UPDATE_MS`
  מפעיל כעת שגיאת אתחול דטרמיניסטית בבניות release (warn‑only ב‑debug/test)
  כך שצמתי ייצור מסתמכים אך ורק על `network.peer_gossip_period_ms`. מלאי ה‑env
  נוצר מחדש כדי לשקף את ה‑guard ואת מסווג ה‑cfg!‑guarded המעודכן שמתחם toggles
  כ‑debug/test.

## הסבות בעדיפות גבוהה (נתיבי ייצור)

- _אין_ (המלאי רוענן עם זיהוי cfg!/debug; ה‑env‑config guard ירוק לאחר הקשחת
  shim ה‑P2P).

## מתגי dev/test שיש לגדר

- סבב נוכחי (Dec 07, 2025): דגלי build‑only של CUDA (`IVM_CUDA_*`) מתוייגים
  כ‑`build` ומתגלים במלאי מתגי harness (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`,
  `IROHA_SKIP_BIND_CHECKS`) כ‑`test`/`debug` (כולל shims שמוגנים ב‑`cfg!`).
  אין צורך בגידור נוסף; שמרו על תוספות עתידיות מאחורי עוזרי `cfg(test)`/bench‑only
  עם הערות TODO כאשר shims זמניים.

## משתני build‑time (להשאיר כמות שהם)

- משתני Cargo/feature (`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD`, וכו') נשארים
  עניינים של build‑script ומחוץ להיקף ההסבה לתצורת runtime.

## פעולות הבאות

1) הריצו `make check-env-config-surface` אחרי עדכוני config‑surface כדי ללכוד
   shims חדשים בייצור מוקדם ולהקצות בעלים/ETAs לתת‑מערכות.  
2) רעננו את המלאי (`make check-env-config-surface`) לאחר כל סבב כדי שהמעקב
   יישאר מסונכרן עם guardrails חדשים ו‑env‑config guard diff יישאר נטול רעש.

</div>

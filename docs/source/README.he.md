<!-- Hebrew translation of docs/source/README.md -->

---
lang: he
direction: rtl
source: docs/source/README.md
status: complete
translator: manual
---

<div dir="rtl">

# אינדקס תיעוד Iroha VM ו־Kotodama

אינדקס זה מקשר למסמכי התכנון והייחוס המרכזיים עבור IVM, ‏Kotodama וצינור ה־IVM-first. לגרסה ביפנית ראו [`README.ja.md`](./README.ja.md).

- ארכיטקטורת IVM ומיפוי שפה: ‏`../../ivm.md`
- ABI של קריאות מערכת ב־IVM: ‏`ivm_syscalls.md`
- קבועי קריאות מערכת גנריים: ‏`ivm_syscalls_generated.md` (הריצו `make docs-syscalls` לרענון)
- כותרת בייטקוד של IVM: ‏`ivm_header.md`
- דקדוק וסמנטיקה של Kotodama: ‏`kotodama_grammar.md`
- דוגמאות Kotodama ומיפויי קריאות מערכת: ‏`kotodama_examples.md`
- צינור עסקאות (IVM-first): ‏`../../new_pipeline.md`
- Torii Contracts API (מניפסטים): ‏`torii_contracts_api.md`
- מעטפת שאילתות JSON (CLI / כלי עזר): ‏`query_json.md`
- ייחוס מודול סטרימינג Norito: ‏`norito_streaming.md`
- דוגמאות ABI בזמן ריצה: ‏`samples/runtime_abi_active.md`, ‏`samples/runtime_abi_hash.md`, ‏`samples/find_active_abi_versions.md`
- ממשק ZK App (צרופות, מוכיח, ספירת קולות): ‏`zk_app_api.md`
- ספר ההפעלה של Torii עבור צרופות/מוכיח ZK: ‏`zk/prover_runbook.md`
- מדריך תפעול Torii ZK App (צרופות/מוכיח; תיעוד הקרייט): ‏`../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- מחזור חיים של VK/הוכחות (רגיסטרי, אימות, טלמטריה): ‏`zk/lifecycle.md`
- עזרי תפעול Torii (נקודות קצה לניראות): ‏`references/operator_aids.md`
- מדריך מהיר וארכיטקטורת MOCHI: ‏`mochi/index.md`
- מדדי/לוחות מחוונים ל־Swift SDK: ‏`references/ios_metrics.md`
- ממשל: ‏`../../gov.md`
- פרומפטים לתיאום הבהרות: ‏`coordination_llm_prompts.md`
- מפת דרכים: ‏`../../roadmap.md`
- שימוש בדימוי בנייה של Docker: ‏`docker_build.md`

טיפים לשימוש
- בנו והריצו דוגמאות תחת `examples/` בעזרת כלים חיצוניים (`koto_compile`, ‏`ivm_run`):
  - `make examples-run` (ו־`make examples-inspect` אם `ivm_tool` זמין)
- בדיקות אינטגרציה אופציונליות (מושבתות כברירת מחדל) לדוגמאות ולבדיקות כותרת נמצאות ב־`integration_tests/tests/`.

תצורת הצינור
- כל ההתנהגות בזמן ריצה מוגדרת דרך קבצי `iroha_config`. לא נעשה שימוש במשתני סביבה בצד המפעיל.
- קיימות ברירות מחדל סבירות; רוב ההטמעות לא יצטרכו שינוי.
- מפתחות רלוונטיים תחת `[pipeline]`:
  - ‏`dynamic_prepass`: הפעלת prepass קריאה־בלבד של IVM להפקת קבוצות גישה (ברירת מחדל: true).
  - ‏`access_set_cache_enabled`: מטמון של קבוצות גישה נגזרות לפי `(code_hash, entrypoint)`; ניתן לכבות כדי לאבחן hints (ברירת מחדל: true).
  - ‏`parallel_overlay`: בניית overlays במקביל; ההתחייבות נשארת דטרמיניסטית (ברירת מחדל: true).
  - ‏`gpu_key_bucket`: דליים אופציונליים למפתחות עבור prepass של המתזמן באמצעות radix יציב על `(key, tx_idx, rw_flag)`; נפילת CPU דטרמיניסטית תמיד פעילה (ברירת מחדל: false).
  - ‏`cache_size`: קיבולת מטמון pre-decode הגלובלי של IVM (streams מפוענחים). ברירת מחדל: 128. הגדלה יכולה לצמצם זמן פענוח להרצות חוזרות.

בדיקות סנכרון תיעוד
- קבועי קריאות מערכת (`docs/source/ivm_syscalls_generated.md`)
  - רענון: `make docs-syscalls`
  - בדיקה בלבד: `bash scripts/check_syscalls_doc.sh`
- טבלת ABI של קריאות מערכת (`crates/ivm/docs/syscalls.md`)
  - בדיקה בלבד: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - עדכון קטעים גנריים (והטבלה בתיעוד הקוד): `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- טבלאות Pointer-ABI (`crates/ivm/docs/pointer_abi.md` ו־`ivm.md`)
  - בדיקה בלבד: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - עדכון קטעים: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- מדיניות כותרת IVM ו־ABI hashes (`docs/source/ivm_header.md`)
  - בדיקה בלבד: `cargo run -p ivm --bin gen_header_doc -- --check` ו־`cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - עדכון קטעים: `cargo run -p ivm --bin gen_header_doc -- --write` ו־`cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- תזרים GitHub Actions‏ `.github/workflows/check-docs.yml` מריץ בדיקות אלה בכל push/PR וייכשל אם המסמכים הגנריים יסטו מהמימוש.

</div>

---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: SoraFS SF1 Determinism Dry-Run
תקציר: רשימת מסמכים ותקצירים עבור תקציר של chunker canonico `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 דטרמיניזם יבש הפעלה

Este relatorio captura או בסיס לריצה יבשה עבור פרפיל chunker canonico
`sorafs.sf1@1.0.0`. Tooling WG פיתח מחדש או רשימת בדיקה או תקף
מרענן את האביזרים או צינורות צרכנים חדשים. הרשמה או תוצאה
comando na tabela para manter um trail auditavel.

## רשימת תיוג

| פאסו | קומנדו | Resultado esperado | Notas |
|------|--------|----------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todos os tests passam; o teste de paridade `vectors` טם הצלחה. | אישור que fixtures canonicos compilam e correspondem and implementacao Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | O script sai com 0; reporta os digests de manifest abaixo. | Verifica que fixtures regeneram limpo e que as assinaturas permanecem anexadas. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | אנטרדה `sorafs.sf1@1.0.0` תואם לתיאור הרישום (`profile_id=1`). | הבטחת מטא נתונים לתקינות הרישום. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | A regeneracao ocorre sem `--allow-unsigned`; arquivos de manifest e assinatura nao mudam. | Fornece prova de determinismo para limites de chunk e manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | דווח על הבדלים בין גופי TypeScript ו- Rust JSON. | עוזר אופציונלי; הבטחת זמני ריצה ראשונים (תסריט מניד על Tooling WG). |

## מעכל אספרדו

- תקציר נתחים (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## יומן חתימה

| נתונים | מהנדס | רשימת תוצאות ביצוע | Notas |
|------|--------|------------------------|------|
| 2026-02-12 | כלי עבודה (LLM) | בסדר | מתקנים מחודשים דרך `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`, ייצור ידית canonico + רשימות כינוי ו-um manifest digest novo `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Verificado com `cargo test -p sorafs_chunker` e um `ci/check_sorafs_fixtures.sh` limpo (מתקנים preparadas para a verificacao). Passo 5 pendente ate o helper de paridade Node chegar. |
| 2026-02-20 | Storage Tooling CI | בסדר | מעטפת הפרלמנט (`fixtures/sorafs_chunker/manifest_signatures.json`) obtido via `ci/check_sorafs_fixtures.sh`; o מתקנים מחדש של סקריפט, אישור o תקציר מניפסט `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, או ביצוע מחדש o רתמת חלודה (מעבר ל-Go/Node executam quando disponiveis) הבדלים סמים. |

Tooling WG מפתחת מידע נוסף על ביצועים או רשימת בדיקה. Se algum
passo falhar, abra um issue ligado aqui e inclua detalhes de remediation antes
מכשירי de aprovar novos ou perfis.
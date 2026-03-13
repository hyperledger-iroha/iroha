---
lang: he
direction: rtl
source: docs/source/zk/proof_retention.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 16b5b6bd9cf1bfdc901d0aa3c863addd77e033a0e91a8187fa354b4d22058bf9
source_last_modified: "2026-01-22T15:38:30.730892+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/zk/proof_retention.md -->

% שמירת הוכחות וגיזום

Iroha שומרת רישום של תוצאות אימות הוכחות (backend + hash) לצורכי ביקורת ושחזור.
השימור נאכף דטרמיניסטית בתוך מסלול הקונצנזוס.

## תצורה

ה‑`zk` config שולט בשימור:

- `zk.proof_history_cap` — מקסימום רשומות לכל backend (0 = ללא הגבלה)
- `zk.proof_retention_grace_blocks` — מינימום בלוקים לשמירה לפני גיזום לפי גיל
- `zk.proof_prune_batch` — מקסימום הסרות בכל מעבר אכיפה (0 = ללא הגבלה)

ערכים אלו נקראים מ‑`iroha_config` (user → actual → defaults).

## אכיפה

- **בהוספה:** `VerifyProof` אוכף את ה‑cap/grace/batch עבור ה‑backend של ההוכחה לאחר רישום הרשומה החדשה.
- **ידני:** ההוראה החדשה `PruneProofs` גוזמת את כל ה‑backends (או backend יחיד כאשר סופק) לפי אותה מדיניות. השתמשו בעזר ה‑CLI:

  ```bash
  iroha app zk proofs prune --backend halo2/ipa
  ```

שני הנתיבים פולטים `ProofEvent::Pruned` עם ה‑backend, מזהים שהוסרו (תחומים לפי
`prune_batch`), ספירה נותרת, cap/grace/batch, גובה, סמכות ומקור (`Insert` או
`Manual`) עבור נתיבי ביקורת.

## חשיפה וכלי עבודה

- נקודת סטטוס: `GET /v2/proofs/retention` מחזירה caps, grace, prune_batch,
  סך רשומות, סך ניתנות לגיזום, וספירות לכל backend.
- CLI: `iroha app zk proofs retention` (סטטוס) ו‑`iroha app zk proofs prune` (אכיפה ידנית).
- אירועים: הירשמו ל‑`DataEvent::Proof(ProofEvent::Pruned)` דרך מסנני SSE/WS כדי לעקוב אחרי פעילות גיזום.

הגיזום דטרמיניסטי ורץ בתוך ביצוע בלוק, כך שכל העמיתים מתכנסים לאותו סט נשמר.
מזהי הוכחות שהוסרו נמחקים גם ממדדי התגים.

</div>

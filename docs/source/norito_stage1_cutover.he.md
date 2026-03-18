---
lang: he
direction: rtl
source: docs/source/norito_stage1_cutover.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f02eb4d7d5fa1a5ffef5c285cd1722799dcd0fd1fce852cd12eb6b9cf477f51
source_last_modified: "2026-01-03T18:07:57.754211+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/norito_stage1_cutover.md -->

## חיתוך Norito שלב 1 (מרץ 2026)

הערה זו מתעדת את הכוונון העדכני של ספי שלב 1 (אינדקס מבני JSON) ושל CRC64 GPU.
הבנצ'מרקים נאספו על Apple Silicon באמצעות:

```
cargo run -p norito --example stage1_cutover --release --features bench-internal \
  > benchmarks/norito_stage1/cutover.csv
```

העזר מפיק שורות `bytes,scalar_ns,kernel_ns,ns_per_byte_*`. ממצאים עיקריים:
- חציית Scalar מול SIMD מתרחשת כעת סביב **6–8 KiB**; ב‑~4 KiB SIMD איטי בכ‑~5%, אך מ‑8 KiB ומעלה הוא מהיר פי ~2.
- תקורת ההפעלה של GPU בשלב 1 מתאמורטת סביב **~192 KiB** במארח הזה עם קרנלים מקוטעים של CRC64/Stage-1; שמירה על cutoff מעל לכך מונעת thrashing ומאפשרת למסמכים גדולים להשתמש ב‑helper dylibs כאשר הם זמינים.
- עזרי CRC64 GPU נהנים מאותו guard של 192 KiB וכעת תומכים בעוקף env לנתיב העוזר כדי שבדיקות וכלי downstream יוכלו להזריק מאיצים מדומים.

ברירות המחדל שנקבעו:
- סף קלט קטן של `build_struct_index` הועלה ל‑**4096 bytes**.
- מינימום GPU לשלב 1: **192 KiB** (`NORITO_STAGE1_GPU_MIN_BYTES` להחלפה).
- מינימום GPU ל‑CRC64: **192 KiB** (`NORITO_GPU_CRC64_MIN_BYTES` להחלפה).
- טוען ה‑CRC64 GPU מקבל כעת את `NORITO_CRC64_GPU_LIB` כדי להצביע על helper מותאם
  (משמש בבדיקת parity עם stubs).

ראו את `benchmarks/norito_stage1/cutover.csv` למדידות הגולמיות.

</div>

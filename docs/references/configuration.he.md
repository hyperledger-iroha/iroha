<!-- Hebrew translation of docs/references/configuration.md -->

---
lang: he
direction: rtl
source: docs/references/configuration.md
status: complete
translator: manual
---

<div dir="rtl">

# האצה

הסעיף `[accel]` שולט בהאצת חומרה אופציונלית עבור ה-IVM והכלים הנלווים. לכל מסלול מואץ קיימת נסיגת CPU דטרמיניסטית; אם ממשק מאיץ נכשל במבחן זהב עצמי בזמן ריצה הוא מושבת אוטומטית והביצוע ממשיך על המעבד.

- `enable_cuda` (ברירת מחדל: true) – שימוש ב-CUDA כאשר הקוד קומפל ונגיש.
- `enable_metal` (ברירת מחדל: true) – שימוש ב-Metal ב-macOS כשזמין.
- `max_gpus` (ברירת מחדל: 0) – מספר ה-GPU המרבי לאתחול; ערך `0` פירושו אוטומטי/ללא מגבלה.
- `merkle_min_leaves_gpu` (ברירת מחדל: 8192) – מספר העלים המינימלי להאצת גיבוב Merkle ל-GPU. הורידו רק עבור GPU מהירים במיוחד.
- מתקדם (רשות; לרוב נשארים עם ברירות המחדל):
  - `merkle_min_leaves_metal` (ברירת מחדל: יורש את `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (ברירת מחדל: יורש את `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (ברירת מחדל: 32768) – מעדיף SHA-2 במעבד עד גודל זה ב-ARMv8 עם הוראות SHA2.
  - `prefer_cpu_sha2_max_leaves_x86` (ברירת מחדל: 32768) – מעדיף SHA-NI במעבד עד גודל זה בארכיטקטורת x86/x86_64.

הערות
- דטרמיניזם לפני הכול: האצה אינה משנה תצפיות. הממשקים מריצים מבחני זהב בעת האתחול ונופלים לסקלרי/‏SIMD אם מתגלה אי-התאמה.
- מגדירים דרך `iroha_config`; הימנעו ממשתני סביבה במסלולי ייצור.
</div>

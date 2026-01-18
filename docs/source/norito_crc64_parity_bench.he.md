<!-- Hebrew translation of docs/source/norito_crc64_parity_bench.md -->

---
lang: he
direction: rtl
source: docs/source/norito_crc64_parity_bench.md
status: complete
translator: manual
---

<div dir="rtl">

# Norito CRC64 — מדריך פריטי ובנצ׳מרק

המדריך מסביר כיצד לוודא התאמה (parity) בין CRC64 בחומרה/פולבק של Norito וכיצד למדוד את ביצועי CRC64 והקידוד/פענוח במכונה המקומית.

Norito מחשב CRC64-XZ (`0x42F0E1EBA9EA3693`) באמצעות הקרייט `crc64fast`. הפונקציות `hardware_crc64` ו-`crc64_fallback` זהות, ולכן התוצאה עקבית בכל פלטפורמה.

## 1) בדיקות פריטי מהירות

- הרצת בדיקות יחידה/אינטגרציה:

```bash
cargo test -p norito --test crc64
cargo test -p norito --test crc64_prop
```

- כפיית האצה (פלאגים של פיתוח בלבד):

```bash
# x86_64
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo test -p norito --test crc64
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo test -p norito --test crc64_prop

# aarch64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo test -p norito --test crc64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo test -p norito --test crc64_prop
```

טיפ: לבדיקת טווחי אורך קטנים — `cargo test -p norito crc64_prop::random_lengths_parity_small -- --nocapture`.

## 2) סקריפט פריטי מאולתר

```rust
use norito::{crc64_fallback, hardware_crc64};

fn main() {
    let sizes = [0usize, 1, 7, 8, 15, 16, 31, 32, 63, 64, 511, 512, 4095, 4096];
    let mut ok = true;
    for &n in &sizes {
        let mut buf = vec![0u8; n];
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for b in &mut buf { x ^= x << 7; x ^= x >> 9; *b = (x as u8) ^ 0xA5; }
        let hw = hardware_crc64(&buf);
        let fb = crc64_fallback(&buf);
        if hw != fb { eprintln!("mismatch at n={n}: hw={hw:016X} fb={fb:016X}"); ok = false; break; }
    }
    if ok { println!("parity OK across sizes: {:?}", sizes); }
}
```

## 3) בנצ׳מרק תפוקת CRC64

```bash
cargo bench -p norito -- benches::bench_crc64 -- --warm-up-time 1 --sample-size 30

RUSTFLAGS='-C target-feature=+neon,+aes' \
  cargo bench -p norito -- benches::bench_crc64 -- --warm-up-time 1 --sample-size 30
```

המספרים תלויים ב-CPU, בתדר ובתפוקת הזיכרון.

## 4) קידוד/פענוח מקצה לקצה

`crates/norito/benches/codec.rs` משווה Norito ל-SCALE ו-bincode (כולל גרסאות דחוסות).

```bash
cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30

# x86_64
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30
# aarch64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30
```

התוצאות:
- Norito כולל כותרת/Hash/CRC64, לכן לפayload זעירים הוא מעט איטי יותר אך בפayload גדולים הוא תחרותי.
- בנצ׳מרק הדחיסה מודד גם את overhead הדחיסה (Norito משתמש ברמת “fast” כברירת מחדל).

## 5) עצות
- לקיצור זמני בנייה: להריץ בדיקות ממוקדות (`cargo test -p norito --lib`).
- עבודה ללא רשת: להוסיף `--offline`.
- Apple Silicon: ודאו `aarch64-apple-darwin`.
- הדטרמיניזם של CRC נבדק ב-CI באמצעות בדיקות הפריטי.

---

שמרו תוצאות בנצ׳מרק (reports של Criterion) יחד עם פרטי חומרה, OS וגרסת קומפיילר לטובת השוואות עתידיות.

</div>

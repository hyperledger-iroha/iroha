---
lang: uz
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T19:17:13.237630+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM bayt kod sarlavhasi


Sehrli
- 4 bayt: ASCII `IVM\0` 0 ofsetda.

Tartib (joriy)
- Ofsetlar va o'lchamlar (jami 17 bayt):
  - 0..4: sehrli `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (xususiyatlar bitlari; pastga qarang)
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (kichik-endian)
  - 16: `abi_version: u8`

Rejim bitlari
- `ZK = 0x01`, `VECTOR = 0x02`, `HTM = 0x04` (zahiralangan/xususiyatga ega).

Maydonlar (ma'nosi)
- `abi_version`: tizimli qo'ng'iroqlar jadvali va ko'rsatgich-ABI sxemasi versiyasi.
- `mode`: ZK tracing/VECTOR/HTM uchun xususiyat bitlari.
- `vector_length`: vektor operatsiyalari uchun mantiqiy vektor uzunligi (0 → o'rnatilmagan).
- `max_cycles`: ZK rejimida va qabul qilishda foydalaniladigan ijro to'ldiruvchisi.

Eslatmalar
- Endianness va tartib amalga oshirish bilan belgilanadi va `version` bilan bog'lanadi. Yuqoridagi simli tartib `crates/ivm_abi/src/metadata.rs` da joriy dasturni aks ettiradi.
- Minimal o'quvchi joriy artefaktlar uchun ushbu tartibga tayanishi mumkin va kelajakdagi o'zgarishlarni `version` orqali boshqarishi kerak.
- Uskuna tezlashuvi (SIMD/Metal/CUDA) har bir xost uchun tanlanadi. Ishlash vaqti `iroha_config` dan `AccelerationConfig` qiymatlarini o'qiydi: `enable_simd` noto'g'ri bo'lsa, skalyar qaytarilishlarni majbur qiladi, `enable_metal` va `enable_cuda` eshiklari esa, hatto ularning kompilyatsiya panellarida ham qo'llaniladi. VM yaratishdan oldin `ivm::set_acceleration_config`.
- Mobil SDK (Android/Swift) bir xil tugmachalarni yuzaga chiqaradi; `IrohaSwift.AccelerationSettings`
  `connect_norito_set_acceleration_config` qo'ng'iroq qiladi, shuning uchun macOS/iOS tuzilmalari Metal /
  NEON deterministik zaxiralarni saqlagan holda.
- Operatorlar, shuningdek, `IVM_DISABLE_METAL=1` yoki `IVM_DISABLE_CUDA=1` ni eksport qilish orqali diagnostika uchun maxsus backendlarni majburiy o'chirib qo'yishlari mumkin. Ushbu muhitni bekor qilish konfiguratsiyadan ustun turadi va VMni deterministik CPU yo'lida ushlab turadi.

Bardoshli holat yordamchilari va ABI yuzasi
- Bardoshli holat yordamchi tizim qoʻngʻiroqlari (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* va JSON/SCHEMA encode/decode) V1 ABI qismidir va `abi_hash` kompilyatsiyasiga kiritilgan.
- CoreHost simlari STATE_{GET,SET,DEL} - WSV tomonidan qo'llab-quvvatlanadigan mustahkam smart-kontrakt holatiga; Dev/sinov xostlari qoplamalar yoki mahalliy qat'iylikdan foydalanishi mumkin, lekin bir xil kuzatilishi mumkin bo'lgan xatti-harakatni saqlashi kerak.

Tasdiqlash
- Tugunni qabul qilish faqat `version_major = 1` va `version_minor = 0` sarlavhalarini qabul qiladi.
- `mode` faqat ma'lum bitlarni o'z ichiga olishi kerak: `ZK`, `VECTOR`, `HTM` (noma'lum bitlar rad etiladi).
- `vector_length` maslahatdir va hatto `VECTOR` biti o'rnatilmagan bo'lsa ham nolga teng bo'lishi mumkin; qabul qilish faqat yuqori chegarani amalga oshiradi.
- Qo'llab-quvvatlanadigan `abi_version` qiymatlari: birinchi versiya faqat `1` (V1) ni qabul qiladi; boshqa qiymatlar qabul paytida rad etiladi.

### Siyosat (yaratilgan)
Quyidagi siyosat xulosasi amalga oshirish natijasida hosil bo'ladi va uni qo'lda tahrirlash mumkin emas.<!-- BEGIN GENERATED HEADER POLICY -->
| Maydon | Siyosat |
|---|---|
| version_major | 1 |
| versiya_minor | 0 |
| rejim (ma'lum bitlar) | 0x07 (ZK=0x01, VEKTOR=0x02, HTM=0x04) |
| abi_version | 1 |
| vektor_uzunligi | 0 yoki 1..=64 (maslahat; VEKTOR bitidan mustaqil) |
<!-- END GENERATED HEADER POLICY -->

### ABI xeshlari (yaratilgan)
Quyidagi jadval amalga oshirish natijasida yaratilgan va qo'llab-quvvatlanadigan siyosatlar uchun kanonik `abi_hash` qiymatlari ro'yxati.

<!-- BEGIN GENERATED ABI HASHES -->
| Siyosat | abi_hash (hex) |
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- Kichik yangilanishlar `feature_bits` orqasida ko'rsatmalar va ajratilgan opcode maydoni qo'shishi mumkin; asosiy yangilanishlar kodlashni o'zgartirishi yoki faqat protokolni yangilash bilan birga olib tashlashi/qayta belgilashi mumkin.
- Syscall diapazonlari barqaror; faol `abi_version` uchun noma'lum `E_SCALL_UNKNOWN` hosil qiladi.
- Gaz jadvallari `version` ga bog'langan va o'zgarishda oltin vektorlarni talab qiladi.

Artefaktlarni tekshirish
- Sarlavha maydonlarining barqaror ko'rinishi uchun `ivm_tool inspect <file.to>` dan foydalaning.
- Rivojlanish uchun misollar / o'rnatilgan artefaktlar ustidan tekshiruvni amalga oshiradigan kichik Makefile maqsadi `examples-inspect` ni o'z ichiga oladi.

Misol (Rust): minimal sehr + o'lchamni tekshirish

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

Eslatma: Sehrgarlikdan tashqari aniq sarlavha tartibi versiyalashtirilgan va amalga oshirish uchun belgilangan; barqaror maydon nomlari va qiymatlari uchun `ivm_tool inspect` ni afzal ko'ring.
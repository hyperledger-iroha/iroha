---
lang: uz
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2025-12-29T18:16:35.943754+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM funksiyasini ishga tushirish va telemetriya nazorat roʻyxati

Ushbu nazorat roʻyxati SRE va operator jamoalariga SM (SM2/SM3/SM4) funksiyasini yoqishga yordam beradi
audit va muvofiqlik eshiklari tozalangandan so'ng xavfsiz tarzda o'rnating. Ushbu hujjatga amal qiling
`docs/source/crypto/sm_program.md` da konfiguratsiya qisqacha va
`docs/source/crypto/sm_compliance_brief.md` da huquqiy/eksport yo'riqnomasi.

## 1. Parvoz oldidan tayyorgarlik
- [ ] Ish joyini chiqarish eslatmalarida `sm` ko'rsatilishini faqat tasdiqlash yoki imzolash sifatida tasdiqlang,
      ishlab chiqarish bosqichiga bog'liq.
- [ ] Filoni oʻz ichiga olgan majburiyat asosida qurilgan binarlar ishlayotganligini tekshiring
      SM telemetriya hisoblagichlari va konfiguratsiya tugmalari. (TBD maqsadini chiqarish; trek
      tarqatish chiptasida.)
- [ ] `scripts/sm_perf.sh --tolerance 0.25` ni bosqichma-bosqich tugunda ishga tushiring (har bir maqsad uchun
      arxitektura) va xulosani arxivlash. Skript endi avtomatik tanlanadi
      tezlashtirish rejimlari uchun taqqoslash maqsadi sifatida skalyar bazaviy chiziq
      (SM3 NEON ishlayotgan paytda `--compare-tolerance` standarti 5,25 ga);
      birlamchi yoki taqqoslash bo'lsa, tekshirish yoki tarqatishni bloklash
      qo'riqchi muvaffaqiyatsiz. Linux/aarch64 Neoverse uskunasida suratga olayotganda, o'ting
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      eksport qilingan `m3-pro-native` medianlarini xostni ushlash bilan qayta yozish uchun
      jo'natishdan oldin.
- [ ] `status.md` ga ishonch hosil qiling va chiqish chiptasi muvofiqlik arizalarini yozib oling
      ularni talab qiladigan yurisdiktsiyalarda ishlaydigan har qanday tugunlar (muvofiqlik qisqachasini ko'ring).
- [ ] Agar validatorlar SM kirish kalitlarini saqlasa, KMS/HSM yangilanishlarini tayyorlang
      apparat modullari.

## 2. Konfiguratsiya o'zgarishlari
1. SM2 kalit inventarini va joylashtirishga tayyor parchani yaratish uchun xtask yordamchisini ishga tushiring:
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   Chiqishlarni tekshirish kerak bo'lganda stdout-ga uzatish uchun `--snippet-out -` (va ixtiyoriy ravishda `--json-out -`) dan foydalaning.
   Agar siz quyi darajadagi CLI buyruqlarini qo'lda boshqarishni afzal ko'rsangiz, ekvivalent oqim:
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   Agar `jq` mavjud bo'lmasa, `sm2-key.json` ni oching, `private_key_hex` qiymatidan nusxa oling va uni to'g'ridan-to'g'ri eksport buyrug'iga o'tkazing.
2. Olingan parchani har bir tugun konfiguratsiyasiga qo'shing (qiymatlar
   faqat tekshirish bosqichi; muhitga qarab sozlang va kalitlarni ko'rsatilgandek tartiblangan holda saqlang):
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. Tugunni qayta ishga tushiring va kutilgandek `crypto.sm_helpers_available` va (agar siz oldindan ko'rish orqa qismini yoqsangiz) `crypto.sm_openssl_preview_enabled` sirtini tasdiqlang:
   - `/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`).
   - Har bir tugun uchun ko'rsatilgan `config.toml`.
4. Ruxsat berilganlar roʻyxatiga SM algoritmlarini qoʻshish uchun manifestlar/genezis yozuvlarini yangilang.
   imzolash keyinroq tarqatishda yoqiladi. `--genesis-manifest-json` dan foydalanilganda
   Oldindan imzolangan genezis blokisiz, `irohad` endi ish vaqti kriptosini ishlab chiqaradi
   manifestning `crypto` blokidan to'g'ridan-to'g'ri suratga olish - manifestning mavjudligiga ishonch hosil qiling
   oldinga siljishdan oldin o'zgartirish rejangizni tekshiring.## 3. Telemetriya va monitoring
- Prometheus so'nggi nuqtalarini qirib tashlang va quyidagi hisoblagichlar/ko'rsatkichlar paydo bo'lishiga ishonch hosil qiling:
  - `iroha_sm_syscall_total{kind="verify"}`
  - `iroha_sm_syscall_total{kind="hash"}`
  - `iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview` (oldindan ko'rish o'tish holatini bildiruvchi 0/1 o'lchov)
  - `iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- SM2 imzolash yoqilgandan so'ng kanca imzolash yo'li; uchun hisoblagichlarni qo'shing
  `iroha_sm_sign_total` va `iroha_sm_sign_failures_total`.
- Quyidagilar uchun Grafana asboblar paneli/ogohlantirishlarini yarating:
  - Ishlamay qolgan hisoblagichlardagi tikanlar (deraza 5 m).
  - SM syscall o'tkazish qobiliyatining keskin pasayishi.
  - Tugunlar orasidagi farqlar (masalan, mos kelmaydigan yoqish).

## 4. Chiqarish bosqichlari
| Bosqich | Amallar | Eslatmalar |
|-------|---------|-------|
| Faqat tekshirish uchun | `crypto.default_hash` ni `sm3-256` ga yangilang, `allowed_signing` ni `sm2` holda qoldiring, tekshirish hisoblagichlarini kuzatib boring. | Maqsad: konsensus farqini xavf ostiga qo'ymasdan, SM tekshirish yo'llarini mashq qiling. |
| Aralash imzo uchuvchisi | Cheklangan SM imzolanishiga ruxsat berish (tasdiqlovchilarning quyi to'plami); imzolash hisoblagichlari va kechikish vaqtini kuzatib boring. | Ed25519 ga qaytish mavjudligini ta'minlash; telemetriya mos kelmasligini ko'rsatsa, to'xtating. |
| GA imzolash | `allowed_signing` ni `sm2`, manifestlarni/SDKlarni yangilash va yakuniy ish kitobini nashr qilish uchun kengaytiring. | Yopiq audit natijalarini, yangilangan muvofiqlik hujjatlarini va barqaror telemetriyani talab qiladi. |

### Tayyorlik sharhlari
- **Faqat tayyorlikni tekshirish (SM-RR1).** Release Eng, Crypto WG, Ops va Legalni chaqirish. Talab qilish:
  - `status.md` muvofiqlik topshirish holatini qayd etadi + OpenSSL kelib chiqishi.
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / ushbu nazorat ro'yxati oxirgi nashr oynasida yangilangan.
  - `defaults/genesis` yoki atrof-muhitga xos manifestda `crypto.allowed_signing = ["ed25519","sm2"]` va `crypto.default_hash = "sm3-256"` (yoki hali birinchi bosqichda bo'lsa, `sm2`siz faqat tekshirish varianti) ko'rsatiladi.
  - Chiptaga biriktirilgan `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` jurnallari.
  - Telemetriya asboblar paneli (`iroha_sm_*`) barqaror holatdagi xatti-harakatlar uchun ko'rib chiqildi.
- **Uchuvchining tayyorligini imzolash (SM-RR2).** Qo'shimcha eshiklar:
  - Xavfsizlik tomonidan imzolangan RustCrypto SM stek yopilgan yoki kompensatsiya boshqaruvlari uchun RFC uchun audit hisoboti.
  - Qayta tiklash/qayta tiklash qadamlari bilan yangilangan operator ish kitoblari (ob'ektga xos).
  - Uchuvchi kohort uchun Ibtido manifestlariga `allowed_signing = ["ed25519","sm2"]` kiradi va ruxsat etilganlar ro'yxati har bir tugun konfiguratsiyasida aks ettiriladi.
  - Chiqish/orqaga qaytarish rejasi hujjatlashtirilgan (`allowed_signing`-ni Ed25519-ga o'zgartirish, manifestlarni tiklash, asboblar panelini tiklash).
- **GA tayyorligi (SM-RR3).** Ijobiy pilot hisoboti, barcha validator yurisdiktsiyalari uchun yangilangan muvofiqlik arizalari, imzolangan telemetriya asoslari va Release Eng + Crypto WG + Ops/Legal triadasidan reliz chiptasini tasdiqlash talab etiladi.## 5. Qadoqlash va muvofiqlikni tekshirish ro'yxati
- **OpenSSL/Tongsuo artefaktlari to‘plami.** OpenSSL/Tongsuo 3.0+ umumiy kutubxonalarini (`libcrypto`/`libssl`) har bir validator paketi bilan jo‘natish yoki tizimning aniq bog‘liqligini hujjatlashtirish. Auditorlar yetkazib beruvchi tuzilmasini kuzatishi uchun versiyani yozib oling, bayroqlarni yarating va SHA256 nazorat summalarini reliz manifestiga yozing.
- **CI davomida tasdiqlang.** Har bir maqsadli platformadagi qadoqlangan artefaktlarga qarshi `scripts/sm_openssl_smoke.sh` ni bajaradigan CI qadamini qo'shing. Agar oldindan ko'rish bayrog'i yoqilgan bo'lsa, lekin provayder ishga tushirilmasa (sarlavhalar etishmayotgan, qo'llab-quvvatlanmaydigan algoritm va h.k.) ish bajarilmasligi kerak.
- **Muvofiqlik eslatmalarini nashr eting.** Yangilash eslatmalarini / `status.md` provayder versiyasi, eksport-nazorat ma'lumotnomalari (GM/T, GB/T) va SM algoritmlari uchun talab qilinadigan har qanday yurisdiktsiyaga oid hujjatlar bilan.
- **Operator runbook yangilanishlari.** Yangilanish jarayonini hujjatlashtiring: yangi umumiy obʼyektlarni sahnalashtiring, `crypto.enable_sm_openssl_preview = true` bilan tengdoshlarni qayta ishga tushiring, `/status` maydonini va `iroha_sm_openssl_preview` oʻlchagichni `iroha_sm_openssl_preview` ga oʻtkazing va qayta tiklash rejasini saqlang to'plam) agar oldindan ko'rish telemetriyasi flot bo'ylab chetga chiqsa.
- **Dalillarni saqlash.** Kelgusi tekshiruvlar kelib chiqish zanjirini takrorlashi uchun validator chiqarish artefaktlari bilan birga OpenSSL/Tongsuo paketlari uchun tuzilish jurnallari va imzolash sertifikatlarini arxivlang.

## 6. Hodisaga javob
- **Tasdiqlashda xatolik yuz berdi:** SM qo‘llab-quvvatlamasdan tuzilishga qayting yoki `sm2`ni olib tashlang
  `allowed_signing` dan (kerak bo'lganda `default_hash` ga qaytariladi) va oldingisiga o'tish
  tergov paytida ozod qilish. Muvaffaqiyatsiz yuklarni, qiyosiy xeshlarni va tugun jurnallarini yozib oling.
- **Umumiy regressiyalar:** SM ko'rsatkichlarini Ed25519/SHA2 asosiy ko'rsatkichlari bilan solishtiring.
  Agar ARM ichki yo'li farqni keltirib chiqarsa, `crypto.sm_intrinsics = "force-disable"` ni o'rnating.
  (xususiyati o'tish-o'tish kutilayotgan amalga oshirish) va xulosalar haqida hisobot.
- **Telemetriya bo'shliqlari:** Agar hisoblagichlar yo'q bo'lsa yoki yangilanmasa, muammoni yozing
  Release Engineering-ga qarshi; bo'shliqqa qadar kengroq rollout bilan davom qilmang
  hal qilinadi.

## 7. Tekshirish ro'yxati shabloni
- [ ] Konfiguratsiya bosqichma-bosqich va qayta ishga tushirildi.
- [ ] Telemetriya hisoblagichlari ko'rinadi va asboblar paneli sozlangan.
- [ ] Muvofiqlik/huquqiy qadamlar qayd etilgan.
- [ ] Crypto WG / Release TL tomonidan tasdiqlangan ishlab chiqarish bosqichi.
- [ ] Chiqarishdan keyingi tekshiruv yakunlandi va topilmalar hujjatlashtirildi.

Chiptadagi ushbu nazorat ro'yxatini saqlang va `status.md` ni yangilang.
fazalar orasidagi flot o'tishlari.
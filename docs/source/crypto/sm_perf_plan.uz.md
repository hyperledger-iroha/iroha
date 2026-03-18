---
lang: uz
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2025-12-29T18:16:35.944695+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## SM Performance Capture & Baseline Plan

Holati: Tayyorlangan — 2025-05-18  
Egalari: Performance WG (etakchi), Infra Ops (laboratoriyani rejalashtirish), QA Guild (CI gating)  
Tegishli yoʻl xaritasi vazifalari: SM-4c.1a/b, SM-5a.3b, FASTPQ 7-bosqich qurilmalar oʻrtasida suratga olish

### 1. Maqsadlar
1. `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json` da Neoverse medianlarini yozib oling. Joriy tayanch chiziqlar `neoverse-proxy-macos` suratga olishdan `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (CPU yorlig‘i `neoverse-proxy-macos`) ostida eksport qilinadi, SM3 solishtirish tolerantligi aarch64 macOS/Linux uchun 0,70 gacha kengaytiriladi. Yalang'och metall vaqti ochilganda, Neoverse xostida `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` ni qayta ishga tushiring va yig'ilgan medianalarni asosiy chiziqqa aylantiring.  
2. `ci/check_sm_perf.sh` ikkala xost sinfini himoya qila olishi uchun mos keladigan x86_64 medianlarini to'plang.  
3. Kelajakdagi perf eshiklari qabilaviy bilimlarga tayanmasligi uchun takrorlanadigan suratga olish protsedurasini (buyruqlar, artefakt tartibi, sharhlovchilar) nashr eting.

### 2. Uskunaning mavjudligi
Joriy ish maydonida faqat Apple Silicon (macOS arm64) xostlariga kirish mumkin. `neoverse-proxy-macos` suratga olish vaqtinchalik Linux bazasi sifatida eksport qilinadi, lekin yalang'och metall Neoverse yoki x86_64 medianalarini olish uchun baribir `INFRA-2751` ostida kuzatilgan umumiy laboratoriya apparati talab qilinadi, laboratoriya oynasi ochilgandan so'ng Performance WG tomonidan boshqariladi. Qolgan suratga olish oynalari endi artefakt daraxtida band qilingan va kuzatilgan:

- Neoverse N2 yalang'och metall (Tokio rack B) 2026-03-12 uchun bron qilingan. Operatorlar 3-bo'limdagi buyruqlarni qayta ishlatadilar va artefaktlarni `artifacts/sm_perf/2026-03-lab/neoverse-b01/` ostida saqlaydilar.
- x86_64 Xeon (Tsyurix rack D) shovqinni kamaytirish uchun SMT o'chirilgan holda 2026-03-19 uchun bron qilingan; artefaktlar `artifacts/sm_perf/2026-03-lab/xeon-d01/` ostida tushadi.
- Ikkala yugurishdan so'ng, medianlarni asosiy JSON-larga targ'ib qiling va `ci/check_sm_perf.sh` da CI darvozasini yoqing (maqsadga o'tish sanasi: 2026-03-25).

Ushbu sanalargacha faqat macOS arm64 bazasini mahalliy sifatida yangilash mumkin.### 3. Rasmga olish tartibi
1. **Asboblar zanjirlarini sinxronlash**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **Tasvirga olish matritsasini yaratish** (har bir xost uchun)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   Yordamchi endi maqsadli katalog ostida `capture_commands.sh` va `capture_plan.json` ni yozadi. Skript har bir rejimda `raw/*.json` suratga olish yo'llarini o'rnatadi, shuning uchun laboratoriya texniklari yugurishlarni aniq to'plashlari mumkin.
3. **Tasvirlarni ishga tushirish**  
   `capture_commands.sh` dan har bir buyruqni bajaring (yoki ekvivalentini qo'lda ishga tushiring), har bir rejim `--capture-json` orqali tuzilgan JSON blobini chiqarishiga ishonch hosil qiling. Har doim `--cpu-label "<model/bin>"` (yoki `SM_PERF_CPU_LABEL=<label>`) orqali xost yorlig'ini taqdim eting, shuning uchun qo'lga olish metama'lumotlari va keyingi asosiy ko'rsatkichlar medianalarni yaratgan aniq uskunani qayd etadi. Yordamchi allaqachon tegishli yo'lni taqdim etadi; qo'lda ishlash uchun naqsh quyidagicha:
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **Natijalarni tasdiqlash**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   Yugurishlar orasidagi farq ±3% ichida qolishiga ishonch hosil qiling. Agar yo'q bo'lsa, ta'sirlangan rejimni qayta ishga tushiring va jurnalga qayta urinib ko'ring.
5. **Medianalarni rag'batlantirish**  
   Medianlarni hisoblash va ularni asosiy JSON fayllariga nusxalash uchun `scripts/sm_perf_aggregate.py` dan foydalaning:
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   Yordamchi guruhlar `metadata.mode` tomonidan suratga oladi, har bir to'plam o'xshashligini tasdiqlaydi
   bir xil `{target_arch, target_os}` uch barobar va bitta yozuv bilan JSON xulosasini chiqaradi
   rejim uchun. Asosiy fayllarga tushishi kerak bo'lgan medianalar ostida yashaydi
   `modes.<mode>.benchmarks`, u bilan birga kelgan `statistics` blok yozuvlari esa
   to'liq namuna ro'yxati, sharhlovchilar va CI uchun min/maks, o'rtacha va aholi stdev.
   Birlashtirilgan fayl mavjud bo'lgach, siz asosiy JSON-larni avtomatik ravishda yozishingiz mumkin (
   standart bardoshlik xaritasi) orqali:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   `--mode` ni kichik toʻplam bilan cheklash yoki `--cpu-label` ni qadash uchun bekor qilish
   jamlangan manba uni o'tkazib yuborsa, qayd etilgan CPU nomi.
   Ikkala xost ham arxitektura tugagach, yangilang:
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (yangi)

   `aarch64_unknown_linux_gnu_*` fayllari endi `m3-pro-native` ni aks ettiradi
   yozib olish (protsessor yorlig'i va metama'lumotlar qaydlari saqlanib qolgan), shuning uchun `scripts/sm_perf.sh`
   aarch64-noma'lum-linux-gnu xostlarini qo'lda bayroqlarsiz avtomatik aniqlash. Qachon
   yalang'och metall laboratoriya ishga tushirildi, `scripts/sm_perf.sh --mode  ni qayta ishga tushiring
   --write-baseline boxes/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   oraliq medianlarning ustiga yozish va haqiqiyni muhrlash uchun yangi suratlar bilan
   xost belgisi.

   > Ma'lumotnoma: 2025 yil iyul oyida Apple Silicon capture (CPU yorlig'i `m3-pro-local`)
   > `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}` ostida arxivlangan.
   > Neoverse/x86 artefaktlarini nashr qilganingizda ushbu tartibni aks ettiring
   > xom/jamlangan natijalarni doimiy ravishda farqlashi mumkin.

### 4. Artefact Layout & Sign-off
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` buyruqlar xeshini, git reviziyasini, operatorni va har qanday anomaliyalarni qayd qiladi.
- Birlashtirilgan JSON fayllari to'g'ridan-to'g'ri asosiy yangilanishlarga kiradi va `docs/source/crypto/sm_perf_baseline_comparison.md` da ishlash ko'rib chiqilishiga biriktirilgan.
- QA gildiyasi asarlar o'zgarishidan oldin artefaktlarni ko'rib chiqadi va `status.md` da "Ishlash" bo'limi ostida imzo chekadi.### 5. CI Gating Xronologiyasi
| Sana | Muhim bosqich | Harakat |
|------|-----------|--------|
| 2025-07-12 | Neoverse suratga olish tugallandi | `sm_perf_baseline_aarch64_*` JSON fayllarini yangilang, `ci/check_sm_perf.sh`ni mahalliy sifatida ishga tushiring, artefaktlar biriktirilgan holda PRni oching. |
| 2025-07-24 | x86_64 to'liq suratga oladi | `ci/check_sm_perf.sh` da yangi asosiy fayllarni + gating qo'shing; kesishgan CI yo'laklari ularni iste'mol qilishiga ishonch hosil qiling. |
| 27.07.2025 | CI ijrosi | `sm-perf-gate` ish jarayonini ikkala xost sinfida ham ishga tushirish; Agar regressiyalar konfiguratsiya qilingan tolerantliklardan oshsa, birlashmalar muvaffaqiyatsiz tugadi. |

### 6. Bog'liqlar va aloqa
- `infra-ops@iroha.tech` orqali laboratoriyaga kirish o'zgarishlarini muvofiqlashtiring.  
- Performance WG har kuni yangilanishlarni `#perf-lab` kanalida suratga olish paytida joylashtiradi.  
- QA gildiyasi taqqoslash farqini (`scripts/sm_perf_compare.py`) tayyorlaydi, shuning uchun sharhlovchilar deltalarni ko'rishlari mumkin.  
- Asosiy chiziqlar birlashgandan so'ng, `roadmap.md` (SM-4c.1a/b, SM-5a.3b) va `status.md` ni suratga olishni yakunlash qaydlari bilan yangilang.

Ushbu reja bilan SM tezlashtirish ishi takrorlanadigan medianlar, CI chegarasi va kuzatilishi mumkin bo'lgan dalillar iziga ega bo'lib, "zaxira laboratoriya oynalari va medianlarni tortib olish" amal bandini qondiradi.

### 7. CI darvozasi va mahalliy tutun

- `ci/check_sm_perf.sh` - kanonik CI kirish nuqtasi. U `SM_PERF_MODES` dagi har bir rejim uchun `scripts/sm_perf.sh` ni tashkil qiladi (standart `scalar auto neon-force`) va `CARGO_NET_OFFLINE=true` ni o'rnatadi, shuning uchun dastgohlar CI tasvirlarida aniq ishlaydi.  
- `.github/workflows/sm-neon-check.yml` endi macOS arm64 runneridagi darvozani chaqiradi, shuning uchun har bir tortishish so'rovi mahalliy sifatida ishlatiladigan bir xil yordamchi orqali skaler/avtomatik/neon-kuch triosini mashq qiladi; qo'shimcha Linux/Neoverse yo'lagi x86_64 quruqlikni egallab olgandan so'ng va Neoverse proksi-serverining asosiy chegaralari yalang'och metall ishga tushirilgandan so'ng ulanadi.  
- Operatorlar rejimlar roʻyxatini mahalliy ravishda bekor qilishi mumkin: `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` tez tutun sinovi uchun yugurishni bir martagacha qisqartiradi, qoʻshimcha argumentlar (masalan, `--tolerance 0.20`) toʻgʻridan-toʻgʻri `scripts/sm_perf.sh` ga yoʻnaltiriladi.  
- `make check-sm-perf` endi ishlab chiquvchiga qulaylik yaratish uchun eshikni o'rab oladi; CI vazifalari to'g'ridan-to'g'ri skriptni ishga tushirishi mumkin, shu bilan birga macOS ishlab chiquvchilari o'z maqsadiga erishadilar.  
- Neoverse/x86_64 bazaviy chiziqqa tushgandan so'ng, xuddi shu skript `scripts/sm_perf.sh`-da mavjud bo'lgan xostni avtomatik aniqlash mantig'i orqali tegishli JSON-ni tanlaydi, shuning uchun har bir xost-pul uchun kerakli rejimlar ro'yxatini o'rnatishdan tashqari ish oqimlarida qo'shimcha simlar kerak emas.

### 8. Har chorakda yangilash yordamchisi- `artifacts/sm_perf/2026-Q1/<label>/` kabi chorak shtamplangan katalogni yaratish uchun `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` ni ishga tushiring. Yordamchi `scripts/sm_perf_capture_helper.sh --matrix` ni o'rab oladi va `capture_commands.sh`, `capture_plan.json` va `quarterly_plan.json` (egasi + chorak metama'lumotlari) chiqaradi, shuning uchun laboratoriya operatorlari qo'lda yozilgan rejalarsiz ishga tushirishni rejalashtirishi mumkin.
- Yaratilgan `capture_commands.sh` ni maqsadli xostda bajaring, xom natijalarni `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` bilan birlashtiring va medianalarni `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite` orqali asosiy JSONlarga targ'ib qiling. Toleranslar yashil bo'lib qolishini tasdiqlash uchun `ci/check_sm_perf.sh` ni qayta ishga tushiring.
- Uskuna yoki asboblar zanjiri o'zgarganda, `docs/source/crypto/sm_perf_baseline_comparison.md`-dagi taqqoslash toleranslari/eslatmalarini yangilang, agar yangi medianalar barqarorlashsa, `ci/check_sm_perf.sh` toleranslarini torting va asboblar paneli/ogohlantirish chegaralarini yangi bazaviy ko'rsatkichlar bilan tekislang, shunda operatsiya signallari mazmunli bo'lib qoladi.
- `quarterly_plan.json`, `capture_plan.json`, `capture_commands.sh` va jamlangan JSON-ni asosiy yangilanishlar bilan birga qabul qiling; kuzatish uchun bir xil artefaktlarni holat/yo‘l xaritasi yangilanishlariga biriktiring.
---
lang: uz
direction: ltr
source: docs/source/crypto/sm_audit_success_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ef9305dc14d477a616923c80445094c692bc6a38d69465f679b54ccd52e92
source_last_modified: "2025-12-29T18:16:35.940844+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Audit muvaffaqiyati mezonlari
% Iroha Kripto ishchi guruhi
% 2026-01-30

#Maqsad

Ushbu nazorat ro'yxati muvaffaqiyatli bo'lish uchun zarur bo'lgan aniq mezonlarni o'z ichiga oladi
SM2/SM3/SM4 tashqi auditini yakunlash. Bu vaqt ichida ko'rib chiqilishi kerak
start, har bir holat nazorat punktida qayta ko'rib chiqiladi va chiqishni tasdiqlash uchun ishlatiladi
ishlab chiqarish validatorlari uchun SM imzolashni yoqishdan oldin shartlar.

# Ulanishdan oldingi tayyorgarlik

- [ ] Shartnoma imzolandi, jumladan, qamrovi, yetkazib berishlari, maxfiyligi va
      tuzatishni qo'llab-quvvatlash tili.
- [ ] Audit jamoasi ombor oynasiga kirish huquqini, CI artefakt paqirini va
      `docs/source/crypto/sm_audit_brief.md` ro'yxatidagi hujjatlar to'plami.
- [ ] Har bir rol uchun zaxira nusxalari bilan tasdiqlangan aloqa nuqtalari
      (kripto, IVM, platforma operatsiyalari, xavfsizlik, hujjatlar).
- [ ] Ichki manfaatdor tomonlar maqsadli chiqarilgan sanaga moslashadi va derazalarni muzlatib qo'yadi.
- [ ] SBOM eksporti (`cargo auditable` + CycloneDX) yaratilgan va birgalikda.
- [ ] OpenSSL/Tongsuo qurilish manbalari paketi tayyorlandi
      (manba tarball xesh, qurish skripti, takroriylik eslatmalari).
- [ ] Oxirgi deterministik test natijalari olingan:
      `scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm` va
      Norito qaytib kelish moslamalari.
- [ ] Torii `/v2/node/capabilities` reklamasi (`iroha runtime capabilities` orqali), `crypto.sm` manifest maydonlari va tezlashtirish siyosati suratini tasdiqlovchi yozib olingan.

# Mashg'ulotni bajarish

- [ ] Maqsadlarni umumiy tushunish bilan boshlang'ich seminar yakunlandi,
      vaqt jadvallari va aloqa ritmi.
- [ ] Haftalik holat hisobotlari qabul qilindi va triajlandi; risklar reestri yangilandi.
- [ ] Topilmalar jiddiyligi aniqlangandan keyin bir ish kuni ichida etkaziladi
      yuqori yoki muhim.
- [ ] Audit jamoasi ≥2 protsessor arxitekturasida determinizm yo'llarini tasdiqlaydi (x86_64,
      aarch64) mos chiqishlar bilan.
- [ ] Yon kanalni ko'rib chiqish doimiy vaqtli dalillar yoki empirik testlarni o'z ichiga oladi
      Rust va FFI yo'llari uchun dalillar.
- [ ] Muvofiqlik va hujjatlarni tekshirish operator ko'rsatmalariga mos kelishini tasdiqlaydi
      tartibga solish majburiyatlari.
- [ ] Malumot dasturlariga nisbatan differentsial test (RustCrypto,
      OpenSSL/Tongsuo) auditor nazorati ostida amalga oshirildi.
- [ ] Fuzz jabduqlar baholandi; bo'shliqlar mavjud bo'lganda yangi urug'lik korpusi taqdim etiladi.

# Tuzatish va chiqish

- [ ] Barcha topilmalar jiddiylik, ta'sir, ekspluatatsiya va
      tavsiya etilgan tuzatish bosqichlari.
- [ ] Yuqori/Muhim masalalar auditor tomonidan tasdiqlangan tuzatishlar yoki yumshatishlarni oladi
      tekshirish; hujjatlashtirilgan qoldiq xavflar.
- [ ] Auditor aniqlangan muammolarni tasdiqlovchi qayta sinov tekshiruvini taqdim etadi (farq, test
      ishlaydi yoki imzolangan attestatsiya).
- [ ] Yakuniy hisobot topshirildi: ijro xulosasi, batafsil xulosalar, metodologiya,
      determinizm hukmi, muvofiqlik hukmi.
- [ ] Ichki ro'yxatdan o'tish yig'ilishi keyingi bosqichlarni, nashrga tuzatishlarni,
      va hujjatlar yangilanishi.
- [ ] `status.md` audit natijalari va mukammal tuzatishlar bilan yangilandi
      kuzatuvlar.
- [ ] `docs/source/crypto/sm_program.md` da o'limdan keyin olingan (darslar)
      o'rganilgan, kelajakdagi chiniqtirish vazifalari).
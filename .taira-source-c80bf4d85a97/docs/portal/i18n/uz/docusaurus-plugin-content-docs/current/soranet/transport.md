---
id: transport
lang: uz
direction: ltr
source: docs/portal/docs/soranet/transport.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet transport overview
sidebar_label: Transport Overview
description: Handshake, salt rotation, and capability guidance for the SoraNet anonymity overlay.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

SoraNet - bu SoraFS diapazonini, Norito RPC oqimini va kelajakdagi Nexus ma'lumotlar yo'laklarini qo'llab-quvvatlaydigan anonimlik qoplamasi. Transport dasturi (yo'l xaritasi elementlari **SNNet-1**, **SNNet-1a** va **SNNet-1b**) deterministik qo'l siqish, post-kvant (PQ) qobiliyatlari bo'yicha muzokaralar va tuzni aylantirish rejasini belgilab berdi, shuning uchun har bir rele, mijoz va shlyuz bir xil xavfsizlik holatini kuzatadi.

## Maqsadlar va tarmoq modeli

- QUIC v1 orqali uch bosqichli sxemalarni (kirish → o'rta → chiqish) yarating, shunda haqoratli tengdoshlar hech qachon Torii ga to'g'ridan-to'g'ri erisha olmaydi.
- Sessiya kalitlarini TLS transkripsiyasiga ulash uchun QUIC/TLS ustiga Noise XX *gibrid* qo‘l siqish (Curve25519 + Kyber768) qatlamini joylashtiring.
- PQ KEM/imzoni qo'llab-quvvatlash, o'tish roli va protokol versiyasini reklama qiluvchi qobiliyatli TLVlarni talab qilish; Kelajakdagi kengaytmalarni o'rnatish uchun noma'lum turlarni GREASE.
- Ko'r-ko'rona tarkibli tuzlarni har kuni aylantiring va 30 kun davomida himoya o'rni o'rnating, shuning uchun katalogning buzilishi mijozlarni anonimlashtira olmaydi.
- Hujayralarni 1024B da mahkamlang, to'ldirish/qo'g'irchoq hujayralarni kiriting va deterministik telemetriyani eksport qiling, shunda pasaytirishga urinishlar tezda ushlanadi.

## Qo'l siqish quvur liniyasi (SNNet-1a)

1. **QUIC/TLS konvert** – mijozlar QUIC v1 orqali releylarni teradi va boshqaruv CA tomonidan imzolangan Ed25519 sertifikatlari yordamida TLS1.3 qo‘l siqishini yakunlaydi. TLS eksportchisi (`tls-exporter("soranet handshake", 64)`) shovqin qatlamini o'rnatadi, shuning uchun transkriptlar ajralmas bo'ladi.
2. **Shovqin XX gibrid** – prolog bilan `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` protokol qatori = TLS eksportchisi. Xabar oqimi:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 DH chiqishi va ikkala Kyber inkapsulyatsiyasi yakuniy simmetrik kalitlarga aralashtiriladi. PQ materiali bo'yicha muzokaralar olib borilmasa, qo'l siqish to'g'ridan-to'g'ri to'xtatiladi - faqat klassik qayta ishlashga yo'l qo'yilmaydi.

3. **Pazzle chiptalari va tokenlar** – o‘rni `ClientHello` dan oldin Argon2id ishini tasdiqlovchi chiptani talab qilishi mumkin. Chiptalar uzunlikdagi prefiksli ramkalar bo'lib, ular xeshlangan Argon2 yechimini olib yuradi va siyosat chegaralarida muddati tugaydi:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   `SNTK` prefiksli kirish tokenlari emitentning ML-DSA-44 imzosi faol siyosat va bekor qilish roʻyxatiga nisbatan tasdiqlansa, jumboqlarni chetlab oʻtadi.

4. **Maqsadli TLV almashinuvi** – yakuniy Shovqin yuki quyida tavsiflangan qobiliyatli TLVlarni tashiydi. Agar biron-bir majburiy qobiliyat (PQ KEM/imzo, rol yoki versiya) mavjud boʻlmasa yoki katalog yozuviga mos kelmasa, mijozlar ulanishni toʻxtatadilar.

5. **Transkript jurnali** – releylar transkript xeshini, TLS barmoq izini va TLV tarkibini pasaytirish detektorlari va muvofiqlik quvurlarini ta'minlash uchun qayd qiladi.

## Qobiliyatli TLV (SNNet-1c)

Imkoniyatlar sobit `typ/length/value` TLV konvertini qayta ishlatadi:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Bugungi kunda aniqlangan turlar:

- `snnet.pqkem` – Kyber darajasi (joriy chiqarish uchun `kyber768`).
- `snnet.pqsig` - PQ imzo to'plami (`ml-dsa-44`).
- `snnet.role` – rele roli (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` – protokol versiyasi identifikatori.
- `snnet.grease` - bo'lajak TLVlarga yo'l qo'yilishini ta'minlash uchun ajratilgan diapazondagi tasodifiy to'ldiruvchi yozuvlar.

Mijozlar talab qilinadigan TLVlarning ruxsat etilgan roʻyxatini saqlab qoladilar va ularni oʻtkazib yuboradigan yoki pasaytirishi mumkin boʻlmagan qoʻl siqishlari. Relelar o'zlarining mikrodeskriptor katalogida bir xil to'plamni nashr etadilar, shuning uchun tekshirish deterministikdir.

## Tuz aylanishi va CIDni ko'r qilish (SNNet-1b)

- Boshqaruv `(epoch_id, salt, valid_after, valid_until)` qiymatlari bilan `SaltRotationScheduleV1` yozuvini nashr etadi. O'rni va shlyuzlar imzolangan jadvalni katalog nashriyotchisidan oladi.
- Mijozlar yangi tuzni `valid_after` da qo'llaydilar, oldingi tuzni 12 soatlik imtiyozli davr uchun saqlaydilar va kechiktirilgan yangilanishlarga toqat qilish uchun 7 davr tarixini saqlab qoladilar.
- Kanonik ko'r identifikatorlar quyidagilardan foydalanadi:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Shlyuzlar ko'r kalitni `Sora-Req-Blinded-CID` orqali qabul qiladi va uni `Sora-Content-CID` da aks ettiradi. O'chirish/so'rovni ko'r qilish (`CircuitBlindingKey::derive`) `iroha_crypto::soranet::blinding` da yuboriladi.
- Agar o'rni davrni o'tkazib yuborsa, u jadvalni yuklamaguncha yangi sxemalarni to'xtatadi va `SaltRecoveryEventV1` signalini chiqaradi, bu esa qo'ng'iroq bo'yicha asboblar panelida peyjing signali sifatida ishlaydi.

## Katalog ma'lumotlari va himoya siyosati

- Mikrodeskriptorlar o'rni identifikatorini (Ed25519 + ML-DSA-65), PQ kalitlarini, qobiliyat TLV'larini, mintaqa teglarini, qo'riqlash huquqini va hozirda e'lon qilingan tuz davrini o'z ichiga oladi.
- Mijozlar 30 kun davomida himoya to'plamlarini o'rnatadilar va imzolangan katalog surati bilan birga `guard_set` keshlarini saqlaydilar. CLI va SDK paketlari kesh barmoq izini ko'rsatadi, shuning uchun sharhlarni o'zgartirish uchun tarqatish dalillari ilova qilinishi mumkin.

## Telemetriya va tarqatish nazorat ro'yxati

- Ishlab chiqarishdan oldin eksport qilinadigan ko'rsatkichlar:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Ogohlantirish chegaralari tuz aylanish SOP SLO matritsasi (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) bilan birga yashaydi va tarmoqni ko'tarishdan oldin Alertmanager-da aks ettirilishi kerak.
- Ogohlantirishlar: 5 daqiqadan ortiq nosozlik darajasi >5%, tuz kechikishi >15 daqiqa yoki ishlab chiqarishda kuzatilgan qobiliyatlarning mos kelmasligi.
- Chiqarish bosqichlari:
  1. Gibrid qo‘l siqish va PQ stek yoqilgan holda sahnalashtirish bo‘yicha reley/mijoz bilan o‘zaro ishlash testlarini bajaring.
  2. Tuzni aylantirish SOP (`docs/source/soranet_salt_plan.md`) ni takrorlang va o'zgartirish yozuviga burg'ulash artefaktlarini biriktiring.
  3. Katalogda imkoniyatlar bo'yicha muzokaralarni yoqing, so'ngra kirish o'rni, o'rta o'rni, chiqish va nihoyat mijozlarga o'ting.
  4. Har bir bosqich uchun qo'riqchi kesh barmoq izlarini, tuz jadvallarini va telemetriya asboblar panelini yozib oling; dalillar to'plamini `status.md` ga biriktiring.

Ushbu nazorat roʻyxatidan soʻng operator, mijoz va SDK guruhlari SNNet yoʻl xaritasida koʻrsatilgan determinizm va audit talablariga javob berish bilan birga SoraNet transportlarini blokirovkada qabul qilish imkonini beradi.
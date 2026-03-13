---
lang: uz
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2e4c6ed5974f623906f51259a634bcad5df703bcec899630ae29f4669b289ab6
source_last_modified: "2026-01-08T21:52:45.509525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Ko'prikning yakuniy dalillari

Ushbu hujjat Iroha uchun dastlabki ko'prikning yakuniyligini isbotlovchi sirtni tavsiflaydi.
Maqsad tashqi zanjirlar yoki engil mijozlarga Iroha blokini tasdiqlashiga ruxsat berishdir.
zanjirdan tashqari hisoblash yoki ishonchli o'rnisiz yakunlanadi.

## Isbot formati

`BridgeFinalityProof` (Norito/JSON) quyidagilarni o'z ichiga oladi:

- `height`: blok balandligi.
- `chain_id`: Iroha zanjir identifikatori zanjir o'zaro takrorlanishining oldini olish uchun.
- `block_header`: kanonik `BlockHeader`.
- `block_hash`: sarlavhaning xeshi (mijozlar tekshirish uchun qayta hisoblashadi).
- `commit_certificate`: validator to'plami + blokni yakunlagan imzolar.
- `validator_set_pops`: Egalikni tasdiqlovchi baytlar validator to'plamiga moslangan
  buyurtma (BLS agregat tekshiruvi uchun talab qilinadi).

Dalil o'z-o'zidan mavjud; hech qanday tashqi manifest yoki noaniq bloblar talab qilinmaydi.
Saqlash: Torii oxirgi sertifikat oynasi uchun yakuniy dalillarni taqdim etadi
(sozlangan tarix qopqog'i bilan chegaralangan; sukut bo'yicha 512 ta yozuv orqali
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). Mijozlar
Agar ular uzoqroq ufqlarga muhtoj bo'lsa, dalillarni keshlashi yoki bog'lashi kerak.
Kanonik kortej `(block_header, block_hash, commit_certificate)`: the
Sarlavhaning xeshi majburiy sertifikat ichidagi xeshga mos kelishi kerak va
zanjir identifikatori dalilni bitta daftarga bog'laydi. Serverlar rad etadi va a
Sertifikat boshqa blokga ishora qilganda `CommitCertificateHashMismatch`
hash.

## Majburiyat to'plami

`BridgeFinalityBundle` (Norito/JSON) aniq dalil bilan asosiy dalilni kengaytiradi
majburiyat va asoslash:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: majburiyat bo'yicha o'rnatilgan vakolatning imzolari
  foydali yuk (majburiy sertifikat imzolarini qayta ishlatadi).
- `block_header`, `commit_certificate`: asosiy dalil bilan bir xil.

Joriy to'ldiruvchi: `mmr_root`/`mmr_peaks` qayta hisoblash orqali olingan
xotirada blok-xeshli MMR; inklyuziya dalillari hali qaytarilmagan. Mijozlar mumkin
hali ham bir xil xeshni majburiyat yuki orqali tasdiqlang.

MMR cho'qqilari chapdan o'ngga tartiblangan. Cho'qqilarni yig'ish orqali `mmr_root` ni qayta hisoblang
o'ngdan chapga: `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.

API: `GET /v2/bridge/finality/bundle/{height}` (Norito/JSON).

Tasdiqlash asosiy dalilga o'xshaydi: `block_hash` dan qayta hisoblang.
sarlavha, majburiyat-sertifikat imzolarini tekshiring va majburiyatni tekshiring
maydonlar sertifikat va blok xeshga mos keladi. To'plam majburiyatni qo'shadi/
ajratishni afzal ko'radigan ko'prik protokollari uchun asoslash paketi.

## Tekshirish bosqichlari1. `block_header` dan `block_hash` ni qayta hisoblang; mos kelmasligi sababli rad etish.
2. `commit_certificate.block_hash` qayta hisoblangan `block_hash` bilan mos kelishini tekshiring;
   nomuvofiq sarlavhani rad etish/sertifikat juftligini topshirish.
3. `chain_id` kutilgan Iroha zanjiriga mos kelishini tekshiring.
4. `commit_certificate.validator_set` dan `validator_set_hash` ni qayta hisoblang va
   yozilgan xesh/versiyaga mos kelishini tekshiring.
5. `validator_set_pops` uzunligi validator toʻplamiga mos kelishiga ishonch hosil qiling va tasdiqlang
   Har bir PoP o'zining BLS ochiq kalitiga qarshi.
6. Sarlavhalar xeshidan foydalangan holda topshiriq sertifikatidagi imzolarni tekshiring
   havola qilingan validator ochiq kalitlari va indekslari; kvorumni amalga oshirish
   (`2f+1`, `n>3`, boshqa `n`) va takroriy/diapazondan tashqari indekslarni rad qiling.
7. Ixtiyoriy ravishda validator to'plami xeshini solishtirish orqali ishonchli nazorat nuqtasiga bog'lang
   bog'langan qiymatga (zaif sub'ektivlik langari).
8. Ixtiyoriy ravishda kutilgan davr langariga bog'lang, shuning uchun eski/yangiroq dalillar
   langar ataylab aylantirilgunga qadar davrlar rad etiladi.

`BridgeFinalityVerifier` (`iroha_data_model::bridge` da) ushbu tekshiruvlarni qo'llaydi,
zanjir identifikatori/balandlik driftini rad etish, validator-set xesh/versiya nomuvofiqligi, etishmayotgan
yoki yaroqsiz PoPlar, ikki nusxadagi/diapazondan tashqarida imzolovchilar, yaroqsiz imzolar va
kvorumni hisoblashdan oldin kutilmagan davrlar, shuning uchun engil mijozlar bittadan qayta foydalanishi mumkin
tekshirgich.

## Malumot tekshirgichi

`BridgeFinalityVerifier` kutilgan `chain_id` va ixtiyoriy ishonchlini qabul qiladi
validator-set va davr langar. U sarlavha/blok-xesh/-ni qo'llaydi.
commit-certificate tuple, validator-set hash/versiyani tasdiqlaydi, tekshiradi
e'lon qilingan validator ro'yxatiga qarshi imzolar/kvorum va eng so'nggisini kuzatib boradi
eskirgan/o'tkazib yuborilgan dalillarni rad etish uchun balandlik. Ankerlar etkazib berilganda, u rad etadi
aniq `UnexpectedEpoch`/ bilan davrlar/ro'yxatlar bo'ylab takrorlar
`UnexpectedValidatorSet` xatolar; langarsiz u birinchi dalilni qabul qiladi
dublikatni/to'ldirilishini qo'llashni davom ettirishdan oldin validator-set xesh va epoch-
deterministik xatolar bilan diapazon/etarsiz imzolar.

## API yuzasi

- `GET /v2/bridge/finality/{height}` - uchun `BridgeFinalityProof` qaytaradi
  talab qilingan blok balandligi. `Accept` orqali kontent muzokaralari Norito yoki
  JSON.
- `GET /v2/bridge/finality/bundle/{height}` - `BridgeFinalityBundle`ni qaytaradi
  (majburiyat + asoslash + sarlavha/sertifikat) so'ralgan balandlik uchun.

## Eslatmalar va kuzatuvlar

- Hozirda isbotlar saqlangan majburiyat sertifikatlaridan olingan. Chegaralangan
  tarix sertifikatni saqlash oynasidan keyin keladi; mijozlar keshlashi kerak
  agar ular uzoqroq ufqlarga muhtoj bo'lsa, langar dalillari. Derazadan tashqaridagi so'rovlar qaytariladi
  `CommitCertificateNotFound(height)`; xatoni yuzaga keltiring va a ga qayting
  langarli nazorat punkti.
- `block_hash` nomuvofiqligi bilan takrorlangan yoki soxta dalil (sarlavha va .
  sertifikat) `CommitCertificateHashMismatch` bilan rad etilgan; mijozlar kerak
  Imzoni tekshirishdan oldin xuddi shu kortej tekshiruvini bajaring va o'chiring
  mos kelmaydigan foydali yuklar.
- Kelajakdagi ishlar isbot hajmini kamaytirish uchun MMR/vakolat tomonidan belgilangan majburiyat zanjirlarini qo'shishi mumkin
  boyroq majburiyat konvertlari ichidagi majburiyat sertifikati.
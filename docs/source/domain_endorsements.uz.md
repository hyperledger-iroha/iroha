---
lang: uz
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Domenni tasdiqlash

Domenni tasdiqlash operatorlarga domenni yaratish va qoʻmita imzolagan bayonot ostida qayta foydalanish imkonini beradi. Tasdiqlash yuki zanjirda qayd etilgan Norito ob'ektidir, shuning uchun mijozlar qaysi domenni kim va qachon tasdiqlaganligini tekshirishlari mumkin.

## Foydali yuk shakli

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: kanonik domen identifikatori
- `committee_id`: odam o'qiy oladigan qo'mita yorlig'i
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: blok balandligi chegarasining amal qilish muddati
- `scope`: ixtiyoriy ma'lumotlar maydoni va ixtiyoriy `[block_start, block_end]` oynasi (shu jumladan), qabul qiluvchi blok balandligini ** qoplashi kerak**
- `signatures`: `body_hash()` orqali imzolar (`signatures = []` bilan tasdiqlash)
- `metadata`: ixtiyoriy Norito metadata (taklif identifikatorlari, audit havolalari va boshqalar)

## Amalga oshirish

- Nexus yoqilgan va `nexus.endorsement.quorum > 0` yoki domenga domen siyosati talab qilinganda domenni belgilaganda tasdiqlar talab qilinadi.
- Tasdiqlash domen/bayonnoma xesh bogʻlanishi, versiya, blok oynasi, maʼlumotlar maydoniga aʼzolik, amal qilish muddati/yoshi va qoʻmita kvorumini taʼminlaydi. Imzolovchilarda `Endorsement` roli bilan jonli konsensus kalitlari boʻlishi kerak. Takrorlashlar `body_hash` tomonidan rad etiladi.
- Domenni ro'yxatdan o'tkazishga biriktirilgan tasdiqlar `endorsement` metama'lumotlar kalitidan foydalanadi. Xuddi shu tekshirish yo'li `SubmitDomainEndorsement` ko'rsatmasi tomonidan qo'llaniladi, u yangi domenni ro'yxatdan o'tkazmasdan tekshirish uchun tasdiqlarni qayd qiladi.

## Qo'mitalar va siyosatlar

- Qo'mitalar zanjirda ro'yxatdan o'tkazilishi mumkin (`RegisterDomainCommittee`) yoki standart konfiguratsiyalardan (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`) olingan bo'lishi mumkin.
- Domen uchun siyosatlar `SetDomainEndorsementPolicy` (qo‘mitaning identifikatori, `max_endorsement_age`, `required` bayrog‘i) orqali sozlangan. Agar yo'q bo'lsa, Nexus standartlaridan foydalaniladi.

## CLI yordamchilari

- Tasdiqlashni yaratish/imzolash (Norito JSON ni stdoutga chiqaradi):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- Tasdiqlash:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Boshqaruvni boshqarish:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

Tekshiruvdagi xatolar barqaror xato qatorlarini qaytaradi (kvorum nomuvofiqligi, eskirgan/muddati o'tgan tasdiqlash, ko'lamdagi nomuvofiqlik, noma'lum ma'lumotlar maydoni, etishmayotgan qo'mita).
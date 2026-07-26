---
lang: mn
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Домэйн Баталгаажуулалт

Домэйн баталгаажуулалт нь операторуудад хорооноос гарын үсэг зурсан мэдэгдлийн дагуу домэйн үүсгэх, дахин ашиглах боломжийг олгодог. Баталгаажуулах ачаалал нь гинжин хэлхээнд бичигдсэн Norito объект бөгөөд үйлчлүүлэгчид хэн, хэзээ аль домэйныг баталгаажуулсан эсэхийг шалгах боломжтой.

## Ачааны хэлбэр

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: каноник домэйн танигч
- `committee_id`: хүн унших боломжтой хорооны шошго
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: блокийн өндрийг хязгаарлах хүчинтэй
- `scope`: нэмэлт өгөгдлийн орон зай, нэмэлт `[block_start, block_end]` цонх (хамааруулсан) нь хүлээн авах блокийн өндрийг **заавал ** хамарна.
- `signatures`: `body_hash()` дээрх гарын үсэг (`signatures = []` баталгаажуулалт)
- `metadata`: нэмэлт Norito мета өгөгдөл (саналын ID, аудитын холбоос гэх мэт)

## Хууль сахиулах

- Nexus идэвхжсэн, `nexus.endorsement.quorum > 0` эсвэл домэйн тус бүрийн бодлого шаардлагатай гэж тэмдэглэгдсэн үед дэмжлэг авах шаардлагатай.
- Баталгаажуулалт нь домэйн/мэдэгдэл хэш холболт, хувилбар, блок цонх, өгөгдлийн орон зайн гишүүнчлэл, хугацаа дуусах/нас, хорооны чуулга зэргийг хэрэгжүүлдэг. Гарын үсэг зурсан хүмүүс `Endorsement` үүрэг бүхий шууд зөвшилцлийн түлхүүртэй байх ёстой. Дахин тоглуулахаас `body_hash` татгалздаг.
- Домэйн бүртгэлд хавсаргасан баталгаа нь `endorsement` мета өгөгдлийн түлхүүрийг ашигладаг. Ижил баталгаажуулалтын замыг `SubmitDomainEndorsement` зааварт ашигладаг бөгөөд энэ нь шинэ домэйн бүртгүүлэхгүйгээр аудит хийх зөвшөөрлийг бүртгэдэг.

## Хороо ба бодлого

- Хороодыг гинжин хэлхээнд (`RegisterDomainCommittee`) бүртгүүлэх эсвэл тохиргооны өгөгдмөлөөс (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`) гаргаж авах боломжтой.
- Домэйн бүрийн удирдамжийг `SetDomainEndorsementPolicy` (хорооны id, `max_endorsement_age`, `required` туг) ашиглан тохируулсан. Байхгүй үед Nexus өгөгдмөлүүдийг ашиглана.

## CLI туслахууд

- Баталгаажуулах/гарын үсэг зурах (Norito JSON-г stdout руу гаргадаг):

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

- Батламж бичгийг илгээх:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

-Засаглалыг удирдах:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

Баталгаажуулалтын алдаа нь тогтвортой алдааны мөрүүдийг буцаана (чуулгын тохиромжгүй, хуучирсан/хугацаа нь дууссан баталгаа, хамрах хүрээний таарахгүй байдал, үл мэдэгдэх өгөгдлийн орон зай, байхгүй хороо).
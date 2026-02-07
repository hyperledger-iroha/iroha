---
lang: mn
direction: ltr
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79b860348fa0a776450d1233e5976b195f02a70ecaed02c569b5e0eba73e05f
source_last_modified: "2025-12-29T18:16:35.206401+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Jurisdictional Review (SNNet-9)
sidebar_label: GAR Jurisdictional Review
description: Signed-off jurisdiction decisions and Blake2b digests to wire into SoraNet compliance configs.
translator: machine-google-reviewed
---

SNNet-9-ийг дагаж мөрдөх зам одоо дууссан. Энэ хуудсанд гарын үсэг зурсан хүмүүсийг жагсаав
харьяаллын шийдвэр, Blake2b-256 digests операторууд нь хуулбарлах ёстой
`compliance.attestations` блокууд, дараагийн хянан үзэх огноо. Гарын үсэг зурсан хэвээр байна
Таны засаглалын архивт байгаа PDF файлууд; Эдгээр задаргаа нь каноник хурууны хээ юм
автоматжуулалт болон аудитын хувьд.

| харьяалал | Шийдвэр | Санамж | Blake2b-256 digest (том зургаан өнцөгт) | Дараагийн тойм |
|-------------|----------|------|------------------------------------|-------------|
| Америкийн Нэгдсэн Улс | Зөвхөн шууд тээвэрлэх шаардлагатай (SoraNet хэлхээ байхгүй) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Канад | Зөвхөн шууд тээвэрлэх шаардлагатай | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| ЕХ/ЕАЭА | Anonymous SoraNet тээвэрлэлтийг SNNet-8 нууцлалын төсвөөр хэрэгжүүлэхийг зөвшөөрсөн | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## Байршуулах хэсэг

```jsonc
{
  "compliance": {
    "operator_jurisdictions": ["US", "CA", "DE"],
    "jurisdiction_opt_outs": ["US", "CA"],
    "blinded_cid_opt_outs": [
      "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828",
      "7F8B1E9D04878F1AEAB553C1DB0A3E3A2AB689F75FE6BE17469F85A4D201B4AC"
    ],
    "attestations": [
      {
        "jurisdiction": "US",
        "document_uri": "norito://gar/attestations/us-2027-q2.pdf",
        "digest_hex": "1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7",
        "issued_at_ms": 1805313600000,
        "expires_at_ms": 1822248000000
      },
      {
        "jurisdiction": "CA",
        "document_uri": "norito://gar/attestations/ca-2027-q2.pdf",
        "digest_hex": "52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063",
        "issued_at_ms": 1805313600000,
        "expires_at_ms": 1822248000000
      },
      {
        "jurisdiction": "EU",
        "document_uri": "norito://gar/attestations/eu-2027-q2.pdf",
        "digest_hex": "30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15",
        "issued_at_ms": 1805313600000
      }
    ]
  }
}
```

## Аудитын шалгах хуудас

- Үйлдвэрлэлийн тохиргоонд яг хуулбарлагдсан аттестатчлалын дижест.
- `jurisdiction_opt_outs` каноник каталогтой таарч байна.
- Танай удирдлагын архивт хадгалагдсан гарын үсэг зурсан PDF файлууд тохирох тоймтой.
- Идэвхжүүлэлтийн цонх ба зөвшөөрчдийг GAR бүртгэлийн дэвтэрт оруулсан.
- Дээрх хүснэгтээс товлосон дараагийн хяналтын сануулгууд.

## Мөн үзнэ үү

- [GAR Operator Inboarding Brief](gar-operator-onboarding)
- [GAR Compliance Playbook (эх сурвалж)](../../../source/soranet/gar_compliance_playbook.md)
---
lang: hy
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

SNNet-9-ի համապատասխանության ուղին այժմ ավարտված է: Այս էջում նշված են ստորագրվածները
իրավասության որոշումները, Blake2b-256 digests օպերատորները պետք է պատճենեն իրենց մեջ
`compliance.attestations` արգելափակում է, և հաջորդ վերանայման ամսաթվերը: Պահպանեք ստորագրվածը
PDF-ներ ձեր կառավարման արխիվում; այս մարսողությունները կանոնական մատնահետքեր են
ավտոմատացման և աուդիտի համար:

| Իրավասություն | Որոշում | Հուշագիր | Blake2b-256 digest (մեծատառ վեցանկյուն) | Հաջորդ վերանայում |
|--------------------------|------------------------------------------|-------------|
| Միացյալ Նահանգներ | Պահանջվում է միայն ուղղակի տրանսպորտ (SoraNet սխեմաներ չկան) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Կանադա | Պահանջվում է միայն ուղղակի տրանսպորտ | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| ԵՄ/ԵՏՄ | Անանուն SoraNet տրանսպորտը թույլատրվում է SNNet-8-ի գաղտնիության բյուջեի ուժով | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## Տեղակայման հատված

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

## Աուդիտի ստուգաթերթ

- Ատեստավորման բովանդակությունը պատճենված է հենց արտադրական կազմաձևերում:
- `jurisdiction_opt_outs`-ը համապատասխանում է կանոնական կատալոգին:
- Ստորագրված PDF-ներ, որոնք պահպանվում են ձեր կառավարման արխիվում՝ համապատասխան ամփոփագրերով:
- Ակտիվացման պատուհանը և հաստատողներն ընդգրկված են GAR մատյանում:
- Հաջորդ վերանայման հիշեցումները նախատեսված են վերը նշված աղյուսակից:

## Տես նաև

- [GAR Operator Onboarding Brief] (gar-operator-onboarding)
- [GAR Compliance Playbook (աղբյուր)] (../../../source/soranet/gar_compliance_playbook.md)
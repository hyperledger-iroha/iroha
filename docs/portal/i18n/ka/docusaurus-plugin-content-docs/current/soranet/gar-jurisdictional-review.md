---
lang: ka
direction: ltr
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: GAR Jurisdictional Review (SNNet-9)
sidebar_label: GAR Jurisdictional Review
description: Signed-off jurisdiction decisions and Blake2b digests to wire into SoraNet compliance configs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SNNet-9 შესაბამისობის ტრეკი ახლა დასრულებულია. ამ გვერდზე ჩამოთვლილია ხელმოწერილები
იურისდიქციის გადაწყვეტილებები, Blake2b-256 digests ოპერატორებმა უნდა დააკოპირონ მათში
`compliance.attestations` ბლოკები და შემდეგი განხილვის თარიღები. შეინახეთ ხელმოწერილი
PDF ფაილები თქვენს მმართველობის არქივში; ეს დაიჯესტები არის კანონიკური თითის ანაბეჭდები
ავტომატიზაციისა და აუდიტისთვის.

| იურისდიქცია | გადაწყვეტილება | შენიშვნა | Blake2b-256 დაიჯესტი (ზედა თექვსმეტობითი) | შემდეგი მიმოხილვა |
|--------------|---------|-----------------------------------------|------------|
| შეერთებული შტატები | საჭიროა მხოლოდ პირდაპირი ტრანსპორტი (SoraNet სქემები არ არის) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| კანადა | საჭიროა მხოლოდ პირდაპირი ტრანსპორტი | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| EU/EEA | ანონიმური SoraNet ტრანსპორტი დაშვებულია SNNet-8 კონფიდენციალურობის ბიუჯეტებით აღსრულებული | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## განლაგების ფრაგმენტი

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

## აუდიტის ჩამონათვალი

- საატესტაციო დაიჯესტები ზუსტად დაკოპირებულია წარმოების კონფიგურაციებში.
- `jurisdiction_opt_outs` შეესაბამება კანონიკურ კატალოგს.
- ხელმოწერილი PDF-ები ინახება თქვენს მმართველობის არქივში შესაბამისი დაიჯესტებით.
- აქტივაციის ფანჯარა და დამმტკიცებლები დაფიქსირებული GAR ჟურნალში.
- შემდეგი მიმოხილვის შეხსენებები დაგეგმილია ზემოთ მოცემული ცხრილიდან.

## აგრეთვე იხილეთ

- [GAR ოპერატორის ჩართვის მოკლე ინფორმაცია] (gar-operator-onboarding)
- [GAR Compliance Playbook (წყარო)] (../../../source/soranet/gar_compliance_playbook.md)
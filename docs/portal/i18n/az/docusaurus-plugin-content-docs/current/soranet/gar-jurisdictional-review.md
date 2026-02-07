---
lang: az
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

SNNet-9 uyğunluq yolu artıq tamamlandı. Bu səhifədə imzalananlar göstərilir
yurisdiksiya qərarları, Blake2b-256 digests operatorları özlərinə kopyalamalıdırlar
`compliance.attestations` blokları və növbəti baxış tarixləri. İmzalı saxlayın
İdarəetmə arxivinizdəki PDF sənədləri; bu həzmlər kanonik barmaq izləridir
avtomatlaşdırma və audit üçün.

| Yurisdiksiya | Qərar | Memo | Blake2b-256 həzm (böyük hex) | Növbəti baxış |
|-------------|----------|------|------------------------------------|-------------|
| Amerika Birləşmiş Ştatları | Yalnız birbaşa nəqliyyat tələb olunur (SoraNet sxemləri yoxdur) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Kanada | Yalnız birbaşa nəqliyyat tələb olunur | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| Aİ/EEA | SNNet-8 məxfilik büdcələri tətbiq edilən anonim SoraNet nəqliyyatına icazə verilir | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## Yerləşdirmə parçası

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

## Audit yoxlama siyahısı

- Tam olaraq istehsal konfiqurasiyalarına kopyalanan attestasiya həzmləri.
- `jurisdiction_opt_outs` kanonik kataloqa uyğun gəlir.
- Müvafiq həzmlərlə idarə arxivinizdə saxlanılan imzalanmış PDF sənədləri.
- Aktivləşdirmə pəncərəsi və təsdiqləyicilər GAR jurnalında qeyd olunur.
- Yuxarıdakı cədvəldən planlaşdırılan növbəti baxış xatırlatmaları.

## Həmçinin bax

- [GAR Operatorunun İşə Qəbul Qısacası](gar-operator-onboarding)
- [GAR Uyğunluğu üzrə Təlim Kitabı (mənbə)](../../../source/soranet/gar_compliance_playbook.md)
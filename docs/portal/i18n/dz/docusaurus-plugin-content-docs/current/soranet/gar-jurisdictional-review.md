---
lang: dz
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

SNNet-9 བསྟར་སྤྱོད་ཀྱི་ལམ་འདི་ད་ལྟ་མཇུག་བསྡུ་ཡོད། ཤོག་ལེབ་འདི་གིས་མིང་རྟགས་བཀོད་ཡོད་པའི་ཐོ་བཀོད་འབདཝ་ཨིན།
དབང་ཆ་ཐག་གཅོད། བེལེཀ་༢བི་-༢༥༦ བཞུ་བཅུག་མི་ཚུ་གིས་ བཀོལ་སྤྱོད་པ་ཚུ་གིས་ ཁོང་རའི་ནང་འདྲ་བཤུས་རྐྱབ་དགོ།
`compliance.attestations` བཀག་ཆ་ཚུ་དང་ ཤུལ་མའི་བསྐྱར་ཞིབ་ཚེས་གྲངས་ཚུ། མཚན་རྟགས་བཀོད་པར་བཞག།
ཁྱོད་ཀྱི་གཞུང་སྐྱོང་གཏན་མཛོད་ནང་ PDFs; འདི་ཚུ་ བཞུ་བཅོས་ཚུ་ ཁྲིམས་ལུགས་ཀྱི་མཛུབ་མོ་གི་པར་ཚུ་ཨིན།
འཕྲུལ་ཆས་དང་རྩིས་ཞིབ་ཀྱི་དོན་ལུ།

| དབང་ཆ་ | གྲོས་ཐག་ | མེམོ | Blake2b-256 བཞུ་བཅོས་ (ཁ་སྟོད་ཀྱི་ཧེགསི་) | ཤུལ་མམ་གྱི་བསྐྱར་ཞིབ་ |
|--------------|----------|------|------------------------------------|-------------|
| ཡུ་ནའི་ཊེཊ་ཨི་སི་ཊེཊ་ | ཐད་ཀར་གྱི་སྐྱེལ་འདྲེན་དགོཔ་ཨིན་ (སོ་ར་ནེཊ་གློག་ལམ་མེན) | I18NI0000004X | I18NI0000000X | ༢༠༢༧-༠༩-༣༠ |
| ཀེ་ན་ཌ་ | ཐད་ཀར་གྱི་སྐྱེལ་འདྲེན་དགོས་མཁོ། | I18NI0000006X | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | ༢༠༢༧-༠༩-༣༠ |
| EU/EEA | མིང་མེད་སོ་ར་ནེཊ་སྐྱེལ་འདྲེན་ SNNet-8 སྒེར་དོན་འཆར་དངུལ་ བསྟར་སྤྱོད་འབད་ཡོདཔ། | I18NI0000008X | I18NI0000009X | ༢༠༢༧-༠༩-༣༠ |

## བཙོངས་ཚད་ཀྱི་ཚད།

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

## རྩིས་ཞིབ་ཀྱི་དཔྱད་གཞི།

- ཨེག་ཊི་གིས་ ཐོན་སྐྱེད་རིམ་སྒྲིག་ནང་ལུ་ ཏན་ཏན་སྦེ་ འདྲ་བཤུས་རྐྱབ་ཡོདཔ་ཨིན།
- I18NI000000010X གིས་ ཀེ་ནོ་ནིག་ཐོ་གཞུང་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
- ཁྱོད་རའི་གཞུང་སྐྱོང་གཏན་མཛོད་ནང་ མཐུན་སྒྲིག་ཅན་གྱི་ བཞུ་བཅོས་ཚུ་དང་གཅིག་ཁར་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ PDFs ཚུ་ བཞག་ཡོདཔ།
- ཇི་ཨར་ དྲན་ཐོ་ཀི་དེབ་ནང་ བཟུང་ཡོད་པའི་ ཤུགས་ལྡན་སྒོ་སྒྲིག་དང་ ཆ་འཇོག་འབད་མི་ཚུ།
- གོང་འཁོད་ཐིག་ཁྲམ་ལས་ དུས་ཚོད་བཀོད་ཡོད་པའི་ ཤུལ་མམ་གྱི་དྲན་སྐུལ་ཚུ།

## ད་དུང་གཟིགས།

- [GAR བཀོལ་སྤྱོད་ཀྱི་མདོར་བསྡུས།](I18NU0000001X)
- [GAR བསྟུན་པའི་རྩེད་དེབ་ (ཐོན་ཁུངས།)](I18NU0000002X)
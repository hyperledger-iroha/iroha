---
lang: kk
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

SNNet-9 сәйкестік жолы енді аяқталды. Бұл бетте қол қойылғандардың тізімі берілген
юрисдикция шешімдерін Blake2b-256 дайджест операторлары олардың ішіне көшіруі керек
`compliance.attestations` блоктары және келесі шолу күндері. Қолтаңбаны сақтаңыз
Басқару мұрағатындағы PDF файлдары; бұл дайджесттер канондық саусақ іздері болып табылады
автоматтандыру және аудит үшін.

| Юрисдикция | Шешім | Жаднама | Blake2b-256 дайджест (бас әріп алтылық) | Келесі шолу |
|-------------|----------|------|------------------------------------|-------------|
| Америка Құрама Штаттары | Тікелей тасымалдау қажет (SoraNet тізбектері жоқ) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 30.09.2027 |
| Канада | Тікелей тасымалдау қажет | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 30.09.2027 |
| ЕО/ЕЭА | Анонимді SoraNet тасымалдауға рұқсат етілген SNNet-8 құпиялылық бюджеттері орындалған | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 30.09.2027 |

## Орналастыру үзіндісі

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

## Аудитті тексеру парағы

- Өндірістік конфигурацияларға дәл көшірілген аттестаттау дайджесттері.
- `jurisdiction_opt_outs` канондық каталогқа сәйкес келеді.
- Сәйкес дайджесттермен басқару мұрағатында сақталған қол қойылған PDF файлдары.
- GAR журналында түсірілген белсендіру терезесі және бекітушілер.
- Жоғарыдағы кестеден жоспарланған келесі шолу еске салғыштары.

## Сондай-ақ қараңыз

- [GAR операторының жұмысқа кірісу туралы қысқашалығы](gar-operator-onboarding)
- [GAR Compliance Playbook (көзі)](../../../source/soranet/gar_compliance_playbook.md)
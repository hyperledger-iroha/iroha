---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Юрисдикционный обзор GAR (SNNet-9)

Трек compliance SNNet-9 завершен. Эта страница перечисляет подписанные юрисдикционные решения,
digests Blake2b-256, которые операторы должны скопировать в блоки `compliance.attestations`, а также
следующие даты ревью. Храните подписанные PDF в архиве управления; эти digests являются каноническими
отпечатками для автоматизации и аудита.

| Юрисдикция | Решение | Memo | Digest Blake2b-256 (hex в верхнем регистре) | Следующее ревью |
|--------------|----------|------|-------------------------------------------|----------------|
| United States | Требуется transport direct-only (без цепочек SoraNet) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Canada | Требуется transport direct-only | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| EU/EEA | Анонимный транспорт SoraNet разрешен при применении privacy budgets SNNet-8 | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## Фрагмент деплоя

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

## Чеклист аудита

- Digests аттестаций скопированы точно в production configs.
- `jurisdiction_opt_outs` совпадает с каноническим каталогом.
- Подписанные PDF сохранены в архиве управления с соответствующими digests.
- Окно активации и апруверы зафиксированы в GAR logbook.
- Напоминания о следующем ревью запланированы по таблице выше.

## См. также

- [GAR Operator Onboarding Brief](gar-operator-onboarding)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)

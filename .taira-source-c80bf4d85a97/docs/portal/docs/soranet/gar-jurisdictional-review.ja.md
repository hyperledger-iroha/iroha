---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79b860348fa0a776450d1233e5976b195f02a70ecaed02c569b5e0eba73e05f
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: GAR 管轄レビュー (SNNet-9)
sidebar_label: GAR 管轄レビュー
description: 署名済み管轄判断と Blake2b digest を SoraNet compliance 設定に反映するための一覧。
---

# GAR 管轄レビュー (SNNet-9)

SNNet-9 compliance トラックは完了した。本ページは署名済みの管轄判断、オペレーターが
`compliance.attestations` にコピーすべき Blake2b-256 digests、次回レビュー日をまとめる。
署名済み PDF はガバナンスアーカイブに保管すること。これらの digests は自動化と監査の
ための canonical fingerprints である。

| 管轄 | 判断 | メモ | Blake2b-256 digest (大文字 hex) | 次回レビュー |
|--------------|----------|------|--------------------------------|-------------|
| United States | direct-only transport 必須 (SoraNet circuits なし) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Canada | direct-only transport 必須 | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| EU/EEA | SNNet-8 privacy budgets を適用した匿名 SoraNet transport を許可 | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## デプロイスニペット

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

## 監査チェックリスト

- Attestation digests を production configs に正確にコピーすること。
- `jurisdiction_opt_outs` が canonical catalogue と一致すること。
- 署名済み PDF を一致する digests と共にガバナンスアーカイブへ保管すること。
- アクティベーション期間と承認者を GAR logbook に記録すること。
- 次回レビューのリマインダーを上記表からスケジュールすること。

## 参照

- [GAR Operator Onboarding Brief](gar-operator-onboarding)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)

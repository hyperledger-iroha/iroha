---
lang: zh-hant
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

SNNet-9 合規性跟踪現已完成。此頁列出了已簽名的
管轄權決定，Blake2b-256 摘要操作員必須將其複製到其
`compliance.attestations` 塊，以及下一次審核日期。保留簽名
您的治理檔案中的 PDF；這些摘要是規範的指紋
用於自動化和審計。

|管轄範圍 |決定|備忘錄 | Blake2b-256 摘要（大寫十六進制）|下一篇評論 |
|--------------|----------|------|------------------------------------|------------------------|
|美國 |需要直接傳輸（無 SoraNet 電路）| `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
|加拿大 |需要直達交通 | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
|歐盟/歐洲經濟區 |允許匿名 SoraNet 傳輸並強制實施 SNNet-8 隱私預算 | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## 部署片段

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

## 審核清單

- 證明摘要準確複製到生產配置中。
- `jurisdiction_opt_outs` 與規範目錄匹配。
- 簽名的 PDF 保留在您的治理檔案中，並帶有匹配的摘要。
- GAR 日誌中捕獲的激活窗口和批准者。
- 從上表中安排的下一次審核提醒。

## 另請參閱

- [GAR 操作員入職簡介](gar-operator-onboarding)
- [GAR 合規手冊（來源）](../../../source/soranet/gar_compliance_playbook.md)
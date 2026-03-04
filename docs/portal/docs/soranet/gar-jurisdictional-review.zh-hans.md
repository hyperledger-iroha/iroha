---
lang: zh-hans
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

SNNet-9 合规性跟踪现已完成。此页列出了已签名的
管辖权决定，Blake2b-256 摘要操作员必须将其复制到其
`compliance.attestations` 块，以及下一次审核日期。保留签名
您的治理档案中的 PDF；这些摘要是规范的指纹
用于自动化和审计。

|管辖范围 |决定|备忘录 | Blake2b-256 摘要（大写十六进制）|下一篇评论 |
|--------------|----------|------|------------------------------------|------------------------|
|美国 |需要直接传输（无 SoraNet 电路）| `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
|加拿大 |需要直达交通 | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
|欧盟/欧洲经济区 |允许匿名 SoraNet 传输并强制执行 SNNet-8 隐私预算 | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

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

## 审核清单

- 证明摘要准确复制到生产配置中。
- `jurisdiction_opt_outs` 与规范目录匹配。
- 签名的 PDF 保留在您的治理档案中，并带有匹配的摘要。
- GAR 日志中捕获的激活窗口和批准者。
- 从上表中安排的下一次审核提醒。

## 另请参阅

- [GAR 操作员入职简介](gar-operator-onboarding)
- [GAR 合规手册（来源）](../../../source/soranet/gar_compliance_playbook.md)
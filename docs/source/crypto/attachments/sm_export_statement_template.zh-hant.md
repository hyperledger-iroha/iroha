---
lang: zh-hant
direction: ltr
source: docs/source/crypto/attachments/sm_export_statement_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6742d2b87b8fbbc1493c5ae2704147b0f8d5d23af78004c2c9a112fe881efb11
source_last_modified: "2025-12-29T18:16:35.937402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 出口管制聲明模板
% Hyperledger Iroha 合規工作組
% 2026-05-06

# 用法

在以下情況下將此聲明嵌入到發行說明、清單或法律信函中：
分發支持 SM 的文物。更新佔位符以匹配版本，
管轄權和適用的許可例外情況。保留一份簽名副本
發布清單。

# 聲明

> **產品：** Hyperledger Iroha {{ RELEASE_VERSION }} (`{{ ARTEFACT_ID }}`)
>
> **包含的算法：** SM2 數字簽名、SM3 哈希、SM4 對稱
> 加密（GCM/CCM）
>
> **出口分類：** 美國 EAR 類別 5，第 2 部分 (5D002.c.1)；
> 歐盟法規 2021/821 附件 1, 5D002。
>
> **許可證例外：** {{ LICENSE_EXCEPTION }}（例如，ENC §740.17(b)(2)，
> TSU §740.13 用於源代碼分發）。
>
> **分發範圍：** {{ DISTRIBUTION_SCOPE }}（例如，“全球，不包括
> 15 CFR 746 中列出的禁運地區”）。
>
> **運營商義務：** 接收者必須遵守適用的出口、
> 進口和使用規定。在中華人民共和國境內的部署
> 中國要求向國家密碼局備案產品和使用情況
> 管理並遵守大陸數據駐留要求。
>
> **聯繫方式：** {{ LEGAL_CONTACT_NAME }} — {{ LEGAL_CONTACT_EMAIL }} /
> {{LEGAL_CONTACT_PHONE}}
>
> 本聲明附有密碼學合規性檢查表和備案
> `docs/source/crypto/sm_compliance_brief.md` 中提供的模板。保留這個
> 文件和相關備案文件至少保存三年。

# 簽名

- 授權代表：________________________
- 頭銜：___________________
- 日期：___________________
---
lang: zh-hans
direction: ltr
source: docs/source/crypto/attachments/sm_export_statement_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6742d2b87b8fbbc1493c5ae2704147b0f8d5d23af78004c2c9a112fe881efb11
source_last_modified: "2025-12-29T18:16:35.937402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 出口管制声明模板
% Hyperledger Iroha 合规工作组
% 2026-05-06

# 用法

在以下情况下将此声明嵌入到发行说明、清单或法律信函中：
分发支持 SM 的文物。更新占位符以匹配版本，
管辖权和适用的许可例外情况。保留一份签名副本
发布清单。

# 声明

> **产品：** Hyperledger Iroha {{ RELEASE_VERSION }} (`{{ ARTEFACT_ID }}`)
>
> **包含的算法：** SM2 数字签名、SM3 哈希、SM4 对称
> 加密（GCM/CCM）
>
> **出口分类：** 美国 EAR 类别 5，第 2 部分 (5D002.c.1)；
> 欧盟法规 2021/821 附件 1, 5D002。
>
> **许可证例外：** {{ LICENSE_EXCEPTION }}（例如，ENC §740.17(b)(2)，
> TSU §740.13 用于源代码分发）。
>
> **分发范围：** {{ DISTRIBUTION_SCOPE }}（例如，“全球，不包括
> 15 CFR 746 中列出的禁运地区”）。
>
> **运营商义务：** 接收者必须遵守适用的出口、
> 进口和使用规定。在中华人民共和国境内的部署
> 中国要求向国家密码局备案产品和使用情况
> 管理并遵守大陆数据驻留要求。
>
> **联系方式：** {{ LEGAL_CONTACT_NAME }} — {{ LEGAL_CONTACT_EMAIL }} /
> {{LEGAL_CONTACT_PHONE}}
>
> 本声明附有密码学合规性检查表和备案
> `docs/source/crypto/sm_compliance_brief.md` 中提供的模板。保留这个
> 文件和相关备案文件至少保存三年。

# 签名

- 授权代表：________________________
- 头衔：___________________
- 日期：___________________
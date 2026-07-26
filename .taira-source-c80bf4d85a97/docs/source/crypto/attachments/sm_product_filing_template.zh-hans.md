---
lang: zh-hans
direction: ltr
source: docs/source/crypto/attachments/sm_product_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7116d28e32d8bd77434edd6767427cc3d2ae0624f4de132b1d0cec3c7d44b86
source_last_modified: "2025-12-29T18:16:35.938246+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 产品备案 (开发备案) 模板
% Hyperledger Iroha 合规工作组
% 2026-05-06

# 说明

向省级提交“产品开发备案”时使用此模板
分发前或市国家密码管理局 (SCA) 办公室
支持 SM 的二进制文件或来自中国大陆的源文物。更换
包含项目特定详细信息的占位符，如果满足以下条件，请将完成的表格导出为 PDF：
要求，并附上清单中引用的文物。

# 1. 申请人和产品摘要

|领域|价值|
|--------|--------|
|组织名称| {{ 组织 }} |
|注册地址 | {{ 地址}} |
|法定代表人 | {{ LEGAL_REP }} |
|主要联系人（姓名/职务/电子邮件/电话）| {{ 联系 }} |
|产品名称 | Hyperledger Iroha {{ RELEASE_NAME }} |
|产品版本/构建 ID | {{ 版本 }} |
|归档类型 |产品开发(开发备案) |
|申请日期 | {{ 年-月-日 }} |

# 2. 密码学使用概述

- 支持的算法：`SM2`、`SM3`、`SM4`（在下面提供使用矩阵）。
- 使用上下文：
  |算法|组件|目的|确定性保障措施 |
  |----------|----------|---------|----------------------------|
  | SM2 | {{ 组件 }} | {{ 目的}} | RFC6979 + 规范 r∥ 执行 |
  | SM3 | {{ 组件 }} | {{ 目的}} |通过 `Sm3Digest` 进行确定性哈希 |
  | SM4 | {{ 组件 }} | {{ 目的}} |具有强制随机数策略的 AEAD (GCM/CCM) |
- 构建中的非 SM 算法：{{ OTHER_ALGORITHMS }}（为了完整性）。

# 3. 开发和供应链控制

- 源代码存储库：{{ REPOSITORY_URL }}
- 确定性构建说明：
  1.`git clone {{ REPOSITORY_URL }} && git checkout {{ COMMIT_SHA }}`
  2. `cargo build --workspace --locked --release --features "sm sm-ffi-openssl"`（根据需要调整）。
  3. 通过 `cargo auditable` / CycloneDX (`{{ SBOM_PATH }}`) 生成 SBOM。
- 持续集成环境总结：
  |项目 |价值|
  |------|--------|
  |构建操作系统/版本 | {{ BUILD_OS }} |
  |编译器工具链 | {{ 工具链 }} |
  | OpenSSL/通索源码| {{ OPENSSL_SOURCE }} |
  |再现性校验和| {{ 校验和 }} |

# 4. 密钥管理和安全

- 默认启用的 SM 功能：{{ DEFAULTS }}（例如，仅验证）。
- 签名所需的配置标志：{{ CONFIG_FLAGS }}。
- 钥匙保管方式：
  |项目 |详情 |
  |------|---------|
  |密钥生成工具 | {{ KEY_TOOL }} |
  |存储介质 | {{ STORAGE_MEDIUM }} |
  |备份策略| {{ BACKUP_POLICY }} |
  |访问控制| {{ ACCESS_CONTROLS }} |
- 事件响应联系人 (24/7)：
  |角色 |名称 |电话 |电子邮件 |
  |------|------|--------|--------|
  |加密货币领先 | {{ 姓名 }} | {{ 电话 }} | {{ 电子邮件 }} |
  |平台运营| {{ 姓名 }} | {{ 电话 }} | {{ 电子邮件 }} |
  |法律联络 | {{ 姓名 }} | {{ 电话 }} | {{ 电子邮件 }} |

# 5. 附件清单- [ ] 源代码快照 (`{{ SOURCE_ARCHIVE }}`) 和哈希值。
- [ ] 确定性构建脚本/再现性注释。
- [ ] SBOM (`{{ SBOM_PATH }}`) 和依赖项清单 (`Cargo.lock` 指纹)。
- [ ] 确定性测试成绩单（`scripts/sm_openssl_smoke.sh`、`cargo test -p iroha_crypto sm`）。
- [ ] 遥测仪表板导出展示 SM 可观察性。
- [ ] 出口控制声明（参见单独的模板）。
- [ ] 审计报告或第三方评估（如果已完成）。

# 6. 申请人声明

> 我确认以上信息准确无误，所披露的
> 加密功能符合适用的中国法律和法规，
> 并且该组织将保留提交的文物至少
> 三年。

- 签名（法定代表人）：________________________
- 日期：___________________
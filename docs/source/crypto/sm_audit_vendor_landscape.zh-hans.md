---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_audit_vendor_landscape.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f39199767280be0fdd582301cdc3e8929497cf372a96f9f300e718f827000a7
source_last_modified: "2025-12-29T18:16:35.941305+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM 审核供应商情况
% Iroha 加密工作组
% 2026-02-12

# 概述

加密货币工作组需要一个由独立审查员组成的常设委员会，他们
了解 Rust 密码学和中国 GM/T (SM2/SM3/SM4) 标准。
本说明列出了具有相关参考文献的公司并总结了审计
我们通常要求的范围，以便提案请求 (RFP) 周期保持快速且
一致。

# 候选公司

## Trail of Bits（CN 密码学实践）

- 记录的约定：2023 年蚂蚁集团通所安全审查
  （支持 SM 的 OpenSSL 发行版）和基于 Rust 的重复审核
  Diem/Libra、Sui 和 Aptos 等区块链。
- 优点：专门的 Rust 密码学团队、自动化恒定时间
  分析工具、验证确定性执行和硬件的经验
  派遣政策。
- 适用于Iroha：可以扩展当前的SM审核SOW或执行独立
  重新测试；使用 Norito 夹具和 IVM 系统调用轻松操作
  表面。

## NCC 集团（亚太地区密码服务）

- 记录的约定：区域支付的 gm/T (SM) 代码检查
  网络和 HSM 供应商；之前对 Parity Substrate、Polkadot 的 Rust 评论
  和 Libra 组件。
- 优势：亚太地区大型工作台，双语报告，整合能力
  通过深入的代码审查进行合规性流程检查。
- 适合 Iroha：非常适合第二意见评估或治理驱动
  与 Trail of Bits 发现一起进行验证。

## 安比实验室（北京）

- 记录的参与：开源 `libsm` Rust 箱的维护者
  由 Nervos CKB 和 CITA 使用；审核了果米对 Nervos、Muta 和
  FISCO BCOS Rust 组件具有双语交付内容。
- 优点：积极使用 Rust 交付 SM 原语的工程师，强
  财产测试能力，对国内合规性有深入的了解
  要求。
- 适合 Iroha：当我们需要能够提供比较的审阅者时很有价值
  测试向量和实施指南以及结果。

## 慢雾安全（成都）

- 记录的参与：Substrate/Polkadot Rust 安全审查，包括
  果米为中国运营商分叉； SM2/SM3/SM4钱包的例行评估
  以及交易所使用的桥接代码。
- 优势：以区块链为中心的审计实践、综合事件响应、
  涵盖核心协议代码和操作员工具的指南。
- 适用于 Iroha：有助于验证 SDK 奇偶性和操作接触点
  除了核心板条箱之外。

## 长亭科技（QAX 404 安全实验室）- 记录参与：GmSSL/Tongsuo 强化和 SM2/SM3/ 的贡献者
  境内金融机构SM4实施指南；成立
  Rust 审计实践涵盖 TLS 堆栈和加密库。
- 优势：深厚的密码分析背景、配对形式验证的能力
  人工审查的文物，长期的监管关系。
- 适用于 Iroha：适用于监管签字或正式证明文物
  需要附有标准代码审查报告。

# 典型的审计范围和可交付成果

- **规范一致性：** 验证 SM2 ZA 计算、签名
  规范化、SM3 填充/压缩以及 SM4 密钥调度和 IV 处理
  对照GM/T 0003-2012、GM/T 0004-2012、GM/T 0002-2012。
- **确定性和恒定时间行为：** 检查分支、查找
  表和硬件功能门（例如 NEON、SM4 指令）以确保
  Rust 和 FFI 调度在支持的硬件上保持确定性。
- **FFI 和提供商集成：** 审查 OpenSSL/Tongsuo 绑定，
  PKCS#11/HSM 适配器和用于共识安全的错误传播路径。
- **测试和夹具覆盖范围：** 评估模糊线束，Norito 往返，
  确定性烟雾测试，并建议差异测试
  出现。
- **依赖性和供应链审查：**确认构建来源、供应商
  补丁策略、SBOM 准确性和可重复的构建指令。
- **文档和操作：** 验证操作员操作手册、合规性
  简介、默认配置和回滚过程。
- **报告预期：** 带有风险评级的执行摘要，详细
  包含代码参考和补救指南、重新测试计划的调查结果，以及
  涵盖决定论保证的证明。

# 后续步骤

- 在询价周期中使用此供应商名册；将上面的范围清单调整为
  在发出 RFP 之前匹配有效的 SM 里程碑。
- 在 `docs/source/crypto/sm_audit_brief.md` 中记录参与结果以及
  合同执行后，`status.md` 中的表面状态会更新。
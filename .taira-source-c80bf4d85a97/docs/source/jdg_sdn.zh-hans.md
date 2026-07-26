---
lang: zh-hans
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2025-12-29T18:16:35.972838+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% JDG-SDN 证明和轮换

本说明捕获了秘密数据节点 (SDN) 证明的执行模型
由司法管辖区数据监护人 (JDG) 流程使用。

## 承诺格式
- `JdgSdnCommitment` 绑定范围（`JdgAttestationScope`），加密
  有效负载哈希和 SDN 公钥。印章是打字签名
  (`SignatureOf<JdgSdnCommitmentSignable>`) 通过域标记的有效负载
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`。
- 结构验证 (`validate_basic`) 强制执行：
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - 有效的块范围
  - 非空密封件
  - 通过运行时与证明的范围相等
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- 重复数据删除由证明验证器处理（签名者+有效负载哈希
  唯一性）以防止隐瞒/重复承诺。

## 注册和轮换政策
- SDN 密钥位于 `JdgSdnRegistry` 中，由 `(Algorithm, public_key_bytes)` 加密。
- `JdgSdnKeyRecord`记录激活高度，可选退役高度，
  和可选的父密钥。
- 旋转由 `JdgSdnRotationPolicy` 控制（当前：`dual_publish_blocks`
  重叠窗口）。注册子密钥会将父密钥更新为
  `child.activation + dual_publish_blocks`，带护栏：
  - 失踪的父母被拒绝
  - 激活必须严格增加
  - 超过宽限窗口的重叠被拒绝
- 注册表助手显示已安装记录（`record`、`keys`）以了解状态
  和 API 暴露。

## 验证流程
- `JdgAttestation::validate_with_sdn_registry` 包含结构
  认证检查和 SDN 实施。 `JdgSdnPolicy` 线程：
  - `require_commitments`：强制存在 PII/秘密有效负载
  - `rotation`：更新父退休时使用的宽限期
- 每项承诺均经过检查：
  - 结构有效性+证明范围匹配
  - 注册关键存在
  - 活动窗口覆盖已证明的区块范围（退休范围已经
    包括双重发布宽限期）
  - 域名标记承诺主体上的有效印章
- 稳定误差显示操作员证据索引：
  `MissingSdnCommitments`、`UnknownSdnKey`、`InactiveSdnKey`、`InvalidSeal`、
  或结构性 `Commitment`/`ScopeMismatch` 故障。

## 操作手册
- **规定：** 在以下时间或之前使用 `activated_at` 注册第一个 SDN 密钥
  第一个秘密区块高度。将密钥指纹发布给 JDG 运营商。
- **旋转：**生成后继密钥，用 `rotation_parent` 注册
  指向当前键，并确认父退休等于
  `child_activation + dual_publish_blocks`。重新密封有效负载承诺
  重叠窗口期间的活动键。
- **审核：**通过 Torii/status 公开注册表快照（`record`、`keys`）
  以便审计员可以确认活动密钥和报废窗口。警报
  如果证明的范围落在活动窗口之外。
- **恢复：** `UnknownSdnKey` → 确保注册表包含密封密钥；
  `InactiveSdnKey` → 旋转或调整激活高度； `InvalidSeal` →
  重新密封有效负载并刷新证明。## 运行时助手
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) 套餐保单 +
  通过 `validate_with_sdn_registry` 注册并验证证明。
- 可以从 Norito 编码的 `JdgSdnKeyRecord` 捆绑包加载注册表（请参阅
  `JdgSdnEnforcer::from_reader`/`from_path`) 或组装
  `from_records`，在注册过程中应用旋转护栏。
- 运营商可以保留 Norito 捆绑包作为 Torii/状态的证据
  浮出水面，同时相同的有效负载为准入和使用的执行器提供信息
  共识守护者。单个全局执行器可以在启动时通过以下方式初始化
  `init_enforcer_from_path` 和 `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  公开状态/Torii 表面的实时策略+关键记录。

## 测试
- `crates/iroha_data_model/src/jurisdiction.rs` 中的回归覆盖率：
  `sdn_registry_accepts_active_commitment`，`sdn_registry_rejects_unknown_key`，
  `sdn_registry_rejects_inactive_key`、`sdn_registry_rejects_bad_signature`、
  `sdn_registry_sets_parent_retirement_window`，
  `sdn_registry_rejects_overlap_beyond_policy`，以及现有的
  结构证明/SDN 验证测试。
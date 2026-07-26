---
lang: zh-hans
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-08T21:57:18.412403+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JDG 证明：保护、轮换和保留

本说明记录了现在在 `iroha_core` 中提供的 v1 JDG 证明防护。

- **委员会清单：** Norito 编码的 `JdgCommitteeManifest` 捆绑包携带每个数据空间轮换
  计划（`committee_id`、有序成员、阈值、`activation_height`、`retire_height`）。
  清单加载了 `JdgCommitteeSchedule::from_path` 并强制严格递增
  退休/激活之间具有可选宽限重叠 (`grace_blocks`) 的激活高度
  委员会。
- **证明防护：** `JdgAttestationGuard` 强制执行数据空间绑定、过期、陈旧边界，
  委员会 ID/阈值匹配、签名者成员资格、支持的签名方案以及可选
  通过 `JdgSdnEnforcer` 进行 SDN 验证。大小上限、最大延迟和允许的签名方案是
  构造函数参数； `validate(attestation, dataspace, current_height)` 返回活动的
  委员会或结构性错误。
  - `scheme_id = 1` (`simple_threshold`)：每个签名者签名，可选签名者位图。
  - `scheme_id = 2` (`bls_normal_aggregate`)：单个预聚合的 BLS-正常签名
    证明哈希；签名者位图可选，默认为证明中的所有签名者。劳工统计局
    聚合验证需要清单中每个委员会成员的有效 PoP；缺失或
    无效的 PoP 拒绝证明。
  通过 `governance.jdg_signature_schemes` 配置允许列表。
- **保留存储：** `JdgAttestationStore` 使用可配置的跟踪每个数据空间的证明
  每个数据空间上限，在插入时修剪最旧的条目。致电 `for_dataspace` 或
  `for_dataspace_and_epoch` 用于检索审核/重播包。
- **测试：** 单位覆盖范围现在执行有效的委员会选择、未知签名者拒绝、陈旧
  证明拒绝、不支持的方案 ID 和保留修剪。参见
  `crates/iroha_core/src/jurisdiction.rs`。

警卫拒绝配置的允许列表之外的方案。
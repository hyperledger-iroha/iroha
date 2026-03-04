---
lang: zh-hans
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 域名背书

域名背书让运营商可以根据委员会签署的声明来控制域名的创建和重用。背书有效负载是记录在链上的 Norito 对象，因此客户端可以审核谁在何时证明了哪个域。

## 有效负载形状

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`：规范域标识符
- `committee_id`：人类可读的委员会标签
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`：区块高度边界有效性
- `scope`：可选数据空间加上可选 `[block_start, block_end]` 窗口（包含），**必须**覆盖接受块高度
- `signatures`：`body_hash()` 上的签名（用 `signatures = []` 背书）
- `metadata`：可选的 Norito 元数据（提案 ID、审核链接等）

## 执行

- 当启用 Nexus 和 `nexus.endorsement.quorum > 0` 时，或者当每个域策略将域标记为必需时，需要认可。
- 验证强制执行域/语句哈希绑定、版本、块窗口、数据空间成员资格、到期/年龄和委员会法定人数。签名者必须拥有具有 `Endorsement` 角色的实时共识密钥。重播被 `body_hash` 拒绝。
- 域注册附加的认可使用元数据密钥 `endorsement`。 `SubmitDomainEndorsement` 指令使用相同的验证路径，该指令记录审核背书，而无需注册新域。

## 委员会和政策

- 委员会可以在链上注册（`RegisterDomainCommittee`）或从配置默认值派生（`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`，id = `default`）。
- 通过 `SetDomainEndorsementPolicy`（委员会 ID、`max_endorsement_age`、`required` 标志）配置每个域的策略。如果不存在，则使用 Nexus 默认值。

## CLI 助手

- 构建/签署背书（将 Norito JSON 输出到标准输出）：

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- 提交认可：

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- 管理治理：
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

验证失败返回稳定的错误字符串（仲裁不匹配、过时/过期的背书、范围不匹配、未知的数据空间、缺少委员会）。
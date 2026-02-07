---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T14:45:02.068538+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ SM配置迁移

# SM配置迁移

推出 SM2/SM3/SM4 功能集需要的不仅仅是使用
`sm` 功能标志。节点控制分层背后的功能
`iroha_config` 配置文件并期望创世清单携带匹配
默认值。本说明捕获了推广时推荐的工作流程
现有网络从“仅 Ed25519”到“启用 SM”。

## 1. 验证构建配置文件

- 使用 `--features sm` 编译二进制文件；仅当您添加 `sm-ffi-openssl`
  计划行使OpenSSL/Tongsuo预览路径。无需 `sm` 即可构建
  即使配置启用，功能也会在准入期间拒绝 `sm2` 签名
  他们。
- 确认 CI 发布 `sm` 工件并且所有验证步骤（“cargo
  测试 -p iroha_crypto --features sm`、集成装置、模糊套件）通过
  关于您打算部署的确切二进制文件。

## 2. 层配置覆盖

`iroha_config` 应用三层：`defaults` → `user` → `actual`。运送 SM
覆盖运营商分发给验证者的 `actual` 配置文件中的内容
仅将 `user` 保留为 Ed25519，以便开发人员默认值保持不变。

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

通过“kagami genesis”将相同的块复制到 `defaults/genesis` 清单中
如果需要，生成…` (add `--allowed-signing sm2 --default-hash sm3-256`
覆盖），因此 `parameters` 块和注入的元数据与
运行时配置。当清单和配置相同时，对等点拒绝启动
快照存在分歧。

## 3. 重新生成创世清单

- 为每个运行 `kagami genesis generate --consensus-mode <mode>`
  环境并提交更新的 JSON 以及 TOML 覆盖。
- 签署清单 (`kagami genesis sign …`) 并分发 `.nrt` 有效负载。
  从未签名的 JSON 清单引导的节点派生运行时加密
  直接从文件进行配置——仍然具有相同的一致性
  检查。

## 4. 流量前验证

- 使用新的二进制文件和配置配置临时集群，然后验证：
  - 一旦对等点重新启动，`/status` 就会公开 `crypto.sm_helpers_available = true`。
  - Torii 入场仍然拒绝 SM2 签名，而 `sm2` 不存在
    `allowed_signing` 并接受混合 Ed25519/SM2 批次时列表
    包括这两种算法。
  - `iroha_cli tools crypto sm2 export …` 往返密钥材料通过新种子播种
    默认值。
- 运行涵盖 SM2 确定性签名的集成烟雾脚本
  SM3 哈希用于确认主机/VM 一致性。

## 5. 回滚计划- 记录逆转：从 `allowed_signing` 中删除 `sm2` 并恢复
  `default_hash = "blake2b-256"`。通过相同的 `actual` 推送更改
  配置文件管道，以便每个验证器单调翻转。
- 将 SM 清单保存在磁盘上；看到配置和创世不匹配的同行
  数据拒绝启动，这可以防止部分回滚。
- 如果涉及OpenSSL/Tongsuo预览版，请包含禁用步骤
  `crypto.enable_sm_openssl_preview` 并从
  运行时环境。

## 参考资料

- [`docs/genesis.md`](../../genesis.md) – 创世清单的结构和
  `crypto` 块。
- [`docs/source/references/configuration.md`](../references/configuration.md) –
  `iroha_config` 部分和默认值的概述。
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – 结束
  用于运输 SM 加密的最终操作员清单。
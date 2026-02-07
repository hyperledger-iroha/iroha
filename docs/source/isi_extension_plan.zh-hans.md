---
lang: zh-hans
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3502fc6de75095282d44ce778b00d1b0d554773de1861d1b92f7dc573dfafa2
source_last_modified: "2025-12-29T18:16:35.969398+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ISI 扩展计划 (v1)

本说明签署了新的 Iroha 特别说明的优先顺序并捕获
每条指令在执行之前都有不可协商的不变量。排序匹配
安全性和可操作性风险第一，用户体验吞吐量第二。

## 优先级堆栈

1. **RotateAccountSignatory** – 在没有破坏性迁移的情况下卫生密钥轮换所需。
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – 提供确定性合约
   针对受损部署的终止开关和存储回收。
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – 将元数据奇偶校验扩展到具体资产
   余额，以便可观察性工具可以标记资产。
4. **BatchMintAsset** / **BatchTransferAsset** – 确定性扇出助手以保持有效负载大小
   虚拟机回退压力可控。

## 指令不变量

### 设置资产键值/删除资产键值
- 重用 `AssetMetadataKey` 命名空间 (`state.rs`)，以便规范 WSV 密钥保持稳定。
- 对帐户元数据帮助程序执行相同的 JSON 大小和架构限制。
- 发出 `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` 以及受影响的 `AssetId`。
- 需要与现有资产元数据编辑相同的权限令牌（定义所有者或
  `CanModifyAssetMetadata` 式补助金）。
- 如果资产记录丢失则中止（无隐式创建）。

### 旋转帐户签名
- `AccountId` 中签名者的原子交换，同时保留帐户元数据和链接
  资源（资产、触发器、角色、权限、待处理事件）。
- 验证当前签名者与调用者匹配（或通过显式令牌委派权限）。
- 如果新的公钥已支持同一域中的另一个帐户，则拒绝。
- 更新所有嵌入帐户 ID 的规范密钥，并在提交前使缓存失效。
- 发出专用的 `AccountEvent::SignatoryRotated`，其中包含旧/新密钥，用于审计跟踪。
- 迁移脚手架：引入 `AccountLabel` + `AccountRekeyRecord` （参见 `account::rekey`）
  现有帐户可以在滚动升级期间映射到稳定标签，而不会出现哈希中断。

### 停用ContractInstance
- 删除或删除 `(namespace, contract_id)` 绑定，同时保留来源数据
  （谁、何时、原因代码）用于故障排除。
- 需要与激活相同的治理权限集，并使用策略挂钩来禁止
  在未经高级批准的情况下停用核心系统名称空间。
- 当实例已处于非活动状态时拒绝以保持事件日志的确定性。
- 发出下游观察者可以使用的 `ContractInstanceEvent::Deactivated`。### 删除SmartContractBytes
- 仅当没有清单或活动实例时才允许 `code_hash` 修剪存储的字节码
  参考工件；否则会因描述性错误而失败。
- 权限门镜注册（`CanRegisterSmartContractCode`）加上操作员级别
  警卫（例如，`CanManageSmartContractStorage`）。
- 在删除之前验证提供的 `code_hash` 与存储的正文摘要匹配，以避免
  陈旧的手柄。
- 发出带有哈希值和调用者元数据的 `ContractCodeEvent::Removed`。

### BatchMintAsset / BatchTransferAsset
- 全有或全无语义：要么每个元组都成功，要么指令无方中止
  影响。
- 输入向量必须是确定性排序的（无隐式排序）并受配置限制
  （`max_batch_isi_items`）。
- 发出每项资产事件，以便下游会计保持一致；批处理上下文是可加的，
  不是替代品。
- 权限检查重用每个目标的现有单项逻辑（资产所有者、定义所有者、
  或授予的能力）在状态突变之前。
- 建议访问集必须联合所有读/写键以保持乐观并发正确。

## 实施脚手架

- 数据模型现在带有用于平衡元数据的 `SetAssetKeyValue` / `RemoveAssetKeyValue` 支架
  编辑（`transparent.rs`）。
- 执行者访问者公开占位符，一旦主机接线登陆，这些占位符将控制权限
  （`default/mod.rs`）。
- 重新生成密钥原型类型 (`account::rekey`) 为滚动迁移提供着陆区。
- 世界状态包括由 `AccountLabel` 键控的 `account_rekey_records`，因此我们可以暂存标签 →
  签名迁移而不触及历史 `AccountId` 编码。

## IVM 系统调用起草

- `DeactivateContractInstance` / `RemoveSmartContractBytes` 的主机垫片作为
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) 和
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44)，两者都消耗镜像 Norito TLV
  规范的 ISI 结构。
- 仅在主机处理程序镜像 `iroha_core` 执行路径后扩展 `abi_syscall_list()` 以保留
  ABI 哈希在开发过程中保持稳定。
- 更新 Kotodama，系统调用数量稳定后降低；为扩展添加黄金覆盖
  同时表面。

## 状态

上述排序和不变量已准备好实施。后续分支应参考
本文档在连接执行路径和系统调用暴露时。
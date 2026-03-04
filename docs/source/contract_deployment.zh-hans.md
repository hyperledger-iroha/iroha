---
lang: zh-hans
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

状态：由 Torii、CLI 和核心入学测试实施和执行（2025 年 11 月）。

## 概述

- 通过将已编译的 IVM 字节码 (`.to`) 提交到 Torii 或通过发布来部署
  `RegisterSmartContractCode`/`RegisterSmartContractBytes`指令
  直接。
- 节点在本地重新计算 `code_hash` 和规范的 ABI 哈希值；不匹配
  果断地拒绝。
- 存储的工件位于链上 `contract_manifests` 和
  `contract_code` 注册表。清单仅引用哈希值并且保持很小；
  代码字节由 `code_hash` 键入。
- 受保护的命名空间可能需要在实施之前制定治理提案
  部署被承认。准入路径查找提案有效负载并
  强制 `(namespace, contract_id, code_hash, abi_hash)` 相等时
  命名空间受到保护。

## 存储的工件和保留

- `RegisterSmartContractCode` 插入/覆盖给定的清单
  `code_hash`。当相同的哈希值已经存在时，它将被新的哈希值替换
  明显。
- `RegisterSmartContractBytes` 将编译后的程序存储在
  `contract_code[code_hash]`。如果哈希的字节已经存在，它们必须匹配
  确切地说；不同的字节会引发不变违规。
- 代码大小受自定义参数 `max_contract_code_bytes` 的限制
  （默认 16 MiB）。之前用 `SetParameter(Custom)` 事务覆盖它
  注册较大的工件。
- 保留不受限制：清单和代码在明确之前保持可用
  在未来的治理工作流程中删除。没有 TTL 或自动 GC。

## 准入管道

- 验证器解析 IVM 标头，强制执行 `version_major == 1`，并检查
  `abi_version == 1`。未知版本立即拒绝；没有运行时间
  切换。
- 当 `code_hash` 的清单已存在时，验证可确保
  存储的 `code_hash`/`abi_hash` 等于提交的计算值
  程序。不匹配会产生 `Manifest{Code,Abi}HashMismatch` 错误。
- 针对受保护名称空间的事务必须包含元数据密钥
  `gov_namespace` 和 `gov_contract_id`。录取路径对比
  反对已颁布的 `DeployContract` 提案；如果不存在匹配的提案
  交易被拒绝，代码为 `NotPermitted`。

## Torii 端点（功能 `app_api`）- `POST /v1/contracts/deploy`
  - 请求正文：`DeployContractDto`（有关字段详细信息，请参阅 `docs/source/torii_contracts_api.md`）。
  - Torii 解码 Base64 有效负载，计算两个哈希值，构建清单，
    并提交 `RegisterSmartContractCode` plus
    `RegisterSmartContractBytes` 在代表签名的交易中
    来电者。
  - 响应：`{ ok, code_hash_hex, abi_hash_hex }`。
  - 错误：base64 无效、ABI 版本不受支持、缺少权限
    (`CanRegisterSmartContractCode`)，超出尺寸上限，治理门控。
- `POST /v1/contracts/code`
  - 接受 `RegisterContractCodeDto`（权限、私钥、清单）并仅提交
    `RegisterSmartContractCode`。当清单单独暂存时使用
    字节码。
- `POST /v1/contracts/instance`
  - 接受 `DeployAndActivateInstanceDto`（权限、私钥、命名空间/contract_id、`code_b64`、可选清单覆盖）并以原子方式部署+激活。
- `POST /v1/contracts/instance/activate`
  - 接受 `ActivateInstanceDto`（权限、私钥、命名空间、contract_id、`code_hash`）并仅提交激活指令。
- `GET /v1/contracts/code/{code_hash}`
  - 返回 `{ manifest: { code_hash, abi_hash } }`。
    其他清单字段在内部保留，但此处省略
    稳定的API。
- `GET /v1/contracts/code-bytes/{code_hash}`
  - 返回 `{ code_b64 }`，其中存储的 `.to` 图像编码为 base64。

所有合约生命周期端点共享一个通过配置的专用部署限制器
`torii.deploy_rate_per_origin_per_sec`（每秒令牌数）和
`torii.deploy_burst_per_origin`（突发令牌）。默认值为 4 req/s，突发
8 对于从 `X-API-Token`、远程 IP 或端点提示派生的每个令牌/密钥。
将任一字段设置为 `null` 以禁用受信任操作员的限制器。当
限制器触发，Torii 递增
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` 遥测计数器和
返回 HTTP 429；任何处理程序错误都会增加
`torii_contract_errors_total{endpoint=…}` 用于警报。

## 治理集成和受保护的命名空间- 设置自定义参数`gov_protected_namespaces`（命名空间的JSON数组
  字符串）以启用准入门控。 Torii 暴露助手
  `/v1/gov/protected-namespaces` 和 CLI 通过以下方式镜像它们
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`。
- 使用 `ProposeDeployContract`（或 Torii）创建的提案
  `/v1/gov/proposals/deploy-contract` 端点）捕获
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`。
- 一旦公投通过，`EnactReferendum` 标记提案已颁布并且
  准入将接受携带匹配元数据和代码的部署。
- 交易必须包含元数据对 `gov_namespace=a namespace` 和
  `gov_contract_id=an identifier`（并且应该设置 `contract_namespace` /
  `contract_id` 用于呼叫时间绑定）。 CLI 帮助程序填充这些
  当您通过 `--namespace`/`--contract-id` 时自动。
- 当启用受保护的命名空间时，队列准入会拒绝尝试
  将现有的 `contract_id` 重新绑定到不同的命名空间；使用已制定的
  建议或在部署到其他地方之前撤销以前的绑定。
- 如果通道清单将验证器法定人数设置为高于 1，请包括
  `gov_manifest_approvers`（验证者帐户 ID 的 JSON 数组），以便队列可以计数
  与交易授权一起的额外批准。车道也拒绝
  引用清单中不存在的命名空间的元数据
  `protected_namespaces` 设置。

## CLI 助手

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  提交 Torii 部署请求（动态计算哈希值）。
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  构建清单（使用提供的密钥签名），注册字节+清单，
  并在一笔交易中激活 `(namespace, contract_id)` 绑定。使用
  `--dry-run` 打印计算出的哈希值和指令计数，无需
  提交，并使用 `--manifest-out` 保存签名的清单 JSON。
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` 计算
  `code_hash`/`abi_hash` 用于编译的 `.to` 并可选择签署清单，
  打印 JSON 或写入 `--out`。
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  运行离线 VM 传递并报告 ABI/哈希元数据以及排队的 ISI
  （计数和指令 ID）无需接触网络。附加
  `--namespace/--contract-id` 用于镜像呼叫时间元数据。
- `iroha_cli app contracts manifest get --code-hash <hex>` 通过 Torii 获取清单
  并可选择将其写入磁盘。
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` 下载
  存储的 `.to` 图像。
- `iroha_cli app contracts instances --namespace <ns> [--table]` 列表已激活
  合约实例（清单+元数据驱动）。
- 治理助手（`iroha_cli app gov deploy propose`、`iroha_cli app gov enact`、
  `iroha_cli app gov protected set/get`）编排受保护的命名空间工作流程并
  公开 JSON 工件以供审核。

## 测试和覆盖范围

- `crates/iroha_core/tests/contract_code_bytes.rs` 覆盖代码下的单元测试
  存储、幂等性和大小上限。
- `crates/iroha_core/tests/gov_enact_deploy.rs` 通过验证清单插入
  制定和 `crates/iroha_core/tests/gov_protected_gate.rs` 练习
  端到端的受保护命名空间准入。
- Torii 路由包括请求/响应单元测试，并且 CLI 命令具有
  集成测试确保 JSON 往返保持稳定。

请参阅 `docs/source/governance_api.md` 了解详细的公投有效负载和
投票工作流程。
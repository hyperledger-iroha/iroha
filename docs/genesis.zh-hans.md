---
lang: zh-hans
direction: ltr
source: docs/genesis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c2eab4379aa346ab7d111e1c51c0230238f260647187f1a33c1819640b9bf2c
source_last_modified: "2026-01-28T14:25:37.056140+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 创世配置

`genesis.json` 文件定义 Iroha 网络启动时运行的第一个事务。该文件是一个包含以下字段的 JSON 对象：

- `chain` – 唯一链标识符。
- `executor`（可选）——执行器字节码的路径 (`.to`)。如果存在的话，
  genesis 包括一个升级指令作为第一笔交易。如果省略，
  不执行升级并使用内置执行器。
- `ivm_dir` – 包含 IVM 字节码库的目录。如果省略，则默认为 `"."`。
- `consensus_mode` – 清单中公布的共识模式。必需的;对于公共 Sora Nexus 数据空间使用 `"Npos"`，对于其他 Iroha3 数据空间使用 `"Permissioned"`/`"Npos"`。 Iroha2 默认为 `"Permissioned"`。
- `transactions` – 按顺序执行的创世交易列表。每个条目可能包含：
  - `parameters` – 初始网络参数。
  - `instructions` – 结构化 Norito 指令（例如 `{ "Register": { "Domain": { "id": "wonderland" }}}`）。不接受原始字节数组，并且此处拒绝 `SetParameter` 指令 - 通过 `parameters` 块提供种子参数，并让规范化/签名注入指令。
  - `ivm_triggers` – 使用 IVM 字节码可执行文件触发。
  - `topology` – 初始对等拓扑。每个条目将对等 ID 和 PoP 保存在一起：`{ "peer": "<public_key>", "pop_hex": "<hex>" }`。 `pop_hex` 在撰写时可以省略，但在签名之前必须存在。
- `crypto` – 从 `iroha_config.crypto` 镜像的加密快照（`default_hash`、`allowed_signing`、`allowed_curve_ids`、`sm2_distid_default`、`sm_openssl_preview`）。 `allowed_curve_ids` 镜像 `crypto.curves.allowed_curve_ids`，因此清单可以通告集群接受哪些控制器曲线。工具强制实施 SM 组合：列出 `sm2` 的清单还必须将哈希值切换为 `sm3-256`，而在没有 `sm` 功能的情况下编译的版本完全拒绝 `sm2`。标准化将 `crypto_manifest_meta` 自定义参数注入到签名的创世中；如果注入的有效负载与通告的快照不一致，节点将拒绝启动。

示例（`kagami genesis generate default --consensus-mode npos` 输出，指令已修剪）：

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [
        { "Register": { "Domain": { "id": "wonderland" } } }
      ],
      "ivm_triggers": [],
      "topology": [
        {
          "peer": "ed25519:...",
          "pop_hex": "ab12cd..."
        }
      ]
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519", "secp256k1"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### 为 SM2/SM3 播种 `crypto` 块

使用 xtask 帮助程序一步生成关键清单和准备粘贴的配置片段：

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

`client-sm2.toml` 现在包含：

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "secp256k1", "sm2"]  # remove "sm2" to stay in verify-only mode
allowed_curve_ids = [1]               # add new curve ids (e.g., 15 for SM2) when controllers are allowed
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```

将 `public_key`/`private_key` 值复制到账户/客户端配置中，并更新 `genesis.json` 的 `crypto` 块，使其与代码片段匹配（例如，将 `default_hash` 设置为 `sm3-256`，添加`"sm2"` 至 `allowed_signing`，并包括右侧的 `allowed_curve_ids`）。 Kagami 将拒绝哈希/曲线设置和签名列表不一致的清单。

> **提示：** 当您只想检查输出时，使用 `--snippet-out -` 将代码片段流式传输到标准输出。还使用 `--json-out -` 在标准输出上发出密钥清单。

如果您更喜欢手动执行较低级别的 CLI 命令，则等效流程为：

```bash
# 1. Produce deterministic key material (writes JSON to disk)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Re-hydrate the snippet that can be pasted into client/config files
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **提示：** 上面使用 `jq` 来节省手动复制/粘贴步骤。如果不可用，则打开 `sm2-key.json`，复制 `private_key_hex` 字段，直接传递给 `crypto sm2 export`。

> **迁移指南：** 将现有网络转换为 SM2/SM3/SM4 时，请按照
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> 用于分层 `iroha_config` 覆盖、清单重新生成和回滚
> 规划。

## 生成并验证

1. 生成模板：
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
   ```
`--consensus-mode` 控制哪些共识参数 Kagami 种子到 `parameters` 块中。公共 Sora Nexus 数据空间需要 `npos` 并且不支持分阶段切换；其他 Iroha3 数据空间可能使用许可或 NPoS。 Iroha2 默认为 `permissioned`，并且可以通过 `--next-consensus-mode`/`--mode-activation-height` 上演 `npos`。当选择 `npos` 时，Kagami 会播种 `sumeragi_npos_parameters` 负载，驱动 NPoS 收集器扇出、选举策略和重新配置窗口；规范化/签名将它们转换为签名块中的 `SetParameter` 指令。
2. （可选）编辑 `genesis.json`，然后验证并签名：
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   要发出 SM2/SM3/SM4 就绪清单，请传递 `--default-hash sm3-256` 并包含 `--allowed-signing sm2`（对于其他算法，请重复 `--allowed-signing`）。如果需要覆盖默认的区别标识符，请使用 `--sm2-distid-default <ID>`。

   当您仅使用 `--genesis-manifest-json`（无签名创世块）启动 `irohad` 时，节点现在会自动从清单中播种其运行时加密配置；如果您还提供了创世块，则清单和配置仍然必须完全匹配。

- 验证说明：
  - Kagami 将 `consensus_handshake_meta`、`confidential_registry_root` 和 `crypto_manifest_meta` 作为 `SetParameter` 指令注入标准化/有符号块中。如果握手元数据或加密快照与编码参数不一致，`irohad` 将重新计算这些有效负载的共识指纹，并导致启动失败。将这些内容保留在清单中的 `instructions` 之外；它们是自动生成的。
- 检查标准化块：
  - 运行 `kagami genesis normalize genesis.json --format text` 以查看最终的有序交易（包括注入的元数据），而无需提供密钥对。
  - 使用 `--format json` 转储适合比较或评论的结构化视图。

`kagami genesis sign` 检查 JSON 是否有效，并生成一个 Norito 编码块，可供通过节点配置中的 `genesis.file` 使用。生成的 `genesis.signed.nrt` 已经采用规范的线形式：版本字节后跟描述有效负载布局的 Norito 标头。始终分发此框架输出。首选 `.nrt` 后缀作为签名的有效负载；如果您不需要在创世时升级执行器，则可以省略 `executor` 字段并跳过提供 `.to` 文件。

签署 NPoS 清单（`--consensus-mode npos` 或仅限 Iroha2 的分阶段切换）时，`kagami genesis sign` 需要 `sumeragi_npos_parameters` 有效负载；使用 `kagami genesis generate --consensus-mode npos` 生成它或手动添加参数。
默认情况下，`kagami genesis sign` 使用清单的 `consensus_mode`；通过 `--consensus-mode` 来覆盖它。

## Genesis 可以做什么

Genesis 支持以下操作。 Kagami 以明确定义的顺序将它们组装成事务，以便对等点确定地执行相同的序列。

- 参数：设置 Sumeragi 的初始值（块/提交时间、漂移）、块（最大 txs）、交易（最大指令、字节码大小）、执行器和智能合约限制（燃料、内存、深度）以及自定义参数。 Kagami 通过 `parameters` 块播种 `Sumeragi::NextMode` 和 `sumeragi_npos_parameters` 有效负载（NPoS 选举、重新配置），以便启动可以从链上状态应用共识旋钮；签名块携带生成的 `SetParameter` 指令。
- 原生指令：注册/注销域名、账户、资产定义；铸造/销毁/转移资产；转移域名和资产定义所有权；修改元数据；授予权限和角色。
- IVM 触发器：执行 IVM 字节码的寄存器触发器（请参阅 `ivm_triggers`）。触发器的可执行文件相对于 `ivm_dir` 进行解析。
- 拓扑：通过任何事务中的 `topology` 数组提供初始对等点集（通常是第一个或最后一个）。每个条目都是 `{ "peer": "<public_key>", "pop_hex": "<hex>" }`； `pop_hex` 在撰写时可以省略，但在签名之前必须存在。
- 执行器升级（可选）：如果存在 `executor`，创世会插入一条升级指令作为第一个交易；否则，创世会直接从参数/指令开始。

### 交易排序

从概念上讲，创世交易按以下顺序处理：

1）（可选）执行器升级
2) 对于 `transactions` 中的每笔交易：
   - 参数更新
   - 原生指令
   - IVM 触发注册
   - 拓扑条目

Kagami 和节点代码确保此排序，以便例如参数在同一事务中的后续指令之前应用。

## 推荐的工作流程

- 从 Kagami 的模板开始：
  - 仅内置 ISI：`kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json`（Sora Nexus 公共数据空间；对于 Iroha2 或私有 Iroha3 使用 `--consensus-mode permissioned`）。
  - 使用自定义执行器升级（可选）：添加 `--executor <path/to/executor.to>`
  - 仅 Iroha2：要在未来切换到 NPoS，请传递 `--next-consensus-mode npos --mode-activation-height <HEIGHT>`（为当前模式保留 `--consensus-mode permissioned`）。
- `<PK>` 是 `iroha_crypto::Algorithm` 识别的任何多重哈希，包括使用 `--features gost` 构建 Kagami 时的 TC26 GOST 变体（例如 `gost3410-2012-256-paramset-a:...`）。
- 编辑时验证：`kagami genesis validate genesis.json`
- 部署签名：`kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- 配置对等点：将 `genesis.file` 设置为已签名的 Norito 文件（例如 `genesis.signed.nrt`），将 `genesis.public_key` 设置为用于签名的相同 `<PK>`。注意事项：
- Kagami 的“默认”模板注册示例域和帐户，铸造一些资产，并仅使用内置 ISI 授予最低权限 - 不需要 `.to`。
- 如果您确实包含执行程序升级，则它必须是第一笔交易。 Kagami 在生成/签名时强制执行此操作。
- 在签名之前使用 `kagami genesis validate` 捕获无效的 `Name` 值（例如空格）和格式错误的指令。

## 使用 Docker/Swarm 运行

提供的 Docker Compose 和 Swarm 工具可以处理这两种情况：

- 没有执行程序：compose 命令会删除丢失/空的 `executor` 字段并对文件进行签名。
- 使用执行器：它将相对执行器路径解析为容器内的绝对路径并对文件进行签名。

这使得在没有预先构建 IVM 示例的机器上进行开发变得简单，同时仍然允许在需要时升级执行器。
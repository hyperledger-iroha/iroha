---
lang: zh-hans
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e153602cfb465bd5f65bab0cf97c44604bba982a7a7f1edc8d5af8fd67a9e29
source_last_modified: "2026-01-22T16:26:46.562262+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 入门

本快速指南展示了编译 Kotodama 合约的最小工作流程，
检查生成的 Norito 字节码，在本地运行并部署它
到 Iroha 节点。

## 先决条件

1. 安装 Rust 工具链（1.76 或更高版本）并查看此存储库。
2. 构建或下载支持的二进制文件：
   - `koto_compile` – 发出 IVM/Norito 字节码的 Kotodama 编译器
   - `ivm_run` 和 `ivm_tool` – 本地执行和检查实用程序
   - `iroha_cli` – 用于通过 Torii 进行合约部署

   存储库 Makefile 需要 `PATH` 上的这些二进制文件。你可以
   下载预构建的工件或从源代码构建它们。如果你编译
   本地工具链，将 Makefile 助手指向二进制文件：

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. 确保到达部署步骤时 Iroha 节点正在运行。的
   下面的示例假设 Torii 可通过您中配置的 URL 访问
   `iroha_cli` 配置文件 (`~/.config/iroha/cli.toml`)。

## 1.编译Kotodama合约

该存储库提供了一个最小的“hello world”合约
`examples/hello/hello.ko`。将其编译为 Norito/IVM 字节码（`.to`）：

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

关键标志：

- `--abi 1` 将合约锁定为 ABI 版本 1（唯一受支持的版本）
  写作时）。
- `--max-cycles 0` 请求无限执行；设置一个正数来绑定
  零知识证明的循环填充。

## 2. 检查 Norito 工件（可选）

使用 `ivm_tool` 验证标头和嵌入元数据：

```sh
ivm_tool inspect target/examples/hello.to
```

您应该看到 ABI 版本、启用的功能标志和导出的条目
点。这是部署前的快速健全性检查。

## 3.本地运行合约

使用 `ivm_run` 执行字节码以确认行为，而无需触摸
节点：

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` 示例记录问候语并发出 `SET_ACCOUNT_DETAIL` 系统调用。
在发布之前迭代合约逻辑时，在本地运行非常有用
它在链上。

## 4. 通过 `iroha_cli` 部署

当您对合同感到满意时，请使用 CLI 将其部署到节点。
提供授权帐户、其签名密钥以及 `.to` 文件或
Base64 有效负载：

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

该命令通过 Torii 提交 Norito 清单 + 字节码包并打印
由此产生的交易状态。事务提交后，代码
响应中显示的哈希可用于检索清单或列出实例：

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. 运行 Torii

注册字节码后，您可以通过提交指令来调用它
引用存储的代码（例如，通过 `iroha_cli ledger transaction submit`
或您的应用程序客户端）。确保帐户权限允许所需的
系统调用（`set_account_detail`、`transfer_asset` 等）。

## 提示和故障排除

- 使用 `make examples-run` 编译并执行所提供的示例
  射击。如果二进制文件未打开，则覆盖 `KOTO`/`IVM` 环境变量
  `PATH`。
- 如果 `koto_compile` 拒绝 ABI 版本，请验证编译器和节点
  两者都以 ABI v1 为目标（运行 `koto_compile --abi`，不带参数列出
  支持）。
- CLI 接受十六进制或 Base64 签名密钥。为了进行测试，您可以使用
  由 `iroha_cli tools crypto keypair` 发出的密钥。
- 调试 Norito 有效负载时，`ivm_tool disassemble` 子命令有帮助
  将指令与 Kotodama 源关联起来。

此流程反映了 CI 和集成测试中使用的步骤。为了更深入
深入了解 Kotodama 语法、系统调用映射和 Norito 内部结构，请参阅：

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
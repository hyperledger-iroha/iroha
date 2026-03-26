---
lang: zh-hans
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e39dc94f52395bd9323177df1a7feeb7bbd4f9a3cdea07b02f9d60e7826e199e
source_last_modified: "2026-01-22T16:26:46.506936+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Quickstart
description: Build, validate, and deploy a Kotodama contract with the release tooling and default single-peer network.
slug: /norito/quickstart
translator: machine-google-reviewed
---

本演练反映了我们希望开发人员在学习时遵循的工作流程
首次Norito和Kotodama：启动确定性单点网络，
编译合约，在本地试运行，然后通过 Torii 发送
参考 CLI。

该示例合约将键/值对写入调用者的帐户，以便您可以
立即使用 `iroha_cli` 验证副作用。

## 先决条件

- [Docker](https://docs.docker.com/engine/install/) 启用 Compose V2（使用
  启动 `defaults/docker-compose.single.yml` 中定义的示例对等点）。
- Rust 工具链（1.76+），用于构建辅助二进制文件（如果您不下载）
  已发表的。
- `koto_compile`、`ivm_run` 和 `iroha_cli` 二进制文件。您可以从以下位置构建它们
  工作区结帐如下所示或下载匹配的发布工件：

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> 上面的二进制文件可以安全地与工作区的其余部分一起安装。
> 它们从不链接到 `serde`/`serde_json`； Norito 编解码器是端到端强制执行的。

## 1. 启动单点开发网络

该存储库包含由 `kagami swarm` 生成的 Docker Compose 捆绑包
（`defaults/docker-compose.single.yml`）。它连接默认的创世、客户端
配置和运行状况探测，以便可以在 `http://127.0.0.1:8080` 处访问 Torii。

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

让容器保持运行（在前台或分离）。全部
后续 CLI 调用通过 `defaults/client.toml` 定位该对等点。

## 2. 编写合同

创建一个工作目录并保存最小的 Kotodama 示例：

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> 更喜欢将 Kotodama 源代码保留在版本控制中。门户托管的示例是
> 如果您也可以在 [Norito 示例库](./examples/) 下找到
> 想要一个更丰富的起点。

## 3. 使用 IVM 编译并试运行

将合约编译为 IVM/Norito 字节码（`.to`）并在本地执行
在接触网络之前确认主机系统调用成功：

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

运行程序打印 `info("Hello from Kotodama")` 日志并执行
`SET_ACCOUNT_DETAIL` 针对模拟主机的系统调用。如果可选`ivm_tool`
二进制可用，`ivm_tool inspect target/quickstart/hello.to` 显示
ABI 标头、功能位和导出的入口点。

## 4.通过Torii提交字节码

在节点仍在运行的情况下，使用 CLI 将编译后的字节码发送到 Torii。
默认的开发身份来自于公钥
`defaults/client.toml`，所以账户ID为
```
<i105-account-id>
```

使用配置文件提供 Torii URL、链 ID 和签名密钥：

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI 使用 Norito 对交易进行编码，使用开发密钥对其进行签名，然后
将其提交给正在运行的对等点。观看 `set_account_detail` 的 Docker 日志
系统调用或监视 CLI 输出以获取已提交的事务哈希。

## 5.验证状态变化

使用相同的 CLI 配置文件来获取合约写入的帐户详细信息：

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

您应该看到 Norito 支持的 JSON 有效负载：

```json
{
  "hello": "world"
}
```

如果该值缺失，请确认 Docker 撰写服务仍然存在
正在运行并且 `iroha` 报告的交易哈希达到了 `Committed`
状态。

## 后续步骤

- 探索自动生成的[示例库](./examples/) 以查看
  更高级的 Kotodama 片段如何映射到 Norito 系统调用。
- 阅读 [Norito 入门指南](./getting-started) 了解更深入的信息
  编译器/运行器工具、清单部署和 IVM 的说明
  元数据。
- 迭代您自己的合约时，请在
  用于重新生成可下载片段的工作区，以便保留门户文档和工件
  与 `crates/ivm/docs/examples/` 下的来源同步。
---
lang: zh-hans
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2025-12-29T18:16:35.984945+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI 包装指南

本指南解释了如何构建 MOCHI 桌面管理程序包、检查
生成的工件，并调整随
捆绑。它通过关注可复制的包装来补充快速入门
和 CI 的使用。

## 先决条件

- Rust 工具链（2024 版/Rust 1.82+）与工作区依赖项
  已经建成了。
- 针对所需目标编译的 `irohad`、`iroha_cli` 和 `kagami`。的
  捆绑器重用 `target/<profile>/` 中的二进制文件。
- `target/` 或自定义下的捆绑输出有足够的磁盘空间
  目的地。

在运行捆绑器之前构建一次依赖项：

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## 构建捆绑包

从存储库根调用专用 `xtask` 命令：

```bash
cargo xtask mochi-bundle
```

默认情况下，这会在 `target/mochi-bundle/` 下生成一个发行包，其名称为
从主机操作系统和体系结构派生的文件名（例如，
`mochi-macos-aarch64-release.tar.gz`）。使用以下标志进行自定义
构建：

- `--profile <name>` – 选择货物配置文件（`release`、`debug` 或
  自定义配置文件）。
- `--no-archive` – 保留扩展目录而不创建 `.tar.gz`
  存档（对于本地测试有用）。
- `--out <path>` – 将捆绑包写入自定义目录而不是
  `target/mochi-bundle/`。
- `--kagami <path>` – 提供预构建的 `kagami` 可执行文件以包含在
  存档。当省略时，捆绑器会重用（或构建）来自
  选定的配置文件。
- `--matrix <path>` – 将捆绑包元数据附加到 JSON 矩阵文件（如果创建
  缺失），因此 CI 管道可以记录在一个进程中生成的每个主机/配置文件工件
  跑。条目包括捆绑目录、清单路径和 SHA-256（可选）
  存档位置以及最新的冒烟测试结果。
- `--smoke` – 将打包的 `mochi --help` 作为轻量级烟门执行
  捆绑后；在发布之前失败会导致缺少依赖项
  人工制品。
- `--stage <path>` – 将完成的捆绑包（以及生成时的存档）复制到
  暂存目录，以便多平台构建可以将工件存放在一个目录中
  无需额外脚本的位置。

该命令复制 `mochi-ui-egui`、`kagami`、`LICENSE`、示例
配置，并将 `mochi/BUNDLE_README.md` 放入捆绑包中。确定性的
`manifest.json` 与二进制文件一起生成，以便 CI 作业可以跟踪文件
哈希值和大小。

## 捆绑包布局和验证

扩展包遵循 `BUNDLE_README.md` 中记录的布局：

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

`manifest.json` 文件列出了每个工件及其 SHA-256 哈希值。验证
将捆绑包复制到另一个系统后：

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

CI 管道可以缓存扩展目录、对存档进行签名或发布
清单和发行说明。清单包含生成器
配置文件、目标三元组和创建时间戳以帮助来源跟踪。

## 运行时覆盖

MOCHI 通过 CLI 标志或发现帮助程序二进制文件和运行时位置
环境变量：- `--data-root` / `MOCHI_DATA_ROOT` – 覆盖用于对等的工作空间
  配置、存储和日志。
- `--profile` – 在拓扑预设之间切换（`single-peer`、
  `four-peer-bft`）。
- `--torii-start`、`--p2p-start` – 更改分配时使用的基本端口
  服务。
- `--irohad` / `MOCHI_IROHAD` – 指向特定的 `irohad` 二进制文件。
- `--kagami` / `MOCHI_KAGAMI` – 覆盖捆绑的 `kagami`。
- `--iroha-cli` / `MOCHI_IROHA_CLI` – 覆盖可选的 CLI 帮助程序。
- `--restart-mode <never|on-failure>` – 禁用自动重启或强制
  指数退避策略。
- `--restart-max <attempts>` – 覆盖重新启动尝试的次数
  在 `on-failure` 模式下运行。
- `--restart-backoff-ms <millis>` – 设置自动重启的基本退避。
- `MOCHI_CONFIG` – 提供自定义 `config/local.toml` 路径。

CLI 帮助 (`mochi --help`) 打印完整的标志列表。环境覆盖
启动时生效，可以与里面的设置对话框结合使用
用户界面。

## CI 使用提示

- 运行`cargo xtask mochi-bundle --no-archive`生成一个目录，可以
  使用特定于平台的工具进行压缩（Windows 为 ZIP，Windows 为 tarball）
  Unix）。
- 使用 `cargo xtask mochi-bundle --matrix dist/matrix.json` 捕获捆绑包元数据
  因此发布作业可以发布列出每个主机/配置文件的单个 JSON 索引
  管道中产生的人工制品。
- 在每个上使用 `cargo xtask mochi-bundle --stage /mnt/staging/mochi` （或类似的）
  构建代理将捆绑包和存档上传到共享目录
  发布作业可以消耗。
- 发布存档和 `manifest.json` 以便操作员可以验证捆绑包
  诚信。
- 将生成的目录存储为构建工件，以种子烟雾测试
  使用确定性打包的二进制文件来锻炼主管。
- 在发行说明或 `status.md` 日志中记录捆绑包哈希以供将来使用
  出处检查。
---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Release Process
summary: Run the CLI/SDK release gate, apply the shared versioning policy, and publish canonical release notes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 发布流程

SoraFS 二进制文件（`sorafs_cli`、`sorafs_fetch`、帮助程序）和 SDK 包
（`sorafs_car`、`sorafs_manifest`、`sorafs_chunker`）一起发货。发布
管道使 CLI 和库保持一致，确保 lint/测试覆盖率，以及
为下游消费者捕获人工制品。为每个项目运行下面的清单
候选标签。

## 0. 确认安全审查签核

在执行技术发布门之前，捕获最新的安全审查
文物：

- 下载最新的 SF-6 安全审查备忘录 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  并在发布票据中记录其 SHA256 哈希值。
- 附上补救票链接（例如 `governance/tickets/SF6-SR-2026.md`）并记下签字
  来自安全工程和工具工作组的批准者。
- 验证备忘录中的补救清单是否已关闭；未解决的项目会阻止发布。
- 准备上传奇偶校验线束日志 (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  与清单包一起。
- 确认您计划运行的签名命令包括 `--identity-token-provider` 和显式
  `--identity-token-audience=<aud>` 因此 Fulcio 范围在发布证据中被捕获。

在通知治理和发布版本时包括这些工件。

## 1. 执行发布/测试门

`ci/check_sorafs_cli_release.sh` 帮助程序运行格式化、Clippy 和测试
跨 CLI 和 SDK 包，带有工作区本地目标目录 (`.target`)
以避免在 CI 容器内执行时发生权限冲突。

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

该脚本执行以下断言：

- `cargo fmt --all -- --check`（工作区）
- `cargo clippy --locked --all-targets` 用于 `sorafs_car`（具有 `cli` 功能），
  `sorafs_manifest` 和 `sorafs_chunker`
- `cargo test --locked --all-targets` 对于那些相同的板条箱

如果任何步骤失败，请在标记之前修复回归。发布版本必须是
与主线连续；不要将修复挑选到发布分支中。大门
还检查无密钥签名标志（`--identity-token-issuer`、`--identity-token-audience`）
在适用的情况下提供；缺少参数导致运行失败。

## 2. 应用版本控制策略

所有 SoraFS CLI/SDK 包都使用 SemVer：

- `MAJOR`：在第一个 1.0 版本中引入。 1.0 之前，`0.y` 略有不同
  **表示 CLI 表面或 Norito 架构中的重大更改**。
  可选策略、遥测添加后面的字段）。
- `PATCH`：错误修复、仅文档版本以及依赖项更新
  不改变可观察到的行为。

始终将 `sorafs_car`、`sorafs_manifest` 和 `sorafs_chunker` 保持在同一位置
版本，以便下游 SDK 消费者可以依赖于单个对齐版本
字符串。当碰撞版本时：

1. 更新每个 crate 的 `Cargo.toml` 中的 `version =` 字段。
2. 通过 `cargo update -p <crate>@<new-version>` 重新生成 `Cargo.lock`（
   工作区强制执行显式版本）。
3. 再次运行释放门以确保没有陈旧的工件残留。

## 3. 准备发行说明

每个版本都必须发布一个 markdown 变更日志，重点介绍 CLI、SDK 和
影响治理的变革。使用中的模板
`docs/examples/sorafs_release_notes.md`（将其复制到您的发布工件中
目录并填写具体细节的部分）。

最低内容：

- **亮点**：CLI 和 SDK 消费者的功能标题。
  要求。
- **升级步骤**：TL;DR命令用于碰撞货物依赖性并重新运行
  确定性的固定装置。
- **验证**：命令输出哈希值或信封以及确切的
  已执行 `ci/check_sorafs_cli_release.sh` 修订版。

将填写的发行说明附加到标签（例如 GitHub 发行正文）并存储
它们与确定性生成的人工制品一起。

## 4. 执行释放钩子

运行 `scripts/release_sorafs_cli.sh` 生成签名包并
每个版本附带的验证摘要。包装器构建 CLI
必要时，调用`sorafs_cli manifest sign`，并立即重放
`manifest verify-signature` 因此故障在标记之前就会出现。示例：

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

温馨提示：

- 跟踪您的发布输入（有效负载、计划、摘要、预期令牌哈希）
  存储库或部署配置，以便脚本保持可重现。 CI 夹具
  `fixtures/sorafs_manifest/ci_sample/` 下的捆绑包显示了规范布局。
- 基于 `.github/workflows/sorafs-cli-release.yml` 的 CI 自动化；它运行
  发布门，调用上面的脚本，并将捆绑包/签名存档为
  工作流程工件。镜像相同的命令顺序（释放门→标志→
  在其他 CI 系统中验证），以便审核日志与生成的哈希值保持一致。
- 保留生成的`manifest.bundle.json`、`manifest.sig`，
  `manifest.sign.summary.json` 和 `manifest.verify.summary.json` 在一起——它们
  形成治理通知中引用的数据包。
- 当版本更新规范固定装置时，复制刷新的清单，
  块计划，并将摘要写入 `fixtures/sorafs_manifest/ci_sample/`（并更新
  `docs/examples/sorafs_ci_sample/manifest.template.json`) 在标记之前。
  下游运营商依赖于已提交的固定装置来重现发布
  捆绑。
- 捕获 `sorafs_cli proof stream` 有界通道验证的运行日志并将其附加到
  发布数据包以证明流媒体防护措施仍然有效。
- 在发行说明中记录签名期间使用的确切 `--identity-token-audience`；治理
  在批准出版之前，根据 Fulcio 政策对受众进行交叉检查。

当版本还带有
网关推出。将其指向同一个清单包以证明证明
匹配候选工件：

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. 标记并发布

检查通过并挂钩完成后：

1. 运行 `sorafs_cli --version` 和 `sorafs_fetch --version` 来确认二进制文件
   报告新版本。
2.在签入的`sorafs_release.toml`中准备发布配置
   （首选）或部署存储库跟踪的另一个配置文件。避免
   依赖临时环境变量；将路径传递给 CLI
   `--config`（或等效），因此释放输入是明确的并且
   可重现。
3. 创建签名标签（首选）或注释标签：
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. 上传工件（CAR 包、清单、证明摘要、发行说明、
   认证输出）到项目注册处进行治理
   [部署指南](./developer-deployment.md) 中的清单。如果释放
   创建新的固定装置，将它们推送到共享固定装置存储库或对象存储中，以便
   审计自动化可以将已发布的捆绑包与源代码控制进行区分。
5. 通知治理渠道，提供签名标签、发行说明、
   清单包/签名哈希值、存档的 `manifest.sign/verify` 摘要、
   以及任何证明信封。包括 CI 作业 URL（或日志存档）
   运行 `ci/check_sorafs_cli_release.sh` 和 `scripts/release_sorafs_cli.sh`。更新
   治理票证，以便审计员可以追踪对人工制品的批准；当
   `.github/workflows/sorafs-cli-release.yml` 职位发布通知，链接
   记录哈希输出而不是粘贴临时摘要。

## 6. 发布后跟进

- 确保文档指向新版本（快速入门、CI 模板）
  已更新或确认无需更改。
- 后续工作的文件路线图条目（例如，迁移标志、弃用
- 为审计员存档发布门输出日志——将它们存储在签名的旁边
  文物。

遵循此管道可以将 CLI、SDK 包和治理抵押品保留在
每个发布周期的锁步。
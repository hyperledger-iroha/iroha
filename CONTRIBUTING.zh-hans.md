---
lang: zh-hans
direction: ltr
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-29T18:16:34.772429+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 贡献指南

感谢您花时间为 Iroha 2 做出贡献！

请阅读本指南，了解您如何做出贡献以及我们希望您遵循哪些准则。这包括有关代码和文档的指南以及有关 git 工作流程的约定。

阅读这些指南将为您节省以后的时间。

## 我如何做出贡献？

您可以通过多种方式为我们的项目做出贡献：

- 报告[错误](#reporting-bugs) 和[漏洞](#reporting-vulnerabilities)
- [建议改进](#suggesting-improvements)并实施
- [提出问题](#asking-questions) 并与社区互动

我们的项目是新的吗？ [做出你的第一个贡献](#your-first-code-contribution)！

### 太长了；博士

- 找到[ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240)。
- 叉子 [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main)。
- 解决您的选择问题。
- 确保您遵循我们的代码和文档的[风格指南](#style-guides)。
- 编写[测试](https://doc.rust-lang.org/cargo/commands/cargo-test.html)。确保它们全部通过 (`cargo test --workspace`)。如果您接触 SM 加密堆栈，还可以运行 `cargo test -p iroha_crypto --features "sm sm_proptest"` 来执行可选的模糊/属性工具。
  - 注意：如果 `defaults/executor.to` 不存在，则执行 IVM 执行器的测试将自动合成最小的确定性执行器字节码。运行测试不需要任何预先步骤。要生成奇偶校验的规范字节码，您可以运行：
    - `cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    - `cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- 如果您更改derive/proc-macro crates，请通过以下方式运行trybuild UI套件
  `make check-proc-macro-ui`（或
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) 并刷新
  `.stderr` 在诊断更改时固定以保持消息稳定。
- 运行 `make dev-workflow`（`scripts/dev_workflow.sh` 的包装）以使用 `--locked` 和 `swift test` 执行 fmt/clippy/build/test；预计 `cargo test --workspace` 需要几个小时，并且仅将 `--skip-tests` 用于快速本地循环。有关完整的操作手册，请参阅 `docs/source/dev_workflow.md`。
- 使用 `make check-agents-guardrails` 强制执行护栏，以阻止 `Cargo.lock` 编辑和新的工作区 crate，`make check-dependency-discipline` 在新依赖项上失败（除非明确允许），并使用 `make check-missing-docs` 来防止新的 `#[allow(missing_docs)]` 垫片、触摸的 crate 上缺少 crate 级文档或新的公共项目没有文档注释（守卫通过 `scripts/inventory_missing_docs.py` 刷新 `docs/source/agents/missing_docs_inventory.{json,md}`）。添加 `make check-tests-guard`，除非单元测试引用它们（内联 `#[cfg(test)]`/`#[test]` 块或包 `tests/`；现有覆盖率计数）和 `make check-docs-tests-metrics`，否则更改的功能将失败，因此路线图更改与文档、测试和指标/仪表板配对。通过 `make check-todo-guard` 保持 TODO 强制执行，以便在没有随附文档/测试的情况下不会删除 TODO 标记。 `make check-env-config-surface` 重新生成环境切换清单，现在当相对于 `AGENTS_BASE_REF` 出现新的 **生产** 环境垫片时会失败；仅在迁移跟踪器中记录有意添加后才设置 `ENV_CONFIG_GUARD_ALLOW=1`。 `make check-serde-guard` 刷新 serde 库存，并在过时快照或新生产 `serde`/`serde_json` 命中时失败；仅在经过批准的迁移计划时设置 `SERDE_GUARD_ALLOW=1`。通过 TODO 面包屑和后续工单让大额延期可见，而不是默默延期。运行 `make check-std-only` 以捕获 `no_std`/`wasm32` cfgs 和 `make check-status-sync` 以确保 `roadmap.md` 未清项目保持仅开放状态，并且路线图/状态一起更改；仅针对固定 `AGENTS_BASE_REF` 后罕见的仅状态拼写错误修复设置 `STATUS_SYNC_ALLOW_UNPAIRED=1`。对于单次调用，请使用 `make agents-preflight` 一起运行所有护栏。
- 在推送之前运行本地序列化防护：`make guards`。
  - 这会拒绝生产代码中的直接 `serde_json`，不允许在允许列表之外添加新的直接 Serde 依赖项，并阻止 `crates/norito` 之外的临时 AoS/NCB 帮助程序。
- 可选择本地试运行 Norito 特征矩阵：`make norito-matrix`（使用快速子集）。
  - 要获得完整覆盖，请运行 `scripts/run_norito_feature_matrix.sh`，而不运行 `--fast`。
  - 每个组合包含下游烟雾（默认板条箱 `iroha_data_model`）：`make norito-matrix-downstream` 或 `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`。
- 对于 proc-macro 包，添加 `trybuild` UI 线束 (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) 并针对失败案例提交 `.stderr` 诊断。保持诊断稳定且不恐慌；使用 `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` 刷新灯具并使用 `cfg(all(feature = "trybuild-tests", not(coverage)))` 保护它们。
- 执行预提交例程，例如格式化和工件重新生成（请参阅 [`pre-commit.sample`](./hooks/pre-commit.sample)）
- 将 `upstream` 设置为跟踪 [Hyperledger Iroha 存储库](https://github.com/hyperledger-iroha/iroha)、`git pull -r upstream main`、`git commit -s`、`git push <your-fork>` 和 [创建拉取请求](https://github.com/hyperledger-iroha/iroha/compare) 到 `main` 分支。确保它遵循 [拉取请求指南](#pull-request-etiquette)。

### 代理工作流程快速入门

- 运行 `make dev-workflow`（`scripts/dev_workflow.sh` 的包装，记录在 `docs/source/dev_workflow.md` 中）。它包含 `cargo fmt --all`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo build/test --workspace --locked`（测试可能需要几个小时）和 `swift test`。
- 使用 `scripts/dev_workflow.sh --skip-tests` 或 `--skip-swift` 实现更快的迭代；在打开拉取请求之前重新运行完整序列。
- Guardrails：避免接触 `Cargo.lock`、添加新的工作区成员、引入新的依赖项、添加新的 `#[allow(missing_docs)]` 垫片、省略箱级文档、更改功能时跳过测试、在没有文档/测试的情况下删除 TODO 标记，或未经批准重新引入 `no_std`/`wasm32` cfgs。运行 `make check-agents-guardrails`（或 `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`）加上 `make check-dependency-discipline`、`make check-missing-docs`（刷新 `docs/source/agents/missing_docs_inventory.{json,md}`）、`make check-tests-guard`（当生产函数在没有单元测试证据的情况下发生更改时失败 - 差异中的测试更改或现有测试必须引用函数）、`make check-docs-tests-metrics`（当路线图更改缺少文档/测试/指标更新时失败）、`make check-todo-guard`、`make check-env-config-surface`（在过时的库存或新的生产环境切换上失败；仅在更新文档后使用 `ENV_CONFIG_GUARD_ALLOW=1` 覆盖）和 `make check-serde-guard`（失败在过时的 Serde 库存或新的生产 Serde 命中上；仅使用已批准的迁移计划在本地覆盖 `SERDE_GUARD_ALLOW=1` 以获取早期信号，`make check-std-only` 用于仅标准保护，并使 `roadmap.md`/`status.md` 与 `make check-status-sync` 保持同步（设置） `STATUS_SYNC_ALLOW_UNPAIRED=1` 仅适用于固定 `AGENTS_BASE_REF` 后罕见的仅状态拼写错误修复）。如果您希望在打开 PR 之前使用单个命令运行所有防护，请使用 `make agents-preflight`。

### 报告错误

*bug* 是 Iroha 中的错误、设计缺陷、故障或故障，导致其产生不正确、意外或非预期的结果或行为。

我们通过标有 `Bug` 标签的 [GitHub Issues](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) 跟踪 Iroha 错误。

当您创建新问题时，有一个模板供您填写。以下是报告错误时应执行的操作的清单：
- [ ] 添加 `Bug` 标签
- [ ] 解释一下问题
- [ ] 提供一个最小的工作示例
- [ ] 附上截图

<details> <summary>最小工作示例</summary>

对于每个错误，您应该提供一个[最小工作示例](https://en.wikipedia.org/wiki/Minimal_working_example)。例如：

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</详情>

---
**注意：** 文档过时、文档不足或功能请求等问题应使用 `Documentation` 或 `Enhancement` 标签。它们不是虫子。

---

### 报告漏洞

虽然我们积极预防安全问题，但您可能会先于我们遇到安全漏洞。

- 在第一个主要版本 (2.0) 之前，所有漏洞都被视为错误，因此请随意将它们作为错误提交[按照上述说明](#reporting-bugs)。
- 第一个主要版本发布后，使用我们的[漏洞赏金计划](https://hackerone.com/hyperledger) 提交漏洞并获得奖励。

：感叹：为了最大程度地减少未修补的安全漏洞造成的损害，您应该尽快直接向 Hyperledger 披露该漏洞，并在合理的时间内**避免公开披露相同的漏洞**。

如果您对我们处理安全漏洞有任何疑问，请随时通过私信联系 Rocket.Chat 目前活跃的维护者。

### 提出改进建议

使用适当的标签（`Optimization`、`Enhancement`）在 GitHub 上创建[问题](https://github.com/hyperledger-iroha/iroha/issues/new)，并描述您建议的改进。您可以将这个想法留给我们或其他人来开发，或者您可以自己实现。

如果您打算自己实施该建议，请执行以下操作：

1. 在开始解决问题之前，将您创建的问题分配给自己。
2. 使用您建议的功能并遵循我们的[代码和文档指南](#style-guides)。
3. 当您准备好打开拉取请求时，请确保遵循 [拉取请求指南](#pull-request-etiquette) 并将其标记为实现之前创建的问题：

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. 如果您的更改需要 API 更改，请使用 `api-changes` 标签。

   **注意：** 需要 API 更改的功能可能需要更长的时间来实施和批准，因为它们需要 Iroha 库制造商更新其代码。### 提问

问题是指既不是错误也不是功能或优化请求的任何讨论。

<详细信息> <摘要> 我如何提问？ </摘要>

请将您的问题发布到【我们的即时通讯平台之一】(#contact)，以便社区工作人员和成员及时为您提供帮助。

作为上述社区的一部分，您也应该考虑帮助其他人。如果您决定提供帮助，请以[尊重的方式](CODE_OF_CONDUCT.md) 提供帮助。

</详情>

## 您的第一个代码贡献

1. 在带有 [good-first-issue](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue) 标签的问题中找到适合初学者的问题。
2. 确保没有其他人正在处理您选择的问题，方法是检查该问题是否未分配给任何人。
3. 将问题分配给自己，以便其他人可以看到有人正在处理该问题。
4. 在开始编写代码之前，请阅读我们的 [Rust 风格指南](#rust-style-guide)。
5. 当您准备好提交更改时，请阅读 [拉取请求指南](#pull-request-etiquette)。

## 拉取请求礼仪

请 [fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) [存储库](https://github.com/hyperledger-iroha/iroha/tree/main) 并[创建功能分支](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) 做出贡献。使用**来自叉子的 PR** 时，请查看[本手册](https://help.github.com/articles/checking-out-pull-requests-locally)。

#### 致力于代码贡献：
- 遵循 [Rust 风格指南](#rust-style-guide) 和 [文档风格指南](#documentation-style-guide)。
- 确保您编写的代码已被测试覆盖。如果您修复了错误，请将重现该错误的最小工作示例转变为测试。
- 当触摸derive/proc-macro crates时，运行`make check-proc-macro-ui`（或
  使用 `PROC_MACRO_UI_CRATES="crate1 crate2"` 进行过滤），因此尝试构建 UI 装置
  保持同步，诊断保持稳定。
- 记录新的公共 API（新项目上的板条箱级 `//!` 和 `///`），并运行
  `make check-missing-docs` 验证护栏。调出您的文档/测试
  添加到您的拉取请求描述中。

#### 提交你的工作：
- 遵循 [Git 风格指南](#git-workflow)。
- [在合并之前](https://www.git-tower.com/learn/git/faq/git-squash/) 或[在合并期间](https://rietta.com/blog/github-merge-types/) 压缩您的提交。
- 如果在准备拉取请求期间您的分支已过时，请使用 `git pull --rebase upstream main` 在本地重新建立基础。或者，您可以使用 `Update branch` 按钮的下拉菜单并选择 `Update with rebase` 选项。

  为了让每个人都更轻松地完成此过程，请尽量不要对拉取请求进行多次提交，并避免重复使用功能分支。

#### 创建拉取请求：
- 按照 [拉取请求礼仪](#pull-request-etiquette) 部分中的指导，使用适当的拉取请求描述。如果可能，请避免偏离这些准则。
- 添加适当格式的[拉取请求标题](#pull-request-titles)。
- 如果您觉得您的代码尚未准备好合并，但您希望维护人员查看它，请创建草稿拉取请求。

#### 合并你的工作：
- 拉取请求在合并之前必须通过所有自动检查。至少，代码必须经过格式化、通过所有测试，并且没有突出的 `clippy` lint。
- 如果没有活跃维护者的两次批准评论，则无法合并拉取请求。
- 每个拉取请求都会自动通知代码所有者。当前维护者的最新列表可以在 [MAINTAINERS.md](MAINTAINERS.md) 中找到。

####复习礼仪：
- 不要自行解决对话。让审稿人做出决定。
- 确认审稿意见并与审稿人互动（同意、不同意、澄清、解释等）。不要忽视评论。
- 对于简单的代码更改建议，如果直接应用它们，就可以解决对话。
- 推送新更改时避免覆盖以前的提交。它混淆了自上次审核以来发生的变化，并迫使审核者从头开始。提交在自动合并之前会被压缩。

### 拉取请求标题

我们解析所有合并的拉取请求的标题以生成变更日志。我们还通过 *`check-PR-title`* 检查来检查标题是否遵循约定。

要通过 *`check-PR-title`* 检查，拉取请求标题必须遵循以下准则：

<details> <summary> 展开以阅读详细的标题指南</summary>

1.遵循[常规提交](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers)格式。

2. 如果拉取请求只有一个提交，则 PR 标题应与提交消息相同。

</详情>

### Git 工作流程

- [Fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) [存储库](https://github.com/hyperledger-iroha/iroha/tree/main) 和[创建功能分支](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) 供您贡献。
- [配置远程](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork) 将您的 fork 与 [Hyperledger Iroha 存储库](https://github.com/hyperledger-iroha/iroha/tree/main) 同步。
- 使用 [Git Rebase 工作流程](https://git-rebase.io/)。避免使用 `git pull`。请改用 `git pull --rebase`。
- 使用提供的 [git hooks](./hooks/) 来简化开发过程。

请遵循以下提交指南：

- **签署每个提交**。如果您不这样做，[DCO](https://github.com/apps/dco) 将不会让您合并。

  使用 `git commit -s` 自动添加 `Signed-off-by: $NAME <$EMAIL>` 作为提交消息的最后一行。您的姓名和电子邮件应与您的 GitHub 帐户中指定的相同。

  我们还鼓励您使用 `git commit -sS` 使用 GPG 密钥签署您的提交（[了解更多](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)）。

  您可以使用 [`commit-msg` 挂钩](./hooks/) 自动签署您的提交。

- 提交消息必须遵循 [常规提交](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) 以及与 [拉取请求标题](#pull-request-titles) 相同的命名架构。这意味着：
  - **使用现在时**（“添加功能”，而不是“添加功能”）
  - **使用祈使语气**（“部署到 docker...”而不是“部署到 docker...”）
- 编写有意义的提交消息。
- 尝试保持提交消息简短。
- 如果您需要更长的提交消息：
  - 将提交消息的第一行限制为 50 个字符或更少。
  - 提交消息的第一行应包含您已完成工作的摘要。如果您需要多行，请在每个段落之间留一个空行，并在中间描述您的更改。最后一行必须是结束行。
- 如果您修改架构（通过使用 `kagami schema` 和 diff 生成架构进行检查），您应该在单独的提交中对架构进行所有更改，并显示消息 `[schema]`。
- 尝试坚持对每个有意义的更改进行一次提交。
  - 如果您在一个 PR 中修复了多个问题，请分别提交它们。
  - 如前所述，对 `schema` 和 API 的更改应在与其余工作分开的适当提交中完成。
  - 在与该功能相同的提交中添加功能测试。

## 测试和基准

- 要运行基于源代码的测试，请在 Iroha 根目录中执行 [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html)。请注意，这是一个漫长的过程。
- 要运行基准测试，请从 Iroha 根目录执行 [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html)。为了帮助调试基准测试输出，请设置 `debug_assertions` 环境变量，如下所示：`RUSTFLAGS="--cfg debug_assertions" cargo bench`。
- 如果您正在处理特定组件，请注意，当您在[工作空间](https://doc.rust-lang.org/cargo/reference/workspaces.html)中运行`cargo test`时，它只会运行该工作空间的测试，通常不包括任何[集成测试](https://www.testingxperts.com/blog/what-is-integration-testing)。
- 如果您想在最小网络上测试您的更改，提供的 [`docker-compose.yml`](defaults/docker-compose.yml) 在 Docker 容器中创建一个由 4 个 Iroha 对等点组成的网络，可用于测试共识和资产传播相关逻辑。我们建议使用 [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python) 或随附的 Iroha 客户端 CLI 与该网络进行交互。
- 不要删除失败的测试。即使被忽略的测试最终也会在我们的管道中运行。
- 如果可能，请在进行更改之前和之后对您的代码进行基准测试，因为显着的性能下降可能会破坏现有用户的安装。

### 序列化防护检查

运行 `make guards` 以在本地验证存储库策略：

- 将生产源中的直接 `serde_json` 列入拒绝名单（首选 `norito::json`）。
- 禁止在允许列表之外直接进行 `serde`/`serde_json` 依赖项/导入。
- 防止在 `crates/norito` 之外重新引入临时 AoS/NCB 帮助程序。

### 调试测试

<details> <summary> 展开以了解如何更改日志级别或将日志写入 JSON。</summary>

如果您的其中一项测试失败，您可能需要降低最大日志记录级别。默认情况下，Iroha 仅记录 `INFO` 级别消息，但保留生成 `DEBUG` 和 `TRACE` 级别日志的能力。可以使用 `LOG_LEVEL` 环境变量进行基于代码的测试，或使用已部署网络中对等点之一上的 `/configuration` 端点来更改此设置。虽然以 `stdout` 打印的日志就足够了，但您可能会发现将 `json` 格式的日志生成到单独的文件中并使用 [node-bunyan](https://www.npmjs.com/package/bunyan) 或 [rust-bunyan](https://crates.io/crates/bunyan) 解析它们更方便。

将 `LOG_FILE_PATH` 环境变量设置为适当的位置来存储日志并使用上述包解析它们。

</详情>

### 使用 tokio 控制台进行调试

<details> <summary> 展开以了解如何使用 tokio 控制台支持编译 Iroha。</summary>

有时，使用 [tokio-console](https://github.com/tokio-rs/console) 分析 tokio 任务可能有助于调试。

在这种情况下，您应该在 tokio 控制台的支持下编译 Iroha，如下所示：

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

tokio 控制台的端口可以通过 `LOG_TOKIO_CONSOLE_ADDR` 配置参数（或环境变量）进行配置。
使用tokio控制台需要日志级别为`TRACE`，可以通过配置参数或环境变量`LOG_LEVEL`启用。

使用 `scripts/test_env.sh` 在 tokio 控制台支持下运行 Iroha 的示例：

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</详情>

### 分析

<details> <summary> 展开以了解如何分析 Iroha。 </摘要>

为了优化性能，分析 Iroha 很有用。

分析构建当前需要夜间工具链。要准备一个，请使用 `profiling` 配置文件和功能来编译 Iroha：

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

然后启动 Iroha 并将您选择的分析器附加到 Iroha pid。

或者，可以在 docker 内部构建 Iroha，并使用探查器支持并以这种方式分析 Iroha。

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

例如使用 perf（仅在 Linux 上可用）：

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

为了能够在 Iroha 分析期间观察执行器的配置文件，应在不剥离符号的情况下编译执行器。
可以通过运行以下命令来完成：

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

启用分析功能后，Iroha 会公开端点以废弃 pprof 配置文件：

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</详情>

## 风格指南

当您向我们的项目贡献代码时，请遵循以下准则：

### Git 风格指南

:book: [阅读 git 指南](#git-workflow)

### Rust 风格指南

<details> <summary> :book: 阅读代码指南</summary>

- 使用 `cargo fmt --all`（2024 版）格式化代码。

代码指南：

- 除非另有说明，请参阅[Rust 最佳实践](https://github.com/mre/idiomatic-rust)。
- 使用 `mod.rs` 样式。 [自命名模块](https://rust-lang.github.io/rust-clippy/master/) 将不会通过静态分析，但 [`trybuild`](https://crates.io/crates/trybuild) 测试除外。
- 使用领域优先的模块结构。

  示例：不要执行 `constants::logger`。相反，反转层次结构，将使用它的对象放在前面：`iroha_logger::constants`。
- 使用带有明确错误消息或无误证明的 [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/)，而不是 `unwrap`。
- 永远不要忽略错误。如果不能`panic`并且无法恢复，至少需要记录在日志中。
- 更愿意返回 `Result` 而不是 `panic!`。
- 在空间上对相关功能进行分组，最好在适当的模块内。

  例如，最好在其旁边放置与 `struct` 相关的 `impl`，而不是使用具有 `struct` 定义的块，然后为每个单独的结构提供 `impl`。
- 实现前声明：`use` 语句和常量位于顶部，单元测试位于底部。
- 如果导入的名称仅使用一次，请尽量避免 `use` 语句。这使得将代码移动到不同的文件中变得更容易。
- 不要随意静音 `clippy` lints。如果您这样做，请通过注释（或 `expect` 消息）解释您的推理。
- 如果 `#[outer_attribute]` 可用，则优先选择 `#![inner_attribute]`。
- 如果您的函数不会改变其任何输入（并且不应改变其他任何内容），请将其标记为 `#[must_use]`。
- 如果可能的话，避免使用 `Box<dyn Error>`（我们更喜欢强类型）。
- 如果您的函数是 getter/setter，请将其标记为 `#[inline]`。
- 如果您的函数是构造函数（即，它从输入参数创建一个新值并调用 `default()`），请将其标记为 `#[inline]`。
- 避免将代码与具体的数据结构联系起来； `rustc` 足够智能，可以在需要时将 `Vec<InstructionExpr>` 转换为 `impl IntoIterator<Item = InstructionExpr>`，反之亦然。

命名准则：
- 在 *public* 结构、变量、方法、特征、常量和模块名称中仅使用完整单词。但是，在以下情况下允许使用缩写：
  - 名称是本地的（例如闭包参数）。
  - 该名称按照 Rust 约定缩写（例如 `len`、`typ`）。
  - 名称是可接受的缩写（例如 `tx`、`wsv` 等）；有关规范缩写，请参阅[项目词汇表](https://docs.iroha.tech/reference/glossary.html)。
  - 全名将被局部变量隐藏（例如 `msg <- message`）。
  - 如果全名超过 5-6 个单词，则代码会变得很麻烦（例如 `WorldStateViewReceiverTrait -> WSVRecvTrait`）。
- 如果您更改命名约定，请确保您选择的新名称比我们之前的名称_更_清晰。

评论指南：
- 在编写非文档注释时，不要描述你的函数做了什么，而是尝试解释它为什么以特定的方式做某事。这将节省您和审稿人的时间。
- 只要您引用为其创建的问题，您就可以在代码中留下 `TODO` 标记。不创建问题意味着它不会被合并。

我们使用固定依赖项。请遵循以下版本控制指南：

- 如果您的工作依赖于特定的板条箱，请查看是否尚未使用 [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) 安装（使用 `bat` 或 `grep`），并尝试使用该版本，而不是最新版本。
- 在 `Cargo.toml` 中使用完整版本“X.Y.Z”。
- 在单独的 PR 中提供版本升级。

</详情>

### 文档风格指南

<details> <summary> :book：阅读文档指南</summary>


- 使用 [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html) 格式。
- 更喜欢单行注释语法。对内联模块使用 `///`，对基于文件的模块使用 `//!`。
- 如果您可以链接到结构/模块/函数的文档，请链接到该文档。
- 如果您可以提供使用示例，请提供。这[也是一个测试](https://doc.rust-lang.org/rustdoc/documentation-tests.html)。
- 如果函数可能出错或出现紧急情况，请避免使用情态动词。示例：`Fails if disk IO fails` 而不是 `Can possibly fail, if disk IO happens to fail`。
- 如果某个函数可能因多种原因而出错或出现紧急情况，请使用故障条件的项目符号列表以及相应的 `Error` 变体（如果有）。
- 函数*做*事情。使用祈使语气。
- 结构*是*事物。进入正题吧。例如，`Log level for reloading from the environment` 优于 `This struct encapsulates the idea of logging levels, and is used for reloading from the environment`。
- 结构有字段，字段也是事物。
- 模块*包含*东西，我们知道这一点。进入正题吧。示例：使用 `Logger-related traits.` 代替 `Module which contains logger-related logic`。


</详情>

## 联系方式

我们的社区成员活跃于：

|服务 |链接 |
|--------------|------------------------------------------------------------------------|
|堆栈溢出 | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
|邮件列表 | https://lists.lfdecentralizedtrust.org/g/iroha |
|电报 | https://t.me/hyperledgeriroha |
|不和谐| https://discord.com/channels/905194001349627914/905205848547155968 |

---
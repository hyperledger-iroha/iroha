---
lang: zh-hans
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 代理开发工作流程

该操作手册整合了 AGENTS 路线图中的贡献者护栏，以便
新补丁遵循相同的默认门。

## 快速入门目标

- 运行 `make dev-workflow`（`scripts/dev_workflow.sh` 的包装）来执行：
  1.`cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3.`cargo build --workspace --locked`
  4.`cargo test --workspace --locked`
  5. `swift test` 来自 `IrohaSwift/`
- `cargo test --workspace` 长时间运行（通常是几个小时）。为了快速迭代，
  使用 `scripts/dev_workflow.sh --skip-tests` 或 `--skip-swift`，然后运行完整的
  发货前的顺序。
- 如果 `cargo test --workspace` 在构建目录锁上停止，请重新运行
  `scripts/dev_workflow.sh --target-dir target/codex-tests`（或设置
  `CARGO_TARGET_DIR` 到隔离路径）以避免争用。
- 所有货物步骤都使用 `--locked` 来尊重存储库策略
  `Cargo.lock` 未受影响。更喜欢扩展现有的板条箱而不是添加
  新的工作区成员；在引入新的板条箱之前寻求批准。

## 护栏- `make check-agents-guardrails`（或 `ci/check_agents_guardrails.sh`）失败，如果
  分支修改 `Cargo.lock`，引入新的工作区成员，或添加新的
  依赖关系。该脚本将工作树和 `HEAD` 进行比较
  默认为`origin/main`；设置 `AGENTS_BASE_REF=<ref>` 以覆盖基数。
- `make check-dependency-discipline`（或 `ci/check_dependency_discipline.sh`）
  比较 `Cargo.toml` 与基础的依赖关系，并在新的箱子上失败；设置
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` 承认故意
  补充。
- `make check-missing-docs`（或`ci/check_missing_docs_guard.sh`）阻止新的
  `#[allow(missing_docs)]` 条目，标记触及板条箱（最近的 `Cargo.toml`）
  其 `src/lib.rs`/`src/main.rs` 缺少 crate 级 `//!` 文档，并拒绝新的
  相对于基本参考没有 `///` 文档的公共项目；设置
  `MISSING_DOCS_GUARD_ALLOW=1` 仅经审阅者批准。守卫也
  验证 `docs/source/agents/missing_docs_inventory.{json,md}` 是否新鲜；
  使用 `python3 scripts/inventory_missing_docs.py` 重新生成。
- `make check-tests-guard`（或 `ci/check_tests_guard.sh`）标记其
  更改后的 Rust 函数缺乏单元测试证据。守卫地图改变了路线
  对于函数，如果差异中的板条箱测试发生变化，则通过，否则扫描
  用于匹配函数调用的现有测试文件，因此预先存在的覆盖范围
  计数；没有任何匹配测试的板条箱将会失败。设置 `TEST_GUARD_ALLOW=1`
  仅当更改真正与测试无关且审阅者同意时。
- `make check-docs-tests-metrics`（或 `ci/check_docs_tests_metrics_guard.sh`）
  强制执行里程碑与文档一起移动的路线图政策，
  测试和指标/仪表板。当 `roadmap.md` 相对于
  `AGENTS_BASE_REF`，警卫预计至少有一项文档更改、一项测试更改，
  以及一项指标/遥测/仪表板更改。设置 `DOC_TEST_METRIC_GUARD_ALLOW=1`
  仅经审稿人批准。
- 当 TODO 标记时，`make check-todo-guard`（或 `ci/check_todo_guard.sh`）失败
  在没有伴随文档/测试更改的情况下消失。添加或更新覆盖范围
  解决 TODO 时，或设置 `TODO_GUARD_ALLOW=1` 进行有意删除。
- `make check-std-only`（或 `ci/check_std_only.sh`）块 `no_std`/`wasm32`
  cfgs 以便工作区保持仅 `std`。仅设置 `STD_ONLY_GUARD_ALLOW=1`
  批准的 CI 实验。
- `make check-status-sync`（或 `ci/check_status_sync.sh`）保持路线图开放
  部分不含已完成的项目，并需要 `roadmap.md`/`status.md`
  一起改变，使计划/状态保持一致；设置
  `STATUS_SYNC_ALLOW_UNPAIRED=1` 仅适用于罕见的仅状态拼写错误修复
  固定 `AGENTS_BASE_REF`。
- `make check-proc-macro-ui`（或 `ci/check_proc_macro_ui.sh`）运行 trybuild
  用于导出/过程宏 crate 的 UI 套件。当触摸 proc-macros 时运行它
  保持 `.stderr` 诊断稳定并捕获令人恐慌的 UI 回归；设置
  `PROC_MACRO_UI_CRATES="crate1 crate2"` 专注于特定的板条箱。
- `make check-env-config-surface`（或`ci/check_env_config_surface.sh`）重建
  环境切换库存 (`docs/source/agents/env_var_inventory.{json,md}`)，
  如果它是陈旧的，则失败，**并且**当新的生产环境垫片出现时失败
  相对于 `AGENTS_BASE_REF`（自动检测；需要时明确设置）。
  添加/删除环境查找后刷新跟踪器
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`；
  仅在记录有意的环境旋钮后才使用 `ENV_CONFIG_GUARD_ALLOW=1`在迁移跟踪器中。
- `make check-serde-guard`（或`ci/check_serde_guard.sh`）重新生成serde
  使用库存 (`docs/source/norito_json_inventory.{json,md}`) 到临时
  位置，如果提交的库存过时则失败，并拒绝任何新的库存
  生产 `serde`/`serde_json` 相对于 `AGENTS_BASE_REF` 命中。套装
  `SERDE_GUARD_ALLOW=1` 仅适用于提交迁移计划后的 CI 实验。
- `make guards` 强制执行 Norito 序列化策略：它拒绝新的
  `serde`/`serde_json` 用法、临时 AoS 帮助程序和外部 SCALE 依赖项
  Norito 长凳 (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`、`scripts/deny_handrolled_aos.sh`、
  `scripts/check_no_scale.sh`）。
- **Proc-macro UI 政策：** 每个 proc-macro 箱都必须运送 `trybuild`
  `trybuild-tests` 后面的线束（带有通过/失败球的 `tests/ui.rs`）
  功能。将快乐路径样本放在 `tests/ui/pass` 下，将拒绝案例放在
  `tests/ui/fail` 具有承诺的 `.stderr` 输出，并保持诊断
  无恐慌且稳定。刷新灯具
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests`（可选
  `CARGO_TARGET_DIR=target-codex` 以避免破坏现有版本）和
  避免依赖覆盖范围构建（预计有 `cfg(not(coverage))` 防护）。
  对于不发出二进制入口点的宏，更喜欢
  夹具中的 `// compile-flags: --crate-type lib` 以保持错误集中。添加
  每当诊断发生变化时就会出现新的阴性病例。
- CI 通过 `.github/workflows/agents-guardrails.yml` 运行护栏脚本
  因此，当违反策略时，拉取请求会快速失败。
- 示例 git hook (`hooks/pre-commit.sample`) 运行护栏、依赖项、
  缺少文档、仅 std、环境配置和状态同步脚本，因此贡献者
  在 CI 之前发现政策违规行为。保留 TODO 面包屑以备不时之需
  后续行动，而不是默默推迟重大变更。
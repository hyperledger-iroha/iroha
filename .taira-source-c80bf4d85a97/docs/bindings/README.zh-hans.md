---
lang: zh-hans
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK 绑定和 Fixture 治理

路线图上的 WP1-E 称“文档/绑定”为保存
跨语言绑定状态。该文件记录了绑定库存，
再生命令、漂移防护和证据位置，以便 GPU 奇偶校验
门 (WP1-E/F/G) 和跨 SDK 节奏委员会有一个参考。

## 共用护栏
- **规范剧本：** `docs/source/norito_binding_regen_playbook.md` 阐明
  Android 的轮换政策、预期证据和升级工作流程，
  Swift、Python 和未来的绑定。
- **Norito 模式奇偶校验：** `scripts/check_norito_bindings_sync.py` （通过调用
  `scripts/check_norito_bindings_sync.sh` 并在 CI 中通过
  `ci/check_norito_bindings_sync.sh`) 当 Rust、Java 或 Python 时阻止构建
  模式工件漂移。
- **Cadence 看门狗：** `scripts/check_fixture_cadence.py` 读取
  `artifacts/*_fixture_regen_state.json` 文件并强制执行周二/周五（Android、
  Python）和星期三（Swift）窗口，因此路线图门具有可审核的时间戳。

## 结合矩阵

|绑定|切入点|夹具/再生命令|漂移护卫|证据|
|--------|--------------|------------------------------------|----------------|----------|
|安卓（Java）| `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`、`ci/check_android_fixtures.sh`、`java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
|斯威夫特 (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh`（可选 `SWIFT_FIXTURE_ARCHIVE`）→ `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`、`ci/check_swift_fixtures.sh`、`scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`，`docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
|蟒蛇 | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`，`python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`，`docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`、`scripts/js_sbom_provenance.sh`、`scripts/js_signed_staging.sh` | `npm run test`、`javascript/iroha_js/scripts/verify-release-tarball.mjs`、`javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`、`artifacts/js/npm_staging/`、`artifacts/js/verification/`、`artifacts/js/sbom/` |

## 绑定细节

### 安卓（Java）
Android SDK 位于 `java/iroha_android/` 下并使用规范的 Norito
由 `scripts/android_fixture_regen.sh` 生产的灯具。那个助手导出
Rust 工具链中的新鲜 `.norito` blob，更新
`artifacts/android_fixture_regen_state.json`，并记录节奏元数据
`scripts/check_fixture_cadence.py` 和治理仪表板消耗。漂移是
由 `scripts/check_android_fixtures.py` 检测到（也连接到
`ci/check_android_fixtures.sh`) 和 `java/iroha_android/run_tests.sh`，其中
练习 JNI 绑定、WorkManager 队列重放和 StrongBox 回退。
轮换证据、失败注释和重新运行记录位于
`artifacts/android/fixture_runs/`。

### Swift (macOS/iOS)
`IrohaSwift/` 通过 `scripts/swift_fixture_regen.sh` 镜像相同的 Norito 有效负载。
该脚本记录旋转所有者、节奏标签和源（`live` 与 `archive`）
在 `artifacts/swift_fixture_regen_state.json` 内并将元数据馈送到
节奏检查器。 `scripts/swift_fixture_archive.py` 允许维护人员摄取
Rust 生成的档案； `scripts/check_swift_fixtures.py` 和
`ci/check_swift_fixtures.sh` 强制执行字节级奇偶校验加上 SLA 期限限制，同时
`scripts/swift_fixture_regen.sh` 支持 `SWIFT_FIXTURE_EVENT_TRIGGER` 手动
轮换。升级工作流程、KPI 和仪表板记录在
`docs/source/swift_parity_triage.md` 和下面的节奏内裤
`docs/source/sdk/swift/`。

###Python
Python 客户端 (`python/iroha_python/`) 共享 Android 设备。跑步
`scripts/python_fixture_regen.sh`拉取最新的`.norito`有效负载，刷新
`python/iroha_python/tests/fixtures/`，并将节奏元数据发送到
`artifacts/python_fixture_regen_state.json` 第一次路线图后轮换
被捕获。 `scripts/check_python_fixtures.py` 和
`python/iroha_python/scripts/run_checks.sh` 门 pytest、mypy、ruff 和夹具
本地和 CI 中的平等。端到端文档 (`docs/source/sdk/python/…`) 和
绑定再生手册描述了如何与 Android 协调旋转
业主。

### JavaScript
`javascript/iroha_js/` 不依赖本地 `.norito` 文件，但 WP1-E 跟踪
其发布证据使 GPU CI 通道继承完整的来源。每次发布
通过 `npm run release:provenance` 捕获出处（由
`javascript/iroha_js/scripts/record-release-provenance.mjs`)，生成并签名
SBOM 与 `scripts/js_sbom_provenance.sh` 捆绑在一起，运行签名的暂存试运行
(`scripts/js_signed_staging.sh`)，并验证注册表工件
`javascript/iroha_js/scripts/verify-release-tarball.mjs`。生成的元数据
落在 `artifacts/js-sdk-provenance/`、`artifacts/js/npm_staging/` 下，
`artifacts/js/sbom/` 和 `artifacts/js/verification/`，提供确定性
路线图 JS5/JS6 和 WP1-F 基准测试运行的证据。出版剧本
`docs/source/sdk/js/` 将自动化联系在一起。
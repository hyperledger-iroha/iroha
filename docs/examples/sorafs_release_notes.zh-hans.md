---
lang: zh-hans
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI 和 SDK — 发行说明 (v0.1.0)

## 亮点
- `sorafs_cli` 现在包含整个打包管道（`car pack`、`manifest build`、
  `proof verify`、`manifest sign`、`manifest verify-signature`），因此 CI 运行程序调用
  单个二进制文件而不是定制的助手。新的无密钥签名流程默认为
  `SIGSTORE_ID_TOKEN`，了解 GitHub Actions OIDC 提供程序，并发出确定性信号
  摘要 JSON 与签名包一起。
- 多源获取 *记分板* 作为 `sorafs_car` 的一部分提供：它标准化
  提供商遥测、强制执行能力处罚、保留 JSON/Norito 报告，以及
  通过共享注册表句柄提供编排器模拟器 (`sorafs_fetch`)。
  `fixtures/sorafs_manifest/ci_sample/` 下的夹具展示了确定性
  CI/CD 预计会进行比较的输入和输出。
- 发布自动化被编入 `ci/check_sorafs_cli_release.sh` 和
  `scripts/release_sorafs_cli.sh`。现在每个版本都会存档清单包，
  签名，`manifest.sign/verify`摘要，以及记分板快照等治理
  审阅者可以追踪人工制品，而无需重新运行管道。

## 升级步骤
1. 更新工作区中对齐的 crate：
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. 在本地（或在 CI 中）重新运行发布门以确认 fmt/clippy/test 覆盖率：
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. 使用策划的配置重新生成签名的工件和摘要：
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   如果出现以下情况，则将刷新的捆绑包/校样复制到 `fixtures/sorafs_manifest/ci_sample/` 中：
   发布更新规范赛程。

## 验证
- 发布门提交：`c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  （门成功后立即`git rev-parse HEAD`）。
- `ci/check_sorafs_cli_release.sh` 输出：存档于
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log`（附加到发行包中）。
- 清单捆绑摘要：`SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  （`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`）。
- 证明摘要摘要：`SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  （`fixtures/sorafs_manifest/ci_sample/proof.json`）。
- 清单摘要（用于下游证明交叉检查）：
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  （来自 `manifest.sign.summary.json`）。

## 操作人员注意事项
- Torii 网关现在强制执行 `X-Sora-Chunk-Range` 功能标头。更新
  允许列表，以便允许提供新流令牌范围的客户端；旧代币
  如果没有范围声明将会受到限制。
- `scripts/sorafs_gateway_self_cert.sh` 集成清单验证。跑步时
  自认证工具，提供新生成的清单包，以便包装器可以
  在签名漂移上快速失败。
- 遥测仪表板应将新的记分板导出 (`scoreboard.json`) 摄取到
  协调提供者的资格、权重分配和拒绝原因。
- 每次推出时存档四个规范摘要：
  `manifest.bundle.json`、`manifest.sig`、`manifest.sign.summary.json`、
  `manifest.verify.summary.json`。治理票证在期间引用了这些确切的文件
  批准。

## 致谢
- 存储团队 — 端到端 CLI 整合、块计划渲染器和记分板
  遥测管道。
- 工具工作组 — 发布管道（`ci/check_sorafs_cli_release.sh`，
  `scripts/release_sorafs_cli.sh`) 和确定性夹具捆绑包。
- 网关操作 — 能力门控、流令牌策略审查和更新
  自我认证手册。
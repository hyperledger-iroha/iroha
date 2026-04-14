<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi 正式模型 (TLA+ / Apalache)

该目录包含 Sumeragi 提交路径安全性和活性的有界正式模型。

## 范围

该模型捕获：
- 阶段进展（`Propose`、`Prepare`、`CommitVote`、`NewView`、`Committed`），
- 投票和法定人数阈值（`CommitQuorum`、`ViewQuorum`），
- NPoS 式提交保护的加权权益法定人数 (`StakeQuorum`)，
- RBC 因果关系 (`Init -> Chunk -> Ready -> Deliver`) 以及标题/摘要证据，
- 商品及服务税和对诚实进步行动的弱公平假设。

它有意抽象出线路格式、签名和完整的网络细节。

## 文件

- `Sumeragi.tla`：协议模型和属性。
- `Sumeragi_fast.cfg`：较小的 CI 友好参数集。
- `Sumeragi_deep.cfg`：更大的应力参数集。

## 属性

不变量：
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

时间属性：
- `EventuallyCommit` (`[] (gst => <> committed)`)，采用 GST 后公平性编码
  在 `Next` 中运行（启用超时/故障抢占防护）
  进展行动）。这使得模型可以通过 Apalache 0.52.x 进行检查，
  不支持检查时间属性内的 `WF_` 公平运算符。

## 运行

从存储库根目录：

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### 可重现的本地设置（不需要 Docker）安装此存储库使用的固定本地 Apalache 工具链：

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

运行程序会自动检测此安装：
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`。
安装后，`ci/check_sumeragi_formal.sh` 应该可以在没有额外环境变量的情况下工作：

```bash
bash ci/check_sumeragi_formal.sh
```

如果 Apalache 不在 `PATH` 中，您可以：

- 将 `APALACHE_BIN` 设置为可执行路径，或者
- 使用 Docker 后备（当 `docker` 可用时默认启用）：
  - 图像：`APALACHE_DOCKER_IMAGE`（默认 `ghcr.io/apalache-mc/apalache:latest`）
  - 需要运行 Docker 守护进程
  - 使用 `APALACHE_ALLOW_DOCKER=0` 禁用回退。

示例：

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## 注释

- 该模型补充（而不是取代）可执行 Rust 模型测试
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  和
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`。
- 检查受 `.cfg` 文件中的常量值限制。
- PR CI 通过 `.github/workflows/pr.yml` 运行这些检查
  `ci/check_sumeragi_formal.sh`。
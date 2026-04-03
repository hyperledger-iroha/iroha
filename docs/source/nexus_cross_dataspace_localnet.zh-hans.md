<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus 跨数据空间本地网络证明

此 Runbook 执行 Nexus 集成证明：

- 启动具有两个受限私有数据空间的 4 对等本地网络（`ds1`、`ds2`），
- 将帐户流量路由到每个数据空间，
- 在每个数据空间中创建资产，
- 跨数据空间双向执行原子交换结算，
- 通过提交资金不足的分支并检查余额保持不变来证明回滚语义。

规范测试是：
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`。

## 快速运行

使用存储库根目录中的包装器脚本：

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

默认行为：

- 仅运行跨数据空间证明测试，
- 集 `NORITO_SKIP_BINDINGS_SYNC=1`，
- 集 `IROHA_TEST_SKIP_BUILD=1`，
- 使用 `--test-threads=1`，
- 通过 `--nocapture`。

## 有用的选项

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` 保留临时对等目录 (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) 用于取证。
- `--all-nexus` 运行 `mod nexus::`（完整的 Nexus 集成子集），而不仅仅是验证测试。

## CI 门

CI 助手：

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

制定目标：

```bash
make check-nexus-cross-dataspace
```

该门执行确定性证明包装器，并且如果跨数据空间原子
交换场景回归。

## 手动等效命令

有针对性的验证测试：

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

完整 Nexus 子集：

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## 预期的证据信号- 测试通过。
- 对于故意失败的资金不足的和解分支，会出现一个预期警告：
  `settlement leg requires 10000 but only ... is available`。
- 最终余额断言在以下情况后成功：
  - 成功的远期互换，
  - 成功的反向交换，
  - 资金不足的掉期失败（回滚未改变的余额）。

## 当前验证快照

截至 **2026 年 2 月 19 日**，此工作流程已通过：

- 针对性测试：`1 passed; 0 failed`，
- 完整 Nexus 子集：`24 passed; 0 failed`。
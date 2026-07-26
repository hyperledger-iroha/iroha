---
lang: zh-hans
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 配置文件

Kagami 提供 Iroha 3 网络的预设，以便操作员可以标记确定性
创世显现无需调整每个网络的旋钮。

- 配置文件：`iroha3-dev`（链 `iroha3-dev.local`，收集器 k=1 r=1，选择 NPoS 时从链 id 派生的 VRF 种子），`iroha3-taira`（链 `iroha3-taira`，收集器 k=3 r=3，当选择 NPoS 时需要 `--vrf-seed-hex`已选择），`iroha3-nexus`（链 `iroha3-nexus`，收集器 k=5 r=3，选择 NPoS 时需要 `--vrf-seed-hex`）。
- 共识：Sora 配置文件网络（Nexus + 数据空间）需要 NPoS 并不允许分阶段切换；获得许可的 Iroha3 部署必须在没有 Sora 配置文件的情况下运行。
- 代：`cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`。将 `--consensus-mode npos` 用于 Nexus； `--vrf-seed-hex` 仅对 NPoS 有效（taira/nexus 需要）。 Kagami 将 DA/RBC 引脚固定在 Iroha3 线上并发出摘要（链、收集器、DA/RBC、VRF 种子、指纹）。
- 验证：`cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` 重播配置文件期望（链 ID、DA/RBC、收集器、PoP 覆盖范围、共识指纹）。仅在验证 taira/nexus 的 NPoS 清单时提供 `--vrf-seed-hex`。
- 示例捆绑包：预生成的捆绑包位于 `defaults/kagami/iroha3-{dev,taira,nexus}/` 下（genesis.json、config.toml、docker-compose.yml、verify.txt、README）。使用 `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` 重新生成。
- Mochi：`mochi`/`mochi-genesis` 接受 `--genesis-profile <profile>` 和 `--vrf-seed-hex <hex>`（仅限 NPoS），将它们转发到 Kagami，并在使用配置文件时将相同的 Kagami 摘要打印到 stdout/stderr。

这些捆绑包将 BLS PoP 与拓扑条目一起嵌入，因此 `kagami verify` 成功
开箱即用；根据本地需要调整配置中的受信任对等点/端口
烟雾缭绕。
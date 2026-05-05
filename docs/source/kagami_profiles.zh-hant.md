---
lang: zh-hant
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

Kagami 提供 Iroha 3 網絡的預設，以便操作員可以標記確定性
創世顯現無需調整每個網絡的旋鈕。

- 配置文件：`iroha3-dev`（鏈 `iroha3-dev.local`，收集器 k=1 r=1，選擇 NPoS 時從鏈 id 派生的 VRF 種子），`iroha3-taira`（鏈 `iroha3-taira`，收集器 k=3 r=3，當選擇 NPoS 時需要 `--vrf-seed-hex`已選擇），`iroha3-nexus`（鏈 `iroha3-nexus`，收集器 k=5 r=3，選擇 NPoS 時需要 `--vrf-seed-hex`）。
- 共識：Sora 配置文件網絡（Nexus + 數據空間）需要 NPoS 並不允許分階段切換；獲得許可的 Iroha3 部署必須在沒有 Sora 配置文件的情況下運行。
- 代：`cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`。將 `--consensus-mode npos` 用於 Nexus； `--vrf-seed-hex` 僅對 NPoS 有效（taira/nexus 需要）。 Kagami 將 DA/RBC 引腳固定在 Iroha3 線上並發出摘要（鏈、收集器、DA/RBC、VRF 種子、指紋）。
- 驗證：`cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` 重播配置文件期望（鏈 ID、DA/RBC、收集器、PoP 覆蓋範圍、共識指紋）。僅在驗證 taira/nexus 的 NPoS 清單時提供 `--vrf-seed-hex`。
- 示例捆綁包：預生成的捆綁包位於 `defaults/kagami/iroha3-{dev,taira,nexus}/` 下（genesis.json、config.toml、docker-compose.yml、verify.txt、README）。使用 `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` 重新生成。
- Mochi：`mochi`/`mochi-genesis` 接受 `--genesis-profile <profile>` 和 `--vrf-seed-hex <hex>`（僅限 NPoS），將它們轉發到 Kagami，並在使用配置文件時將相同的 Kagami 摘要打印到 stdout/stderr。

這些捆綁包將 BLS PoP 與拓撲條目一起嵌入，因此 `kagami verify` 成功
開箱即用；根據本地需要調整配置中的受信任對等點/端口
煙霧繚繞。
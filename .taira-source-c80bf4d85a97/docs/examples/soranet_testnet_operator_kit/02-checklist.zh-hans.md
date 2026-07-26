---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] 查看硬件规格：8 个以上内核、16 GiB RAM、NVMe 存储 ≥ 500 MiB/s。
- [ ] 确认两个 IPv4 + IPv6 地址并且上行允许 QUIC/UDP 443。
- [ ] 为中继身份密钥提供 HSM 或专用安全飞地。
- [ ] 同步规范退出目录 (`governance/compliance/soranet_opt_outs.json`)。
- [ ] 将合规性块合并到 Orchestrator 配置中（请参阅 `03-config-example.toml`）。
- [ ] 获取管辖权/全球合规性证明并填充 `attestations` 列表。
- [ ] 运行 `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json`（或您的实时快照）并查看通过/失败报告。
- [ ] 生成中继准入 CSR 并获取治理签名的信封。
- [ ] 导入保护描述符种子并针对已发布的哈希链进行验证。
- [ ] 运行 `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`。
- [ ] 试运行遥测导出：确保 Prometheus 抓取在本地成功。
- [ ] 安排停电演习窗口并记录升级联系人。
- [ ] 签署演习证据包：`cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`。
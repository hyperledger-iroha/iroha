---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] 查看硬件規格：8 個以上內核、16 GiB RAM、NVMe 存儲 ≥ 500 MiB/s。
- [ ] 確認兩個 IPv4 + IPv6 地址並且上行允許 QUIC/UDP 443。
- [ ] 為中繼身份密鑰提供 HSM 或專用安全飛地。
- [ ] 同步規範退出目錄 (`governance/compliance/soranet_opt_outs.json`)。
- [ ] 將合規性塊合併到 Orchestrator 配置中（請參閱 `03-config-example.toml`）。
- [ ] 獲取管轄權/全球合規性證明並填充 `attestations` 列表。
- [ ] 運行 `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json`（或您的實時快照）並查看通過/失敗報告。
- [ ] 生成中繼准入 CSR 並獲取治理簽名的信封。
- [ ] 導入保護描述符種子並針對已發布的哈希鏈進行驗證。
- [ ] 運行 `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`。
- [ ] 試運行遙測導出：確保 Prometheus 抓取在本地成功。
- [ ] 安排停電演習窗口並記錄升級聯繫人。
- [ ] 簽署演習證據包：`cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`。
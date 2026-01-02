---
lang: ja
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-11-21T14:24:59.232929+00:00"
translation_last_reviewed: 2026-01-01
---

- [ ] ハードウェア仕様を確認: 8+ cores, 16 GiB RAM, NVMe >= 500 MiB/s.
- [ ] IPv4 + IPv6 を2つずつ確認し、上流が QUIC/UDP 443 を許可していることを確認。
- [ ] relay のアイデンティティ鍵用に HSM または専用の安全なエンクレーブを用意。
- [ ] 公式 opt-out カタログを同期 (`governance/compliance/soranet_opt_outs.json`).
- [ ] compliance ブロックを orchestrator 設定にマージ (see `03-config-example.toml`).
- [ ] 管轄/グローバル compliance の証明を収集し、`attestations` リストに記入。
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (またはライブ snapshot) を実行し pass/fail レポートを確認。
- [ ] relay 受け入れ CSR を生成し、governance 署名済みエンベロープを取得。
- [ ] guard descriptor seed をインポートし、公開 hash chain と照合。
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` を実行。
- [ ] テレメトリの dry-run エクスポート: Prometheus scrape がローカルで成功することを確認。
- [ ] brownout drill のウィンドウをスケジュールし、エスカレーション連絡先を記録。
- [ ] drill 証跡バンドルに署名: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.

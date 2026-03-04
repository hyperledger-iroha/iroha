---
lang: ja
direction: ltr
source: docs/source/soranet/reports/circuit_stability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb8844aac273cd70d69eb927e6c8f22487fc47641bf4bff0afecdc9229898fdf
source_last_modified: "2025-11-15T12:19:16.080861+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet/reports/circuit_stability.md -->

# 回線安定性 Soak レポート

この soak は、SNNet-5 向けに出荷された新しい SoraNet 回線ライフサイクル
マネージャを検証する。ハーネスは `crates/sorafs_orchestrator/src/soranet.rs` の
`circuit_manager_soak_maintains_latency_stability` テストとして実装され、
決定論的なレイテンシサンプルで 3 回の guard ローテーションをシミュレートする。

| ローテーション | Guard Relay | サンプル (ms)     | 平均 (ms) | 最小 (ms) | 最大 (ms) |
|----------|-------------|------------------|--------------|----------|----------|
| 0        | 0x04…04     | 50, 52, 49       | 50.33        | 49       | 52       |
| 1        | 0x04…04     | 51, 53, 50       | 51.33        | 50       | 53       |
| 2        | 0x04…04     | 52, 54, 51       | 52.33        | 51       | 54       |

ローテーション履歴は 3 回の更新で 2 ms の範囲内に収まり、少なくとも 3 回の
guard ローテーションでレイテンシが安定するというロードマップ要件を満たす。
テストはさらに、マネージャが決定論的なローテーションテレメトリを記録し、
設定された TTL を強制することを確認する。

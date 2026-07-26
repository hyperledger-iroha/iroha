---
lang: ja
direction: ltr
source: docs/source/soranet/reports/pow_resilience.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8c1066361e5b9c10dcdd40ba8a6fc99c7c63038eac3900c9b401479f2f4bb12
source_last_modified: "2025-11-15T12:19:16.080861+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet/reports/pow_resilience.md -->

# PoW レジリエンス Soak レポート

`tools/soranet-relay/tests/adaptive_and_puzzle.rs` の
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` テストは、
SNNet-6a の Argon2 ゲートを持続的負荷下で検証する。ハーネスは
`DoSControls` 実装を 6 リクエストの burst ウィンドウと 300 ms ハンドシェイク SLO で
駆動し、プロダクションのパズルポリシー（メモリ 4 MiB、単一レーン、time cost 1）を
難易度 6 で使用する。

| フェーズ | 試行 | レイテンシサンプル (ms) | クールダウン | 備考 |
|-------|----------|----------------------|----------|-------|
| Burst soak | 6 | 190, 190, 190, 190, 190, 190 | リモートクールダウン 4 s | `puzzle::mint_ticket`/`verify` によりチケットを発行・検証し、300 ms SLO 内に収める。 |
| Slowloris penalty | 3 | 340, 340, 340 | slowloris ペナルティ 5 s | SLO を 3 回超過すると設定済み slowloris ペナルティが発動し、relay メトリクスにクールダウンが記録される。 |

両フェーズでパズル難易度は PoW 難易度と一致し、ボリュメトリックな DoS 試行下でも
Argon2 チケットが適応ポリシーの判断と整合することを保証する。

<!-- Japanese translation of docs/source/crypto/gost_performance.md -->

---
lang: ja
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
translator: manual
---

# GOST パフォーマンスワークフロー

TC26 GOST 署名バックエンドの性能を追跡・拘束するための手順を記載します。

## ローカル実行

```bash
make gost-bench                             # ベンチ + 許容差チェック
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # 許容差の上書き
make gost-dudect                            # コンスタントタイム検査
./scripts/update_gost_baseline.sh           # ベンチ + ベースライン更新ヘルパー
```

これらは内部的に `scripts/gost_bench.sh` を呼び出し、以下を実行します。

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` を実行。
2. `gost_perf_check` で `target/criterion` を解析し、ベースライン（`crates/iroha_crypto/benches/gost_perf_baseline.json`）と比較。
3. `GITHUB_STEP_SUMMARY` が利用可能な場合は Markdown サマリを追記。

回帰／改善を承認後にベースラインを更新するには以下を実行します。

```bash
make gost-bench-update
```

または直接:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` はベンチ＋チェッカーを実行し、ベースライン JSON を上書きして最新中央値を表示します。更新した JSON は `crates/iroha_crypto/docs/gost_backend.md` の記録とともにコミットしてください。

### 現在の参照中央値

| アルゴリズム            | Median (µs) |
|-------------------------|-------------|
| ed25519                 | 69.67       |
| gost256_paramset_a      | 1136.96     |
| gost256_paramset_b      | 1129.05     |
| gost256_paramset_c      | 1133.25     |
| gost512_paramset_a      | 8944.39     |
| gost512_paramset_b      | 8963.60     |
| secp256k1               | 160.53      |

## CI

`.github/workflows/gost-perf.yml` も同スクリプトを利用し、dudect タイミングガードを実行します。測定中央値がベースラインを許容差（既定 20%）以上に上回るか、タイミングガードが漏洩を検出すると CI が失敗し回帰を防ぎます。

## サマリ出力

`gost_perf_check` はローカルに比較表を表示し、同じ内容を `GITHUB_STEP_SUMMARY` に追記するため、CI ログと実行サマリで同じ数値を確認できます。


<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi 正式モデル (TLA+ / Apalache)

このディレクトリには、Sumeragi コミット パスの安全性と活性性のための限定された形式モデルが含まれています。

## 範囲

モデルは以下をキャプチャします。
- 位相進行 (`Propose`、`Prepare`、`CommitVote`、`NewView`、`Committed`)、
- 投票および定足数のしきい値 (`CommitQuorum`、`ViewQuorum`)、
- NPoS スタイルのコミット ガードの加重ステーク クォーラム (`StakeQuorum`)、
- ヘッダー/ダイジェスト証拠を含む RBC 因果関係 (`Init -> Chunk -> Ready -> Deliver`)、
- 正直な進捗状況に対する GST と弱い公平性の仮定。

ワイヤ形式、署名、および完全なネットワークの詳細を意図的に抽象化します。

## ファイル

- `Sumeragi.tla`: プロトコル モデルとプロパティ。
- `Sumeragi_fast.cfg`: CI に適した小さいパラメーター セット。
- `Sumeragi_deep.cfg`: より大きなストレス パラメーター セット。

## プロパティ

不変条件:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

時間的性質:
- `EventuallyCommit` (`[] (gst => <> committed)`)、GST 後の公平性エンコードあり
  `Next` で動作可能 (タイムアウト/障害プリエンプション ガードが有効になっています)
  進行中のアクション)。これにより、Apalache 0.52.x でモデルをチェックできるようになります。
  は、チェックされた一時プロパティ内の `WF_` 公平性演算子をサポートしません。

## ランニング

リポジトリのルートから:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### 再現可能なローカル設定 (Docker は必要ありません)このリポジトリで使用される固定されたローカル Apalache ツールチェーンをインストールします。

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

ランナーは次の場所でこのインストールを自動検出します。
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`。
インストール後、`ci/check_sumeragi_formal.sh` は追加の環境変数なしで動作するはずです。

```bash
bash ci/check_sumeragi_formal.sh
```

Apalache が `PATH` にない場合は、次のことができます。

- `APALACHE_BIN` を実行可能パスに設定する、または
- Docker フォールバックを使用します (`docker` が使用可能な場合はデフォルトで有効になります)。
  - 画像: `APALACHE_DOCKER_IMAGE` (デフォルト `ghcr.io/apalache-mc/apalache:latest`)
  - Docker デーモンの実行が必要です
  - `APALACHE_ALLOW_DOCKER=0` でフォールバックを無効にします。

例:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## 注意事項

- このモデルは、実行可能な Rust モデル テストを補完します (置き換えるものではありません)。
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  そして
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`。
- チェックは、`.cfg` ファイル内の定数値によって制限されます。
- PR CI は、`.github/workflows/pr.yml` でこれらのチェックを実行します。
  `ci/check_sumeragi_formal.sh`。
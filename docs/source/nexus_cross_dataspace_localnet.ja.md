<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus クロスデータスペースローカルネットプルーフ

この Runbook は、次のような Nexus 統合証明を実行します。

- 2 つの制限されたプライベート データスペース (`ds1`、`ds2`) を使用して 4 ピア ローカルネットを起動します。
- アカウント トラフィックを各データスペースにルーティングします。
- 各データスペースにアセットを作成します。
- データスペース全体で両方向にアトミック スワップ決済を実行します。
- 資金不足のレッグを送信し、残高が変化していないことを確認することで、ロールバック セマンティクスを証明します。

正規のテストは次のとおりです。
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`。

## クイックラン

リポジトリ ルートからラッパー スクリプトを使用します。

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

デフォルトの動作:

- データスペース間の証明テストのみを実行します。
- `NORITO_SKIP_BINDINGS_SYNC=1` を設定します。
- `IROHA_TEST_SKIP_BUILD=1` を設定します。
- `--test-threads=1` を使用します。
- `--nocapture` に合格します。

## 便利なオプション

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` は、フォレンジック用に一時的なピア ディレクトリ (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) を保持します。
- `--all-nexus` は、実証テストだけでなく、`mod nexus::` (完全な Nexus 統合サブセット) を実行します。

## CI ゲート

CI ヘルパー:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

ターゲットを作成します:

```bash
make check-nexus-cross-dataspace
```

このゲートは決定論的証明ラッパーを実行し、クロスデータスペースのアトミックな場合にはジョブを失敗させます。
スワップシナリオは後退する。

## 手動による同等のコマンド

対象となる実証テスト:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

完全な Nexus サブセット:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## 予想される証拠信号- テストに合格しました。
- 意図的に失敗した資金不足の決済レッグに対して、予期される警告が 1 つ表示されます。
  `settlement leg requires 10000 but only ... is available`。
- 最終残高アサーションは次の後に成功します。
  - フォワードスワップが成功した、
  - 逆スワップが成功した、
  - 資金不足のスワップが失敗しました (未変更の残高をロールバック)。

## 現在の検証スナップショット

**2026 年 2 月 19 日** の時点で、このワークフローは次のように成功しました。

- 対象となるテスト: `1 passed; 0 failed`、
- 完全な Nexus サブセット: `24 passed; 0 failed`。
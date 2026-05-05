---
lang: ja
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# キット・ダドレス ローカル -> グローバル

モノリポジトリの Cette ページは `docs/source/sns/local_to_global_toolkit.md` を参照します。ヘルパー CLI と Runbook を再グループ化するには、アイテム ロードマップ **ADDR-5c** が必要です。

## アペルク

- CLI `scripts/address_local_toolkit.sh` カプセル化 `iroha` の製造:
  - `audit.json` -- `iroha tools address audit --format json` の構造を出撃します。
  - `normalized.txt` -- literaux i105 (優先) / 圧縮 (`sora`) (2 番目の選択) は、ドメイン ローカルの選択を変換します。
- ダッシュボードのアドレス取り込みスクリプトの関連付け (`dashboards/grafana/address_ingest.json`)
  その他の規則 Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) は、ローカル 8 のカットオーバーを実行します /
  Local-12 est sur. Local-8 および Local-12 およびアラートの衝突監視
  `AddressLocal8Resurgence`、`AddressLocal12Collision`、および `AddressInvalidRatioSlo` の前処理
  マニフェストの変更を促進する。
- [アドレス表示ガイドライン](address-display-guidelines.md) などを参照してください。
  [アドレス マニフェスト ランブック](../../../source/runbooks/address_manifest_ops.md) UX のコンテキストとインシデントへの対応を提供します。

## 使用法

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

オプション:

- `--format i105` 出撃 `sora...` i105 を使用します。
- `domainless output (default)` を注いでください。
- `--audit-only` 無視者が変換を実行します。
- `--allow-errors` は、不正な形式の機器のスキャンを継続します (CLI の対応)。

最後の実行までのスクリプトの作成。ジョイネス・レ・ドゥ・フィシエ
投票チケットの変更の平均キャプチャ Grafana qui prouve zero
検出ローカル 8 およびゼロ衝突ローカル 12 ペンダント >= 30 時間。

## 統合 CI

1. ランセはスクリプトを作成し、出撃とアップロードを行います。
2. ブロックは、Quand `audit.json` signale des selecteurs Local (`domain.kind = local12`) をマージします。
   デフォルトの `true` の値 (クラスタの開発/テスト期間中は `false` に合格)
   回帰診断) et ajoutez
   `iroha tools address normalize` CI プール クエリ回帰
   エコーエント・アヴァン・ラ・プロダクション。

文書ソースと詳細、証拠とスニペットのチェックリストを閲覧する
リリースノートは、クライアントをカットオーバーするアナウンサーを再利用します。
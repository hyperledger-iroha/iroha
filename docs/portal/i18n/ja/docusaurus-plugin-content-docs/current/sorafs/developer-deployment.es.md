---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者デプロイメント
title: SoraFS に関する記述
サイドバーラベル: ノートの説明
説明: SoraFS の CI 製造パイプラインのプロモーターの検証リスト。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/developer/deployment.md` のページを参照してください。定期的にバージョンを確認し、ドキュメントを保存し、引退します。
:::

# ノート・ド・デスリーグ

SoraFS の決定は、CI の運用に必要な保護措置を講じる必要があります。米国は、ゲートウェイとアルマセナミエントの現実を証明するために、ラ・ヘルラミエンタのリストを作成します。

## 事前準備

- **登録情報** — チャンカーとマニフェストの参照ファイル `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`) を確認します。
- **承認政治** — `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) に必要な別名証明の改訂。
- **ピン レジストリのランブック** — 管理 `docs/source/sorafs/runbooks/pin_registry_ops.md` 回復のための管理 (エイリアスの回転、複製の失敗)。

## エントルノの構成

- ゲートウェイは、ストリーミングおよび証明のエンドポイントで使用可能 (`POST /v2/sorafs/proof/stream`)、CLI からテレメトリの再開を可能にします。
- 政治 `sorafs_alias_cache` を `iroha_config` または CLI ヘルパー (`sorafs_cli manifest submit --alias-*`) で事前決定するための設定を行います。
- Proporciona ストリーム トークン (Torii の認証情報) は、秘密の中央値です。
- テレメトリーの輸出入 (`torii_sorafs_proof_stream_*`、`torii_sorafs_chunk_range_*`) はスタック Prometheus/OTel を参照します。

## 絶望的な戦略

1. **青/緑で表示されます**
   - 米国 `manifest submit --summary-out` パラ アーカイブ ラス レスプエスタ デ カーダ デスリーグ。
   - Vigila `torii_sorafs_gateway_refusals_total` パラディテクター デサジャスト デ キャパシダーデス テンプラノ。
2. **証明の検証**
   - Trata los fallos en `sorafs_cli proof stream` como bloqueadores del despliegue;ロス ピコス デ レイテンシア スエレン インディカル スロットル デル プローブまたは層のマル構成。
   - `proof verify` デベ・フォーマル・パート・デル・スモーク・テスト後部アル・ピン・パラ・アセグラル・クエリ・カル・アロハド・ポル・ロス・プロベドーレス・シグエの偶然の一致とマニフェストのダイジェスト。
3. **テレメトリのダッシュボード**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` と Grafana をインポートします。
   - ピン レジストリ (`docs/source/sorafs/runbooks/pin_registry_ops.md`) のチャンク範囲に関する追加パネル。
4. **マルチソースのハビリタシオン**
   - `docs/source/sorafs/runbooks/multi_source_rollout.md` で、スコアボード/テレメトリのスコアボード/テレメトリのアーカイブとアクティベーション エル オルケスタドールの情報を入手できます。

## 事件の発生

- `docs/source/sorafs/runbooks/` でのエスカラドの進行状況:
  - `sorafs_gateway_operator_playbook.md` ゲートウェイとストリーム トークンの処理。
  - `dispute_revocation_runbook.md` 複製に関する紛争が発生しました。
  - `sorafs_node_ops.md` は、完璧なノードを管理します。
  - `multi_source_rollout.md` パラは、オルケスタドール、ピアのリスト、およびエタパスのデスリーグをオーバーライドします。
- GovernanceLog での証拠や遅延の異常を登録し、PoR トラッカーの API が存在するかどうかを確認し、検証を行います。

## プロキシモス パソス

- 自動制御システム (`sorafs_car::multi_fetch`) マルチソース フェッチ (SF-6b) の制御機能を統合します。
- Sigue lasactualizaciones de PDP/PoTR bajo SF-13/SF-14; CLI とドキュメントの進化に関する説明者計画と段階の選択は、安全性の証明を確立します。

クイック スタートと CI のテストを組み合わせて、パイプライン SoraFS の製造プロセスを再現し、観察可能な場所で実験を行います。
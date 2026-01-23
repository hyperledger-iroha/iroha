---
lang: ja
direction: ltr
source: docs/source/sorafs_gateway_dns_design_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7524a4c4d40b54eb27376abafaaf2f5deedf1c7e670fbe695d729fe61b3ad41f
source_last_modified: "2025-11-02T17:18:17.688536+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/sorafs_gateway_dns_design_agenda.md -->

# SoraFS Gateway & DNS Design Kickoff — アジェンダ

**日付:** 2025-03-03  
**時間:** 16:00–17:00 UTC（60 分）  
**ファシリテーター:** Networking TL  
**会議種別:** 意思決定ワークショップ（Zoom/Meet + 共有ノート）

## 1. 参加者名簿

| 役割 | 名前 / エイリアス | 担当 |
|------|-------------------|------|
| Networking TL（ファシリテーター） | `networking.tl@soranet` | 成果重視の議論を主導し、決定事項を記録し、フォローアップを担当。 |
| Ops リード | `ops.lead@soranet` | DNS 自動化、ロールアウトランブック、運用準備。 |
| Storage チーム代表 | `storage.rep@sorafs` | マニフェスト統合、chunker フィクスチャ、クライアントオーケストレーションへの影響。 |
| Tooling WG 代表 | `tooling.wg@sorafs` | コンフォーマンスハーネスの維持、CLI/ツール変更。 |
| ガバナンス連絡役 | `governance@sora` | GAR ポリシー整合、エスカレーション経路、成果物アーカイブ。 |
| QA ギルドリード | `qa.guild@sorafs` | テスト範囲計画、負荷スイート資源、回帰の責任者。 |
| Docs/DevRel オブザーバー | `docs.devrel@sora` | オペレーターランブック、Docusaurus 更新、対外コミュニケーション。 |
| Torii プラットフォーム代表 | `torii.platform@soranet` | API 統合、テレメトリパイプライン、設定面。 |
| セキュリティエンジニアリング オブザーバー | `security@soranet` | GAR 強制の脅威モデリング、監査証跡要件。 |

> **アクション:** 2025-02-26 までに全参加者の可否を確認し、必要なら役割を差し替える。

## 2. 事前資料の確認（0–5 分）
- 全員が以下を読んだことを確認:
  - `docs/source/sorafs_gateway_dns_design_pre_read.md`
  - `docs/source/sorafs_gateway_profile.md`
  - `docs/source/sorafs_gateway_conformance.md`
  - `docs/source/sorafs_gateway_deployment_handbook.md`
- 回覧後に追加された資料があれば共有。

## 3. 決定論的 DNS の範囲（5–20 分）
1. **ホスト導出ルール（5 分）:**
   - 提案されている命名スキーム（`<capability>.<lane>.gateway.sora`）。
   - ネームスペース予約の所有者と衝突対応。
2. **別名証明 & TTL ポリシー（5 分）:**
   - SF-4a 要件とキャッシュ済み証明の無効化フローの確認。
3. **自動化パス（5 分）:**
   - ツール選定: Terraform + RFC2136 vs Torii 管理。
   - シークレット管理、監査ログ、GAR 連携。
4. **決定事項の記録（5 分）:**
   - 共有ノートに最終決定を記録。
   - 決定事項をドキュメント／設定に反映する担当を割り当て。

## 4. ゲートウェイ強制とランタイム（20–40 分）
1. **GAR ポリシーエンジン（8 分）:**
   - 統合方式（ライブラリ vs Norito キャッシュ）。
   - 設定ノブ、ロールアウト用トグル。
2. **Trustless プロファイル整合（5 分）:**
   - `sorafs_gateway_profile.md` の未解決項目を確認。
3. **Direct モード & レート制限（4 分）:**
   - マニフェスト能力の強制、denylist フック要件。
4. **テレメトリ & アラート（8 分）:**
   - 収集するメトリクス（`torii_sorafs_gar_violations_total`、レイテンシヒストグラム）。
   - ガバナンス／当番へのアラートルーティング。
5. **決定事項の記録（5 分）:**
   - 受け入れ基準と実装 PR のオーナーを明文化。

## 5. コンフォーマンスハーネス計画（40–50 分）
1. **カバレッジ確認（5 分）:**
   - `sorafs_gateway_conformance.md` に基づくリプレイ、ネガティブ、負荷スイート。
2. **アテステーションパイプライン（3 分）:**
   - Norito エンベロープ形式、`sorafs-gateway-cert` の責務。
3. **リソース & タイムライン確認（2 分）:**
   - QA ギルドの体制、Tooling WG のスケジュール、CI 統合。

## 6. 依存関係とロードマップ整合（50–55 分）
- 決定事項をロードマップ項目（SF-4、SF-4a、SF-5、SF-5a）に紐づける。
- SF-2/SF-3 の成果物に起因するブロッカーを洗い出す。
- `status.md` と Docusaurus ポータルの更新が必要か確認。

## 7. アクションレジスタ & 次のステップ（55–60 分）
- 決定事項と未解決アクションを要約。
- オーナー、期日、フォローアップのチェックポイントを割り当てる。
- 会議ノートと成果物の公開計画を確認。

## 8. 事前作業チェックリスト（オーナー）

| タスク | オーナー | 期限 | メモ |
|--------|----------|------|------|
| 参加者の可否確認 | Networking TL | 2025-02-26 | `docs/source/sorafs_gateway_dns_design_invite.txt` のテンプレートを使用。 |
| アジェンダと事前資料の配布 | Networking TL | 2025-02-27 | アジェンダ + 事前資料を 1 通にまとめて送付。 |
| GAR メトリクスの現状スナップショット収集 | Ops リード / Torii 代表 | 2025-02-28 | `scripts/telemetry/run_schema_diff.sh` の派生版を実行（アクション項目参照）。 |
| 図の準備（DNS フロー、ポリシーエンジン） | Tooling WG / セキュリティ | 2025-03-01 | `docs/source/images/sorafs_gateway_kickoff/` に格納。 |
| メモ文書のドラフト | Docs/DevRel | 2025-02-28 | 決定事項／アクションのアウトラインを用意。 |

## 9. 会議後フォローアップ・テンプレート
- **24 時間以内:** ノートとアクションレジスタをロードマップ／ステータスへ公開。
- **48 時間以内:** 参照ドキュメントを決定事項で更新。
- **72 時間以内:** 実装 Issue/PR を起票し、進捗確認をスケジュール。

---

調整や追加議題がある場合は、この文書にコメントするか `networking.tl@soranet` に連絡する。
会議を意思決定に集中させるため、変更はセッション 24 時間前までに凍結すること。

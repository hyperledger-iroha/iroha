---
lang: ja
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd0430f0b524a2190852369c66407faf5b4f8b91bf0b2c65c68bfa367b366d9a
source_last_modified: "2026-01-03T18:07:57.772749+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/governance_playbook.md -->

# ガバナンス・プレイブック

このプレイブックは、Sora Network のガバナンス評議会を日々整合させる儀式を
まとめたものです。リポジトリ内の権威ある参照を集約し、個々のセレモニーを
簡潔に保ちつつ、運用者が全体プロセスへアクセスできる単一の入口を提供します。

## 評議会のセレモニー

- **Fixture ガバナンス** – [Sora Parliament Fixture Approval](sorafs/signing_ceremony.md)
  を参照し、SoraFS chunker 更新をレビューする際に議会の Infrastructure Panel が
  従う on-chain 承認フローを確認する。
- **投票集計の公開** – [Governance Vote Tally](governance_vote_tally.md) を参照し、
  CLI の手順とレポートテンプレートを利用する。

## 運用 Runbooks

- **API 統合** – [Governance API reference](governance_api.md) は評議会サービスが
  提供する REST/gRPC サーフェス、認証要件、ページング規則を列挙する。
- **テレメトリダッシュボード** – `docs/source/grafana_*` 配下の Grafana JSON 定義が
  “Governance Constraints” と “Scheduler TEU” ボードを規定する。各リリース後に
  JSON を Grafana に取り込み、正規レイアウトと同期する。

## Data Availability の監督

### Retention クラス

DA manifests を承認する議会パネルは、投票前に強制された retention ポリシーを
参照しなければならない。下表は `torii.da_ingest.replication_policy` によって
強制されるデフォルトを反映しており、レビュー担当が TOML の出典を探さずに
不一致を検出できるようにする。【docs/source/da/replication_policy.md:1】

| ガバナンスタグ | Blob クラス | ホット保持 | コールド保持 | 必要レプリカ数 | ストレージクラス |
|----------------|------------|------------|--------------|----------------|------------------|
| `da.taikai.live` | `taikai_segment` | 24 h | 14 d | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6 h | 7 d | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12 h | 180 d | 3 | `cold` |
| `da.default` | _その他すべて_ | 6 h | 30 d | 3 | `warm` |

Infrastructure Panel は `docs/examples/da_manifest_review_template.md` の
テンプレートを各投票に添付し、manifest digest、retention tag、Norito
アーティファクトがガバナンス記録で結び付くようにする。

### 署名済み manifest の監査トレイル

投票が議題に上がる前に、評議会スタッフは、レビュー対象の manifest bytes が
議会エンベロープと SoraFS アーティファクトに一致することを証明する必要がある。
既存ツールで証跡を収集する。

1. Torii から manifest バンドルを取得する（`iroha da get-blob --storage-ticket <hex>`
   もしくは同等の SDK helper）。全員がゲートウェイに到達した同一 bytes をハッシュ
   するため。
2. 署名済みエンベロープで manifest stub 検証を実行する:
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   これにより BLAKE3 digest が再計算され、`chunk_digest_sha3_256` が検証され、
   `manifest_signatures.json` に埋め込まれた Ed25519 署名が全て検証される。
   追加オプションは `docs/source/sorafs/manifest_pipeline.md` を参照。
3. digest、`chunk_digest_sha3_256`、profile handle、署名者リストをレビュー
   テンプレートに転記する。検証結果が “profile mismatch” または署名欠落を
   示す場合は投票を停止し、修正済みエンベロープを要求する。
4. 検証出力（または `ci/check_sorafs_fixtures.sh` の CI アーティファクト）を
   Norito `.to` payload と一緒に保存し、監査者が内部ゲートウェイ無しで
   証跡を再現できるようにする。

この監査パックにより、manifest がホットストレージからローテーションされた後でも、
議会はすべてのハッシュと署名チェックを再現できる。

### レビューチェックリスト

1. 議会が承認した manifest エンベロープを取得し（`docs/source/sorafs/signing_ceremony.md`）、
   BLAKE3 digest を記録する。
2. manifest の `RetentionPolicy` ブロックが上表のタグと一致するか確認する。
   Torii は不一致を拒否するが、評議会は監査証跡を残す必要がある。
   【docs/source/da/replication_policy.md:32】
3. 提出された Norito payload が同じ retention tag と blob クラスを参照していることを
   確認する。
4. ポリシーチェックの証跡（CLI 出力、`torii.da_ingest.replication_policy` ダンプ、
   または CI アーティファクト）をレビュー・パケットに添付し、SRE が判断を再現
   できるようにする。
5. 提案が `docs/source/sorafs_reserve_rent_plan.md` に依存する場合は、計画された
   補助金支出または賃料調整を記録する。

### エスカレーション・マトリクス

| リクエスト種別 | 所管パネル | 添付すべき証跡 | 期限とテレメトリ | 参照 |
|---------------|------------|----------------|------------------|------|
| 補助金 / 賃料調整 | Infrastructure + Treasury | 完成済み DA パケット、`reserve_rentd` の rent 差分、更新済み reserve 予測 CSV、評議会投票議事録 | Treasury 更新前に rent 影響を記録。Finance が次の決済ウィンドウで整合できるよう、30 日ローリング buffer テレメトリを添付する。 | `docs/source/sorafs_reserve_rent_plan.md`, `docs/examples/da_manifest_review_template.md` |
| モデレーションの削除 / コンプライアンス対応 | Moderation + Compliance | コンプライアンスチケット（`ComplianceUpdateV1`）、proof tokens、署名済み manifest digest、アピール状況 | Gateway compliance SLA に従う（24h 以内の受領、≤72h で完全削除）。アクションを示す `TransparencyReportV1` 抜粋を添付。 | `docs/source/sorafs_gateway_compliance_plan.md`, `docs/source/sorafs_moderation_panel_plan.md` |
| 緊急凍結 / ロールバック | Parliament moderation panel | 事前承認パケット、新しい凍結命令、ロールバック manifest digest、インシデントログ | 凍結通知を即時公開し、次のガバナンススロット内にロールバック住民投票を計画。緊急性を示す buffer saturation + DA replication テレメトリを添付する。 | `docs/source/sorafs/signing_ceremony.md`, `docs/source/sorafs_moderation_panel_plan.md` |

インテークチケットのトリアージ時にこの表を使用し、各パネルが必要なアーティファクトを
確実に受け取れるようにする。

### レポート成果物

各 DA-10 の判断には以下のアーティファクトが必須（投票で参照される Governance DAG
エントリに添付する）。

- `docs/examples/da_manifest_review_template.md` の完成済み Markdown パケット
  （署名・エスカレーション節を含む）。
- 署名済み Norito manifest (`.to`) と `manifest_signatures.json` エンベロープ、
  または fetch digest を証明する CI 検証ログ。
- アクションに伴う透明性更新:
  - テイクダウンやコンプライアンス凍結に対する `TransparencyReportV1` 差分。
  - 補助金のための rent/reserve 台帳差分または `ReserveSummaryV1` スナップショット。
- レビュー中に取得したテレメトリ・スナップショット（replication depth、buffer headroom、
  moderation backlog）へのリンク。

## モデレーション & エスカレーション

ゲートウェイのテイクダウン、補助金のクロー バック、DA 凍結は、
`docs/source/sorafs_gateway_compliance_plan.md` に記載されたコンプライアンス
パイプラインと `docs/source/sorafs_moderation_panel_plan.md` のアピールツールに従う。
パネルは以下を行う:

1. 起点となるコンプライアンスチケット（`ComplianceUpdateV1` または
   `ModerationAppealV1`）を記録し、関連する proof tokens を添付する。
   【docs/source/sorafs_gateway_compliance_plan.md:20】
2. 要求がモデレーション・アピールパス（市民パネル投票）か、緊急議会凍結かを確認。
   両方とも新テンプレートで取得した manifest digest と retention tag を引用する。
   【docs/source/sorafs_moderation_panel_plan.md:1】
3. エスカレーション期限（アピールの commit/reveal ウィンドウ、緊急凍結期間）を列挙し、
   フォローアップの担当評議会/パネルを明記する。
4. アクションの根拠となったテレメトリ・スナップショット（buffer headroom、moderation backlog）
   を保存し、後続監査が決定と実状態を突合できるようにする。

コンプライアンス/モデレーションパネルは、週次の透明性レポートを決済ルーター運用者と
同期し、テイクダウンと補助金が同じ manifest セットに影響するようにする。

## レポートテンプレート

すべての DA-10 レビューは署名済み Markdown パケットを要求する。
`docs/examples/da_manifest_review_template.md` をコピーし、manifest メタデータ、
retention 検証テーブル、パネル投票サマリを記入し、完成した文書と参照 Norito/JSON
アーティファクトを Governance DAG エントリにピン留めする。パネルはガバナンス議事録に
リンクし、将来のテイクダウンや補助金更新が元の manifest digest を再利用できるようにする。

## インシデント & 取り消しフロー

緊急アクションは on-chain で行う。fixture リリースをロールバックする場合は、
ガバナンスチケットを作成し、以前承認された manifest digest を指す議会 revert 提案を
起案する。Infrastructure Panel が投票を担当し、確定後に Nexus ランタイムが rollback
イベントを発行する。ローカル JSON アーティファクトは不要。

## プレイブックを最新に保つ

- 新しいガバナンス向け runbook が追加されたら本ファイルを更新する。
- 新しいセレモニーをここにクロスリンクし、評議会の索引を発見可能に保つ。
- 参照ドキュメントが移動した場合（例: 新しい SDK パス）、同じ PR 内でリンクを更新して
  ステール参照を避ける。

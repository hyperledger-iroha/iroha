<!-- Japanese translation of docs/source/global_feature_matrix.md -->

---
lang: ja
direction: ltr
source: docs/source/global_feature_matrix.md
status: complete
translator: manual
---

# グローバル機能マトリクス

凡例: `◉` 実装済み · `○` ほぼ実装 · `▲` 一部実装 · `△` 着手済み · `✖︎` 未着手

## コンセンサス & ネットワーク

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| 複数コレクター K/r サポート & first-commit-certificate-wins | ◉ | 決定論的コレクター選定、冗長ファンアウト、オンチェーンの K/r パラメータ、最初の commit certificate 採用がテスト付きで出荷済み。 | status.md:255; status.md:314 |
| ペースメーカーのバックオフ／RTT 下限／決定論的ジッタ | ◉ | コンフィグで制御可能なタイマにジッタ帯域を実装し、テレメトリとドキュメントを整備。 | status.md:251 |
| NEW_VIEW ゲーティング & Highest commit certificate 追跡 | ◉ | NEW_VIEW / Evidence を伝搬し、Highest commit certificate は単調増加で採用。ハンドシェイクがフィンガープリントを保護。 | status.md:210 |
| availability evidence ゲーティング | ○ | `da_enabled=true` で Availability 投票 / availability evidence を発行しコミットを制御。追加の磨き込みは継続。 | status.md:190 |
| 信頼できるブロードキャスト（DA ペイロード輸送） | ◉ | `da_enabled=true` で RBC (Init/Chunk/Ready/Deliver) が有効化され、ペイロード配布/欠落回復に利用。コミットは availability evidence でゲート（ローカルの `DELIVER` は条件ではありません）。 | status.md:283-284 |
| ExecutionQC 収集 & ゲーティング | ○ | Exec 投票、ウィットネス封筒、ゲーティングを実装済み。集約署名の最適化が未完。 | status.md:177 |
| エビデンス伝搬 & 監査エンドポイント | ◉ | `ControlFlow::Evidence`、Torii エビデンス API、ネガティブテストを実装。 | status.md:176; status.md:760-761 |
| RBC テレメトリ、準備／DELIVER メトリクス | ◉ | `/v1/sumeragi/rbc*` エンドポイントとテレメトリカウンタ／ヒストグラムを提供。 | status.md:283-284; status.md:772 |
| コンセンサスパラメータ広告 & トポロジ検証 | ◉ | ノードが `(collectors_k, redundant_send_r)` をブロードキャストし、ピア間の一致を検証。 | status.md:255 |
| 前ブロックハッシュでのローテーション | ◉ | `rotated_for_prev_block_hash` が決定論的にローテーションを提供し、テスト済み。 | status.md:259 |

## パイプライン／Kura／状態

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| 隔離レーンの上限 & テレメトリ | ◉ | 設定ノブと決定論的オーバーフロー処理、テレメトリを実装。 | status.md:263 |
| パイプラインワーカープール設定 | ◉ | `[pipeline].workers` をステート初期化へ伝播し、環境変数のテストを追加。 | status.md:264 |
| スナップショットクエリレーン（ストア／エフェメラルカーソル） | ◉ | Torii と連携するストアドカーソルモード、ブロッキングワーカープールを実装。 | status.md:265; status.md:371; status.md:501 |
| 静的 DAG 指紋リカバリサイドカー | ◉ | Kura にサイドカーを保存し、起動時に検証。ミスマッチ時は警告を発行。 | status.md:106; status.md:349 |
| Kura ブロックストアのハッシュデコード強化 | ◉ | 32 バイト生データ読み出しへ移行し、Norito 非依存の往復テストを追加。 | status.md:608; status.md:668 |
| Norito 適応テレメトリ | ◉ | AoS / NCB 選択メトリクスを Norito へ追加。 | status.md:156 |
| Torii 経由のスナップショット WSV クエリ | ◉ | Torii がブロッキングワーカーを用いるスナップショットレーンを実装。 | status.md:501 |
| トリガーの by-call 実行チェーン | ◉ | データトリガーが by-call 実行直後に決定論的順序で連鎖。 | status.md:668 |

## Norito シリアライズ & ツール

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| Norito JSON への移行 | ◉ | Serde を本番から排除し、インベントリとガードレールで Norito のみを維持。 | status.md:112; status.md:124 |
| Serde deny-list & CI ガードレール | ◉ | ワークフローとスクリプトで新たな Serde 依存を防止。 | status.md:218 |
| Norito コーデックゴールデン & AoS/NCB テスト | ◉ | AoS/NCB ゴールデン、切り詰めテスト、ドキュメント同期を実装。 | status.md:140-150; status.md:332; status.md:666 |
| Norito 機能マトリクスツール | ◉ | `scripts/run_norito_feature_matrix.sh` が下流のスモークテストを支援。CI で packed-seq/struct をカバー。 | status.md:146; status.md:152 |
| Norito 言語バインディング（Python/Java） | ◉ | Python / Java コーデックを同期スクリプトと共に維持。 | status.md:74; status.md:81 |
| Norito Stage-1 SIMD 構造分類器 | ◉ | NEON/AVX2 の Stage-1 分類器とクロスアーキテクチャのゴールデン／ランダムテストを実装。 | status.md:241 |

## ガバナンス & ランタイムアップグレード

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| ランタイムアップグレード受理（ABI ゲート） | ◉ | 有効 ABI 集合を受理時に強制し、構造化エラーとテストが整備済み。 | status.md:196 |
| 保護ネームスペースでのデプロイ制御 | ▲ | デプロイメタデータ要件とゲートを実装済み。ポリシー／UX は継続中。 | status.md:171 |
| Torii ガバナンス読み取りエンドポイント | ◉ | `/v1/gov/*` 読み取り API を実装し、ルーターテストで検証。 | status.md:212 |
| 検証鍵レジストリライフサイクル & イベント | ◉ | VK 登録／更新／廃止、イベント、CLI フィルタ、保持セマンティクスを実装。 | status.md:236-239; status.md:595; status.md:603 |

## ゼロ知識基盤

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| 添付ファイルストレージ API | ◉ | `POST/GET/LIST/DELETE` エンドポイントを実装し、決定論的 ID、テストを整備。 | status.md:231 |
| バックグラウンドプローバー & レポート TTL | ▲ | フラグ付きスタブ、TTL GC、設定ノブを実装。パイプライン全体は進行中。 | status.md:212; status.md:233 |
| エンベロープハッシュの CoreHost バインディング | ◉ | CoreHost 経由でエンベロープハッシュを検証し、監査パルスを公開。 | status.md:250 |
| シールドルート履歴ゲーティング | ◉ | ルートスナップショットを CoreHost に接続し、履歴上限と空ルート設定を実装。 | status.md:303 |
| ZK 投票実行 & ガバナンスロック | ○ | ヌリファイア導出、ロック更新、検証トグルを実装。完全な証明ライフサイクルは進行中。 | status.md:126-128; status.md:194-195 |
| 証明添付の事前検証 & 重複排除 | ◉ | バックエンドタグ、重複排除、実行前の証明レコード永続化を実装。 | status.md:348; status.md:602 |
| ZK CLI ヘルパー & Base64 デコード刷新 | ◉ | CLI を非推奨 API から更新し、テストを追加。 | status.md:174 |
| ZK Torii 証明取得エンドポイント | ◉ | `/v1/zk/proof/{backend}/{hash}` が証明レコード（ステータス、高さ、vk_ref/コミットメント）を公開。 | status.md:94 |

## IVM & Kotodama 連携

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| CoreHost syscall→ISI ブリッジ | ○ | ポインター TLV デコードとシステムコールキューイングが動作。カバレッジ拡充を予定。 | status.md:299-307; status.md:477-486 |
| ポインターコンストラクタ & ドメイン組込み | ◉ | Kotodama のビルトインが Norito TLV と SCALL を生成し、IR/e2e テストとドキュメントを整備。 | status.md:299-301 |
| ポインター ABI の厳格検証 & ドキュメント同期 | ◉ | TLV ポリシーをホスト／IVM の両方で強制し、ゴールデンテストと生成ドキュメントを整備。 | status.md:227; status.md:317; status.md:344; status.md:366; status.md:527 |
| ZK システムコールの CoreHost ゲーティング | ◉ | オペレーション毎のキューが証明とハッシュ一致を確認し、ISI 実行前にゲート。 | crates/iroha_core/src/smartcontracts/ivm/host.rs:213;同:279 |
| Kotodama ポインター ABI ドキュメント & 文法 | ◉ | Grammar / Docs を実装と同期。 | status.md:299-301 |
| ISO 20022 スキーマ駆動エンジン & Torii ブリッジ | ◉ | 正規の ISO 20022 スキーマを組み込み、決定論的 XML 解析と `/v1/iso20022/status/{MsgId}` を公開。 | status.md:65-70 |

## ハードウェアアクセラレーション

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| SIMD テイル／ミスアライン整合テスト | ◉ | ランダム化テストで SIMD ベクトル演算がスカラーと一致することを検証。 | status.md:243 |
| Metal/CUDA フォールバック & 自己テスト | ◉ | GPU バックエンドがゴールデンセルフテストを実行し、ミスマッチ時にスカラー／SIMDへフォールバック。 | status.md:244-246 |

## ネットワーク時間 & コンセンサスモード

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| ネットワークタイムサービス (NTS) | ✖︎ | `new_pipeline.md` に設計のみ存在。実装は未着手。 | new_pipeline.md |
| 指名 PoS コンセンサスモード | ✖︎ | Nexus 文書に closed-set / NPoS の設計があるが、実装はこれから。 | new_pipeline.md; nexus.md |

## Nexus レジャーロードマップ

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| Space Directory コントラクト素体 | ✖︎ | グローバルレジストリコントラクトは未実装。 | nexus.md |
| Data Space マニフェスト形式 & ライフサイクル | ✖︎ | Norito マニフェストスキーマ、バージョン管理、ガバナンスフローは未実装。 | nexus.md |
| DS ガバナンス & バリデータローテーション | ✖︎ | DS メンバーシップとローテーション手続きが未実装 | nexus.md |
| Cross-DS アンカー & Nexus ブロック構成 | ✖︎ | 構成レイヤーとアンカーコミットメントは設計段階。 | nexus.md |
| Kura/WSV ためのイレージャーコーデッドストレージ | ✖︎ | 公開／非公開 DS 向けの冗長ストレージは未実装。 | nexus.md |
| DS ごとの ZK/楽観的証明ポリシー | ✖︎ | DS ごとの証明要件と強制手段は未実装。 | nexus.md |
| DS ごとの手数料／クォータ分離 | ✖︎ | 手数料・クォータの仕組みは今後の課題。 | nexus.md |

## カオス & フォールトインジェクション

| 機能 | 状態 | メモ | エビデンス |
|------|------|------|-------------|
| Izanami カオスネットオーケストレーション | ○ | Izanami が資産定義、メタデータ、NFT、トリガー連鎖のレシピを駆動し、新テストを追加。 | crates/izanami/src/instructions.rs; 同 tests |

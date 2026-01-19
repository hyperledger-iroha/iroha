<!-- Japanese translation of docs/source/security_hardening_requirements.md -->

---
lang: ja
direction: ltr
source: docs/source/security_hardening_requirements.md
status: complete
translator: manual
---

# ランタイムハードニング要件

本ノートでは、脅威モデルで挙げられた残余リスクを解消するための具体的な要件をまとめます。各項目はスコープ、機能要件、可観測性、デプロイ考慮点を列挙し、実装の指針とします。

## メンバーシップ View-Hash テレメトリ

**スコープ:** Sumeragi コンセンサスノードが、ローカルで計算したバリデータロスター（高さ／ビュー）とピアが広告するロスターの不一致を検知・表面化できること。

**要件**
- 新しいコンセンサスパラメータを確立するたびに、順序付きピア ID のリストに基づく `(height, view)` ごとの決定論的ハッシュを算出すること。
- ピアが広告するパラメータまたはロスターハッシュがローカル状態と一致しない場合、Prometheus カウンタをラベル付きで増加させ、構造化ログを出力すること。
- `peer_id`、`height`、`view` を含むラベル付きのゲージ／カウンタを公開し、継続的な不一致を運用者が特定できるようにすること。
- アラート閾値を設定するコンフィグを用意すること（既定: 5 分間に 0 超の不一致でトリガー）。

**可観測性**
- カウンタ: `sumeragi_membership_mismatch_total{peer,height,view}`。
- オプションゲージ: `sumeragi_membership_mismatch_active`。
- `/v1/sumeragi/status` の JSON にミスマッチ状態、ドロップカウンタ、Highest/Locked QC ハッシュ、`pacemaker_backpressure_deferrals_total` 等を含めること。

**実装状況:** `sumeragi_membership_view_hash`／`sumeragi_membership_height`／`sumeragi_membership_view`／`sumeragi_membership_epoch` ゲージと `/v1/sumeragi/status.membership` オブジェクトによって (height/view/epoch) 単位の決定論的ロスターハッシュを公開済みです。ミスマッチカウンタと併用することで、各ピアのロスター整合性を直接比較できます。

**デプロイ考慮**
- 既存のテレメトリエクスポータを壊さないように登録すること。
- 再起動時にカウンタは単調増加を維持し、アクティブゲージは 0 にリセットすること。

## Torii 事前認証接続ゲーティング

**スコープ:** Torii は `preauth_*` 設定で全体／IP ごとの接続数、レート、BAN を制限します。REST/WebSocket ingress はアプリ層認証前に悪意あるクライアントを拒否する必要があります。

**要件**
- `iroha_config::parameters::actual::Torii` の設定に従い、全体／IP ごとの同時接続上限を強制すること。
- TLS/TCP accept および HTTP アップグレード（WebSocket）に対し、バースト／持続レートを設定可能なトークンバケットレート制御を実装すること。
- 違反クライアントには指数バックオフを適用し、設定可能な期間で仮 BAN を追跡すること。
- CIDR アロウリストをサポートし、信頼済みオペレータ等にはレート制御を免除できるようにすること。
- 評価不能な場合（状態枯渇等）は fail closed（新規接続を拒否）とすること。

**可観測性**
- カウンタ: `torii_pre_auth_reject_total{reason}`（`ip_cap`, `global_cap`, `rate`, `ban`, `error`）。
- ゲージ: `torii_active_connections_total{scheme}`。
- BAN 発生時に WARN レベルの構造化ログを出力すること。

**デプロイ考慮**
- 可能であればホットリロード対応（非対応なら再起動要件を明記）。
- 既存テストの受理経路が影響を受けないようにし、拒否経路の回 regress テストを追加すること。
- 統合テスト用にゲーティングを無効化する開発者向けオプションを用意すること。

## 添付ファイルサニタイズパイプライン

**スコープ:** Torii 経由ですべての添付ファイル（マニフェスト、証明、Blob など）はフォーマット検証を通し、Decompression Bomb や悪意あるペイロードを防ぐ必要があります。

**要件**
- マジックバイト検出でコンテンツタイプを推定し、未知／禁止フォーマットを拒否すること。
- ディスク保存前に展開サイズを検査し、圧縮アーカイブの場合は総展開サイズとネスト深度を制限すること。
- サニタイズは `torii.attachments_sanitize_timeout_ms` と OS レベルの CPU/メモリ制限（rlimits）を適用したサブプロセスで実行し、`torii.attachments_sanitizer_mode` で実行モードを選択。
- 送出時は `provenance` が無い旧データを再サニタイズしてから返す。
- 保存時にハッシュ、推測タイプ、サニタイズ結果等のメタデータを記録すること。

**可観測性**
- カウンタ: `torii_attachment_reject_total{reason}`（`type`, `expansion`, `sandbox`, `checksum`）。
- ヒストグラム: `torii_attachment_sanitize_ms`（サンドボックス実行時間）。
- INFO ログで添付 ID と理由を記録し、DEBUG ログでサンドボックス診断を出力すること。

**デプロイ考慮**
- フォーマットごとの有効／無効、展開閾値を設定できるようにすること。
- フォールバック経路はノード間で決定論的であること。サニタイズ結果を格納するか、拒否を統一すること。
- 代表的なファイルの加エテストを追加すること。

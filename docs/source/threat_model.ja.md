<!-- Japanese translation of docs/source/threat_model.md -->

---
lang: ja
direction: ltr
source: docs/source/threat_model.md
status: complete
translator: manual
---

# Hyperledger Iroha v2 脅威モデル

_最終レビュー: 2025-11-07 — 次回予定: 2026-02-05_

メンテナンス周期: セキュリティ WG と各コンポーネントオーナー（90 日以内）。各改訂の要約はチケットリンク付きで `status.md` に記録します。

## 前提条件と非ゴール

- **前提**: 許可型バリデータネットワーク。ピアは mTLS + 鍵ピニングで相互認証。クライアントは信頼されない。オペレーター侵害と大規模 DoS は対象内。
- **非ゴール**: 完全なサプライチェーン証跡（ビルドハードニング文書で追跡）、量子耐性、UI/UX フィッシング。ランタイムに影響するサプライチェーン対策はここに記載します。

## スコープ

- ランタイムアップグレードとマニフェスト処理
- Sumeragi コンセンサス（リーダー選定、投票、RBC、ビュー変更テレメトリ）
- Torii アプリケーション API（REST/WebSocket、認証／認可、レート制御）
- 添付ファイルとペイロードハンドリング（トランザクション、マニフェスト、大容量 BLOB）
- ZK パイプライン（証明生成・検証・回路ガバナンス）
- 鍵・アイデンティティ管理（バリデータ、オペレータ、リリース署名）
- ネットワーク／トランスポート（P2P／Ingress ハンドシェイク、接続管理、DoS）
- テレメトリとログ（エクスポート、プライバシー、整合性）
- メンバーシップとガバナンス（バリデータ追加・追放、構成ドリフト）

脅威アクター: ビザンチンピア、悪意のあるクライアント、侵害されたオペレーター、インターネット規模の DoS。サプライチェーン／コンパイラ脅威はビルドハードニング側で扱い、ランタイムのエンフォースはここで追跡します。

## アセット分類

影響スケール: **Critical**（安全性／可用性破壊や取り返しのつかない台帳破損）、**High**（運用介入を要する停止／境界突破）、**Moderate**（サービス劣化／限定的露出）、**Low**（軽微な影響）。

| アセット | 説明 | 完整性 | 可用性 | 機密性 | オーナー |
| --- | --- | --- | --- | --- | --- |
| 台帳状態（WSV/ブロック） | 正式なレプリケーション履歴 | Critical | Critical | Moderate | Core WG |
| アップグレードマニフェスト／成果物 | スクリプト、バイナリ、証跡 | Critical | High | High | Runtime WG |
| リリース署名鍵 | マニフェストやリリースの承認 | Critical | High | High | Security WG |
| コンセンサス鍵 | バリデータ投票／コミット鍵 | Critical | High | High | Validators |
| クライアント資格情報 | API キー／トークン等 | High | High | Critical | Torii WG |
| ピアメンバーシップレジストリ | エポックごとのピア一覧 | Critical | High | Moderate | Consensus WG |
| 添付ファイルストレージ | オフチェーン BLOB + オンチェーン参照 | High | High | Moderate | Runtime WG |
| Norito/Kotodama コーデック | シリアライズルール | Critical | High | Moderate | Data Model WG |
| ZK 証明素材 | 回路、検証鍵、証人 | High | Moderate | High | ZK WG |
| テレメトリ／監査ログ | メトリクス、健全性、セキュリティログ | Moderate | High | Low/Moderate | Observability WG |
| ノード構成と秘密 | mTLS 証明書、トークン、設定 | High | High | High | Ops |

## 信頼境界

- ピア境界（mTLS 認証済 P2P。ピアはビザンチン化し得る）
- クライアント入口境界（Torii REST/WS、CLI）
- アップグレード制御境界（マニフェスト／署名／設定へのアクセス）
- 添付ファイル入口境界（Torii 経由またはゴシップ）
- ZK ワークロード境界（オフチェーン証明）
- テレメトリエクスポート境界（外部へ送信）
- メンバーシップガバナンス境界（バリデータ追加／追放）

## リスク評価基準

- **Likelihood**: Rare / Unlikely / Possible / Likely / Almost certain
- **Severity**: Low / Moderate / High / Critical
- **Risk level**: 上記の組み合わせ。Critical/High は GA 前に対策必須（セキュリティ WG と担当 WG の承認が必要）。受容したリスクは `status.md` でトラッキング。
- **対応目標**: P1 <=7 日, P2 <=30 日, P3 <=90 日（受容済み除く）。

## 脅威シナリオ

以下で領域別に **現行対策** と **未対応ギャップ** を整理します。

### ランタイムアップグレード

**現行対策**
- マニフェスト署名検証（リリース鍵）。
- 受理時の `abi_hash`／`code_hash` 照合（`runtime_upgrade_admission` テストで検証）。
- ガバナンス承認を経たロールアウトと決定論的ロールバック。
- `iroha_config` による ABI バージョンピン留め。
- SBOM/SLSA 証跡と信頼署名しきい値を受理時に強制し、拒否テレメトリを記録。

**未対応ギャップ**
- リリース署名鍵がバリデータ運用環境と同居（**残余リスク: Release-signing key separation**）。

### Sumeragi コンセンサス

**現行対策**
- ダブル投票エビデンスを保持（Wei...)...

<!-- 省略: 原文と同様に、他セクションを要約 -->

### 残余リスクとトラッキング

| リスク | 状況 | 対策計画 | 担当 | 目標 |
| --- | --- | --- | --- | --- |
| Upgrade SBOM provenance gap | 解決済み | 受理時に SBOM/SLSA 証跡と信頼署名しきい値を強制（`docs/source/runtime_upgrades.md` 参照）。 | Security WG | 2025-11-30 |
| Aggregator fairness audit | 未解決 | サードパーティ監査を Milestone 2 GA 前に実施 (`SUM-203`) | Consensus WG | 2025-12-15 |
| Torii operator auth hardening | 解決済み | WebAuthn/mTLS のオペレーター認証を提供し、資格情報の永続化・セッション・テレメトリを実装。 | Torii WG | 2025-11-15 |
| Hardware-accelerated hashing | 未解決 | 決定論的フォールバック付きマルチバージョンハッシュを実装 (`RNT-092`) | Runtime WG | 2025-12-01 |
| ZK circuit governance | 未解決 | ガバナンスプロトコルとツールを整理 (`ZK-077`) | ZK WG | 2025-11-20 |
| Validator key HSM adoption | 未解決 | HSM ポリシー策定（チケット割当予定） | Security WG | 2025-11-15 |
| Release-signing key separation | 未解決 | オフラインルート + しきい値署名（チケット割当予定） | Security WG | 2025-10-31 |
| Membership registry reconciliation | 未解決 | View-hash チェックと mismatch での停止を実装 (`SUM-203` フォローアップ) | Consensus WG | 2025-10-25 |
| Pre-auth DoS controls | 未解決 | 接続ゲーティング／ハンドシェイク上限を実装、チューニング継続 | Torii & Core WG | 2025-10-31 |
| Telemetry redaction policy | 未解決 | Redaction lint/CI を整備 | Observability WG | 2025-10-20 |
| Time and NTP hardening | 未解決 | NTS もしくは複数ソース検証（チケット予定） | Runtime WG & Ops | 2025-11-10 |
| Membership mismatch telemetry | 未解決 | `sumeragi_membership_mismatch_total` のアラートを整備 | Consensus WG | 2025-10-15 |
| Attachment sanitisation | 解決済み | Magic-byte 判定／サブプロセスサニタイズ／エクスポート再サニタイズを実装 | Runtime WG | 2025-11-30 |
| Witness retention audit | 未解決 | 証人削除の自動検証を実装 | ZK WG | 2025-11-05 |
| Peer churn telemetry | 進行中 | `p2p_peer_churn_total` のアラート・ダッシュボード整備 | Core WG | 2025-10-25 |

## レビュー手順

1. セキュリティ WG が 90 日以内および RC 前にレビュー実施。
2. コンポーネントオーナーが対策・テレメトリ・リスク状況を更新。
3. 文書を改訂し、`status.md` にチケット付きサマリを追記。
4. インシデント／ニアミス発生時は 7 日以内に補遺を作成。

## 署名チェックリスト

| WG | POC | 状況 | メモ |
| --- | --- | --- | --- |
| Security WG | security@iroha | 未 | 2025-10-05 までに確認・チケット紐付け |
| Core WG | core@iroha | 未 | pre-auth DoS 計画と churn テレメトリ確認 |
| Runtime WG | runtime@iroha | 未 | 添付サニタイズ完了の確認 |
| Torii WG | torii@iroha | 未 | 認証強化 (`TOR-118`) の検証と接続ゲート計画 |
| Consensus WG | consensus@iroha | 未 | 公平性監査とメンバーシップテレメトリ |
| Data Model WG | data-model@iroha | 未 | Norito/Kotodama カバレッジとファズコーパス |
| ZK WG | zk@iroha | 未 | 回路ガバナンス (`ZK-077`) と証人保持監査 |
| Observability WG | observability@iroha | 未 | Redaction 施策とログ改ざん検出 |

---
lang: ja
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2026-01-03T18:07:58.674563+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Connect アーキテクチャのフィードバック チェックリスト

このチェックリストは、Connect セッション アーキテクチャからの未解決の質問をまとめています。
ストローマンは、Android および JavaScript リードからの入力を必要とします。
2026 年 2 月クロス SDK ワークショップ。コメントを非同期で収集し、追跡するために使用します。
所有権を取得し、ワークショップの議題のブロックを解除します。

> ステータス / メモ列には、Android および JS リードからの最終応答が記録されています。
> 2026 年 2 月のワークショップ前の同期。新しいフォローアップ問題をインラインでリンクする場合の決定
> 進化します。

## セッションのライフサイクルとトランスポート

|トピック | Android オーナー | JSオーナー |ステータス/メモ |
|------|------|----------|--------------|
| WebSocket 再接続バックオフ戦略 (指数関数的 vs. 上限付き線形) | Android ネットワーキング TL | JSリード | ✅ ジッターを伴う指数関数的なバックオフ、上限を 60 秒とすることに同意。 JS は、ブラウザ/ノードのパリティとして同じ定数をミラーリングします。 |
|オフライン バッファ容量のデフォルト (現在のストローマン: 32 フレーム) | Android ネットワーキング TL | JSリード | ✅ 設定オーバーライドによる 32 フレームのデフォルトを確認しました。 Android は `ConnectQueueConfig` 経由で永続化し、JS は `window.connectQueueMax` を尊重します。 |
|プッシュ スタイルの再接続通知 (FCM/APNS とポーリング) | Android ネットワーキング TL | JSリード | ✅ Android はウォレット アプリ用のオプションの FCM フックを公開します。 JS は、ブラウザーのプッシュ制約に注意して、指数関数的なバックオフを伴うポーリング ベースのままです。 |
|モバイル クライアント用のピンポン ケイデンス ガードレール | Android ネットワーキング TL | JSリード | ✅ 3倍のミス耐性を備えた標準化された30秒ping。 Android は Doze の影響のバランスをとり、JS はブラウザーのスロットリングを避けるために 15 秒以上に制限します。 |

## 暗号化と鍵管理

|トピック | Android オーナー | JSオーナー |ステータス/メモ |
|------|------|----------|--------------|
| X25519 キー ストレージの期待値 (StrongBox、WebCrypto セキュア コンテキスト) | Android 暗号化 TL | JSリード | ✅ Android は、利用可能な場合、X25519 を StrongBox に保存します (TEE にフォールバックします)。 JS は、dApps にセキュア コンテキスト WebCrypto を義務付け、Node のネイティブ `iroha_js_host` ブリッジにフォールバックします。 |
| ChaCha20-Poly1305 SDK 間でのノンス管理の共有 | Android 暗号化 TL | JSリード | ✅ 64 ビット ラップ ガードと共有テストを備えた共有 `sequence` カウンター API を採用。 JS は、Rust の動作に一致させるために BigInt カウンターを使用します。 |
|ハードウェアによる認証ペイロード スキーマ | Android 暗号化 TL | JSリード | ✅ 完成したスキーマ: `attestation { platform, evidence_b64, statement_hash }`; JS はオプション (ブラウザー)、ノードは HSM プラグイン フックを使用します。 |
|紛失したウォレットの回復フロー (キーローテーションハンドシェイク) | Android 暗号化 TL | JSリード | ✅ ウォレットローテーションハンドシェイクが受け入れられました: dApp は `rotate` コントロールを発行し、ウォレットは新しい公開鍵 + 署名付き確認応答で応答します。 JS は WebCrypto マテリアルを即座に再キー化します。 |

## 権限と証明のバンドル|トピック | Android オーナー | JSオーナー |ステータス/メモ |
|------|------|----------|--------------|
| GA の最小権限スキーマ (メソッド/イベント/リソース) | Android データ モデル TL | JSリード | ✅ GA ベースライン: `methods`、`events`、`resources`、`constraints`; JS は TypeScript の型を Rust マニフェストに合わせます。 |
|ウォレット拒否ペイロード (`reason_code`、ローカライズされたメッセージ) | Android ネットワーキング TL | JSリード | ✅ 完成したコード (`user_declined`、`permissions_mismatch`、`compliance_failed`、`internal_error`) とオプションの `localized_message`。 |
|プルーフ バンドルのオプション フィールド (コンプライアンス/KYC 添付ファイル) | Android データ モデル TL | JSリード | ✅ すべての SDK はオプションの `attachments[]` (Norito `AttachmentRef`) および `compliance_manifest_id` を受け入れます。行動を変える必要はありません。 |
| Norito JSON スキーマとブリッジ生成構造体のアライメント | Android データ モデル TL | JSリード | ✅ 決定: ブリッジ生成の構造体を優先します。 JSON パスはデバッグ用にのみ残り、JS は `Value` アダプターを保持します。 |

## SDK ファサードと API 形状

|トピック | Android オーナー | JSオーナー |ステータス/メモ |
|------|------|----------|--------------|
|高レベルの非同期インターフェイス (`Flow`、非同期反復子) パリティ | Android ネットワーキング TL | JSリード | ✅ Android は `Flow<ConnectEvent>` を公開します。 JS は `AsyncIterable<ConnectEvent>` を使用します。どちらも共有 `ConnectEventKind` にマップされます。 |
|エラー分類マッピング (`ConnectError`、型付きサブクラス) | Android ネットワーキング TL | JSリード | ✅ プラットフォーム固有のペイロード詳細を含む共有列挙型 {`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`} を採用します。 |
|飛行中の署名リクエストのキャンセル セマンティクス | Android ネットワーキング TL | JSリード | ✅ `cancelRequest(hash)` コントロールを導入しました。どちらの SDK も、ウォレットの確認に関してキャンセル可能なコルーチン/プロミスを表示します。 |
|共有テレメトリ フック (イベント、メトリクスの名前付け) | Android ネットワーキング TL | JSリード | ✅ 調整されたメトリクス名: `connect.queue_depth`、`connect.latency_ms`、`connect.reconnects_total`。サンプルエクスポータが文書化されています。 |

## オフライン永続化とジャーナリング

|トピック | Android オーナー | JSオーナー |ステータス/メモ |
|------|------|----------|--------------|
|キューに入れられたフレームのストレージ形式 (バイナリ Norito 対 JSON) | Android データ モデル TL | JSリード | ✅ バイナリ Norito (`.to`) をどこにでも保存します。 JS は IndexedDB `ArrayBuffer` を使用します。 |
|ジャーナルの保存ポリシーとサイズの上限 | Android ネットワーキング TL | JSリード | ✅ デフォルトの保持期間は 24 時間、セッションごとに 1MiB。 `ConnectQueueConfig` 経由で設定可能。 |
|双方がフレームを再生する場合の競合解決 | Android ネットワーキング TL | JSリード | ✅ `sequence` + `payload_hash` を使用します。重複は無視され、競合によりテレメトリ イベントで `ConnectError.Internal` がトリガーされます。 |
|キューの深さとリプレイの成功に関するテレメトリ | Android ネットワーキング TL | JSリード | ✅ `connect.queue_depth` ゲージと `connect.replay_success_total` カウンターを出力します。両方の SDK は、共有 Norito テレメトリ スキーマにフックします。 |

## 実装のスパイクとリファレンス- **Rust ブリッジ フィクスチャ:** `crates/connect_norito_bridge/src/lib.rs` および関連テストは、すべての SDK で使用される正規のエンコード/デコード パスをカバーします。
- **Swift デモ ハーネス:** `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` 演習 セッション フローをモック化されたトランスポートに接続します。
- **Swift CI ゲーティング:** Connect アーティファクトを更新するときに `make swift-ci` を実行して、フィクスチャ パリティ、ダッシュボード フィード、および Buildkite `ci/xcframework-smoke:<lane>:device_tag` メタデータを検証してから、他の SDK と共有します。
- **JavaScript SDK 統合テスト:** `javascript/iroha_js/test/integrationTorii.test.js` は、Torii に対して接続ステータス/セッション ヘルパーを検証します。
- **Android クライアントの回復力に関するメモ:** `java/iroha_android/README.md:150` は、キュー/バックオフのデフォルトのきっかけとなった現在の接続実験を文書化しています。

## ワークショップ準備アイテム

- [x] Android: 上の表の各行にポイント担当者を割り当てます。
- [x] JS: 上の表の各行にポイント担当者を割り当てます。
- [x] 既存の実装スパイクまたは実験へのリンクを収集します。
- [x] 2026 年 2 月の評議会の前に事前作業レビューをスケジュールします (Android TL、JS リード、Swift リードで 2026 年 1 月 29 日 15:00 UTC に予約)。
- [x] 受け入れられた回答で `docs/source/connect_architecture_strawman.md` を更新します。

## 先読みパッケージ

- ✅ `artifacts/connect/pre-read/20260129/` で記録されたバンドル (ストローマン、SDK ガイド、およびこのチェックリストを更新した後、`make docs-html` 経由で生成)。
- 📄 概要と配布手順は `docs/source/project_tracker/connect_architecture_pre_read.md` にあります。 2026 年 2 月のワークショップの招待状と `#sdk-council` リマインダーにリンクを含めてください。
- 🔁 バンドルを更新するときは、先読みメモ内のパスとハッシュを更新し、アナウンスを IOS7/AND7 準備ログの下の `status.md` にアーカイブします。
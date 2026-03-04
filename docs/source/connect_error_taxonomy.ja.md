---
lang: ja
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 接続エラー分類 (Swift ベースライン)

このノートでは、IOS-CONNECT-010 を追跡し、IOS-CONNECT-010 の共有エラー分類を文書化します。
Nexus SDK を接続します。 Swift は正規の `ConnectError` ラッパーをエクスポートするようになりました。
これにより、すべての接続関連の障害が 6 つのカテゴリのいずれかにマッピングされるため、テレメトリ、
ダッシュボードと UX コピーはプラットフォーム間で整合性が保たれます。

> 最終更新日: 2026-01-15  
> 所有者: Swift SDK リード (分類管理者)  
> ステータス: Swift + Android + JavaScript の実装は **着陸**;キューの統合は保留中です。

## カテゴリ

|カテゴリー |目的 |代表的な情報源 |
|----------|-----------|------|
| `transport` |セッションを終了する WebSocket/HTTP トランスポート障害。 | `URLError(.cannotConnectToHost)`、`ConnectClient.ClientError.closed` |
| `codec` |フレームのエンコード/デコード中にシリアル化/ブリッジが失敗しました。 | `ConnectEnvelopeError.invalidPayload`、`DecodingError` |
| `authorization` |ユーザーまたはオペレーターの修復が必要な TLS/構成証明/ポリシーの障害。 | `URLError(.secureConnectionFailed)`、Torii 4xx 応答 |
| `timeout` |アイドル/オフラインの有効期限とウォッチドッグ (キュー TTL、リクエスト タイムアウト)。 | `URLError(.timedOut)`、`ConnectQueueError.expired` |
| `queueOverflow` | FIFO バックプレッシャー信号により、アプリは正常に負荷を軽減できます。 | `ConnectQueueError.overflow(limit:)` |
| `internal` |その他: SDK の誤用、Norito ブリッジの欠落、ジャーナルの破損。 | `ConnectSessionError.missingDecryptionKeys`、`ConnectCryptoError.*` |

すべての SDK は、分類法に準拠して公開されるエラー タイプを公開します。
構造化テレメトリ属性: `category`、`code`、`fatal`、およびオプション
メタデータ (`http_status`、`underlying`)。

## 迅速なマッピング

Swift は、`ConnectError`、`ConnectErrorCategory`、およびヘルパー プロトコルをエクスポートします。
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`。すべてのパブリック接続エラー
型は `ConnectErrorConvertible` に準拠しているため、アプリは `error.asConnectError()` を呼び出すことができます
そして結果をテレメトリ/ロギングレイヤーに転送します。|迅速なエラー |カテゴリー |コード |メモ |
|-----------|----------|------|------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` |二重 `start()` を示します。開発者のミス。 |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` |クローズ後の送受信時に発生します。 |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket はバイナリを予期しながらテキストのペイロードを配信しました。 |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` |カウンターパーティが予期せずストリームを閉じました。 |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` |アプリケーションが対称キーの構成を忘れました。 |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito ペイロードに必須フィールドがありません。 |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` |古い SDK によって認識される将来のペイロード。 |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito ブリッジが見つからないか、フレーム バイトのエンコード/デコードに失敗しました。 |
| `ConnectCryptoError.*` | `internal` | `crypto.*` |ブリッジが使用できないか、キーの長さが一致していません。 |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` |オフライン キューの長さが設定された制限を超えました。 |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | `URLSessionWebSocketTask` によって浮上しました。 |
| `URLError` TLS ケース | `authorization` | `network.tls_failure` | ATS/TLS ネゴシエーションの失敗。 |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON デコード/エンコードが SDK の他の場所で失敗しました。メッセージは Swift デコーダー コンテキストを使用します。 |
|その他の `Error` | `internal` | `unknown_error` |全てを網羅することを保証。メッセージは `LocalizedError` をミラーリングします。 |

単体テスト (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) のロックダウン
このマッピングにより、将来のリファクタリングでカテゴリやコードがサイレントに変更されることがなくなります。

### 使用例

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## テレメトリーとダッシュボード

Swift SDK は `ConnectError.telemetryAttributes(fatal:httpStatus:)` を提供します
これは正規の属性マップを返します。輸出者はこれらを転送する必要があります
オプションの追加機能を備えた `connect.error` OTEL イベントへの属性:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

ダッシュボードは `connect.error` カウンターをキューの深さと関連付けます (`connect.queue_depth`)
ヒストグラムを再接続して、ログを調べずに回帰を検出します。

## Android マッピングAndroid SDK は、`ConnectError`、`ConnectErrorCategory`、`ConnectErrorTelemetryOptions`、
`ConnectErrorOptions`、および以下のヘルパー ユーティリティ
`org.hyperledger.iroha.android.connect.error`。ビルダー スタイルのヘルパーは、あらゆる `Throwable` を変換します。
分類法に準拠したペイロードに変換し、トランスポート/TLS/コーデックの例外からカテゴリを推測します。
OpenTelemetry/サンプリング スタックが
カスタムアダプターなしの結果。【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` はすでに `ConnectErrorConvertible` を実装しており、queueOverflow/timeout を発行しています
オーバーフロー/有効期限条件のカテゴリ。オフライン キューの計測を同じフローに接続できるようにします。【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK README では分類法が参照され、トランスポート例外をラップする方法が示されています。
テレメトリを発行する前に、dApp ガイダンスを Swift ベースラインに合わせて維持します。【java/iroha_android/README.md:167】

## JavaScript マッピング

Node.js/ブラウザ クライアント インポート `ConnectError`、`ConnectQueueError`、`ConnectErrorCategory`、および
`connectErrorFrom()`から`@iroha/iroha-js`。共有ヘルパーは HTTP ステータス コードを検査します。
ノード エラー コード (ソケット、TLS、タイムアウト)、`DOMException` 名、およびコーデックの出力失敗
このノートに記載されているのと同じカテゴリ/コードですが、TypeScript の定義はテレメトリをモデル化しています。
属性がオーバーライドされるため、ツールは手動でキャストせずに OTEL イベントを発行できます。【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README にはワークフローが文書化されており、この分類法にリンクされているため、アプリケーション チームは次のことを行うことができます。
インストルメンテーションのスニペットをそのままコピーします。【javascript/iroha_js/README.md:1387】

## 次のステップ (クロス SDK)

- **キューの統合:** オフライン キューが出荷されたら、デキュー/ドロップ ロジックを確認します。
  は `ConnectQueueError` 値を表面化するため、オーバーフロー テレメトリの信頼性は維持されます。
<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# セキュリティ監査レポート

日付: 2026-03-26

## エグゼクティブサマリー

この監査は、現在のツリー内で最もリスクの高いサーフェイス、Torii HTTP/API/認証フロー、P2P トランスポート、シークレット処理 API、SDK トランスポート ガード、および添付ファイル サニタイザー パスに焦点を当てました。

実行可能な問題が 6 つ見つかりました。

- 2 重大度の高い所見
- 4 中程度の重症度の所見

最も重要な問題は次のとおりです。

1. Torii は現在、すべての HTTP リクエストの受信リクエスト ヘッダーをログに記録します。これにより、ベアラー トークン、API トークン、オペレーター セッション/ブートストラップ トークン、および転送された mTLS マーカーがログに公開される可能性があります。
2. 複数のパブリック Torii ルートと SDK は引き続き、生の `private_key` 値のサーバーへの送信をサポートしているため、呼び出し元に代わって Torii に署名できます。
3. 一部の SDK では、機密シード導出や正規リクエスト認証など、いくつかの「秘密」パスが通常のリクエスト本文として扱われます。

## メソッド

- Torii、P2P、暗号化/VM、および SDK シークレット処理パスの静的レビュー
- 対象を絞った検証コマンド:
  - `cargo check -p iroha_torii --lib --message-format short` -> 合格
  - `cargo check -p iroha_p2p --message-format short` -> 合格
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> 合格
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> 合格、重複バージョンの警告のみ
- このパスでは完了していません:
  - 完全なワークスペースのビルド/テスト/クリッピー
  - Swift/Gradle テスト スイート
  - CUDA/Metal ランタイム検証

## 調査結果

### SA-001 高: Torii は機密リクエスト ヘッダーをグローバルにログに記録します影響: リクエスト トレースを配布する展開では、ベアラー/API/オペレーター トークンおよび関連する認証マテリアルがアプリケーション ログに漏洩する可能性があります。

証拠:

- `crates/iroha_torii/src/lib.rs:20752` は `TraceLayer::new_for_http()` を有効にします
- `crates/iroha_torii/src/lib.rs:20753` は `DefaultMakeSpan::default().include_headers(true)` を有効にします
- 機密ヘッダー名は、同じサービス内の他の場所で積極的に使用されています。
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

これが重要な理由:

- `include_headers(true)` は、完全な受信ヘッダー値をトレース スパンに記録します。
- Torii は、`Authorization`、`x-api-token`、`x-iroha-operator-session`、`x-iroha-operator-token`、および `x-forwarded-client-cert` などのヘッダー内の認証マテリアルを受け入れます。
- したがって、ログ シンクの侵害、デバッグ ログの収集、またはサポート バンドルは、資格情報の開示イベントになる可能性があります。

推奨される修復:

- 本番スパンに完全なリクエスト ヘッダーを含めるのをやめます。
- デバッグのためにヘッダーのログ記録が依然として必要な場合は、セキュリティに依存するヘッダーに明示的な編集を追加します。
- データが積極的に許可リストに登録されていない限り、デフォルトでリクエスト/レスポンスのログを秘密が含まれるものとして扱います。

### SA-002 高: パブリック Torii API はサーバー側署名用の生の秘密キーを引き続き受け入れます

影響: クライアントは、サーバーがクライアントに代わって署名できるように、ネットワーク経由で未加工の秘密キーを送信することが推奨され、API、SDK、プロキシ、サーバー メモリ層で不要な秘密漏洩チャネルが作成されます。

証拠：- ガバナンス ルートのドキュメントでは、サーバー側の署名を明示的にアドバタイズします。
  - `crates/iroha_torii/src/gov.rs:495`
- ルート実装は、提供された秘密キーを解析し、サーバー側に署名します。
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK は、`private_key` を JSON 本文にアクティブにシリアル化します。
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

注:

- このパターンは 1 つのルート ファミリに分離されていません。現在のツリーには、ガバナンス、オフライン キャッシュ、サブスクリプション、その他のアプリ向け DTO 全体にわたる同じ利便性モデルが含まれています。
- HTTPS のみのトランスポート チェックは、偶発的な平文トランスポートを減らしますが、サーバー側の機密処理やロギング/メモリ漏洩のリスクは解決しません。

推奨される修復:

- 生の `private_key` データを伝送するすべてのリクエスト DTO を非推奨にします。
- クライアントにローカルで署名し、署名または完全に署名されたトランザクション/エンベロープを送信することを要求します。
- 互換性期間が終了したら、`private_key` の例を OpenAPI/SDK から削除します。

### SA-003 中: 機密キーの導出により、機密シード マテリアルが Torii に送信され、それがエコー バックされます。

影響: 機密キー導出 API は、シード マテリアルを通常のリクエスト/レスポンス ペイロード データに変換し、プロキシ、ミドルウェア、ログ、トレース、クラッシュ レポート、またはクライアントの悪用を通じてシードが漏洩する可能性を高めます。

証拠：- リクエストはシードマテリアルを直接受け入れます。
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- 応答スキーマはシードを 16 進数と Base64 の両方でエコーバックします。
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- ハンドラーは明示的に再エンコードしてシードを返します。
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK はこれを通常のネットワーク メソッドとして公開し、エコーされたシードを応答モデルに保持します。
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

推奨される修復:

- CLI/SDK コードでのローカル キー導出を優先し、リモート導出ルートを完全に削除します。
- ルートを維持する必要がある場合は、応答でシードを決して返さず、すべてのトランスポート ガードおよびテレメトリ/ロギング パスでシードを持つ本体を機密としてマークしてください。

### SA-004 中: SDK トランスポート感度検出には、`private_key` 以外の秘密マテリアルに対する盲点がある

影響: 一部の SDK は、生の `private_key` リクエストに対して HTTPS を強制しますが、その他のセキュリティに依存するリクエスト素材が安全でない HTTP または不一致のホストに送信されることを許可します。

証拠：- Swift は、正規のリクエスト認証ヘッダーを機密情報として扱います。
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- しかし、Swift は依然として `"private_key"` でのみボディ一致します。
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin は `authorization` および `x-api-token` ヘッダーのみを認識し、その後同じ `"private_key"` 本文ヒューリスティックにフォールバックします。
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android にも同じ制限があります。
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java 正規リクエスト署名者は、独自のトランスポート ガードによって機密として分類されていない追加の認証ヘッダーを生成します。
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

推奨される修復:

- ヒューリスティックな本文スキャンを明示的なリクエスト分類に置き換えます。
- 正規認証ヘッダー、シード/パスフレーズ フィールド、署名付き変更ヘッダー、および将来の秘密が含まれるフィールドを、部分文字列の一致ではなく契約によって機密として扱います。
- Swift、Kotlin、Java 全体で機密性ルールを調整します。

### SA-005 中: 添付ファイル「サンドボックス」はサブプロセスに `setrlimit` を加えたものにすぎません影響: 添付ファイルのサニタイザは「サンドボックス化」されていると説明および報告されていますが、実装はリソース制限のある現在のバイナリのフォーク/実行にすぎません。パーサーまたはアーカイブのエクスプロイトは、Torii と同じユーザー、ファイル システム ビュー、およびアンビエント ネットワーク/プロセス権限で実行されます。

証拠:

- 外側のパスは、子を生成した後に結果をサンドボックスとしてマークします。
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- 子はデフォルトで現在の実行可能ファイルになります。
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- サブプロセスは明示的に `AttachmentSanitizerMode::InProcess` に戻ります。
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- 適用される強化は CPU/アドレス空間 `setrlimit` のみです。
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

推奨される修復:

- 実際の OS サンドボックス (たとえば、namespaces/seccomp/landlock/jail スタイルの分離、特権ドロップ、ネットワークなし、制約されたファイルシステム) を実装するか、結果に `sandboxed` というラベルを付けるのをやめてください。
- 真の分離が実現するまで、API、テレメトリ、ドキュメントの「サンドボックス化」ではなく、現在の設計を「サブプロセス分離」として扱います。

### SA-006 中: オプションの P2P TLS/QUIC トランスポートは証明書検証を無効にします影響: `quic` または `p2p_tls` が有効な場合、チャネルは暗号化を提供しますが、リモート エンドポイントを認証しません。パス上のアクティブな攻撃者は依然としてチャネルを中継または終了することができ、通信事業者が TLS/QUIC に関連する通常のセキュリティ期待を打ち破ることができます。

証拠:

- QUIC は、許容的な証明書検証を明示的に文書化します。
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC ベリファイアはサーバー証明書を無条件に受け入れます。
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP トランスポートでも同様のことが行われます。
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

推奨される修復:

- ピア証明書を検証するか、上位層の署名付きハンドシェイクとトランスポート セッションの間に明示的なチャネル バインディングを追加します。
- 現在の動作が意図的なものである場合は、オペレーターが完全な TLS ピア認証と誤解しないように、機能を未認証の暗号化トランスポートとして名前変更または文書化します。

## 推奨される修復順序1. ヘッダー ログを編集または無効にして、SA-001 を直ちに修正します。
2. 未加工の秘密キーが API 境界を越えないように、SA-002 の移行計画を設計して出荷します。
3. リモート機密キー導出ルートを削除または絞り込み、シードを含む物体を機密として分類します。
4. Swift/Kotlin/Java 全体で SDK トランスポートの機密性ルールを調整します。
5. 添付ファイルのサニテーションに実際のサンドボックスが必要か、それとも正直な名前変更/再スコープが必要かを決定します。
6. 事業者が認証済み TLS を期待するトランスポートを有効にする前に、P2P TLS/QUIC 脅威モデルを明確にして強化します。

## 検証メモ

- `cargo check -p iroha_torii --lib --message-format short` が合格しました。
- `cargo check -p iroha_p2p --message-format short` が合格しました。
- サンドボックスの外で実行した後、`cargo deny check advisories bans sources --hide-inclusion-graph` が合格しました。重複バージョンの警告が表示されましたが、`advisories ok, bans ok, sources ok` が報告されました。
- 機密の派生キーセット ルートに対する集中的な Torii テストがこの監査中に開始されましたが、レポートが作成される前に完了しませんでした。いずれにせよ、この発見は直接の情報源検査によって裏付けられます。
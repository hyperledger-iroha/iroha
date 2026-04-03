<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# セキュリティのベストプラクティスレポート

日付: 2026-03-25

## エグゼクティブサマリー

以前の Torii/Soracloud レポートを現在のワークスペースに対して更新しました
コードを作成し、最もリスクの高いサーバー、SDK、および
暗号化/シリアル化サーフェス。この監査では当初、次の 3 つが確認されました。
入力/認証の問題: 重大度が高の 2 つと中程度の重大度が 1 つ。
これら 3 つの検出結果は、修復により現在のツリーで閉じられています。
以下に説明します。追跡輸送と社内派遣の検討を確認
追加で中程度の重大度の問題が 9 件: アウトバウンド P2P ID バインディングが 1 件
ギャップ、アウトバウンド P2P TLS ダウングレードのデフォルトが 1 つ、Torii 信頼境界のバグが 2 つ
Webhook 配信と MCP 内部ディスパッチ、1 つのクロス SDK 機密トランスポート
Swift、Java/Android、Kotlin、および JS クライアントのギャップ、1 つの SoraFS
信頼できるプロキシ/クライアント IP ポリシー ギャップ、1 つの SoraFS ローカル プロキシ
バインド/認証ギャップ、1 つのピア テレメトリ ジオルックアップ平文フォールバック、
および 1 つのオペレータ認証リモート IP フェールオープン ロックアウト/レート キーイング ギャップ。それら
その後の結果も現在のツリーで閉じられます。以前に報告された 4 件の Soracloud の生の秘密鍵に関する調査結果は、
HTTP、内部専用のローカル読み取りプロキシ実行、従量制のパブリック ランタイム
フォールバック、およびリモート IP アタッチメント テナントは現在のコードでは存在しなくなりました。
これらは、更新されたコード参照とともに以下で終了/置き換えられているとマークされています。これは、徹底的なレッドチームの演習ではなく、コード中心の監査のままでした。
外部から到達可能な Torii イングレス パスとリクエスト認証パスを優先しました。
スポットチェックされた IVM、`iroha_crypto`、`norito`、Swift/Android/JS SDK
リクエスト署名ヘルパー、ピア テレメトリ地理パス、および SoraFS
ワークステーション プロキシ ヘルパーと SoraFS ピン/ゲートウェイ クライアント IP ポリシー
表面。そのイングレス/認証、エグレスポリシーからのライブ確認済みの問題はありません。
ピア テレメトリ ジオメトリ、サンプルされた P2P トランスポート デフォルト、MCP ディスパッチ、サンプルされた SDK
トランスポート、オペレーター認証ロックアウト/レート キーイング、SoraFS 信頼できるプロキシ/クライアント IP
このレポートの修正後も、ポリシーまたはローカル プロキシ スライスは残ります。
フォローアップの強化により、フェールクローズされたスタートアップの真理セットも拡張されました。
サンプリングされた IVM CUDA/メタル アクセラレータ パス。その作業では新しいことは確認されませんでした
フェールオープンの問題。サンプリングされたメタル Ed25519
複数の ref10 ドリフトを修正した後、署名パスがこのホストで復元されるようになりました。
Metal/CUDA ポートのポイント: 検証における正のベースポイントの処理、
`d2` 定数、正確な `fe_sq2` リダクション パス、漂遊最終パス
`fe_mul` キャリー ステップと、四肢を許可する術後フィールド正規化が欠落しています
境界はスカラーラダーを横切って漂流します。メタル回帰を重点的にカバーするようになりました
署名パイプラインを有効に保ち、`[true, false]` を検証します。CPU 参照パスに対するアクセラレータ。サンプリングされたスタートアップの真実のセットの現在
また、ライブベクター (`vadd64`、`vand`、`vxor`、`vor`) を直接プローブします。
バックエンドの前に Metal と CUDA の両方でシングルラウンド AES バッチ カーネル
有効なままにしておきます。その後の依存関係スキャンにより、サードパーティのライブ検出結果が 7 件追加されました
バックログに追加されましたが、現在のツリーはアクティブな `tar` を両方とも削除しました。
`xtask` Rust `tar` 依存関係を削除し、
`iroha_crypto` `libsodium-sys-stable` OpenSSL を利用した相互運用テスト
同等品。現在のツリーは、直接の PQ 依存関係も置き換えています。
そのスイープでフラグが立てられ、`soranet_pq`、`iroha_crypto`、および `ivm` を移行
`pqcrypto-dilithium` / `pqcrypto-kyber` から
既存の ML-DSA を維持しながら `pqcrypto-mldsa` / `pqcrypto-mlkem` /
ML-KEM API サーフェス。その後、同じ日の依存関係パスによってワークスペースが固定されました
`reqwest` / `rustls` バージョンからパッチ適用済みパッチ リリースまで、
現在の解決の固定 `0.103.10` 行の `rustls-webpki`。唯一の
残りの依存関係ポリシーの例外は、保守されていない推移的な 2 つのポリシーです。
マクロ クレート (`derivative`、`paste`)。これらは現在、明示的に受け入れられています。
`deny.toml` 安全なアップグレードは存在せず、それらを削除するには必要があるためです。
複数の上流スタックの交換または販売。の残りのアクセルワークは
ミラーリングされた CUDA 修正と拡張された CUDA 真実セットの実行時検証
ライブ CUDA ドライバー サポートを備えたホスト。正確性またはフェールオープンが確認されていない
現在のツリーの問題。

## 高重大度

### SEC-05: アプリの正規リクエスト検証でマルチシグしきい値が回避される (2026 年 3 月 24 日終了)

影響:

- マルチシグ制御されたアカウントの単一のメンバー キーで認証できます。
  しきい値または重み付けを必要とするアプリ向けのリクエスト
  定足数。
- これは、`verify_canonical_request` を信頼するすべてのエンドポイントに影響します。
  Soracloud の署名付きミューテーションのイングレス、コンテンツ アクセス、および署名付きアカウント ZK
  付属テナント。

証拠:

- `verify_canonical_request` はマルチシグ コントローラーをフル メンバーに拡張します
  公開鍵リストを作成し、リクエストを検証する最初の鍵を受け入れます。
  しきい値や累積重みを評価しない署名:
  `crates/iroha_torii/src/app_auth.rs:198-210`。
- 実際のマルチシグ ポリシー モデルは、`threshold` と加重値の両方を伝送します。
  メンバーを特定し、しきい値が合計の重みを超えるポリシーを拒否します。
  `crates/iroha_data_model/src/account/controller.rs:92-95`、
  `crates/iroha_data_model/src/account/controller.rs:163-178`、
  `crates/iroha_data_model/src/account/controller.rs:188-196`。
- ヘルパーは、Soracloud ミューテーションイングレスの認証パス上にあります。
  `crates/iroha_torii/src/lib.rs:2141-2157`、コンテンツ署名付きアカウント アクセス
  `crates/iroha_torii/src/content.rs:359-360`、および接続テナンシー
  `crates/iroha_torii/src/lib.rs:7962-7968`。

これが重要な理由:- リクエストの署名者は、HTTP アドミッションのアカウント権限として扱われます。
  しかし、実装はマルチシグアカウントを「任意の単一のアカウント」にサイレントにダウングレードします。
  メンバーは単独で行動してもよい。」
- これにより、多層防御の HTTP 署名層が認証に変わります。
  マルチシグ保護されたアカウントの場合はバイパスします。

推奨事項:

- マルチシグで制御されたアカウントをアプリ認証層で拒否するか、
  適切な監視形式が存在するか、HTTP リクエストが受信できるようにプロトコルを拡張します。
  しきい値と条件を満たす完全なマルチシグ監視セットを伝送および検証します。
  重量。
- Soracloud ミューテーション ミドルウェア、コンテンツ認証、ZK をカバーするリグレッションを追加
  しきい値を下回るマルチシグ署名の添付ファイル。

修復ステータス:

- 現在のコードでは、マルチシグで制御されているアカウントをフェールクローズすることでクローズされます。
  `crates/iroha_torii/src/app_auth.rs`。
- 検証者は、「単一のメンバーが署名できる」セマンティクスを受け入れなくなりました。
  マルチシグHTTP認証。マルチシグリクエストは、
  しきい値を満たす証人形式が存在します。
- 回帰カバレッジには、専用のマルチシグ拒否ケースが含まれるようになりました。
  `crates/iroha_torii/src/app_auth.rs`。

## 高重大度

### SEC-06: アプリの正規リクエスト署名は無期限に再生可能でした (2026 年 3 月 24 日に終了)

影響:- 署名されたメッセージには何も含まれていないため、キャプチャされた有効なリクエストを再実行できます。
  タイムスタンプ、ノンス、有効期限、またはリプレイ キャッシュ。
- これにより、状態を変更する Soracloud ミューテーション リクエストと再発行を繰り返すことができます。
  元のクライアントからかなり時間が経過した後、アカウントにバインドされたコンテンツ/添付ファイルの操作
  彼らを意図したのです。

証拠:

- Torii は、アプリの正規リクエストを次のように定義します
  `METHOD + path + sorted query + body hash` で
  `crates/iroha_torii/src/app_auth.rs:1-17` および
  `crates/iroha_torii/src/app_auth.rs:74-89`。
- ベリファイアは `X-Iroha-Account` および `X-Iroha-Signature` のみを受け入れ、
  鮮度を強制したり、リプレイ キャッシュを維持したりしません。
  `crates/iroha_torii/src/app_auth.rs:137-218`。
- JS、Swift、および Android SDK ヘルパーは、同じリプレイ傾向のヘッダーを生成します
  nonce/timestamp フィールドのないペア:
  `javascript/iroha_js/src/canonicalRequest.js:50-82`、
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`、および
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`。
- Torii のオペレーター署名パスは、より強力なパターンをすでに使用しています。
  アプリ側のパスが欠落しています: タイムスタンプ、ノンス、およびリプレイ キャッシュ
  `crates/iroha_torii/src/operator_signatures.rs:1-21` および
  `crates/iroha_torii/src/operator_signatures.rs:266-294`。

これが重要な理由:

- HTTPS だけでは、リバース プロキシ、デバッグ ロガー、
  侵害されたクライアント ホスト、または有効なリクエストを記録できる仲介者。
- 同じスキームがすべての主要なクライアント SDK に実装されているため、リプレイ
  弱点はサーバーのみではなく全体的なものです。

おすすめ：- アプリ認証リクエストに署名済みの鮮度マテリアルを追加します（少なくともタイムスタンプ）
  および nonce を使用し、期限付きリプレイ キャッシュを使用して古いタプルまたは再利用されたタプルを拒否します。
- Torii と SDK ができるように、アプリの正規リクエスト形式を明示的にバージョン管理します。
  古い 2 ヘッダー スキームを安全に廃止します。
- Soracloud のミューテーション、コンテンツのリプレイ拒否を証明する回帰を追加
  アクセスと添付ファイルの CRUD。

修復ステータス:

- 現在のコードでは閉じられています。 Torii には 4 ヘッダー スキームが必要になりました
  (`X-Iroha-Account`、`X-Iroha-Signature`、`X-Iroha-Timestamp-Ms`、
  `X-Iroha-Nonce`) に署名/検証します
  `METHOD + path + sorted query + body hash + timestamp + nonce` で
  `crates/iroha_torii/src/app_auth.rs`。
- 鮮度検証では、制限されたクロック スキュー ウィンドウが適用され、検証が行われるようになりました。
  nonce の形状を変更し、メモリ内リプレイ キャッシュで再利用された nonce を拒否します。
  ノブは `crates/iroha_config/src/parameters/{defaults,actual,user}.rs` を通じて表示されます。
- JS、Swift、Android ヘルパーは、同じ 4 ヘッダー形式を出力するようになりました。
  `javascript/iroha_js/src/canonicalRequest.js`、
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`、および
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`。
- リグレッション カバレッジには、ポジティブな署名の検証とリプレイが含まれるようになりました。
  タイムスタンプが古い場合と鮮度が欠落している場合の拒否ケース
  `crates/iroha_torii/src/app_auth.rs`。

## 中程度の重大度

### SEC-07: mTLS 施行はスプーフィング可能な転送ヘッダーを信頼しました (2026 年 3 月 24 日終了)

影響:- `require_mtls` に依存するデプロイメントは、Torii が直接の場合はバイパスできます。
  到達可能か、フロント プロキシがクライアント指定のデータを削除しません
  `x-forwarded-client-cert`。
- この問題は構成に依存しますが、トリガーされると要求された問題になります。
  クライアント証明書の要件をプレーンヘッダーチェックに組み込みます。

証拠:

- Norito-RPC ゲーティングは、呼び出しにより `require_mtls` を強制します
  `norito_rpc_mtls_present`、次のことのみをチェックします。
  `x-forwarded-client-cert` は存在し、空ではありません:
  `crates/iroha_torii/src/lib.rs:1897-1926`。
- オペレーター認証ブートストラップ/ログイン フローは `check_common` を呼び出し、拒否のみを行います。
  `mtls_present(headers)` が false の場合:
  `crates/iroha_torii/src/operator_auth.rs:562-570`。
- `mtls_present` も空ではない `x-forwarded-client-cert` チェックインにすぎません
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`。
- これらのオペレーター認証ハンドラーは引き続きルートとして公開されます。
  `crates/iroha_torii/src/lib.rs:16658-16672`。

これが重要な理由:

- 転送ヘッダー規則は、Torii がヘッダーの背後にある場合にのみ信頼できます。
  ヘッダーを削除して書き換える強化されたプロキシ。コードが検証されない
  その展開の前提自体。
- リバースプロキシの健全性に静かに依存するセキュリティ制御は簡単に実行できます。
  ステージング、カナリア、またはインシデント対応ルーティングの変更中に設定を誤る。

おすすめ：- 可能な場合は、トランスポート状態を直接適用することを好みます。プロキシが必要な場合は、
  使用する場合は、認証されたプロキシから Torii チャネルを信頼し、許可リストが必要です
  または、生のヘッダーの存在の代わりに、そのプロキシからの署名された証明書。
- 直接公開された Torii リスナーでは `require_mtls` が安全でないことを文書化します。
- Norito-RPC での偽造 `x-forwarded-client-cert` 入力に対するネガティブ テストを追加
  およびオペレーター認証ブートストラップ ルート。

修復ステータス:

- 転送ヘッダーの信頼を構成されたプロキシにバインドすることにより、現在のコードで閉じられます
  生のヘッダーのみが存在するのではなく、CIDR が存在します。
- `crates/iroha_torii/src/limits.rs` は共有を提供するようになりました。
  `has_trusted_forwarded_header(...)` ゲートと Norito-RPC の両方
  (`crates/iroha_torii/src/lib.rs`) およびオペレーター認証
  (`crates/iroha_torii/src/operator_auth.rs`) 呼び出し元の TCP ピアで使用します
  住所。
- `iroha_config` は、両方の `mtls_trusted_proxy_cidrs` を公開するようになりました。
  オペレーター認証および Norito-RPC。デフォルトはループバックのみです。
- 回帰カバレッジは、からの偽造 `x-forwarded-client-cert` 入力を拒否するようになりました。
  オペレーター認証と共有制限ヘルパーの両方で信頼できないリモート。

## 中程度の重大度

### SEC-08: アウトバウンド P2P ダイヤルは、認証されたキーを目的のピア ID にバインドしませんでした (2026 年 3 月 25 日終了)

影響:- ピア `X` へのアウトバウンド ダイヤルは、そのキーを持つ他のピア `Y` として完了できます。
  アプリ層ハンドシェイクに正常に署名しました。
  「この接続上のキー」を認証しましたが、そのキーが
  ネットワーク アクターが到達することを意図したピア ID。
- 許可されたオーバーレイでは、後のトポロジ/許可リスト チェックでも依然として
  キーが間違っているため、これは主に置換/到達可能性のバグでした
  直接コンセンサス偽装バグよりも。パブリックオーバーレイでは、
  侵害されたアドレス、DNS 応答、またはリレー エンドポイントを別のアドレスに置き換える
  発信ダイヤルのオブザーバー ID。

証拠:

- 送信ピアの状態は、目的の `peer_id` を
  `crates/iroha_p2p/src/peer.rs:5153-5179`、ただし古いハンドシェイク フロー
  署名検証の前にその値を削除しました。
- `GetKey::read_their_public_key` は署名されたハンドシェイク ペイロードを検証し、
  その後、すぐにアドバタイズされたリモート公開鍵から `Peer` を構築しました
  `crates/iroha_p2p/src/peer.rs:6266-6355` では、
  `peer_id` は元々 `connecting(...)` に提供されていました。
- 同じトランスポート スタックが TLS / QUIC 証明書を明示的に無効にする
  `crates/iroha_p2p/src/transport.rs` で P2P を検証するため、バインド
  目的のピア ID に対するアプリケーション層で認証されたキーが重要です
  アウトバウンド接続での ID チェック。

これが重要な理由:- 意図的にピア認証をトランスポート上にプッシュする設計
  ハンドシェイク キー チェックを唯一の耐久性のある ID バインディングにするレイヤー
  発信ダイヤルで。
- そのチェックがなければ、ネットワーク層は黙って「正常に処理」する可能性があります。
  一部のピアを認証しました」は「ダイヤルしたピアに到達しました」と同等です。
  これは保証が弱く、トポロジ/評判の状態を歪める可能性があります。

推奨事項:

- 意図したアウトバウンド `peer_id` を署名付きハンドシェイク ステージを通じて伝送し、
  検証されたリモートキーが一致しない場合はフェールクローズされます。
- からの有効に署名されたハンドシェイクを証明する焦点を絞った回帰を維持します。
  間違ったキーは拒否されますが、通常の署名付きハンドシェイクは引き続き成功します。

修復ステータス:

- 現在のコードでは閉じられています。 `ConnectedTo` と現在のダウンストリーム ハンドシェイクの状態
  予想されるアウトバウンド `PeerId` を伝送し、
  `GetKey::read_their_public_key` は、一致しない認証キーを拒否します。
  `crates/iroha_p2p/src/peer.rs`の`HandshakePeerMismatch`。
- 焦点を絞った回帰カバレッジには以下が含まれるようになりました
  `outgoing_handshake_rejects_unexpected_peer_identity` と既存の
  正の `handshake_v1_defaults_to_trust_gossip` パス
  `crates/iroha_p2p/src/peer.rs`。

### SEC-09: HTTPS/WSS Webhook 配信により、接続時に精査されたホスト名が再解決されました (2026 年 3 月 25 日終了)

影響:- Webhook に対する安全な Webhook 配信検証済みの宛先 DNS 応答
  egress ポリシーを適用しましたが、それらの精査されたアドレスは破棄され、クライアントは
  スタックは、実際の HTTPS または WSS 接続中にホスト名を再度解決します。
- 攻撃者が検証から接続までの間に DNS に影響を与える可能性があります。
  以前に許可されたホスト名をブロックされたプライベートまたはホスト名に再バインドする可能性があります。
  オペレーター専用の宛先を指定し、CIDR ベースの Webhook ガードをバイパスします。

証拠:

- 出力ガードは、候補となる宛先アドレスを解決し、フィルタリングします。
  `crates/iroha_torii/src/webhook.rs:1746-1829`、および安全な配信パス
  これらの精査されたアドレス リストを HTTPS / WSS ヘルパーに渡します。
- 古い HTTPS ヘルパーは、元の URL に対して汎用クライアントを構築しました。
  ホストは `crates/iroha_torii/src/webhook.rs` にあり、接続をバインドしませんでした
  これは、DNS 解決が内部で再度行われたことを意味します。
  HTTP クライアント。
- 古い WSS ヘルパーも同様に `tokio_tungstenite::connect_async(url)` と呼ばれました
  元のホスト名に対して、代わりにホストも再解決されました。
  すでに承認されているアドレスを再利用します。

これが重要な理由:

- 宛先許可リストは、チェックされたアドレスがそのアドレスである場合にのみ機能します。
  クライアントは実際に接続します。
- ポリシー承認後の再解決により、DNS 再バインド/TOCTOU ギャップが作成されます。
  SSRF スタイルの封じ込めのためにオペレータが信頼する可能性が高いパス。

おすすめ：- 検証された DNS 回答を保持しながら、実際の HTTPS 接続パスにピン留めします。
  SNI/証明書検証用の元のホスト名。
- WSS の場合、TCP ソケットを検証済みのアドレスに直接接続し、TLS を実行します。
  ホスト名ベースの呼び出しの代わりに、そのストリーム上で WebSocket ハンドシェイクを行う
  便利なコネクタ。

修復ステータス:

- 現在のコードでは閉じられています。 `crates/iroha_torii/src/webhook.rs` が派生するようになりました
  `https_delivery_dns_override(...)` および
  検証済みのアドレス セットからの `websocket_pinned_connect_addr(...)`。
- HTTPS 配信では `reqwest::Client::builder().resolve_to_addrs(...)` が使用されるようになりました
  そのため、TCP 接続が確立されている間、元のホスト名は TLS に表示されたままになります。
  すでに承認されたアドレスに固定されます。
- WSS 配信は、未加工の `TcpStream` を精査されたアドレスに開き、実行するようになりました。
  そのストリーム上の `tokio_tungstenite::client_async_tls_with_config(...)`、
  これにより、ポリシー検証後の 2 回目の DNS ルックアップが回避されます。
- 回帰カバレッジには次のものが含まれます
  `https_delivery_dns_override_pins_vetted_domain_addresses`、
  `https_delivery_dns_override_skips_ip_literals`、および
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` で
  `crates/iroha_torii/src/webhook.rs`。

### SEC-10: MCP 内部ルート ディスパッチ スタンプ付きループバックと継承された許可リスト権限 (2026 年 3 月 25 日終了)

影響:- Torii MCP が有効になっている場合、内部ツール ディスパッチはすべてのリクエストを次のように書き換えました。
  実際の呼び出し元に関係なくループバックします。呼び出し元の CIDR を信頼するルート
  したがって、特権またはスロットル バイパスでは、MCP トラフィックが次のように認識される可能性があります。
  `127.0.0.1`。
- MCP はデフォルトで無効になっており、
  影響を受けるルートは依然としてホワイトリストまたは同様のループバック信頼に依存しています
  しかし、ポリシーが確立されると、MCP は権限昇格ブリッジに変わりました。
  ノブも一緒に有効になりました。

証拠:

- 以前に挿入された `crates/iroha_torii/src/mcp.rs` 内の `dispatch_route(...)`
  `x-iroha-remote-addr: 127.0.0.1` および合成ループバック `ConnectInfo`
  内部的にディスパッチされたすべてのリクエスト。
- `iroha.parameters.get` は読み取り専用モードで MCP 表面に公開されており、
  `/v1/parameters` は、発信者 IP が
  `crates/iroha_torii/src/lib.rs:5879-5888` に設定された許可リスト。
- `apply_extra_headers(...)` は、からの任意の `headers` エントリも受け入れました。
  MCP の呼び出し元であるため、次のような内部信頼ヘッダーが予約されています。
  `x-iroha-remote-addr` と `x-forwarded-client-cert` は明示的にはありませんでした
  保護されました。

これが重要な理由:- 内部ブリッジ層は、元の信頼境界を保持する必要があります。交換中
  ループバックを備えた実際の呼び出し元は、すべての MCP 呼び出し元を効果的に扱います。
  リクエストがブリッジを通過すると、内部クライアントに送信されます。
- 外部から見える MCP プロファイルは依然として見えるため、バグは微妙です
  読み取り専用ですが、内部 HTTP ルートはより特権のあるオリジンを参照します。

推奨事項:

- 外側の `/v1/mcp` リクエストがすでに受信した発信者 IP を保持します。
  Torii のリモート アドレス ミドルウェアから `ConnectInfo` を合成します
  ループバックの代わりにその値を使用します。
- `x-iroha-remote-addr` や `x-iroha-remote-addr` などの入力専用の信頼ヘッダーを処理します。
  `x-forwarded-client-cert` は予約された内部ヘッダーであるため、MCP 呼び出し元はアクセスできません
  `headers` 引数を通じてそれらを密輸またはオーバーライドします。

修復ステータス:

- 現在のコードでは閉じられています。 `crates/iroha_torii/src/mcp.rs` は、
  外部リクエストの注入から内部的にディスパッチされたリモート IP
  `x-iroha-remote-addr` ヘッダーとその実体から `ConnectInfo` を合成します
  ループバックの代わりに発信者 IP を使用します。
- `apply_extra_headers(...)` は、`x-iroha-remote-addr` と `x-iroha-remote-addr` の両方をドロップするようになりました。
  `x-forwarded-client-cert` は予約された内部ヘッダーであるため、MCP 呼び出し元
  ツールの引数を介してループバック/イングレスプロキシの信頼を偽装することはできません。
- 回帰カバレッジには次のものが含まれます
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`、および
  `apply_extra_headers_blocks_reserved_internal_headers` で
  `crates/iroha_torii/src/mcp.rs`。### SEC-11: SDK クライアントは、安全でないトランスポートまたはクロスホスト トランスポートを介した機密リクエスト マテリアルを許可しました (2026 年 3 月 25 日終了)

影響:

- サンプリングされた Swift、Java/Android、Kotlin、および JS クライアントは、
  一貫して、機密性の高いすべてのリクエスト シェイプをトランスポートに依存するものとして扱います。
  ヘルパーによっては、呼び出し元がベアラー/API トークン ヘッダーを生のまま送信する可能性があります。
  `private_key*` JSON フィールド、またはアプリの正規認証署名マテリアル
  プレーン `http` / `ws`、またはクロスホスト絶対 URL オーバーライドを使用します。
- 特に JS クライアントでは、`canonicalAuth` ヘッダーが次の後に追加されました。
  `_request(...)` は転送チェックを終了し、本体のみの `private_key`
  JSON は機密トランスポートとしてまったくカウントされませんでした。

証拠：- スウィフトはガードを集中化するようになりました。
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` から適用します
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`、
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`、および
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`;このパスの前に
  これらのヘルパーは単一の交通政策ゲートを共有していませんでした。
- Java/Android では同じポリシーが集中管理されるようになりました。
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  `NoritoRpcClient.java`、`ToriiRequestBuilder.java`、から適用します。
  `OfflineToriiClient.java`、`SubscriptionToriiClient.java`、
  `stream/ToriiEventStreamClient.java`、および
  `websocket/ToriiWebSocketClient.java`。
- Kotlin はそのポリシーをミラーリングするようになりました。
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  そして、一致する JVM client/request-builder/event-stream / からそれを適用します。
  Webソケットの表面。
- JS `ToriiClient._request(...)` は `canonicalAuth` と JSON ボディを処理するようになりました
  `private_key*` フィールドを機密トランスポート マテリアルとして含む
  `javascript/iroha_js/src/toriiClient.js`、およびテレメトリ イベントの形状
  `javascript/iroha_js/index.d.ts` は `hasSensitiveBody` を記録するようになりました /
  `allowInsecure` が使用される場合は、`hasCanonicalAuth`。

これが重要な理由:

- モバイル、ブラウザ、およびローカル開発ヘルパーは、多くの場合、変更可能な開発/ステージングを指します
  ベース URL。クライアントが機密リクエストを設定済みのリクエストにピン留めしない場合
  スキーム/ホスト、便利な絶対 URL オーバーライド、またはプレーン HTTP ベースを使用できます。
  SDK を秘密漏洩または署名付きリクエストのダウングレード パスに変えます。
- リスクは無記名トークンよりも広いです。生の `private_key` JSON および新鮮
  canonical-auth 署名はネットワーク上でもセキュリティに敏感であり、
  トランスポート ポリシーを黙ってバイパスしないでください。

おすすめ：- 各 SDK でトランスポート検証を一元化し、ネットワーク I/O の前に適用します。
  すべての機密性の高いリクエスト形式: 認証ヘッダー、アプリの正規認証署名、
  および生の `private_key*` JSON。
- `allowInsecure` を明示的なローカル/開発エスケープ ハッチとしてのみ保持し、出力します
  発信者がオプトインしたときのテレメトリ。
- 共有リクエスト ビルダーのみではなく、それに焦点を当てた回帰を追加します。
  より高レベルの便利なメソッドなので、将来のヘルパーは同じガードを継承します。

修復ステータス:- 現在のコードでは閉じられています。サンプルされた Swift、Java/Android、Kotlin、および JS
  クライアントは、機密情報の安全でないトランスポートまたはクロスホストトランスポートを拒否するようになりました。
  呼び出し元が文書化された開発専用をオプトインしない限り、上記のシェイプをリクエストします。
  不安定なモード。
- Swift に焦点を当てたリグレッションは、安全でない Norito-RPC 認証ヘッダーをカバーするようになりました。
  insecure WebSocket トランスポートと raw-`private_key` Torii リクエストを接続します
  身体。
- Kotlin に焦点を当てたリグレッションは、安全でない Norito-RPC 認証ヘッダーをカバーするようになりました。
  オフライン/サブスクリプション `private_key` ボディ、SSE 認証ヘッダー、および WebSocket
  認証ヘッダー。
- Java/Android に焦点を当てたリグレッションが、安全でない Norito-RPC 認証をカバーするようになりました。
  ヘッダー、オフライン/サブスクリプション `private_key` ボディ、SSE 認証ヘッダー、および
  共有 Gradle ハーネスを介した websocket 認証ヘッダー。
- JS に焦点を当てたリグレッションは、安全でない問題やクロスホストをカバーするようになりました。
  `private_key`-body リクエストと安全でない `canonicalAuth` リクエスト
  `javascript/iroha_js/test/transportSecurity.test.js`、一方
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` は正の結果を実行するようになりました
  安全なベース URL 上の canonical-auth パス。

### SEC-12: SoraFS ローカル QUIC プロキシはクライアント認証なしで非ループバック バインドを受け入れました (2026 年 3 月 25 日終了)

影響:- `LocalQuicProxyConfig.bind_addr` は以前は `0.0.0.0` に設定できました。
  LAN IP、または「ローカル」を公開するその他の非ループバック アドレス
  リモートから到達可能な QUIC リスナーとしてのワークステーション プロキシ。
- そのリスナーはクライアントを認証しませんでした。到達可能なピアであれば、
  QUIC/TLS セッションを完了し、バージョンが一致したハンドシェイクを送信することができます。
  次に、`tcp`、`norito`、`car`、または `kaigi` ストリームを開きます。
  ブリッジモードが設定されました。
- `bridge` モードでは、オペレータの設定ミスがリモート TCP に影響しました。
  オペレータ ワークステーション上のリレーおよびローカル ファイル ストリーミング サーフェス。

証拠:

- `LocalQuicProxyConfig::parsed_bind_addr(...)` で
  `crates/sorafs_orchestrator/src/proxy.rs` は以前はソケットのみを解析していました
  アドレスを指定し、非ループバック インターフェイスを拒否しませんでした。
- 同じファイル内の `spawn_local_quic_proxy(...)` は、QUIC サーバーを起動します。
  自己署名証明書と `.with_no_client_auth()`。
- `handle_connection(...)` は、`ProxyHandshakeV1` を持つクライアントを受け入れました
  バージョンは、サポートされている単一のプロトコルのバージョンと一致し、
  アプリケーションストリームループ。
- `handle_tcp_stream(...)` は、経由で任意の `authority` 値をダイヤルします。
  `TcpStream::connect(...)`、`handle_norito_stream(...)`、
  `handle_car_stream(...)` および `handle_kaigi_stream(...)` ストリーム ローカル ファイル
  設定されたスプール/キャッシュ ディレクトリから。

これが重要な理由:- 自己署名証明書は、クライアントが次の場合にのみサーバー ID を保護します。
  それを検証することを選択します。クライアントは認証されません。一度プロキシ
  オフループバックで到達可能であり、ハンドシェイク パスはバージョンのみになりました
  入学。
- API とドキュメントでは、このヘルパーをローカル ワークステーション プロキシとして説明しています。
  ブラウザと SDK の統合により、リモートからアクセス可能なバインド アドレスを許可するようになりました。
  信頼境界の不一致であり、意図されたリモート サービス モードではありません。

推奨事項:

- 非ループバック `bind_addr` でフェールクローズされるため、現在のヘルパーを実行できません
  ローカルワークステーションを超えて公開されます。
- リモート プロキシの公開が製品要件になった場合は、
  ではなく、明示的なクライアント認証/機能許可を最初に行います。
  バインドガードを緩める。

修復ステータス:

- 現在のコードでは閉じられています。現在 `crates/sorafs_orchestrator/src/proxy.rs`
  非ループバック バインド アドレスを `ProxyError::BindAddressNotLoopback` で拒否します
  QUIC リスナーが開始する前。
- 設定フィールドのドキュメント
  `docs/source/sorafs/developer/orchestrator.md` および
  `docs/portal/docs/sorafs/orchestrator-config.md` がドキュメントになりました
  `bind_addr` はループバック専用です。
- 回帰カバレッジには次のものが含まれます
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` と既存の
  ローカルブリッジ検査で陽性反応が出た
  `proxy::tests::tcp_stream_bridge_transfers_payload` で
  `crates/sorafs_orchestrator/src/proxy.rs`。

### SEC-13: アウトバウンド P2P TLS-over-TCP はデフォルトでサイレントにプレーンテキストにダウングレードされました (2026 年 3 月 25 日終了)

影響:- Enabling `network.tls_enabled=true` did not actually enforce TLS-only
  outbound transport unless operators also discovered and set
  `tls_fallback_to_plain=false`。
- Any TLS handshake failure or timeout on the outbound path therefore
  downgraded the dial to plaintext TCP by default, which removed transport
  confidentiality and integrity against on-path attackers or misbehaving
  ミドルボックス。
- The signed application handshake still authenticated the peer identity, so
  this was a transport-policy downgrade rather than a peer-spoofing bypass.

証拠:

- `tls_fallback_to_plain` はデフォルトで `true` になりました
  `crates/iroha_config/src/parameters/user.rs`、つまりフォールバックがアクティブでした
  unless operators explicitly overrode it in config.
- `crates/iroha_p2p/src/peer.rs` の `Connecting::connect_tcp(...)` は、
  TLS dial whenever `tls_enabled` is set, but on TLS errors or timeouts it
  logs a warning and falls back to plaintext TCP whenever
  `tls_fallback_to_plain` が有効になります。
- The operator-facing sample config in `crates/iroha_kagami/src/wizard.rs` and
  the public P2P transport docs in `docs/source/p2p*.md` also advertised
  デフォルトの動作として平文フォールバック。

これが重要な理由:- オペレーターが TLS をオンにすると、より安全な期待はフェイルクローズされます。
  セッションを確立できません。ダイヤルは静かにではなく失敗するはずです
  輸送時の保護を解除します。
- ダウングレードをデフォルトでオンのままにすると、デプロイメントは次のような影響を受けやすくなります。
  ネットワーク パスの異常、プロキシの干渉、およびアクティブなハンドシェイクの中断
  ロールアウト中に見落としがちな方法です。

推奨事項:

- プレーンテキストのフォールバックを明示的な互換性ノブとして保持しますが、デフォルトでは
  `false` したがって、`network.tls_enabled=true` は、オペレータが選択しない限り TLS のみを意味します
  ダウングレード動作に移行します。

修復ステータス:

- 現在のコードでは閉じられています。現在 `crates/iroha_config/src/parameters/user.rs`
  デフォルトは `tls_fallback_to_plain` から `false` です。
- デフォルト設定フィクスチャのスナップショット、Kagami サンプル設定、およびデフォルトのような
  P2P/Torii テスト ヘルパーは、強化されたランタイムのデフォルトを反映するようになりました。
- 複製された `docs/source/p2p*.md` ドキュメントでは、平文フォールバックについて次のように説明されています。
  出荷時のデフォルトではなく明示的なオプトイン。

### SEC-14: ピア テレメトリ地理ルックアップは、プレーンテキストのサードパーティ HTTP にサイレント フォールバックします (2026 年 3 月 25 日終了)

影響:- 明示的なエンドポイントなしで `torii.peer_geo.enabled=true` を有効にすると、
  Torii: ピアのホスト名を組み込みの平文に送信します
  `http://ip-api.com/json/...` サービス。
- ピア テレメトリ ターゲットが認証されていないサードパーティの HTTP に漏洩したこと
  依存関係を遮断し、パス上の攻撃者または侵害されたエンドポイントのフィードを偽造させます。
  場所のメタデータを Torii に戻します。
- この機能はオプトインでしたが、公開テレメトリ ドキュメントとサンプル構成は
  組み込みのデフォルトをアドバタイズしたため、デプロイメント パターンが安全ではなくなりました
  おそらく、オペレータがピア地理検索を有効にしていたと考えられます。

証拠:

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` は以前に定義されました
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` および
  `construct_geo_query(...)` は常にそのデフォルトを使用しました
  `GeoLookupConfig.endpoint` は `None` でした。
- ピア テレメトリ モニターは常に `collect_geo(...)` を生成します
  `geo_config.enabled` は true
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`、つまり平文
  フォールバックは、テスト専用のコードではなく、出荷されたランタイム コードで到達可能でした。
- 設定のデフォルトは `crates/iroha_config/src/parameters/defaults.rs` であり、
  `crates/iroha_config/src/parameters/user.rs` は `endpoint` を設定しないままにし、
  `docs/source/telemetry*.md` プラスの重複したテレメトリ ドキュメント
  `docs/source/references/peer.template.toml` は明示的に文書化しました。
  オペレーターが機能を有効にした場合の組み込みフォールバック。

これが重要な理由:- ピア テレメトリは、プレーンテキスト HTTP 経由でピア ホスト名をサイレントに出力しないでください。
  オペレーターが利便性フラグをオンにすると、サードパーティのサービスに移行します。
- 隠れた安全でないデフォルトも変更レビューを台無しにします。オペレーターは次のようなことができます。
  外部を導入していることに気づかずに地理検索を有効にする
  サードパーティのメタデータの開示と未認証の応答の処理。

推奨事項:

- デフォルトの組み込み地理エンドポイントを削除します。
- ピア地理検索が有効になっている場合、明示的な HTTPS エンドポイントが必要です。
  それ以外の場合は検索をスキップします。
- 不足しているエンドポイントまたは非 HTTPS エンドポイントがフェールクローズされたことを証明する集中的な回帰を維持します。

修復ステータス:

- 現在のコードでは閉じられています。 `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  欠落しているエンドポイントを `MissingEndpoint` で拒否し、非 HTTPS を拒否するようになりました。
  エンドポイントを `InsecureEndpoint` に設定し、代わりにピア地理検索をスキップします。
  静かに平文の組み込みサービスにフォールバックします。
- `crates/iroha_config/src/parameters/user.rs` は暗黙的な
  解析時のエンドポイントなので、設定されていない設定状態はすべてのエンドポイントで明示的に維持されます。
  実行時検証の方法。
- 複製されたテレメトリ ドキュメントと正規のサンプル構成
  `docs/source/references/peer.template.toml` は次のように述べています
  `torii.peer_geo.endpoint` は、HTTPS を使用して明示的に構成する必要があります。
  機能が有効になっています。
- 回帰カバレッジには次のものが含まれます
  `construct_geo_query_requires_explicit_endpoint`、
  `construct_geo_query_rejects_non_https_endpoint`、
  `collect_geo_requires_explicit_endpoint_when_enabled`、および
  `collect_geo_rejects_non_https_endpoint` で
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`。### SEC-15: SoraFS ピンとゲートウェイ ポリシーに信頼できるプロキシ対応クライアント IP 解決が欠落していました (2026 年 3 月 25 日終了)

影響:

- リバースプロキシされた Torii 展開では、以前は個別の SoraFS が崩壊する可能性がありました
  ストレージ ピン CIDR を評価する際のプロキシ ソケット アドレスへの呼び出し元
  ホワイトリスト、クライアントごとのストレージピンスロットル、およびゲートウェイクライアント
  指紋。
- `/v1/sorafs/storage/pin` および SoraFS に対する不正行為の制御が弱体化されました。
  ゲートウェイ ダウンロード サーフェスは、一般的なリバース プロキシ トポロジで作成されます。
  複数のクライアントが 1 つのバケットまたは 1 つの許可リスト ID を共有します。
- デフォルトのルーターは依然として内部リモート メタデータを挿入するため、これは問題ではありませんでした。
  新たな未認証のイングレスバイパスですが、それは実際の信頼境界ギャップでした
  プロキシ対応の展開とハンドラー境界の過剰信頼の場合
  内部リモート IP ヘッダー。

証拠：- `crates/iroha_config/src/parameters/user.rs` および
  `crates/iroha_config/src/parameters/actual.rs` には以前は一般的な機能がありませんでした
  `torii.transport.trusted_proxy_cidrs` ノブ、プロキシ対応の正規クライアント
  IP 解決は、一般的な Torii 入力境界では構成できませんでした。
- `inject_remote_addr_header(...)` の `crates/iroha_torii/src/lib.rs`
  以前に内部 `x-iroha-remote-addr` ヘッダーを上書きしました
  `ConnectInfo` のみ。信頼できる転送されたクライアント IP メタデータがドロップされました。
  本物のリバースプロキシ。
- `PinSubmissionPolicy::enforce(...)` で
  `crates/iroha_torii/src/sorafs/pin.rs` および
  `gateway_client_fingerprint(...)` の `crates/iroha_torii/src/sorafs/api.rs`
  信頼されたプロキシ対応の正規 IP 解決ステップを共有しませんでした。
  ハンドラー境界。
- `crates/iroha_torii/src/sorafs/pin.rs` のストレージ ピン スロットリングもキー化
  トークンが存在する場合は必ずベアラー トークンのみを使用します。これは複数のことを意味します。
  1 つの有効な PIN トークンを共有するプロキシされたクライアントは同じレートに強制されました
  クライアント IP が識別された後でもバケットに保存されます。

これが重要な理由:

- リバース プロキシは、Torii の通常の展開パターンです。ランタイムの場合
  信頼できるプロキシと信頼できない呼び出し元、IP を一貫して区別できない
  ホワイトリストとクライアントごとのスロットルは、オペレーターが考えている意味を持たなくなる
  意味。
- SoraFS ピンとゲートウェイ パスは明らかに不正行為に敏感なサーフェスであるため、
  発信者をプロキシ IP にまとめたり、古い転送を過剰に信頼したりする
  メタデータは、ベースルートがまだ残っている場合でも運用上重要です
  別途入場が必要です。

おすすめ：- 一般的な Torii `trusted_proxy_cidrs` 構成サーフェスを追加し、
  `ConnectInfo` からの正規クライアント IP 1 回と既存の転送済み IP
  ヘッダーは、ソケット ピアがその許可リストに含まれている場合にのみ使用されます。
- 代わりに、SoraFS ハンドラー パス内の正規 IP 解決を再利用します。
  内部ヘッダーを盲目的に信頼します。
- トークンと正規クライアント IP によるスコープ共有トークン ストレージ ピン スロットル
  両方が存在する場合。

修復ステータス:

- 現在のコードでは閉じられています。 `crates/iroha_config/src/parameters/defaults.rs`、
  `crates/iroha_config/src/parameters/user.rs`、および
  `crates/iroha_config/src/parameters/actual.rs` が公開されるようになりました
  `torii.transport.trusted_proxy_cidrs`、デフォルトでは空のリストになります。
- `crates/iroha_torii/src/lib.rs` は正規のクライアント IP を解決するようになりました。
  イングレスミドルウェア内の `limits::ingress_remote_ip(...)` と書き換え
  内部 `x-iroha-remote-addr` ヘッダーは信頼できるプロキシからのみ送信されます。
- `crates/iroha_torii/src/sorafs/pin.rs` および
  `crates/iroha_torii/src/sorafs/api.rs` が正規のクライアント IP を解決するようになりました
  storage-pin のハンドラー境界での `state.trusted_proxy_nets` に対する
  ポリシーおよびゲートウェイクライアントのフィンガープリンティングのため、直接ハンドラーパスは使用できません。
  転送された古い IP メタデータを過剰に信頼します。
- ストレージ ピン スロットリングは、共有ベアラー トークンを「トークン + 正規化」によってキー化するようになりました。
  クライアント IP ` 両方が存在する場合、共有用にクライアントごとのバケットを保持します
  ピントークン。
- 回帰カバレッジには次のものが含まれます
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`、
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`、
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`、
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`、
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`、および
  構成フィクスチャ
  `torii_transport_trusted_proxy_cidrs_default_to_empty`。### SEC-16: 注入されたリモート IP ヘッダーが欠落している場合、オペレーター認証ロックアウトとレート制限キーイングは共有匿名バケットにフォールバックしました (2026 年 3 月 25 日終了)

影響:

- オペレーター認証アドミッションは、受け入れられたソケット IP をすでに受信していますが、
  ロックアウト/レート制限キーはそれを無視し、リクエストを共有に折りたたんだ
  内部 `x-iroha-remote-addr` ヘッダーが
  欠席。
- これは、デフォルト ルーター上の新たなパブリック Ingress バイパスではありませんでした。
  Ingress ミドルウェアは、これらのハンドラーが実行される前に内部ヘッダーを書き換えます。
  それは依然として、より狭い内部のフェールオープンの信頼境界ギャップでした。
  ハンドラー パス、直接テスト、および到達する将来のルート
  インジェクションミドルウェアの前の `OperatorAuth`。
- このような場合、1 人の発信者がレート制限バジェットを消費したり、他の発信者をロックアウトしたりする可能性があります。
  発信者は発信元 IP によって分離されるべきでした。

証拠：- `OperatorAuth::check_common(...)` で
  `crates/iroha_torii/src/operator_auth.rs` はすでに受信済みです
  `remote_ip: Option<IpAddr>`、ただし以前は `auth_key(headers)` と呼ばれていました
  そしてトランスポートIPを完全に削除しました。
- 以前の `crates/iroha_torii/src/operator_auth.rs` の `auth_key(...)`
  `limits::REMOTE_ADDR_HEADER` のみを解析し、それ以外の場合は `"anon"` を返します。
- `crates/iroha_torii/src/limits.rs` の一般的な Torii ヘルパーには、すでに
  `Effective_remote_ip(headers,
  リモート）`、挿入された正規ヘッダーを優先しますが、
  ハンドラーの直接呼び出しがミドルウェアをバイパスするときに受け入れられるソケット IP。

これが重要な理由:

- ロックアウトおよびレート制限状態は、同じ有効な発信者 ID をキーにする必要があります
  Torii の残りの部分はポリシーの決定に使用します。共有にフォールバックする
  匿名バケットは、欠落している内部メタデータホップをクロスクライアントホップに変換します
  影響を真の発信者に限定するのではなく、干渉を軽減します。
- オペレーター認証は悪用に敏感な境界であるため、重大度が中程度であっても
  バケット衝突の問題は明示的に解決する価値があります。

推奨事項:

- Operator-auth キーを `limits::Effective_remote_ip(headers,
  Remote_ip)` なので、挿入されたヘッダーが存在する場合でも優先されますが、直接
  ハンドラー呼び出しは、`"anon"` ではなくトランスポート アドレスにフォールバックします。
- 内部ヘッダーと
  トランスポート IP が利用できません。

修復ステータス:- 現在のコードでは閉じられています。 `crates/iroha_torii/src/operator_auth.rs` が呼び出すようになりました
  `check_common(...)` から `auth_key(headers, remote_ip)`、および `auth_key(...)`
  からロックアウト/レート制限キーを導出するようになりました。
  `limits::effective_remote_ip(headers, remote_ip)`。
- 回帰カバレッジには次のものが含まれます
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` および
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` で
  `crates/iroha_torii/src/operator_auth.rs`。

## 以前のレポートで解決された、または置き換えられた調査結果

- 以前の生の秘密鍵の Soracloud の検出: 終了しました。現在の突然変異の進入
  インライン `authority` / `private_key` フィールドを拒否します
  `crates/iroha_torii/src/soracloud.rs:5305-5308`、HTTP 署名者を
  `crates/iroha_torii/src/soracloud.rs:5310-5315` における突然変異の起源、および
  サーバーが署名済みのトランザクションを送信する代わりに、ドラフトのトランザクション指示を返します。
  `crates/iroha_torii/src/soracloud.rs:5556-5565` のトランザクション。
- 以前の内部専用ローカル読み取りプロキシ実行の検出結果: クローズされました。公共
  ルート解決では、非パブリックおよび更新/プライベート更新ハンドラーがスキップされるようになりました。
  `crates/iroha_torii/src/soracloud.rs:8445-8463`、ランタイムが拒否
  非パブリックのローカル読み取りルート
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`。
- 以前のパブリック ランタイムの従量制フォールバックの検出結果: 記載どおりに終了しました。公共
  ランタイムイングレスでは、レート制限とインフライトキャップが強制されるようになりました。
  パブリック ルートを解決する前は `crates/iroha_torii/src/lib.rs:8837-8852`
  `crates/iroha_torii/src/lib.rs:8858-8860`。
- 以前のリモート IP アタッチメント テナントの検索: 終了しました。現在の添付ファイルのテナント
  の認証済みの署名済みアカウントが必要です
  `crates/iroha_torii/src/lib.rs:7962-7968`。
  アタッチメント テナントは以前は SEC-05 および SEC-06 を継承していました。その遺産
  上記の現在の app-auth 修復によって閉じられます。## 依存関係の調査結果- `cargo deny check advisories bans sources --hide-inclusion-graph` が実行されるようになりました
  追跡されている `deny.toml` に対して直接、3 つのライブ レポートが表示されます。
  生成されたワークスペース ロックファイルから検出された依存関係。
- `tar` アドバイザリはアクティブな依存関係グラフに存在しなくなりました。
  `xtask/src/mochi.rs` は、固定引数を指定して `Command::new("tar")` を使用するようになりました。
  ベクトル、および `iroha_crypto` は、`libsodium-sys-stable` をプルしなくなりました。
  Ed25519 は、これらのチェックを OpenSSL に切り替えた後の相互運用テストです。
- 現在の調査結果:
  - `RUSTSEC-2024-0388`: `derivative` はメンテナンスされていません。
  - `RUSTSEC-2024-0436`: `paste` はメンテナンスされていません。
- 影響のトリアージ:
  - 以前に報告された `tar` アドバイザリは、アクティブなユーザーに対しては終了しました。
    依存関係グラフ。 `cargo tree -p xtask -e normal -i tar`、
    `cargo tree -p iroha_crypto -e all -i tar`、および
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` はすべて失敗するようになりました
    「パッケージ ID の仕様 ... どのパッケージとも一致しませんでした」、および
    `cargo deny` は `RUSTSEC-2026-0067` を報告しなくなりました。
    `RUSTSEC-2026-0068`。
  - 以前に報告された直接 PQ 交換に関する勧告は現在終了しています。
    現在のツリー。 `crates/soranet_pq/Cargo.toml`、
    `crates/iroha_crypto/Cargo.toml` および `crates/ivm/Cargo.toml` は依存するようになりました
    `pqcrypto-mldsa` / `pqcrypto-mlkem`、およびタッチされた ML-DSA / ML-KEM
    ランタイム テストは移行後も引き続き合格します。
  - 以前に報告された `rustls-webpki` アドバイザリーは、現在はアクティブではありません。
    現在の決意。ワークスペースは、`reqwest` / `rustls` を`rustls-webpki` を `0.103.10` に維持するパッチ適用済みパッチ リリース。
    勧告範囲外です。
  - `derivative` および `paste` は、ワークスペース ソースでは直接使用されません。
    これらは、以下の BLS / arkworks スタックを介して推移的に入ります。
    `w3f-bls` およびその他のいくつかの上流クレートのため、それらを削除するには
    ローカルマクロのクリーンアップではなく、アップストリームまたは依存関係スタックの変更。
    現在のツリーは、これら 2 つの勧告を明示的に受け入れるようになりました。
    `deny.toml` と記録された理由。

## 取材メモ- サーバー/ランタイム/構成/ネットワーク: SEC-05、SEC-06、SEC-07、SEC-08、SEC-09、
  SEC-10、SEC-12、SEC-13、SEC-14、SEC-15、および SEC-16 は、期間中に確認されました。
  監査が完了し、現在のツリーで閉じられています。追加硬化
  現在のツリーでは、Connect WebSocket/セッションの受付も失敗するようになりました
  内部に挿入されたリモート IP ヘッダーが見つからない場合は閉じられます。
  その条件をデフォルトでループバックに設定します。
- IVM/crypto/serialization: この監査で追加の確認結果はありません
  スライス。肯定的な証拠には、機密キーマテリアルのゼロ化が含まれます。
  `crates/iroha_crypto/src/confidential.rs:53-60` およびリプレイ対応 Soranet PoW
  `crates/iroha_crypto/src/soranet/pow.rs:823-879` での署名付きチケットの検証。
  フォローアップ強化により、不正なアクセラレータ出力も 2 つの段階で拒否されるようになりました。
  サンプリングされた Norito パス: `crates/norito/src/lib.rs` は高速化された JSON を検証します
  `TapeWalker` より前のステージ 1 テープはオフセットを逆参照し、現在は
  動的にロードされた Metal/CUDA Stage-1 ヘルパーは、
  アクティブ化前のスカラー構造インデックス ビルダー、および
  `crates/norito/src/core/gpu_zstd.rs` は GPU から報告された出力長を検証します
  エンコード/デコード バッファを切り詰める前に。 `crates/norito/src/core/simd_crc64.rs`
  動的にロードされた GPU CRC64 ヘルパーを自己テストするようになりました。
  `hardware_crc64` が信頼する前の正規フォールバックは形式が不正です
  Norito チェックサムをサイレントに変更する代わりに、ヘルパー ライブラリがフェール クローズされます。行動。無効なヘルパー結果はパニックリリースではなくフォールバックするようになりました。
  ビルドまたはドリフトチェックサムパリティ。 IVM 側で、サンプリングされたアクセラレータ
  起動ゲートは CUDA Ed25519 `signature_kernel`、CUDA BN254 もカバーするようになりました。
  add/sub/mul カーネル、CUDA `sha256_leaves` / `sha256_pairs_reduce`、ライブ
  CUDA ベクター/AES バッチ カーネル (`vadd64`、`vand`、`vxor`、`vor`、
  `aesenc_batch`、`aesdec_batch`)、および一致するメタル
  これらのパスが信頼される前に、`sha256_leaves`/vector/AES バッチ カーネル。の
  サンプリングされた Metal Ed25519 署名パスも復活しました
  このホスト上のライブ アクセラレータ セット内: 以前のパリティ エラーは
  スカラーラダー全体で ref10 のリム境界正規化を復元することで修正されました。
  そして、焦点を当てたメタル回帰により、`[s]B`、`[h](-A)`、
  2 のべき乗ベースポイント ラダー、および完全な `[true, false]` バッチ検証
  Metal では CPU 参照パスに対して。ミラーリングされた CUDA ソースの変更
  `--features cuda --tests` でコンパイルし、CUDA 起動の真実を設定します。
  ライブ Merkle リーフ/ペア カーネルが CPU からドリフトするとフェイルクローズされる
  参照パス。この場合、ランタイム CUDA 検証はホスト限定のままです。
  環境。
- SDK/例: SEC-11 は、トランスポートに焦点を当てたサンプリング中に確認されました。
  Swift、Java/Android、Kotlin、および JS クライアント間で渡されます。現在のツリーでは検索が閉じられています。 JS、Swift、Android
  canonical-request ヘルパーも新しいものに更新されました。
  鮮度を意識した4ヘッダー方式。
  サンプリングされた QUIC ストリーミング トランスポート レビューでも、ライブ サウンドは生成されませんでした。
  現在のツリー内のランタイム検索: `StreamingClient::connect(...)`、
  `StreamingServer::bind(...)`、および機能ネゴシエーション ヘルパーは次のとおりです。
  現在、`crates/iroha_p2p` のテスト コードからのみ実行されており、
  `crates/iroha_core`、つまりそのヘルパーの寛容な自己署名検証ツール
  パスは現在、出荷された入力サーフェスではなく、テスト/ヘルパー専用です。
- サンプルとモバイル サンプル アプリは抜き取りチェック レベルでのみレビューされ、
  徹底的に監査されたものとして扱われるべきではありません。

## 検証と適用範囲のギャップ- `cargo deny check advisories bans sources --hide-inclusion-graph` が実行されるようになりました
  追跡対象の `deny.toml` を直接使用します。現在のスキーマを実行すると、
  `bans` と `sources` はクリーンですが、`advisories` は 5 つで失敗します
  上記の依存関係の発見。
- クローズされた `tar` の結果に対する依存関係グラフのクリーンアップ検証に合格しました。
  `cargo tree -p xtask -e normal -i tar`、
  `cargo tree -p iroha_crypto -e all -i tar`、および
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` がすべてレポートされるようになりました
  「パッケージ ID の仕様 ... どのパッケージとも一致しませんでした。」
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`、
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`、
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`、
  そして
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  すべて合格しました。
- `bash scripts/fuzz_smoke.sh` は、経由で実際の libFuzzer セッションを実行するようになりました。
  `cargo +nightly fuzz` ですが、IVM スクリプトの半分は期限内に終了しませんでした
  `tlv_validate` の最初の夜間ビルドがまだ残っていたため、これはパスしました。
  引き継ぎ時の進捗状況。そのビルドは、実行できる程度まで完了しました。
  libFuzzer バイナリを直接生成します。
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  libFuzzer 実行ループに到達し、200 回の実行後に正常に終了します。
  空のコーパス。 Norito は修正後、半分正常に完了しました。
  ハーネス/マニフェスト ドリフトと `json_from_json_equiv` ファズターゲット コンパイル
  休憩する。
- Torii 修復検証には次のものが含まれるようになりました。
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`- `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - 狭い方の `--no-default-features --features app_api,app_api_https` Torii
    テスト マトリックスには、DA に関連性のない既存のコンパイル エラーがまだ存在します /
    Soracloud でゲートされた lib-test コードなので、このパスは出荷されたものを検証しました
    デフォルト機能の MCP パスと `app_api_https` Webhook パスではなく
    最小限の機能を完全にカバーしていると主張します。
- Trusted-proxy/SoraFS 修復検証には以下が含まれるようになりました。
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- P2P 修復検証には次のものが含まれるようになりました。
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- SoraFS ローカル プロキシ修復検証に次のものが含まれるようになりました。
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- P2P TLS のデフォルト修復検証に次のものが含まれるようになりました。
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- SDK 側の修復検証には次のものが含まれるようになりました。
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests`
  - `cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  - `cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  - `node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- Norito フォローアップ検証には以下が含まれるようになりました。
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh` (Norito は `json_parse_string` をターゲットにします、
    `json_parse_string_ref`、`json_skip_value`、および `json_from_json_equiv`ハーネス/ターゲットの修正後に渡されます)
- IVM ファズのフォローアップ検証には以下が含まれるようになりました。
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- IVM アクセラレータのフォローアップ検証に以下が含まれるようになりました。
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- 集中的な CUDA lib-test の実行は、このホスト上の環境に制限されたままになります。
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  CUDA ドライバー シンボル (`cu*`) が使用できないため、依然としてリンクに失敗します。
- Focused Metal ランタイム検証は、このアクセラレータで完全に実行されるようになりました。
  ホスト: サンプリングされた Ed25519 署名パイプラインは起動中も有効なままです
  セルフテスト、および `metal_ed25519_batch_matches_cpu` が `[true, false]` を検証する
  CPU 参照パスに対して Metal 上で直接実行します。
- ワークスペース全体の Rust テスト スイープ、完全な `npm test`、または
  この修復パス中は完全な Swift/Android スイート。

## 優先順位付けされた修復バックログ

### 次のトランシェ- 明示的に受け入れられた推移的な上流の置換を監視する
  `derivative` / `paste` マクロ負債が発生し、次の場合に `deny.toml` 例外を削除します。
  BLS / Halo2 / PQ / を不安定にすることなく安全なアップグレードが可能になります。
  UI 依存関係スタック。
- ウォーム キャッシュ上で完全な夜間 IVM ファズスモーク スクリプトを再実行します。
  `tlv_validate` / `kotodama_lower` は、次の安定した記録結果を持っています。
  現在は緑色の Norito ターゲット。 `tlv_validate` バイナリの直接実行が完了しました。
  しかし、完全にスクリプト化された夜の煙は依然として顕著です。
- CUDA ドライバーを備えたホスト上で、フォーカスされた CUDA lib-test セルフテスト スライスを再実行します。
  ライブラリがインストールされているため、拡張された CUDA 起動真理セットが検証されます
  `cargo check` とミラーリングされた Ed25519 正規化修正に加え、
  新しいベクター/AES 起動プローブは実行時に実行されます。
- より広範な JS/Swift/Android/Kotlin スイートを無関係なスイートレベルで再実行する
  このブランチ上のブロッカーがクリアされるため、新しい canonical-request と
  輸送警備員は、上記の集中的なヘルパー テストを超えてカバーされます。
- 長期的なアプリ認証マルチシグのストーリーを維持すべきかどうかを決定する
  フェイルクローズするか、ファーストクラスの HTTP マルチシグ監視フォーマットを拡張します。

### モニター- `ivm` ハードウェア アクセラレーション/安全でないパスの重点的なレビューを継続します
  残りの `norito` ストリーミング/暗号境界。 JSON ステージ 1
  GPU zstd ヘルパー ハンドオフは、フェール クローズされるように強化されました。
  リリース ビルド、およびサンプリングされた IVM アクセラレータ起動真理セットは現在、
  より広範ですが、より広範な安全性と決定性のレビューはまだ未完了です。
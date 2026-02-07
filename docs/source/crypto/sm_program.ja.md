---
lang: ja
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T18:50:10.586502+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Hyperledger Iroha v2 の SM2/SM3/SM4 有効化アーキテクチャの概要。

# SM プログラム アーキテクチャの概要

## 目的
決定的な実行と監査可能性を維持しながら、Iroha v2 スタック全体に中国国家暗号 (SM2/SM3/SM4) を導入するための技術計画、サプライ チェーンの態勢、リスク境界を定義します。

## 範囲
- **コンセンサス クリティカル パス:** `iroha_crypto`、`iroha`、`irohad`、IVM ホスト、Kotodama 組み込み。
- **クライアント SDK とツール:** Rust CLI、Kagami、Python/JS/Swift SDK、genesis ユーティリティ。
- **設定とシリアル化:** `iroha_config` ノブ、Norito データ モデル タグ、マニフェスト処理、マルチコーデック更新。
- **テストとコンプライアンス:** ユニット/プロパティ/相互運用スイート、Wycheproof ハーネス、パフォーマンス プロファイリング、輸出/規制ガイダンス。 *(ステータス: RustCrypto-backed SM スタックがマージされました。オプションの `sm_proptest` ファズ スイートと OpenSSL パリティ ハーネスが拡張 CI で利用可能です。)*

範囲外: PQ アルゴリズム、コンセンサス パスでの非決定的ホスト アクセラレーション。 wasm/`no_std` ビルドは廃止されました。

## アルゴリズムの入力と成果物
|アーティファクト |オーナー |期限 |メモ |
|----------|----------|-----|----------|
| SM アルゴリズム機能設計 (`SM-P0`) |暗号WG | 2025-02 |機能ゲーティング、依存関係監査、リスク登録。 |
|コア Rust 統合 (`SM-P1`) |暗号WG / データモデル | 2025-03 | RustCryptoベースのverify/hash/AEADヘルパー、Norito拡張機能、フィクスチャ。 |
|署名 + VM システムコール (`SM-P2`) | IVM コア/SDK プログラム | 2025年4月 |確定的署名ラッパー、syscall、Kotodama のカバレッジ。 |
|オプションのプロバイダーと運用の有効化 (`SM-P3`) |プラットフォーム運用/パフォーマンス WG | 2025-06 | OpenSSL/Tongsuo バックエンド、ARM 組み込み、テレメトリ、ドキュメント。 |

## 選択されたライブラリ
- **プライマリ:** `rfc6979` 機能が有効になっており、SM3 が決定論的なノンスにバインドされている RustCrypto クレート (`sm2`、`sm3`、`sm4`)。
- **オプションの FFI:** 認定スタックまたはハードウェア エンジンを必要とする展開用の OpenSSL 3.x プロバイダー API または Tongsuo。機能ゲート型であり、コンセンサスバイナリではデフォルトで無効になっています。### コア ライブラリの統合ステータス
- `iroha_crypto::sm` は、統一された `sm` 機能の下で SM3 ハッシュ、SM2 検証、および SM4 GCM/CCM ヘルパーを公開し、SDK で利用できる確定的な RFC6979 署名パスを使用します。 `Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Norito/Norito-JSON タグとマルチコーデック ヘルパーは SM2 公開キー/署名と SM3/SM4 ペイロードをカバーするため、命令は全体にわたって決定論的にシリアル化されます。 hosts.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tests/sm_norito_roundtrip.rs:12】
- 既知の回答スイートは RustCrypto 統合 (`sm3_sm4_vectors.rs`、`sm2_negative_vectors.rs`) を検証し、CI の `sm` 機能ジョブの一部として実行され、ノードが署名を継続している間、検証の決定性を維持します。 Ed25519.【crates/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】
- オプションの `sm` 機能ビルド検証: `cargo check -p iroha_crypto --features sm --locked` (コールド 7.9 秒 / ウォーム 0.23 秒) と `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` (1.0 秒) は両方とも成功します。機能を有効にすると、11 個のクレート (`base64ct`、`ghash`、`opaque-debug`、`pem-rfc7468`、`pkcs8`、`polyval`、`primeorder`、 `sm2`、`sm3`、`sm4`、`sm4-gcm`)。調査結果は `docs/source/crypto/sm_rustcrypto_spike.md` に記録されました。【docs/source/crypto/sm_rustcrypto_spike.md:1】
- BouncyCastle/GmSSL ネガティブ検証フィクスチャは `crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json` の下で稼働し、正規の障害ケース (r=0、s=0、識別 ID の不一致、改ざんされた公開キー) が広く導入されているものと確実に一致するようにします。プロバイダー.【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json:1】
- `sm-ffi-openssl` は、ベンダーの OpenSSL 3.x ツールチェーン (`openssl` クレート `vendored` 機能) をコンパイルするようになりました。そのため、システム LibreSSL/OpenSSL に SM アルゴリズムがない場合でも、プレビュー ビルドとテストは常に最新の SM 対応プロバイダーをターゲットにします。【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` は、実行時に AArch64 NEON を検出し、構成ノブ `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) を尊重しながら、x86_64/RISC-V ディスパッチを通じて SM3/SM4 フックをスレッドするようになりました。ベクターバックエンドが存在しない場合でも、ディスパッチャはスカラー RustCrypto パスを介してルーティングされるため、ベンチとポリシーの切り替えはホスト間で一貫して動作します。【crates/iroha_crypto/src/sm.rs:702】【crates/iroha_crypto/src/sm.rs:733】

### Norito スキーマとデータモデル サーフェス| Norito タイプ / コンシューマー |代表 |制約と注意事項 |
|--------------------------|----------------|----------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) |ベア: 32 バイト BLOB · JSON: 大文字の 16 進文字列 (`"4F4D..."`) |正規 Norito タプル ラッピング `[u8; 32]`。 JSON/Bare デコードでは長さ ≠32 が拒否されます。ラウンドトリップは `sm_norito_roundtrip::sm3_digest_norito_roundtrip` でカバーされます。 |
| `Sm2PublicKey` / `Sm2Signature` |マルチコーデックプレフィックス付き BLOB (`0x1306` 暫定) |公開キーは非圧縮の SEC1 ポイントをエンコードします。署名は、DER 解析ガード付きの `(r∥s)` (各 32 バイト) です。 |
| `Sm4Key` |ベア: 16 バイト BLOB | Kotodama/CLI に公開されるゼロ化ラッパー。 JSON シリアル化は意図的に省略されています。オペレーターは、BLOB (コントラクト) または CLI 16 進数 (`--key-hex`) を介してキーを渡す必要があります。 |
| `sm4_gcm_seal/open` オペランド | 4 つの BLOB のタプル: `(key, nonce, aad, payload)` |キー = 16 バイト。ノンス = 12 バイト;タグの長さは 16 バイトに固定されています。 `(ciphertext, tag)` を返します。 Kotodama/CLI は hex ヘルパーと Base64 ヘルパーの両方を出力します。【crates/ivm/tests/sm_syscalls.rs:728】 |
| `sm4_ccm_seal/open` オペランド | `r14` の 4 つの BLOB (キー、ノンス、aad、ペイロード) + タグの長さのタプル |ノンス 7 ～ 13 バイト。タグの長さ ∈ {4,6,8,10,12,14,16}。 `sm` 機能は、`sm-ccm` フラグの背後にある CCM を公開します。 |
| Kotodama 組み込み (`sm::hash`、`sm::seal_gcm`、`sm::open_gcm`、…) |上記の SCALL にマップ |入力検証はホスト ルールを反映します。不正なサイズでは `ExecutionError::Type` が発生します。 |

使用法のクイックリファレンス:
- **コントラクト/テストでの SM3 ハッシュ:** `Sm3Digest::hash(b"...")` (Rust) または Kotodama `sm::hash(input_blob)`。 JSON では 64 個の 16 進文字が必要です。
- **CLI 経由の SM4 AEAD:** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` は、16 進数/base64 暗号文とタグのペアを生成します。一致する `gcm-open` を使用して復号化します。
- **マルチコーデック文字列:** SM2 公開キー/署名は、`PublicKey::from_str`/`Signature::from_bytes` によって受け入れられるマルチベース文字列との間で解析され、Norito マニフェストとアカウント ID が SM 署名者を保持できるようになります。

データモデルの利用者は、SM4 キーとタグを一時的な BLOB として扱う必要があります。生のキーをチェーン上に永続化しないでください。監査が必要な場合、契約では暗号文/タグ出力または派生ダイジェスト (キーの SM3 など) のみを保存する必要があります。

### サプライチェーンとライセンス
|コンポーネント |ライセンス |緩和 |
|----------|-----------|----------|
| `sm2`、`sm3`、`sm4` | Apache-2.0 / MIT |アップストリームのコミットを追跡し、ロックステップリリースが必要な場合はベンダーを追跡し、バリデーターが GA に署名する前にサードパーティの監査をスケジュールします。 |
| `rfc6979` | Apache-2.0 / MIT |すでに他のアルゴリズムで使用されています。 SM3 ダイジェストとの決定的な `k` 結合を確認します。 |
|オプションの OpenSSL/Tongsuo | Apache-2.0 / BSD スタイル | `sm-ffi-openssl` 機能を維持し、明示的なオペレーターのオプトインとパッケージ化チェックリストを必要とします。 |### 機能フラグと所有権
|表面 |デフォルト |メンテナ |メモ |
|----------|----------|---------------|----------|
| `iroha_crypto/sm-core`、`sm-ccm`、`sm` |オフ |暗号WG | RustCrypto SM プリミティブを有効にします。 `sm` には、認証された暗号化を必要とするクライアント用の CCM ヘルパーがバンドルされています。 |
| `ivm/sm` |オフ | IVM コアチーム | SM システムコール (`sm3_hash`、`sm2_verify`、`sm4_gcm_*`、`sm4_ccm_*`) を構築します。ホスト ゲーティングは `crypto.allowed_signing` (`sm2` の存在) から派生します。 |
| `iroha_crypto/sm_proptest` |オフ | QA / 暗号WG |不正な形式の署名/タグをカバーするプロパティ テスト ハーネス。拡張 CI でのみ有効になります。 |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`、`blake2b-256` |構成WG / オペレーターWG | `sm2` と `sm3-256` ハッシュが存在すると、SM syscall/signature が有効になります。 `sm2` を削除すると、検証専用モードに戻ります。 |
|オプションの `sm-ffi-openssl` (プレビュー) |オフ |プラットフォーム運用 | OpenSSL/Tongsuo プロバイダー統合のためのプレースホルダー機能。認証とパッケージ化 SOP が適用されるまで無効のままです。 |

ネットワーク ポリシーは `network.require_sm_handshake_match` を公開し、
`network.require_sm_openssl_preview_match` (両方のデフォルトは `true`)。どちらかのフラグをクリアすると、
Ed25519 のみのオブザーバーが SM 対応のバリデーターに接続する混合デプロイメント。不一致は
`WARN` でログに記録されますが、コンセンサス ノードは偶発的な事故を防ぐためにデフォルトを有効にしておく必要があります。
SM を認識するピアと SM が無効なピアの間の相違。
CLI は、「iroha_cli app sorafs handshake update」を介してこれらの切り替えを表示します。
--allow-sm-handshake-mismatch` and `--allow-sm-openssl-preview-mismatch`, or the matching `--require-*`
厳格な施行を復元するためのフラグ。#### OpenSSL/Tongsuo プレビュー (`sm-ffi-openssl`)
- **スコープ。** OpenSSL ランタイムの可用性を検証し、オプトインのままで OpenSSL を利用した SM3 ハッシュ、SM2 検証、および SM4-GCM 暗号化/復号化を公開するプレビュー専用のプロバイダー シム (`OpenSslProvider`) を構築します。コンセンサスバイナリは RustCrypto パスを使用し続ける必要があります。 FFI バックエンドはエッジ検証/署名パイロットに対して厳密にオプトインされています。
- **ビルドの前提条件。** `cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` でコンパイルし、ツールチェーンが OpenSSL/Tongsuo 3.0+ (SM2/SM3/SM4 をサポートする `libcrypto`) に対してリンクしていることを確認します。静的リンクは推奨されません。オペレータによって管理される動的ライブラリを優先します。
- **開発者スモーク テスト。** `scripts/sm_openssl_smoke.sh` を実行して `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"` を実行し、続いて `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture` を実行します。 OpenSSL ≥3 開発ヘッダーが使用できない (または `pkg-config` が欠落している) 場合、ヘルパーは自動的にスキップし、スモーク出力を表示するので、開発者は SM2 検証が実行されたか Rust 実装にフォールバックしたかを確認できます。
- **Rust スキャフォールディング** `openssl_sm` モジュールは、SM3 ハッシュ、SM2 検証 (ZA プレハッシュ + SM2 ECDSA)、および SM4 GCM 暗号化/復号化を OpenSSL 経由でルーティングするようになり、プレビューの切り替えや無効なキー/ノンス/タグの長さをカバーする構造化エラーが発生します。 SM4 CCM は、追加の FFI シムが適用されるまで純粋な Rust のみのままです。
- **スキップ動作。** OpenSSL ≥3.0 ヘッダーまたはライブラリがない場合、スモーク テストはスキップ バナー (`-- --nocapture` 経由) を出力しますが、それでも正常に終了するため、CI は環境のギャップと本物の回帰を区別できます。
- **ランタイム ガードレール** OpenSSL プレビューはデフォルトで無効になっています。 FFI パスを使用する前に、構成 (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) で有効にしてください。プロバイダーが卒業するまで実稼働クラスターを検証専用モード (`allowed_signing` から `sm2` を省略) に保ち、決定論的な RustCrypto フォールバックに依存し、署名パイロットを隔離された環境に限定します。
- **パッケージ化チェックリスト** プロバイダーのバージョン、インストール パス、整合性ハッシュを展開マニフェストに文書化します。オペレーターは、承認された OpenSSL/Tongsuo ビルドをインストールし、それを OS トラスト ストアに登録し (必要な場合)、メンテナンス期間中にアップグレードを固定するインストール スクリプトを提供する必要があります。
- **次のステップ。** 将来のマイルストーンには、決定論的な SM4 CCM FFI バインディング、CI スモーク ジョブ (`ci/check_sm_openssl_stub.sh` を参照)、およびテレメトリが追加されます。 `roadmap.md` の SM-P3.1.x で進行状況を追跡します。

#### コード所有権のスナップショット
- **暗号WG:** `iroha_crypto`、SMフィクスチャ、コンプライアンス文書。
- **IVM コア:** syscall 実装、Kotodama 組み込み、ホスト ゲーティング。
- **構成 WG:** 番号 `crypto.allowed_signing`/`default_hash`、番号、番号、番号。
- **SD​​K プログラム:** CLI/Kagami/SDK にわたる SM 対応ツール、共有フィクスチャ。
- **プラットフォーム運用およびパフォーマンス WG:** アクセラレーション フック、テレメトリ、オペレーターの有効化。

## 構成移行ハンドブックEd25519 のみのネットワークから SM 対応の展開に移行するオペレータは、次のことを行う必要があります。
～の段階的なプロセスに従う
[`sm_config_migration.md`](sm_config_migration.md)。ガイドではビルドについて説明しています
検証、`iroha_config` 階層化 (`defaults` → `user` → `actual`)、ジェネシス
`kagami` オーバーライド (`kagami genesis generate --allowed-signing sm2 --default-hash sm3-256` など)、プリフライト検証、ロールバックによる再生成
構成スナップショットとマニフェストが全体にわたって一貫性を保つように計画します。
艦隊。

## 決定的なポリシー
- SDK 内のすべての SM2 署名パスおよびオプションのホスト署名に対して RFC6979 由来の nonce を強制します。検証者は正規の r∥s エンコーディングのみを受け入れます。
- コントロール プレーン通信 (ストリーミング) は Ed25519 のままです。 SM2 は、ガバナンスが拡張を承認しない限り、データプレーン署名に限定されます。
- 組み込み (ARM SM3/SM4) は、ランタイム機能検出およびソフトウェア フォールバックを備えた決定論的検証/ハッシュ操作に制限されています。

## Norito およびエンコーディング プラン
1. `iroha_data_model` のアルゴリズム列挙型を `Sm2PublicKey`、`Sm2Signature`、`Sm3Digest`、`Sm4Key` で拡張します。
2. DER の曖昧さを避けるために、SM2 シグネチャをビッグエンディアン固定幅 `r∥s` 配列 (32+32 バイト) としてシリアル化します。アダプターで処理される変換。 *(完了: `Sm2Signature` ヘルパーに実装されました。Norito/JSON ラウンドトリップが導入されました。)*
3. マルチフォーマットを使用している場合はマルチコーデック識別子 (`sm3-256`、`sm2-pub`、`sm4-key`) を登録し、フィクスチャとドキュメントを更新します。 *(進行状況: `sm2-pub` 暫定コード `0x1306` は派生キーで検証されました。SM3/SM4 コードは最終割り当て待ちで、`sm_known_answers.toml` 経由で追跡されます。)*
4. ラウンドトリップと不正なエンコーディング (短い/長い r または s、無効な曲線パラメーター) の拒否をカバーする Norito ゴールデン テストを更新します。## ホストと VM の統合計画 (SM-2)
1. 既存の GOST ハッシュ シムをミラーリングするホスト側 `sm3_hash` システムコールを実装します。 `Sm3Digest::hash` を再利用し、決定的なエラー パスを公開します。 *(着陸: ホストは Blob TLV を返します。`DefaultHost` 実装と `sm_syscalls.rs` リグレッションを参照してください。)*
2. 正規の r∥s 署名を受け入れ、識別 ID を検証し、失敗を確定的なリターン コードにマップする `sm2_verify` を使用して VM syscall テーブルを拡張します。 *(完了: ホスト + Kotodama 組み込みは `1/0` を返します。回帰スイートは、切り捨てられた署名、不正な形式の公開キー、非 BLOB TLV、および UTF-8/空/不一致の `distid` ペイロードをカバーするようになりました。)*
3. 明示的な nonce/タグ サイズ設定を使用して `sm4_gcm_seal`/`sm4_gcm_open` (およびオプションで CCM) システムコールを提供します (RFC 8998)。 *(完了: GCM は固定 12 バイト ノンス + 16 バイト タグを使用します。CCM は `r14` 経由で制御されるタグ長 {4,6,8,10,12,14,16} の 7 ～ 13 バイトのノンスをサポートします。Kotodama はこれらを `sm::seal_gcm/open_gcm` として公開し、 `sm::seal_ccm/open_ccm`.) 開発者ハンドブックに nonce 再利用ポリシーを文書化します。*
4. ポジティブなケースとネガティブなケース (タグの変更、署名の不正、サポートされていないアルゴリズム) をカバーする Kotodama スモーク コントラクトと IVM 統合テストをワイヤリングします。 *(SM3/SM2/SM4 のホスト回帰をミラーリングする `crates/ivm/tests/kotodama_sm_syscalls.rs` 経由で実行されます。)*
5. 新しいエントリを追加した後、syscall ホワイトリスト、ポリシー、および ABI ドキュメント (`crates/ivm/docs/syscalls.md`) を更新し、ハッシュされたマニフェストを更新します。

### ホストと VM の統合ステータス
- DefaultHost、CoreHost、および WsvHost は SM3/SM2/SM4 syscall を公開し、`sm_enabled` でゲートし、ランタイム フラグが次の場合に `PermissionDenied` を返します。 false.【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- `crypto.allowed_signing` ゲートはパイプライン/エグゼキューター/状態を介してスレッド化されるため、運用ノードは構成を通じて決定的にオプトインします。 `sm2` を追加すると、SM ヘルパーの可用性が切り替わります。`【crates/iroha_core/src/smartcontracts/ivm/host.rs:170】【crates/iroha_core/src/state.rs:7673】【crates/iroha_core/src/executor.rs:683】
- 回帰カバレッジは、SM3 ハッシュ、SM2 検証、SM4 GCM/CCM シール/オープンの有効パスと無効パス (DefaultHost/CoreHost/WsvHost) の両方を実行します。フロー.【crates/ivm/tests/sm_syscalls.rs:129】【crates/ivm/tests/sm_syscalls.rs:733】【crates/ivm/tests/sm_syscalls.rs:1036】

## 構成スレッド
- `crypto.allowed_signing`、`crypto.default_hash`、`crypto.sm2_distid_default`、およびオプションの `crypto.enable_sm_openssl_preview` を `iroha_config` に追加します。データ モデル機能の配管が暗号クレートをミラーリングしていることを確認します (`iroha_data_model` は `sm` → `iroha_crypto/sm` を公開します)。
- 設定をアドミッション ポリシーに関連付けて、マニフェスト/ジェネシス ファイルで許容されるアルゴリズムを定義します。コントロールプレーンはデフォルトでは Ed25519 のままです。### CLI と SDK の動作 (SM-3)
1. **Torii CLI** (`crates/iroha_cli`): SM2 keygen/import/export (distid 対応)、SM3 ハッシュ ヘルパー、および SM4 AEAD 暗号化/復号化コマンドを追加します。インタラクティブなプロンプトとドキュメントを更新します。
2. **Genesis ツール** (`xtask`、`scripts/`): マニフェストで許可された署名アルゴリズムとデフォルトのハッシュを宣言できるようにします。対応する構成ノブなしで SM が有効になっている場合は失敗します。 *(完了: `RawGenesisTransaction` は、`default_hash`/`allowed_signing`/`sm2_distid_default` を含む `crypto` ブロックを保持するようになりました。`ManifestCrypto::validate` と `kagami genesis validate` は、一貫性のない SM 設定を拒否し、 defaults/genesis マニフェストはスナップショットをアドバタイズします。)*
3. **SDK サーフェス**:
   - Rust (`iroha_client`): SM2 署名/検証ヘルパー、SM3 ハッシュ、SM4 AEAD ラッパーを決定論的なデフォルトで公開します。
   - Python/JS/Swift: Rust API をミラーリングします。 `sm_known_answers.toml` のステージングされたフィクスチャを言語間テストに再利用します。
4. CLI/SDK クイックスタートで SM を有効にするためのオペレーター ワークフローを文書化し、JSON/YAML 構成が新しいアルゴリズム タグを受け入れることを確認します。

#### CLI の進行状況
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` は、`client.toml` スニペット (`public_key_config`、`private_key_hex`、`distid`) とともに SM2 キー ペアを記述する JSON ペイロードを発行するようになりました。このコマンドは、確定的生成のために `--seed-hex` を受け入れ、ホストによって使用される RFC 6979 派生を反映します。
- `cargo xtask sm-operator-snippet --distid CN12345678901234` は keygen/export フローをラップし、同じ `sm2-key.json`/`client-sm2.toml` 出力を 1 ステップで書き込みます。 `--json-out <path|->` / `--snippet-out <path|->` を使用してファイルをリダイレクトするか、標準出力にストリーミングし、自動化のための `jq` 依存関係を削除します。
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` は既存の素材から同じメタデータを抽出するため、オペレーターは入場前に識別 ID を検証できます。
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` は、構成スニペット (`allowed_signing`/`sm2_distid_default` ガイダンスを含む) を出力し、オプションでスクリプト用の JSON キー インベントリを再発行します。
- `iroha_cli tools crypto sm3 hash --data <string>` は任意のペイロードをハッシュします。 `--data-hex` / `--file` はバイナリ入力をカバーし、このコマンドはマニフェスト ツールの 16 進ダイジェストと Base64 ダイジェストの両方を報告します。
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>` (および `gcm-open`) は、ホスト SM4-GCM ヘルパーをラップし、`ciphertext_hex`/`tag_hex` またはプレーンテキスト ペイロードを表示します。 `sm4 ccm-seal` / `sm4 ccm-open` は、ノンス長 (7 ～ 13 バイト) とタグ長 (4、6、8、10、12、14、16) の検証が組み込まれた CCM に同じ UX を提供します。どちらのコマンドもオプションで生のバイトをディスクに出力します。## テスト戦略
### 単元/既知の解答テスト
- SM3 用の GM/T 0004 および GB/T 32905 ベクター (例: `"abc"`)。
- SM4 用の GM/T 0002 および RFC 8998 ベクトル (ブロック + GCM/CCM)。
- SM2 の GM/T 0003/GB/T 32918 の例（Z 値、署名検証）。ID `ALICE123@YAHOO.COM` の付録例 1 を含む。
- 暫定フィクスチャ ステージング ファイル: `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`。
- Wycheproof 由来の SM2 回帰スイート (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) には、ビットフリップ、メッセージ改ざん、および切り捨てられた署名のネガを含む決定論的フィクスチャ (付録 D、SDK シード) を階層化する 52 件のコーパスが含まれるようになりました。サニタイズされた JSON は `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` に存在し、`sm2_fuzz.rs` はそれを直接使用するため、ハッピー パスと改ざんの両方のシナリオがファズ/プロパティの実行全体にわたって調整されます。 벡터들은 표준 곡선뿐만 아니라 Annex 영역도 다루며, 필요 시 내장 `Sm2PublicKey` 검 이 BigInt 백업 루틴이 추적을 완료합니다。
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>` (または `--input-url <https://…>`) は、アップストリーム ドロップ (ジェネレータ タグはオプション) を決定的にトリミングし、`crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` を書き換えます。 C2SP が公式コーパスを公開するまでは、フォークを手動でダウンロードし、ヘルパーを通じてフィードします。キー、カウント、フラグを正規化して、レビュー担当者が差分を推論できるようにします。
- SM2/SM3 Norito ラウンドトリップは `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` で検証されました。
- `crates/ivm/tests/sm_syscalls.rs` での SM3 ホスト システムコールの回帰 (SM 機能)。
- SM2 は、`crates/ivm/tests/sm_syscalls.rs` でのシステムコールの回帰を検証します (成功 + 失敗のケース)。

### プロパティと回帰テスト
- 無効な曲線、非標準的な r/s、ノンスの再利用を拒否する SM2 のプロプテスト。 *(`crates/iroha_crypto/tests/sm2_fuzz.rs` で利用可能、`sm_proptest` の背後でゲートされ、`cargo test -p iroha_crypto --features "sm sm_proptest"` 経由で有効になります。)*
- さまざまなモードに適応した Wycheproof SM4 ベクトル (ブロック/AES モード)。 SM2 追加のアップストリームを追跡します。 `sm3_sm4_vectors.rs` は、GCM と CCM の両方に対してタグのビット反転、切り捨てられたタグ、および暗号文の改ざんを実行するようになりました。

### 相互運用性とパフォーマンス
- SM2 署名/検証、SM3 ダイジェスト、SM4 ECB/GCM 用の RustCrypto ↔ OpenSSL/Tongsuo パリティ スイートは `crates/iroha_crypto/tests/sm_cli_matrix.rs` に存在します。 `scripts/sm_interop_matrix.sh` で呼び出します。 CCM パリティ ベクトルは `sm3_sm4_vectors.rs` で実行されるようになりました。アップストリーム CLI が CCM ヘルパーを公開すると、CLI マトリックス サポートが追加されます。
- SM3 NEON ヘルパーは、`sm_accel::is_sm3_enabled` を介したランタイム ゲーティングを使用して、Armv8 圧縮/パディング パスをエンドツーエンドで実行するようになりました (SM3/SM4 全体でミラーリングされる機能 + 環境オーバーライド)。ゴールデン ダイジェスト (ゼロ/`"abc"`/ロングブロック + ランダム化された長さ) と強制無効テストは、スカラー RustCrypto バックエンドとの同等性を維持し、Criterion マイクロベンチ (`crates/sm3_neon/benches/digest.rs`) は、AArch64 ホスト上のスカラーと NEON のスループットをキャプチャします。
- パフォーマンス ハーネス ミラーリング `scripts/gost_bench.sh` を使用して、Ed25519/SHA-2 と SM2/SM3/SM4 を比較し、許容差のしきい値を検証します。#### Arm64 ベースライン (ローカル Apple Silicon; Criterion `sm_perf`、2025 年 12 月 5 日に更新)
- `scripts/sm_perf.sh` は Criterion ベンチを実行し、`crates/iroha_crypto/benches/sm_perf_baseline.json` に対して中央値を適用します (aarch64 macOS で記録。デフォルトでは許容値 25%、ベースライン メタデータはホスト トリプルをキャプチャします)。新しい `--mode` フラグを使用すると、エンジニアはスクリプトを編集せずに、スカラー データポイント、NEON データポイント、`sm-neon-force` データポイントをキャプチャできるようになります。現在のキャプチャ バンドル (生の JSON + 集約されたサマリー) は `artifacts/sm_perf/2026-03-lab/m3pro_native/` の下に存在し、すべてのペイロードに `cpu_label="m3-pro-native"` をスタンプします。
- 加速モードは、比較ターゲットとしてスカラー ベースラインを自動選択するようになりました。 `scripts/sm_perf.sh` スレッド `--compare-baseline/--compare-tolerance/--compare-label` ～ `sm_perf_check` は、スカラー参照に対してベンチマークごとのデルタを発行し、速度低下が設定されたしきい値を超えると失敗します。ベースラインからのベンチマークごとの許容値により比較ガードが強化されます (SM3 は Apple スカラー ベースラインで 12% に制限されていますが、SM3 比較デルタではフラッピングを避けるためにスカラー リファレンスに対して最大 70% が許容されます)。 Linux ベースラインは、`neoverse-proxy-macos` キャプチャからエクスポートされるため、同じ比較マップを再利用します。中央値が異なる場合は、ベアメタル Neoverse の実行後にそれらを強化します。より厳密な境界 (`--compare-tolerance 0.20` など) をキャプチャする場合は、`--compare-tolerance` を明示的に渡し、代替参照ホストに注釈を付けるには `--compare-label` を使用します。
- CI リファレンス マシンに記録されたベースラインは、`crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`、`sm_perf_baseline_aarch64_macos_auto.json`、および `sm_perf_baseline_aarch64_macos_neon_force.json` に保存されるようになりました。 `scripts/sm_perf.sh --mode scalar --write-baseline`、`--mode auto --write-baseline`、または `--mode neon-force --write-baseline` (キャプチャする前に `SM_PERF_CPU_LABEL` を設定) でそれらを更新し、生成された JSON を実行ログと一緒にアーカイブします。レビュー担当者がすべてのサンプルを監査できるように、集約されたヘルパー出力 (`artifacts/.../aggregated.json`) を PR に保存します。 Linux/Neoverse ベースラインは、`artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json` から昇格した `sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json` で出荷されるようになりました (CPU ラベル `neoverse-proxy-macos`、aarch64 macOS/Linux の SM3 比較許容値 0.70)。許容値を厳しくするために利用可能な場合は、ベアメタル Neoverse ホストで再実行します。
- ベースライン JSON ファイルには、ベンチマークごとのガードレールを強化するためのオプションの `tolerances` オブジェクトが含まれるようになりました。例:
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` は、リストされていないベンチマークに対してグローバル CLI 許容値を使用しながら、これらの小数制限 (この例では 8% と 12%) を適用します。
- 比較ガードは、比較ベースラインで `compare_tolerances` を尊重することもできます。これを使用すると、直接ベースライン チェックに対してプライマリ `tolerances` を厳密に保ちながら、スカラー参照 (たとえば、スカラー ベースラインの `\"sm3_vs_sha256_hash/sm3_hash\": 0.70`) に対してより緩やかなデルタを許可できます。- チェックインされた Apple Silicon ベースラインには具体的なガードレールが付属しています。SM2/SM4 の操作では分散に応じて 12 ～ 20% のドリフトが許容されますが、SM3/ChaCha の比較では 8 ～ 12% にとどまります。スカラー ベースラインの `sm3` 許容値は 0.12 に厳しくなりました。 `unknown_linux_gnu` ファイルは、同じ許容マップ (SM3 比較 0.70) と、ベアメタル Neoverse 再実行が利用可能になるまで Linux ゲート用に出荷されることを示すメタデータ メモを使用して、`neoverse-proxy-macos` エクスポートをミラーリングします。
- SM2 署名: オペレーションごとに 298µs (Ed25519: 32µs) ⇒ 約 9.2 倍遅くなります。検証: 267µs (Ed25519: 41µs) ⇒ 約 6.5 倍遅くなります。
- SM3 ハッシュ (4KiB ペイロード): 11.2μs、11.3μs で SHA-256 と実質的に同等 (≈356MiB/s 対 353MiB/s)。
- SM4-GCM シール/オープン (1KiB ペイロード、12 バイトノンス): 15.5μs 対 ChaCha20-Poly1305 で 1.78μs (≈64MiB/s 対 525MiB/s)。
- 再現性を確保するためにキャプチャされたベンチマーク アーティファクト (`target/criterion/sm_perf*`)。 Linux ベースラインは `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (CPU ラベル `neoverse-proxy-macos`、SM3 比較許容値 0.70) から取得されており、許容値を厳しくするためにラボの時間が開始されたら、ベアメタル Neoverse ホスト (`SM-4c.1`) で更新できます。

#### クロスアーキテクチャキャプチャチェックリスト
- **ターゲット マシン** (x86_64 ワークステーション、Neoverse ARM サーバーなど) で `scripts/sm_perf_capture_helper.sh` を実行します。 `--cpu-label <host>` を渡して、キャプチャをスタンプし、(マトリックス モードで実行している場合) ラボ スケジュール用に生成されたプラン/コマンドを事前に設定します。ヘルパーは、次のようなモード固有のコマンドを出力します。
  1. 正しい機能セットを使用して Criterion スイートを実行し、
  2. 中央値を `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json` に書き込みます。
- 最初にスカラー ベースラインをキャプチャし、次に `auto` (および AArch64 プラットフォームでは `neon-force`) のヘルパーを再実行します。レビュー担当者が JSON メタデータ内のホストの詳細を追跡できるように、意味のある `SM_PERF_CPU_LABEL` を使用します。
- 各実行後に、生の `target/criterion/sm_perf*` ディレクトリをアーカイブし、生成されたベースラインとともに PR に含めます。 2 回の連続実行が安定したらすぐに、ベンチマークごとの許容値を厳しくします (参照形式については、`sm_perf_baseline_aarch64_macos_*.json` を参照)。
- このセクションに中央値と許容値を記録し、新しいアーキテクチャがカバーされる場合は `status.md`/`roadmap.md` を更新します。 Linux ベースラインが `neoverse-proxy-macos` キャプチャからチェックインされるようになりました (メタデータには、aarch64-unknown-linux-gnu ゲートへのエクスポートが記録されています)。これらのラボ スロットが利用可能になった場合、フォローアップとしてベアメタル Neoverse/x86_64 ホストで再実行します。

#### ARMv8 SM3/SM4 組み込み関数とスカラー パスの比較
`sm_accel` (`crates/iroha_crypto/src/sm.rs:739` を参照) は、NEON をサポートする SM3/SM4 ヘルパーのランタイム ディスパッチ層を提供します。この機能は 3 つのレベルで保護されています。|レイヤー |コントロール |メモ |
|------|-------|------|
|コンパイル時間 | `--features sm` (`aarch64` で `sm-neon` を自動的に取り込むようになりました) または `sm-neon-force` (テスト/ベンチマーク) | NEON モジュールを構築し、`sm3-neon`/`sm4-neon` をリンクします。 |
|ランタイム自動検出 | `sm4_neon::is_supported()` | AES/PMULL 相当物を公開する CPU (Apple M シリーズ、Neoverse V1/N2 など) にのみ当てはまります。 NEON または FEAT_SM4 をマスクする VM は、スカラー コードにフォールバックします。 |
|演算子オーバーライド | `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`)起動時に適用される構成主導のディスパッチ。 `force-enable` は信頼できる環境でのプロファイリングにのみ使用し、スカラー フォールバックを検証する場合は `force-disable` を優先します。 |

**パフォーマンスエンベロープ (Apple M3 Pro、中央値は `sm_perf_baseline_aarch64_macos_{mode}.json` に記録):**

|モード | SM3 ダイジェスト (4KiB) | SM4-GCM シール (1KiB) |メモ |
|------|------|-----------|------|
|スカラー | 11.6μs | 15.9μs |決定的な RustCrypto パス。 `sm` 機能がコンパイルされるすべての場所で使用されますが、NEON は使用できません。 |
|ネオンオート |スカラーよりも約 2.7 倍高速 |スカラーよりも約 2.3 倍高速 |現在の NEON カーネル (SM-5a.2c) は、スケジュールを一度に 4 ワードずつ拡張し、デュアル キュー ファンアウトを使用します。正確な中央値はホストごとに異なるため、ベースラインの JSON メタデータを参照してください。 |
|ネオンフォース | NEON auto をミラーリングしますが、フォールバックを完全に無効にします。 NEONオートと同じ | `scripts/sm_perf.sh --mode neon-force` 経由で実行されます。デフォルトでスカラー モードになるホスト上でも CI を正直に保ちます。 |

**決定論と展開ガイダンス**
- 組み込み関数は観察可能な結果を変更しません。`sm_accel` は、アクセラレーションされたパスが利用できないため、スカラー ヘルパーが実行される場合に `None` を返します。したがって、スカラー実装が正しい限り、コンセンサス コード パスは決定論的なままになります。
- NEON パスが使用されたかどうかについてビジネス ロジックをゲート**しない**。加速度を純粋にパフォーマンスのヒントとして扱い、テレメトリのみを介してステータスを公開します (例: `sm_intrinsics_enabled` ゲージ)。
- SM コードに触れた後は常に `ci/check_sm_perf.sh` (または `make check-sm-perf`) を実行して、Criterion ハーネスが各ベースライン JSON に埋め込まれた許容値を使用してスカラー パスとアクセラレーション パスの両方を検証するようにします。
- ベンチマークまたはデバッグを行う場合は、コンパイル時フラグよりも設定ノブ `crypto.sm_intrinsics` を優先します。 `sm-neon-force` を使用して再コンパイルすると、スカラー フォールバックは完全に無効になりますが、`force-enable` は実行時の検出を微調整するだけです。
- 選択したポリシーをリリース ノートに文書化します。実稼働ビルドではポリシーを `Auto` のままにし、同じバイナリ アーティファクトを共有しながら、各バリデータがハードウェア機能を個別に検出できるようにする必要があります。
- 同じディスパッチおよびテスト フローを尊重しない限り、静的にリンクされたベンダー組み込み (サードパーティの SM4 ライブラリなど) を混合するバイナリの出荷を避けてください。そうしないと、ベースライン ツールでパフォーマンスの低下が検出されなくなります。#### x86_64 Rosetta ベースライン (Apple M3 Pro、2025 年 12 月 1 日にキャプチャ)
- ベースラインは `crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`) に存在し、生のキャプチャと集約されたキャプチャは `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/` の下にあります。
- x86_64 でのベンチマークごとの許容値は、SM2 では 20%、Ed25519/SHA-256 では 15%、SM4/ChaCha では 12% に設定されています。 `scripts/sm_perf.sh` では、非 AArch64 ホストでのアクセラレーション比較許容値がデフォルトで 25% に設定されるようになりました。これにより、Neoverse の再実行が完了するまで、AArch64 で共有 `m3-pro-native` ベースラインの 5.25 のスラックを残しつつ、スカラー対自動がタイトなままになります。

|ベンチマーク |スカラー |自動 |ネオンフォース |オート vs スカラー |ネオン vs スカラー |ネオン vs オート |
|----------|----------|------|----------|-----|--------------|--------------|
| sm2_vs_ed25519_sign/ed25519_sign |    57.43 |  57.12 |      55.77 |          -0.53% |         -2.88% |        -2.36% |
| sm2_vs_ed25519_sign/sm2_sign |   572.76 | 568.71 |     557.83 |          -0.71% |         -2.61% |        -1.91% |
| sm2_vs_ed25519_verify/検証/ed25519 |    69.03 |  68.42 |      66.28 |          -0.88% |         -3.97% |        -3.12% |
| sm2_vs_ed25519_verify/検証/sm2 |   521.73 | 514.50 |     502.17 |          -1.38% |         -3.75% |        -2.40% |
| sm3_vs_sha256_hash/sha256_hash |    16.78 |  16.58 |      16.16 |          -1.19% |         -3.69% |        -2.52% |
| sm3_vs_sha256_hash/sm3_hash |    15.78 |  15.51 |      15.04 |          -1.71% |         -4.69% |        -3.03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt |     1.96 |   1.97 |       1.97 |           0.39% |          0.16% |        -0.23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 |  16.38 |      16.26 |           0.72% |         -0.01% |        -0.72% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1.96 |   2.00 |       1.93 |           2.23% |         -1.14% |        -3.30% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    16.60 |  16.58 |      16.15 |          -0.10% |         -2.66% |        -2.57% |

#### x86_64 / その他の非 aarch64 ターゲット
- 現在のビルドは、x86_64 上の決定論的な RustCrypto スカラー パスのみを依然として出荷します。 `sm` を有効のままにしますが、SM-4c.1b が導入されるまで外部 AVX2/VAES カーネルを挿入しないでください。ランタイム ポリシーは ARM をミラーリングします。デフォルトは `Auto`、`crypto.sm_intrinsics` を尊重し、同じテレメトリ ゲージを表示します。
- Linux/x86_64 キャプチャは記録されたままです。そのハードウェアでヘルパーを再利用し、上記のロゼッタ ベースラインと許容値マップと並んで中央値を `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json` にドロップします。**よくある落とし穴**
1. **仮想化された ARM インスタンス:** 多くのクラウドは NEON を公開しますが、`sm4_neon::is_supported()` がチェックする SM4/AES 拡張機能を非表示にします。これらの環境でのスカラー パスを予測し、それに応じてパフォーマンス ベースラインをキャプチャします。
2. **部分的なオーバーライド:** 実行間で永続的な `crypto.sm_intrinsics` 値を混合すると、パフォーマンスの読み取り値が不一致になります。新しいベースラインを取得する前に、意図したオーバーライドを実験チケットに文書化し、構成をリセットします。
3. **CI パリティ:** 一部の macOS ランナーでは、NEON がアクティブな間、カウンターベースのパフォーマンス サンプリングが許可されません。 `scripts/sm_perf_capture_helper.sh` 出力を PR に添付しておくと、ランナーがこれらのカウンターを非表示にしても、レビュー担当者が加速パスが実行されたことを確認できます。
4. **将来の ISA バリアント (SVE/SVE2):** 現在のカーネルは NEON レーン形状を想定しています。 SVE/SVE2 に移植する前に、専用のバリアントで `sm_accel::NeonPolicy` を拡張し、CI、テレメトリ、およびオペレータ ノブを調整できるようにします。

SM-5a/SM-4c.1 で追跡されるアクション アイテムにより、CI はすべての新しいアーキテクチャのパリティ証明を確実に取得し、Neoverse/x86 ベースラインと NEON 対スカラーの許容差が収束するまでロードマップは 🈺 に留まります。

## コンプライアンスと規制に関する注意事項

### 標準と規範的参照
- **GM/T 0002-2012** (SM4)、**GM/T 0003-2012** + **GB/T 32918 シリーズ** (SM2)、**GM/T 0004-2012** + **GB/T 32905/32907** (SM3)、および **RFC 8998** がアルゴリズム定義、テストを管理します。フィクスチャが消費するベクトルと KDF バインディング。【docs/source/crypto/sm_vectors.md#L79】
- `docs/source/crypto/sm_compliance_brief.md` のコンプライアンス概要は、エンジニアリング、SRE、および法務チームの提出/輸出責任と並行して、これらの標準を相互リンクしています。 GM/T カタログが改訂されるたびに、この概要を更新してください。

### 中国本土の規制ワークフロー
1. **製品提出 (开发备案):** SM 対応バイナリを中国本土から出荷する前に、アーティファクト マニフェスト、決定論的ビルド ステップ、および依存関係リストを省暗号管理局に提出します。ファイリング テンプレートとコンプライアンス チェックリストは、`docs/source/crypto/sm_compliance_brief.md` および添付ファイル ディレクトリ (`sm_product_filing_template.md`、`sm_sales_usage_filing_template.md`、`sm_export_statement_template.md`) にあります。
2. **販売/使用申請 (销售/使用計画):** 陸上で SM 対応ノードを実行している事業者は、展開範囲、主要な管理体制、およびテレメトリ計画を登録する必要があります。提出する際には、署名付きマニフェストと `iroha_sm_*` メトリック スナップショットを添付してください。
3. **認定テスト:** 重要なインフラストラクチャのオペレーターは、認定されたラボレポートを必要とする場合があります。再現可能なビルド スクリプト、SBOM エクスポート、および Wycheproof/相互運用アーティファクト (以下を参照) を提供して、下流の監査人がコードを変更せずにベクターを再現できるようにします。
4. **ステータス追跡:** 完了した申請をリリース チケットと `status.md` に記録します。ファイリングが不足していると、検証のみから署名パイロットへの昇格が妨げられます。### 輸出と流通の姿勢
- SM 対応バイナリを **US EAR カテゴリ 5 パート 2** および **EU 規則 2021/821 Annex 1 (5D002)** に基づく規制品目として扱います。ソースの公開は引き続きオープンソース/ENC カーブアウトの対象となりますが、禁輸対象国への再配布には依然として法的審査が必要です。
- リリース マニフェストには、ENC/TSU ベースを参照するエクスポート ステートメントをバンドルし、FFI プレビューがパッケージ化されている場合は OpenSSL/Tongsuo ビルド識別子をリストする必要があります。
- 通信事業者が国境を越えた転送の問題を回避するために陸上配送が必要な場合は、地域ローカルのパッケージング (例: 本土ミラー) を好みます。

### オペレーターの文書と証拠
- このアーキテクチャの概要を、`docs/source/crypto/sm_operator_rollout.md` のロールアウト チェックリストおよび `docs/source/crypto/sm_compliance_brief.md` のコンプライアンス ファイリング ガイドと組み合わせます。
- ジェネシス/オペレーターのクイックスタートを `docs/genesis.md`、`docs/genesis.he.md`、および `docs/genesis.ja.md` 間で同期させます。 SM2/SM3 CLI ワークフローには、`crypto` マニフェストをシードするためのオペレータ向けの信頼できる情報源があります。
- OpenSSL/Tongsuo の出所、`scripts/sm_openssl_smoke.sh` 出力、および `scripts/sm_interop_matrix.sh` パリティ ログをすべてのリリース バンドルでアーカイブするため、コンプライアンスおよび監査パートナーは決定的なアーティファクトを取得できます。
- プログラムの状態を検出可能な状態に保つために、コンプライアンスの範囲が変更されるたびに (新しい管轄区域、提出の完了、または輸出の決定)、`status.md` を更新します。
- `docs/source/release_dual_track_runbook.md` でキャプチャされた段階的な準備レビュー (`SM-RR1` ～ `SM-RR3`) に従ってください。検証のみ、パイロット、および GA 署名フェーズ間のプロモーションには、そこに列挙されているアーティファクトが必要です。

## 相互運用レシピ

### RustCrypto ↔ OpenSSL/Tongsuo マトリックス
1. OpenSSL/Tongsuo CLI が使用可能であることを確認します (`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` では明示的なツールの選択が可能です)。
2. `scripts/sm_interop_matrix.sh` を実行します。 `cargo test -p iroha_crypto --test sm_cli_matrix --features sm` を呼び出し、各プロバイダーに対して SM2 署名/検証、SM3 ダイジェスト、および SM4 ECB/GCM フローを実行し、存在しない CLI をスキップします。【scripts/sm_interop_matrix.sh#L1】
3. 結果として得られる `target/debug/deps/sm_cli_matrix*.log` ファイルをリリース アーティファクトとともにアーカイブします。

### OpenSSL プレビュー スモーク (パッケージング ゲート)
1. OpenSSL ≥3.0 開発ヘッダーをインストールし、`pkg-config` がそれらを見つけられることを確認します。
2. `scripts/sm_openssl_smoke.sh` を実行します。ヘルパーは `cargo check`/`cargo test --test sm_openssl_smoke` を実行し、SM3 ハッシュ、SM2 検証、および FFI バックエンドを介した SM4-GCM ラウンドトリップを実行します (テスト ハーネスはプレビューを明示的に有効にします)。【scripts/sm_openssl_smoke.sh#L1】
3. スキップ以外の失敗はリリース ブロッカーとして扱います。監査証拠としてコンソール出力をキャプチャします。

### 確定的なフィクスチャのリフレッシュ
- 各コンプライアンス申請の前に SM フィクスチャ (`sm_vectors.md`、`fixtures/sm/…`) を再生成し、パリティ マトリックスとスモーク ハーネスを再実行して、監査人が申請と一緒に新しい決定論的な記録を受け取るようにします。## 外部監査の準備
- `docs/source/crypto/sm_audit_brief.md` は、外部レビューのコンテキスト、範囲、スケジュール、連絡先をパッケージ化します。
- 監査アーティファクトは、`docs/source/crypto/attachments/` (OpenSSL スモーク ログ、カーゴ ツリー スナップショット、カーゴ メタデータ エクスポート、ツールキット来歴) および `fuzz/sm_corpus_manifest.json` (既存の回帰ベクトルをソースとする決定論的 SM ファズ シード) の下に存在します。 macOS では現在、ワークスペースの依存関係サイクルにより `cargo check` が妨げられているため、スモーク ログにスキップされた実行が記録されます。サイクルのない Linux ビルドでは、プレビュー バックエンドが完全に実行されます。
- RFQ の発送に先立って調整するために、2026 年 1 月 30 日に暗号 WG、プラットフォーム運用、セキュリティ、およびドキュメント/開発責任者に回覧されました。

### 監査の実施状況

- **ビットの軌跡 (CN 暗号化の実践)** — 作業記述書は **2026-02-21** に実行され、キックオフ **2026-02-24**、フィールドワーク期間 **2026-02-24–2026-03-22**、最終報告書の期限 **2026-04-15**。毎週水曜日の 09:00UTC に、Crypto WG リーダーとセキュリティ エンジニアリング担当者による週次ステータス チェックポイントが行われます。連絡先、成果物、証拠の添付ファイルについては、[`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) を参照してください。
- **NCC グループ APAC (緊急枠)** — 追加の調査結果や規制当局の要請によりセカンドオピニオンが必要になった場合に備えて、フォローアップ/並行レビューとして 2026 年 5 月の期間を予約しました。エンゲージメントの詳細とエスカレーション フックは、`sm_audit_brief.md` の Trail of Bits エントリと一緒に記録されます。

## リスクと軽減策

完全なレジスタ: 詳細については、[`sm_risk_register.md`](sm_risk_register.md) を参照してください。
確率/影響スコアリング、モニタリングトリガー、サインオフ履歴。の
以下の概要は、リリース エンジニアリングに浮上した見出し項目を追跡しています。
|リスク |重大度 |オーナー |緩和 |
|------|----------|------||----------|
| RustCrypto SM クレートに対する外部監査の欠如 |高 |暗号WG | Bits/NCC Group の契約証跡。監査報告書が受理されるまで検証のみを保持します。 |
| SDK 間の決定論的なノンス回帰 |高 | SDK プログラム リード | SDK CI 全体でフィクスチャを共有します。正規の r∥s エンコーディングを強制します。クロス SDK 統合テストを追加します (SM-3c で追跡)。 |
|組み込み関数における ISA 固有のバグ |中 |パフォーマンスWG |機能ゲート組み込み、ARM 上の CI カバレッジが必要、ソフトウェア フォールバックの維持。ハードウェア検証マトリックスは `sm_perf.md` で維持されます。 |
|コンプライアンスの曖昧さが導入を遅らせる |中 |書類と法的連絡 | GA の前にコンプライアンス概要とオペレーター チェックリスト (SM-6a/SM-6b) を公開します。法的な情報を収集します。ファイリング チェックリストは `sm_compliance_brief.md` として出荷されます。 |
|プロバイダーの更新による FFI バックエンドのドリフト |中 |プラットフォーム運用 |プロバイダーのバージョンを固定し、パリティ テストを追加し、パッケージが安定するまで FFI バックエンドのオプトインを維持します (SM-P3)。 |## 未解決の質問/フォローアップ
1. Rust の SM アルゴリズムに経験のある独立監査パートナーを選択します。
   - **回答 (2026-02-24):** Trail of Bits の CN 暗号化業務は、一次監査 SOW (キックオフ 2026 年 2 月 24 日、納品 2026 年 4 月 15 日) に署名しており、NCC グループ APAC は 5 月の緊急枠を確保しているため、規制当局は調達を再開することなく 2 回目の審査を要求できます。エンゲージメント スコープ、連絡先、チェックリストは [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) に存在し、`sm_audit_vendor_landscape.md` にミラーリングされます。
2. 公式 Wycheproof SM2 データセットを求めてアップストリームの追跡を続けます。ワークスペースは現在、厳選された 52 ケース スイート (確定的フィクスチャ + 合成されたタンパー ケース) を出荷し、それを `sm2_wycheproof.rs`/`sm2_fuzz.rs` にフィードします。上流の JSON が到着したら、`cargo xtask sm-wycheproof-sync` 経由でコーパスを更新します。
   - Bouncy Castle および GmSSL ネガティブ ベクター スイートを追跡します。ライセンスが承認されたら、既存のコーパスを補完するために `sm2_fuzz.rs` にインポートします。
3. SM 導入監視のためのベースライン テレメトリ (メトリクス、ログ) を定義します。
4. Kotodama/VM エクスポージャの SM4 AEAD デフォルトが GCM であるか CCM であるかを決定します。
5. 付属例 1 (ID `ALICE123@YAHOO.COM`) の RustCrypto/OpenSSL パリティを追跡: フィクスチャを回帰テストに昇格できるように、公開された公開鍵と `(r, s)` のライブラリ サポートを確認します。

## アクションアイテム
- [x] 依存関係の監査を終了し、セキュリティ トラッカーでキャプチャします。
- [x] RustCrypto SM クレートに対する監査パートナーの関与を確認します (SM-P0 フォローアップ)。 Trail of Bits (CN 暗号化実務) は、`sm_audit_brief.md` に記録されたキックオフ/納品日による一次レビューを所有し、NCC Group APAC は規制当局またはガバナンスのフォローアップを満たすために 2026 年 5 月の緊急枠を保持しました。
- [x] SM4 CCM 改ざんケース (SM-4a) の Wycheproof 適用範囲を拡張します。
- [x] 正規の SM2 署名フィクスチャを SDK 全体に配置し、CI (SM-3c/SM-1b.1) に接続します。 `scripts/check_sm2_sdk_fixtures.py` によって保護されています (`ci/check_sm2_sdk_fixtures.sh` を参照)。

## コンプライアンス付録 (国家商用暗号化)

- **分類:** SM2/SM3/SM4 は、中国の *国家商業暗号* 制度 (中国暗号法、第 3 条) に基づいて出荷されます。これらのアルゴリズムを Iroha ソフトウェアで出荷しても、プロジェクトはコア/共通 (国家機密) 層に**ありません**。ただし、中国の展開でそれらのアルゴリズムを使用するオペレーターは、商用暗号ファイルの提出と MLPS の義務に従う必要があります。【docs/source/crypto/sm_chinese_crypto_law_brief.md:14】
- **標準の系譜:** 公開ドキュメントを GM/T 仕様の公式 GB/T 変換と一致させます。

|アルゴリズム | GB/T リファレンス | GM/T の起源 |メモ |
|----------|-----|---------------|------|
| SM2 | GB/T32918 (全パーツ) | GM/T0003 | ECC デジタル署名 + 鍵交換。 Iroha は、コア ノードでの検証と SDK への確定的署名を公開します。 |
| SM3 | GB/T32905 | GM/T0004 | 256ビットハッシュ。スカラー パスと ARMv8 アクセラレーション パスにわたる決定論的ハッシュ。 |
| SM4 | GB/T32907 | GM/T0002 | 128 ビットのブロック暗号。 Iroha は GCM/CCM ヘルパーを提供し、実装全体でビッグエンディアンのパリティを保証します。 |- **機能マニフェスト:** Torii `/v1/node/capabilities` エンドポイントは次の JSON 形式をアドバタイズするため、オペレーターとツールはプログラムで SM マニフェストを使用できます。

```json
{
  "supported_abi_versions": [1],
  "default_compile_target": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

CLI サブコマンド `iroha runtime capabilities` は、同じペイロードをローカルに表示し、コンプライアンス証拠収集のために JSON 広告と一緒に 1 行の概要を出力します。

- **ドキュメントの成果物:** 上記のアルゴリズム/標準を特定するリリース ノートと SBOM を公開し、完全なコンプライアンス概要 (`sm_chinese_crypto_law_brief.md`) をリリース アーティファクトにバンドルして保管し、オペレーターが州の提出書類に添付できるようにします。【docs/source/crypto/sm_chinese_crypto_law_brief.md:59】
- **オペレーターのハンドオフ:** MLPS2.0/GB/T39786-2021 では、暗号化アプリケーションの評価、SM キー管理 SOP、および 6 年以上の証拠保持が必要であることを導入担当者に思い出させます。コンプライアンス概要のオペレータ チェックリストを参照してください。【docs/source/crypto/sm_chinese_crypto_law_brief.md:43】【docs/source/crypto/sm_chinese_crypto_law_brief.md:74】

## コミュニケーション計画
- **対象者:** 暗号 WG コア メンバー、リリース エンジニアリング、セキュリティ レビュー委員会、SDK プログラム リード。
- **アーティファクト:** `sm_program.md`、`sm_lock_refresh_plan.md`、`sm_vectors.md`、`sm_wg_sync_template.md`、ロードマップの抜粋 (SM-0 .. SM-7a)。
- **チャネル:** 毎週の Crypto WG 同期アジェンダ + アクション アイテムを要約し、ロックの更新と依存関係の取り込みの承認を要求するフォローアップ メール (草案は 2025 年 1 月 19 日に回覧されました)。
- **オーナー:** 暗号 WG リーダー (代理人可)。
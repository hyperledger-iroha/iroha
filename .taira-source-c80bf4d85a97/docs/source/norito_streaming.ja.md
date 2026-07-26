<!-- Japanese translation of docs/source/norito_streaming.md -->

---
lang: ja
direction: ltr
source: docs/source/norito_streaming.md
status: complete
translator: manual
---

# Norito ストリーミング（`norito::streaming`）

このモジュールは Norito Streaming Codec (NSC) で使用されるオンワイヤ構造、ヘルパー、および基準コーデックを実装します。型はリポジトリルートの `norito_streaming.md` にある仕様と 1 対 1 に対応します。

## マニフェストとコントロールフレーム

`ManifestV1`、`PrivacyRoute*`、各種コントロールフレーム列挙型は、パブリッシャー・リレー・ビューアが利用するシリアライズ面を公開します。これらはプレーンなデータコンテナに過ぎず、署名検証やチケットポリシー整合性の確認などは上位レイヤーが実施する必要があります。

## セグメントのエンコードと検証

`streaming::chunk` にはチャンクハッシュ、Merkle 証明、データ可用性コミットメントのユーティリティが含まれます。`streaming::codec` サブモジュールはテストおよび参照実装として使える基準エンコーダ／デコーダを提供します。

- `BaselineEncoder::encode_segment` は `SegmentHeader`、`ChunkDescriptor`、チャンクペイロードから成る `EncodedSegment` を生成します。チャンク ID の単調性、フレームタイムスタンプのオーバーフロー、`duration_ns` が設定値を守るかを検証します。
- `EncodedSegment::verify_manifest` はチャンクコミットメントを `ManifestV1` と照合し、パブリッシャーの主張を検証します。
- `BaselineDecoder::decode_segment` はコミットメント検証に加えて、フレーム PTS (`pts_ns`) が `timeline_start_ns + i * frame_duration_ns` の算術通りかをチェックし、ドリフトを拒否します。

これらのチェックは仕様書「Segment Layout」の要件に対応し、`SegmentError`、`CodecError` を通じて診断しやすいエラーを提供します。

## キー管理

`streaming::StreamingSession` はコントロールプレーンハンドシェイクのヘルパーを提供します。`build_key_update` は Ed25519 アイデンティティ鍵で制御フレームに署名し、スイート固有のエフェメラルペイロードを生成します。`process_remote_key_update` は署名検証、カウンタの単調性確認、セッショントランスポートキーの確立を行います。Kyber ベースの HPKE ハンドシェイクは `set_kyber_remote_public` / `set_kyber_local_secret` を通じて静的鍵素材を設定します。フィンガープリントは `nsc_kyber_pk` ドメインと `Sha3-256` を用いて計算します（将来的に BLAKE3 へ移行予定）。

`streaming::StreamingKeyMaterial` はノード所有の Ed25519 アイデンティティ鍵ペアと任意の Kyber 鍵素材をラップします。`set_kyber_keys` は提供されたバイト列を検証し、Kyber768 フィンガープリントをキャッシュし、ドロップ時に秘密鍵をゼロ化します。新しい `StreamingSession` には `install_into_session` でローカル Kyber 秘密鍵を設定し、`build_key_update` でアイデンティティ鍵を再利用して署名できます。設定ファイルでは `streaming.kyber_public_key` と `streaming.kyber_secret_key` に 16 進文字列を指定し、起動時に `StreamingHandle::with_key_material` に渡します。片方のみを指定することはできません。併せて `streaming.kyber_suite` で想定する ML-KEM プロファイル（`mlkem512` / `mlkem768`〈デフォルト〉/ `mlkem1024`。`kyber512/768/1024` も許容）を選択できます。現在のハンドシェイクは `mlkem768` のみをネゴシエートし、その他のスイートは将来のアップグレード向けに予約されていますが、設定段階で指定しておくと運用チームが対応する鍵長を用意できます。ノードが Ed25519 以外のバリデータ鍵を使用している場合でも、`streaming.identity_public_key` / `streaming.identity_private_key` に Ed25519 multihash を指定することでコントロールプレーン署名を満たせます。省略時はメイン鍵ペアが再利用されます。`streaming.session_store_dir` は暗号化されたセッションスナップショットの保存先（既定 `./storage/streaming`）であり、スナップショット暗号鍵はストリーミングアイデンティティから決定論的に導出されます。`StreamingHandle::load_snapshots()` は再起動後にネゴシエート済みトランスポート鍵と最新 GCK メタデータを漏えいなく復元できます。`StreamingSession::snapshot_state` / `restore_from_snapshot` は Norito エンコードされたコンパクトな blob に `{session_id, key_counter, sts_root}` とネゴシエート済みスイート、Kyber フィンガープリント、ケイデンス状態、最新 GCK メタデータを含め、再起動後もコントロールストリームを再生せずに合流できます。スナップショットは `kura/store_dir/streaming_sessions/sessions.norito` に保存され、Ed25519 アイデンティティ由来の ChaCha20-Poly1305 鍵で暗号化されます。

## コントロールフレーム列挙

`ControlFrame` 列挙は Norito TLV で表現される制御メッセージ群を定義します。`KeyUpdate` は HPKE ペイロード、`ManifestAnnounce` はマニフェストとケイデンス、`FeedbackHint` / `ReceiverReport` は FEC フィードバックを運びます。未知バリアントは `ControlFrameError::UnknownVariant` として拒否します。

## 機能とネゴシエーション

- `CapabilityReport` はサポート機能をビットマスクで広告し、`CapabilityAck` が交渉結果を確定させます。両者は Norito エンコードされ `ControlFrame::Capability` として輸送されます。
- 既知ビットは HPKE スイート、DATAGRAM サポート、プライバシーバケット、FEC 最小間隔などを示します。未知ビットはプロトコル違反として扱い、コーデック面の決定論を維持します。
- `streaming.feature_bits`（既定 `0b11`）がパブリッシャーの広告マスクを制御し、ビューアは未広告のビットを要求してはなりません。
- メディアは DATAGRAM が利用可能なら DATAGRAM を、そうでなければセグメントウィンドウ単位の単方向ストリームを使用します。コントロールメッセージは最優先の双方向ストリームで扱います。
- MTU 交渉は `StreamingConnection::max_datagram_size` / `datagram_enabled` を通じて公開され、ゼロ長 DATAGRAM を許容しません。統合テストは DATAGRAM 有効・無効の経路をカバーします。
- 制約の厳しいネットワークでは `streaming.feature_bits` を調整し、チャンクフレーミングとプライバシーマスクが経路差によらず同一となるよう保証する必要があります。

## 輻輳制御と FEC モデル

- 輻輳制御は BBRv1 固定 ( `startup_gain = 2.885`, `drain_gain = 1/2.885`, パーシングゲイン `[1.25, 0.75, 1.0, …]`, 最小 RTT 5 ms)。
- ビューアは `FeedbackHint` を所定の間隔で送信し、Q16.16 固定小数で `(loss_ewma, latency_grad, observed_rtt_ms)` を算出します。3 回ごとに `ReceiverReport` を送り `parity_applied` と予算を報告します。
- パブリッシャーは `parity_chunks = clamp(ceil((loss_ewma * 1.25 + 0.005) * 12), 0, 6)` でパリティを計算し、冗長度変更はテレメトリに記録します。冗長度はセッション内で減少しません。
- `ManifestPublisher` は `StreamingHandle::populate_manifest` でネゴシエート済み機能ハッシュ、ケイデンス、単調パリティをマニフェストに反映し、フィードバック不在時はフォールバック値を使用します。

## テレメトリとプライバシー

- セッション ID とマニフェスト ID はドメイン分離済み BLAKE3 でハッシュ化してからメトリクスに出力します。
- 主なカウンタ: `streaming_hpke_rekeys_total{suite_id}`、`streaming_quic_datagrams_sent_total`、`streaming_quic_datagrams_dropped_total`、`streaming_fec_parity_current{bucket}`、`streaming_feedback_timeout_total`、`streaming_privacy_redaction_fail_total`。
- エンコード／デコード／ネットワーク／エネルギーのテレメトリはヒストグラムとカウンタ、並びに `TelemetryEvent::{Encode, Decode, Network, Energy, AuditOutcome}` に出力されます。
- 損失 EWMA バケット `[0,0.01)`, `[0.01,0.05)`, `[0.05,0.1)`, `[0.1,0.2)`, `[0.2,0.4)`, `[0.4,1.0]`、レイテンシ勾配 `<-5`, `[-5,-1)`, `[-1,1)`, `[1,5)`, `[5,10)`, `>=10`、FEC パリティ `0`, `1`, `2`, `3`, `4`, `>=5` を使用します。
- 生のフィードバックサンプルは暗号化スナップショットにのみ存在し、36 時間以内またはマニフェスト失効時に削除されます。エクスポートは 1 分に 1 回までに制限されます。
- HPKE リキーや GCK ローテーション時に `TelemetryEvent::Security` で累積カウンタと使用スイートを通知し、監査を支援します。
- TRACE 監査の完了時には `TelemetryEvent::AuditOutcome` が `trace_id`、スロット高 (`slot_height`)、レビュア、ステータス、および任意の `mitigation_url` を届けます。

## クロス言語コンフォーマンスベクトル

Milestone D7 のクロス言語ハーネスはリポジトリに同梱されており、各 SDK チームがすぐに利用できます。

- 正準の Rust 統合テストは `integration_tests/tests/norito_streaming_{end_to_end,feedback,fec,negative}.rs` にあり、共有ヘルパーは `integration_tests/tests/streaming/mod.rs` にまとまっています。これらのテストはパブリッシャー／ビューアのハンドシェイク、パリティ復旧、エラーパスを決定論的スケジュールで検証します。
- テストフィクスチャは `integration_tests/fixtures/norito_streaming/rans/baseline.json` および `docs/assets/nsc/conformance/` 以下に公開されています。各 SDK は JSON ベクトルを読み込み、`docs/assets/nsc/conformance/entropy.json` に記載されたダイジェストと一致させてください。
- JavaScript での統合手順は `docs/source/sdk/js/quickstart.md` の「Torii Queries & Streaming」節にまとまっています。他言語の SDK も同じフィクスチャを用いた往復試験と統計を実装し、Rust ハーネスと同じ指標をコンフォーマンスレポートに含める必要があります。

別言語からハーネスを再利用する場合は、`integration_tests/tests/streaming/mod.rs::baseline_test_vector_with_frames` が生成するマニフェストとフレーム列を再現し、コンフォーマンスバンドルのハッシュと照合してください。CI では `cargo test -p integration_tests --tests norito_streaming_*` を参照オラクルとして実行できます。

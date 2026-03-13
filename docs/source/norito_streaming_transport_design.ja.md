<!-- Japanese translation of docs/source/norito_streaming_transport_design.md -->

---
lang: ja
direction: ltr
source: docs/source/norito_streaming_transport_design.md
status: complete
translator: manual
---

# Norito ストリーミング輸送 & コントロールプレーン設計

## 目的

本設計ノートは、ロードマップ Milestone D2–D7（Norito ストリーミング）を実現するために必要な輸送、暗号、可観測性の選択肢を統合したものです。`norito_streaming.md` に記載された正規のコーデック仕様を繰り返すのではなく、Rust ワークスペース内でコントロールプレーン、QUIC トランスポート、テレメトリ層を実装する際に従うべき具体的な選択を示します。

## スコープ

- `StreamingSession` における HPKE プロファイル選定、鍵ライフタイム、ハンドシェイクフレーミング。
- QUIC 機能ネゴシエーション（ストリーム／DATAGRAM の使い分け、優先度、決定論的フォールバック）。
- ハードウェアを問わず同一の冗長決定を保証する FEC（Forward Error Correction）のフィードバック式、タイマ、不変条件。
- オペレーター向けツールおよびロードマップ受け入れ基準で参照されるカウンタを維持しつつテナントプライバシーを守るテレメトリ発行ルール。

以下の選択は `iroha_p2p`、`iroha_crypto::streaming`、`norito::streaming`、Milestone D3–D7 で提供されるオペレーター向けドキュメントに対して拘束力を持ちます。

## HPKE プロファイル

### ゴール

- ポスト量子の前方秘匿性を確保しつつ、ノード間で決定論的挙動を維持する。
- `StreamingSession::snapshot_state` ペイロードとのオンワイヤ互換性を保つ。
- クリティカルパスでのスイート交渉を避け、全ピアが同一の暗号スイート集合に収束するようにする。

### 選択したプロファイル

パブリッシャー／ビューアは以下 2 つの HPKE スイートをリスト順に実装しなければなりません（MUST）。

1. `mode = AuthPsk`, `kem = Kyber768`, `kdf = HKDF-SHA3-256`, `aead = ChaCha20Poly1305`
2. `mode = AuthPsk`, `kem = Kyber1024`, `kdf = HKDF-SHA3-256`, `aead = ChaCha20Poly1305`

スイート #1 は必須使用。スイート #2 は将来のハードウェア向けに予約され、より大きな暗号文を許容できる場合に用います。ノードは [QUIC 機能ネゴシエーション](#quic-機能ネゴシエーション) で定義されるビットセットでサポートを広告しますが、現時点ではすべてのデプロイがスイート #1 を搭載しているため交渉は形式的です。両ピアが `streaming_capabilities.hpke_kyber1024 = true` を設定した場合のみスイート #2 を使用できます。それ以外はパブリッシャーはスイート #1 にフォールバックしなければなりません。

### PSK（Pre-Shared Key）の生成元

PSK は Norito マニフェストのコミットハッシュから決定論的に導出します。

```
psk = BLAKE3("nsc-hpke-psk" || manifest.commitment_hash)
psk_id = manifest.commitment_hash[0..15]
```

`psk_id` はコントロールフレームに埋め込まれ、ビューアは同一の PSK を導出できたか検証します。不一致の場合は `StreamingProtocolError::PskMismatch` を発生させます。マニフェストハッシュは不変であるため、同一ブロードキャストの全ピアがアウトオブバンド共有なしで PSK を共有できます。

### ローテーションとカウンタ

- `StreamingSession::key_counter` は署名済み `KeyUpdate` ごとにインクリメントされます。回転ポリシーは `norito_streaming.md` 5.4 節と同じく「暗号化ペイロード 64 MiB ごと、または 5 分ごと」の早い方を採用します。
- パブリッシャーは更新ごとに HPKE エフェメラル鍵ペアを再生成し、コントロールフレームの `HpkePayload` にエンコードしなければなりません。
- ビューアは、最後に受理したカウンタ以下の `KeyUpdate` を拒否し、ストリームの単調性を保つ必要があります。
- セッションはネゴシエート済みスイート識別子（スイート #1 は `0x0001`、スイート #2 は `0x0002`）をスナップショットに記録します。復元時にはカウンタとスイートを再生し、再開したビューアが異なるプロファイルを選択することを防ぎます。

### 決定論に関する注意

- Kyber カプセル化は FIPS 203 8.5 節で定義された決定論的バリアントを使用します。乱数コインは `BLAKE3("nsc-kyber-coins" || session_id || key_counter || role)` から導出し、再起動やハードウェア差異でも署名を再現可能にします。
- ChaCha20Poly1305 のノンスは QUIC DATAGRAM 層が発行する `sequence_number` を利用します。両ピアは 96 ビットノンスをゼロパディングし、HPKE コンテキストから導出した共有ソルトと XOR して決定論的な AEAD 入力を生成します。

## QUIC 機能ネゴシエーション

### コントロールストリームレイアウト

- ストリーム `0x0` で QUIC ハンドシェイク時に Norito エンコードされた `TransportCapabilities` フレームを送受信します。フレームは以下を列挙します。
  - `hpke_suite_mask`（bit0=Kyber768、bit1=Kyber1024）
  - `supports_datagram`（ブール）
  - `max_segment_datagram_size`（バイト単位の上限）
  - `fec_feedback_interval_ms`
  - `privacy_bucket_granularity`（後述のテレメトリ規則に対応）
- フレーム交換は方向ごとに 1 回のみ。パブリッシャーは最初のフレームをメディア DATAGRAM より前に送信し、ビューアはフレーム処理前にフィードバックを送ってはいけません。

### 機能解決

1. 双方が `TransportCapabilities` を解析。
2. 解決された HPKE スイートは両マスクの共通ビット中、最小インデックスのものです。共通ビットがない場合は `TransportError::MissingHpkeSuite` で接続を拒否します。
3. `supports_datagram` は双方が true のときのみ true。false の場合、パブリッシャーは順序保証された単方向ストリームを使用し、後述の優先度に従います。
4. `max_segment_datagram_size` は両者の最小値に収束。パブリッシャーは全ビューアで同一パケット化となるよう断片化します。
5. `fec_feedback_interval_ms` は最大値に収束。遅いフィードバックに統一することで低速ハードウェアでも計算予算を揃えます。

標準デプロイでは `streaming.feature_bits = 0b11` を広告し、フィードバックヒント（bit0）とプライバシーオーバレイサポート（bit1）を示します。オペレーターが機能を無効化する際はマスクを下げられますが、広告外のビットを要求するビューアはハンドシェイクで拒否しなければなりません。

### ストリーム／DATAGRAM の使用法

- DATAGRAM が有効な場合、各セグメントウィンドウを連続した DATAGRAM シーケンスに割り当て、最初にチャンク、その後にパリティシャードを送信します。シーケンス番号を飛ばしてはならず、決定論保持のため必要に応じてセッション ID のみを載せたパディング DATAGRAM を挿入します。
- ストリームへフォールバックする際、パブリッシャーはウィンドウごとに単方向ストリームを 1 本開きます。チャンクとパリティシャードは `chunk_id`、`is_parity`、`payload_len` を含む決定論的ヘッダーで順序付けされます。ストリームオフセットは単調増加し、再送は QUIC に委ねます。
- コントロールメッセージ（`KeyUpdate`、`FeedbackHint`、`ReceiverReport`）は専用の双方向ストリームで送信し、優先度 256 とします。メディアストリームは 128、アーカイブ／マニフェストストリームは 64 を使用し、リキーとフィードバックがスケジューラ負荷下でも決定論的に処理されるようにします。

### 機能ハッシュ

マニフェストの再現性を保証するため、解決済み機能タプル `(suite_id, datagram, max_dgram, fec_interval)` から `transport_capabilities_hash` を算出します。

```
BLAKE3("nsc-transport-capabilities" || le_u16(suite_id) || u8(datagram) || le_u16(max_dgram) || le_u16(fec_interval))
```

ジェネシスマニフェストやオンチェーンバリデータは、新しいパブリッシャーを受け入れる際にこのハッシュを照合します。

## 決定論的 FEC フィードバック

### フィードバックメッセージ

- ビューアは `fec_feedback_interval_ms` ごとに `FeedbackHint` フレームを送信し、セッション開始からの間隔境界に揃えます。ヒントには `(loss_ewma, latency_gradient, observed_rtt_ms)` を含みます。
- 3 回に 1 回、つまり `3 * fec_feedback_interval_ms` ごとに `ReceiverReport` を追加送信し、`delivered_sequence`、`parity_applied`、`fec_budget` を報告します。
- パブリッシャーは集約した損失 EWMA とビューアカウンタを `StreamingSession::feedback_state` に保存します。`ManifestPublisher`（`iroha_core::streaming::ManifestPublisher`）は `StreamingHandle::populate_manifest` をラップし、マニフェスト生成時にネゴシエート済み機能ハッシュを注入しつつ、受信フィードバックから単調増加するパリティとケイデンスを算出します。まだフィードバックが無い場合はフォールバック値を使用します。
- リアルタイム会議モードでは `Kaigi` ISI（`CreateKaigi`、`JoinKaigi`、`LeaveKaigi`、`EndKaigi`、`RecordKaigiUsage`）を用いて Nexus 上のルームを調整します。呼状態は `kaigi__` プレフィックスのドメインメタデータに保存され、マニフェスト／課金／参加者更新を決定論的かつ標準メタデータ購読者が取得可能にします。

### 損失推定

- 指数移動平均（EWMA）の係数は `alpha = 0.2`。ビューアは区間ごとの損失率（DATAGRAM シーケンスギャップまたはストリーム時の QUIC ACK レンジ）から EWMA を求めます。
- 浮動小数による差異を避けるため Q16.16 固定小数で計算します。

```
loss_ewma_fp = loss_ewma_fp + ((sample_fp - loss_ewma_fp) * alpha_fp) >> 16
```

ここで `alpha_fp = round(0.2 * 2^16) = 13107`。パブリッシャー側も同じ演算でフィードバックを統合します。

### 冗長度計算

- パブリッシャーは次ウィンドウ（`w = 12` チャンク）のパリティ数を以下で求めます。

```
parity = clamp(ceil((loss_ewma * 1.25 + 0.005) * w), 0, 6)
```

- すべて Q16.16 で実装し、`ceil` は `(value_fp + 0xFFFF) >> 16` で表現。定数 `0.005` は固定小数で `327`。
- 計算結果が前ウィンドウと異なる場合、変更を適用する前にテレメトリシンクへログ出力します。パブリッシャーは **決して** パリティを減らしません（最小は前回値）ので、冗長度は単調非減少な振る舞いになります。
- 追加の冗長度は `parity_applied` に記録され、ビューアがレポートした `parity_applied` と照合してパイプラインが整合しているか確認します。

### タイムラインとタイマ

- フィードバックタイマは `StreamingSession::feedback_timer` が管理し、パブリッシャー／ビューアともにセッション開始時刻から決定論的にスケジュールします。
- 低遅延構成に合わせるため、最低タイマ解像度は 10 ms とし、`fec_feedback_interval_ms` はその整数倍。
- 視聴者がタイマより遅れてヒントを発行した場合でも、**次の** 整列時刻で再スケジュールし、ドリフトを抑えます。

## プライバシー対応テレメトリ

- テナント毎のストリーム統計は `privacy_bucket_granularity` で定義されたバケットに丸めます。既定では 30 秒単位で集約し、それより細かいタイムスタンプは公開しません。
- カウンタは `stream_id` ではなく `session_id` + バケットキーでラベル付けし、1 日あたり最大 1024 セッションに制限。これによりメトリクスのレート制限と匿名化が同時に成立します。
- パブリッシャーとビューアは、提供したテレメトリが同一になるよう固定小数の切り捨てルール（負方向切り捨て）を共有します。たとえば平均 RTT は `floor(rtt_ms / 5) * 5` に丸めてから出力します。
- イベントストリームの `StreamingTelemetryEvent` は `tenant_hash` を含みません。Norito の `TenantTelemetry` メッセージにはハッシュ済み ID とバケットキーのみが載ります。

## オペレーター設定

- `iroha_config::StreamingConfig`
  - `hpke_suite_mask`（既定 `0b01`）
  - `supports_datagram`（既定 true）
  - `max_segment_datagram_size`（既定 1408）
  - `fec_feedback_interval_ms`（既定 200）
  - `privacy_bucket_granularity_ms`（既定 30000）
- CLI (`iroha_cli streaming capabilities`) で現在の機能とネゴシエート結果を確認可能。
- ブートストラップマニフェストに `transport_capabilities_hash` を埋め込み、観測者は `/v2/streaming/manifest` を通じて照合できます。

## 依存関係と参照

- `crates/iroha_crypto::streaming/hpke.rs`: プロファイル実装。
- `crates/iroha_p2p/src/streaming/quic`: QUIC 機能ネゴシエーションとストリーム管理。
- `crates/norito/src/streaming`: Norito コントロールフレーム定義。
- `docs/source/norito_streaming.md`: コーデック仕様。

</div>

<!-- Japanese translation of docs/source/kaigi_privacy_design.md -->

---
lang: ja
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
translator: manual
---

# Kaigi プライバシー & リレー設計

このドキュメントは、決定論性や台帳の監査可能性を損なうことなく、ゼロ知識ベースの参加証明とオニオン型リレーを導入するプライバシー拡張の設計をまとめたものです。

# 概要

設計は次の 3 層で構成されます。

- **ロスターのプライバシー** – ホストの権限管理や請求処理を維持したまま、オンチェーンで参加者の実 ID を秘匿する。
- **利用状況の秘匿性** – 区間ごとの詳細を公開せずにホストが課金ログを残せるようにする。
- **オーバーレイリレー** – 複数ホップのピアを経由してパケットを配送し、ネットワーク観測者が通信当事者を把握できないようにする。

追加される機能はすべて Norito を第一級とし、ABI バージョン 1 上で、異なるハードウェア環境でも決定論的に動作する必要があります。

# 目的

1. ゼロ知識証明を用いて参加者の入退室を処理し、台帳上に生のアカウント ID を露出させない。
2. 強固な会計保証を維持する。参加・離脱・利用イベントはすべて決定論的に突合できなければならない。
3. 制御チャネル／データチャネル向けのオニオンルートを表現し、オンチェーンで監査できるリレーマニフェストを任意で提供する。
4. プライバシーを必要としない環境では従来の透過的ロスター（完全公開）にフォールバックできるようにする。
5. `KaigiRecord` を Norito コーデックと後方互換な形で拡張し、データモデルのハードフォークを避ける。

# 脅威モデル概要

- **敵対者:** ネットワーク観測者（ISP など）、好奇心旺盛なバリデータ、悪意のあるリレー運営者、準正直（semi-honest）なホスト
- **保護対象:** 参加者の ID、参加タイミング、区間別の利用・課金情報、ネットワークルーティングメタデータ
- **前提:** ホストはオフチェーンで真の参加者集合を把握している。台帳ピアは決定論的に証明を検証する。オーバーレイリレーは信用できないがレート制限されている。HPKE や SNARK のプリミティブはコードベースに既存。

# データモデルの変更

すべての型は `iroha_data_model::kaigi` に定義されます。

```rust
/// 参加者 ID へのコミットメント（アカウント + ドメインソルトの Poseidon ハッシュ）。
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// 各 Join に固有のヌリファイア。証明の二重使用を防ぐ。
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// オニオンルーティングをセットアップする際に用いるリレーパス。
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` には次のフィールドが追加されます。

- `roster_commitments: Vec<KaigiParticipantCommitment>` – プライバシーモード有効時、公開されていた `participants` リストの代わりになります。移行期間中は両方を併用することも可能です。
- `nullifier_log: Vec<KaigiParticipantNullifier>` – 追記のみのログ。メタデータサイズを制御するローリングウィンドウで上限を設けます。
- `relay_manifest: Option<KaigiRelayManifest>` – Norito でエンコードされた構造化マニフェスト。ホップ、HPKE 鍵、重みを JSON なしで正規化します。
- `privacy_mode: KaigiPrivacyMode` – 下記のモード種別。

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` にも同様の任意フィールドを追加し、作成時点でプライバシーを有効化できるようにします。

Norito スキーマ互換性:

- 各フィールドに `#[norito(with = "...")]` ヘルパーを適用し、整数のリトルエンディアン化やホップ順序の正規化など、カノニカルなエンコードを強制します。
- `KaigiRecord::from_new` は新しいベクタを空で初期化し、渡されたリレーマニフェストをコピーします。
- シリアライズの往復テストで、各プライバシーモードの互換性を保証します。

# 命令インターフェイスの変更

## `CreateKaigi`

- `privacy_mode` がホストの権限と合致するか検証します。
- `relay_manifest` が指定された場合は、ホップ数が 3 以上、重みがゼロでない、HPKE 鍵が存在する、重複がないといった条件を満たすか確認し、オンチェーンの監査可能性を維持します。
- コミットメント／ヌリファイアログを空で初期化します。

## `JoinKaigi`

パラメータ:

- `proof: ZkProof`（Norito バイトラッパー） – 呼び出し元が `(account_id, domain_salt)` を知っており、その Poseidon ハッシュが指定された `commitment` に一致することを示す Groth16 証明
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – 参加者ごとに次ホップを上書きする任意指定

実行手順:

1. `record.privacy_mode == Transparent` の場合は従来挙動にフォールバック。
2. 証明を回路レジストリ `KAIGI_ROSTER_V1` で検証。
3. `record.nullifier_log` に同一ヌリファイアが存在しないことを確認。
4. コミットメント／ヌリファイアを追記。`relay_hint` があれば、オンチェーンではなくセッション内のインメモリ状態で該当参加者のリレー経路を更新。

## `LeaveKaigi`

透過モードでは従来どおり。

プライベートモードでは:

1. 呼び出し元が `record.roster_commitments` 内のコミットメントを知っていることを証明する。
2. ヌリファイアを更新し、一度きりの離脱であることを示す。
3. コミットメント／ヌリファイアを削除する。監査要件はログの保持ポリシーで満たす。

## `RecordKaigiUsage`

- 回路レジストリ `KAIGI_USAGE_V1` に対する証明を検証し、コミットメントが記録された利用量と一致することを確認。
- プライバシーモードでは、イベントに露出するのは集計値とコミットメントのみ。
- 従来モードでは現行動作を継続。

## `SetKaigiRelayManifest`

- 権限者のみ実行可能。
- 新しいマニフェストを Norito スキーマ経由で検証（ホップ数、期限、HPKE 鍵、重みなど）。
- マニフェストを更新または削除し、`KaigiRelayManifestUpdated` イベントを発行。

# プルーフ検証と登録

- Norito レジストリに `KaigiProofRegistry` を追加し、参加・離脱・利用に用いる回路 ID とバージョンを保持。
- `ProofConfig` は `register_verifying_key(role, backend, circuit)` を通じて検証鍵を登録します。`role` には `KaigiRosterJoin` / `KaigiRosterLeave` / `KaigiUsage` を使用。
- `iroha_core::smartcontracts::isi::kaigi::privacy` ではデフォルトで完全なロスター検証を行います。`zk.kaigi_roster_join_vk`（Join 用）、`zk.kaigi_roster_leave_vk`（Leave 用）を設定から解決し、WSV で該当する `VerifyingKeyRef` を取得。レコードが `Active` であること、バックエンド／回路 ID が一致すること、コミットメントが整合することを確認し、バイト課金を行ってから設定済みの ZK バックエンドを呼び出します。
- `kaigi_privacy_mocks` フィーチャは決定論的なスタブ検証器を保持し、単体・統合テストや制約のある CI で Halo2 バックエンドなしに実行できます。本番ビルドではフィーチャを無効化し、実証明を強制します。
- `kaigi_privacy_mocks` がテスト／`debug_assertions` 以外で有効になっている場合、コンパイルエラーを発生させ、本番バイナリにスタブが混入するのを防ぎます。
- オペレーターは (1) ガバナンス経由でロスター検証器セットを登録し、(2) `iroha_config` に `zk.kaigi_roster_join_vk`, `zk.kaigi_roster_leave_vk`, `zk.kaigi_usage_vk` を設定してホストが実行時に参照できるようにする必要があります。鍵が登録されるまでは、プライバシー Join/Leave/Usage は決定論的に失敗します。
- `crates/kaigi_zk` には Halo2 ベースの回路（Join/Leave/Usage コミットメント）と再利用可能なコンプレッサー（`commitment`, `nullifier`, `usage`）を同梱しました。ロスター回路は Merkle ルート（リトルエンディアン 64bit×4 リム）を追加の公開入力として持ち、ホストが証明を検証する前に保存済みロスター根と照合できます。Usage コミットメントは `(duration, gas, segment)` をオンチェーンハッシュに結び付ける `KaigiUsageCommitmentCircuit` で保証します。
- `Join` 回路の入力: `(commitment, nullifier, domain_salt)`（公開）と `(account_id)`（秘密）。公開入力はコミットメント、ヌリファイア、およびコミットメントツリーの Merkle ルート（リトルエンディアン 64bit×4）。
- 決定論性: Poseidon パラメータ、回路バージョン、レジストリ上のインデックスを固定します。変更が必要な場合は `KaigiPrivacyMode` を `ZkRosterV2` に上げ、テストとゴールデンファイルを更新します。

# オニオンルーティングオーバーレイ

## リレー登録

- リレーはドメインメタデータ `kaigi_relay::<relay_id>` として登録し、HPKE 鍵素材と帯域クラスを含めます。
- `RegisterKaigiRelay` 命令はメタデータにディスクリプタを保存し、HPKE フィンガープリントと帯域クラスを含む `KaigiRelayRegistered` イベントを発行。鍵のローテーションも決定論的に行えます。
- ガバナンスは既存のメタデータ管理機構を利用して許可リストを調整できます。

## マニフェスト作成

- ホストは利用可能なリレーから 3 ホップ以上の経路を構築します。マニフェストには AccountId の並びと、オニオン暗号化に必要な HPKE 公開鍵を含めます。
- オンチェーンに保存する `relay_manifest` は Norito で構造化され、ホップ情報と有効期限を保持します。セッション毎のエフェメラル鍵やオフセットは HPKE を用いてオフチェーンで交換します。

## シグナリングとメディア

- SDP / ICE の交換はこれまでどおり Kaigi メタデータ経由で行いますが、各ホップで暗号化されます。バリデータが確認できるのは HPKE 暗号文とヘッダインデックスのみです。
- メディアパケットは QUIC を用いてリレーを共有し、ペイロードは逐次復号されます。各ホップは次ホップアドレスだけを取得し、最終受信者が全レイヤーを剥がした後にメディアストリームを受信します。

## フェイルオーバー

- クライアントはドメインメタデータ `kaigi_relay_feedback::<relay_id>` に保存された署名付きフィードバックを通じてリレーの健全性を監視します。リレーに障害が発生した場合、ホストは更新版マニフェストを発行し、`KaigiRelayManifestUpdated` イベントでログ化します（後述）。
- ホストは `SetKaigiRelayManifest` 命令でオンチェーンのマニフェストを更新または削除します。削除時には `hop_count = 0` のサマリが記録され、オペレーターが直接通信に戻ったことを観測できます。
- Prometheus メトリクス `kaigi_relay_registered_total`, `kaigi_relay_registration_bandwidth_class`, `kaigi_relay_manifest_updates_total`, `kaigi_relay_manifest_hop_count` を追加し、リレーの変動やホップ数の分布を可視化できます。

# イベント

`DomainEvent` を以下のバリアントで拡張します。

- `KaigiRosterSummary` – ロスターが変更されるたびに匿名化した参加者数と現在のロスター根を発行します（透過モードでは `root = None`）。
- `KaigiRelayRegistered` – リレー登録／更新のたびに発行。
- `KaigiRelayManifestUpdated` – マニフェスト更新・削除時に発行。
- `KaigiUsageSummary` – 各利用区間の後に集計値のみを公開。

イベントは Norito でシリアライズされ、露出されるのはコミットメントハッシュと集計値だけです。

CLI ツール（`iroha kaigi …`）は各 ISI をラップしており、オペレーターはトランザクションを手書きすることなくセッション登録・ロスター更新・利用記録を行えます。リレーマニフェストやプライバシー証明は JSON / Hex ファイルとして CLI に渡せるため、ステージング環境でのスクリプト自動化も容易です。

# ガス計算

- `crates/iroha_core/src/gas.rs` に以下の定数を追加:
  - `BASE_KAIGI_JOIN_ZK`, `BASE_KAIGI_LEAVE_ZK`, `BASE_KAIGI_USAGE_ZK` – Halo2 検証時間（ロスター Join/Leave ≈1.6 ms、Usage ≈1.2 ms @ Apple M2 Ultra）に基づいて調整。
  - 証明バイト数に比例するサーチャージは `PER_KAIGI_PROOF_BYTE` で継続。
- `RecordKaigiUsage` はコミットメントサイズと証明検証コストに応じた追加料金を支払います。
- キャリブレーション用ハーネスは機密資産インフラを再利用し、固定シードで測定を実施します。

# テスト戦略

- `KaigiParticipantCommitment`, `KaigiRelayManifest` の Norito エンコード／デコード単体テスト。
- JSON ビューのゴールデンテスト（正規化順序の確認）。
- ミニネットワークを起動する統合テスト（`crates/iroha_core/tests/kaigi_privacy.rs` 参照）:
  - モック証明（`kaigi_privacy_mocks` フィーチャ）でのプライベート Join/Leave サイクル。
  - リレーマニフェスト更新がメタデータイベントに反映されるかの検証。
- Trybuild UI テストでホスト設定ミス（例: プライバシーモードなのにマニフェストがない）をカバー。
- 制約のある環境（Codex サンドボックス等）でユニット／統合テストを実行する際は、`crates/norito/build.rs` が強制する Norito バインディング同期チェックを迂回するため `NORITO_SKIP_BINDINGS_SYNC=1` をエクスポートする。

# 移行計画

1. ✅ `KaigiPrivacyMode::Transparent` を既定値としたままデータモデルの追加をリリース。
2. ✅ 二重パス検証を実装：本番では `kaigi_privacy_mocks` を無効にし、`zk.kaigi_roster_vk` を解決して実証明を検証。テストではフィーチャを有効化して決定論的スタブを利用。
3. ✅ Halo2 回路を含む専用クレート `kaigi_zk` を導入し、ガスを調整。統合テストで実証明をエンドツーエンド検証（スタブはテスト用途のみ）。
4. ⬜ すべてのコンシューマがコミットメント形式を理解したあとで、透過モードの `participants` ベクタを廃止する。

# 設計決定（2026-03-02 更新）

本番ローンチ前に未決となっていた論点は以下の方針で合意済みです。

## Merkle ツリーの永続化
- **基本方針:** ルートのみオンチェーンにコミットし、完全なツリーは各リレーおよび Kaigi ホストが管理するオフチェーンストレージに保持する。
- **同期メカニズム:**
  - 各スロット終了時に `KaigiRosterSummary` イベントへ最新ルートとスロット番号を記録。
  - リプレイ用に `kaigi_roster_snapshots` テーブルを Torii から同期し、最後の 256 スロット分を保存。
- **再構築:** オフチェーンツリーが欠損した場合は、ホストがイベントログからリプレイしつつ、最新の `KaigiRelayManifest` と `KaigiRosterSummary` で差分を復元する。必要に応じて `kaigi_zk` の証明を要求して整合性を担保する。
- **オペレーション:** ダッシュボードにツリー再構築時間・スナップショット遅延を監視項目として追加し、閾値を越えた場合は PagerDuty (`svc_kaigi_roster`) を起票する。

## リレーマニフェストのマルチパス対応
- **サポート方式:** `KaigiRelayManifest` を拡張し、`hops` 配列に複数経路を記述できるようにする。各 hop には `path_id` と `weight` を付与し、ホストは `weight` に基づき確率的に経路を選択する。
- **互換性:** 単一路経路のみをサポートしている既存リレーは `path_id = 0` を想定。一方、マルチパス対応リレーは `relay_manifest.version = 2` を宣言し、Torii がバリデーション時に互換性チェックを行う。
- **テレメトリ:** 経路別の成功率を `relay_path_success_total{path_id=…}` メトリクスで公開し、リレー評価スコアに反映する。

## リレー評価とガバナンス
- **スコアリング:** `relay_score = availability_bps * quality_multiplier * stake_multiplier` とし、`availability_bps` はスロット単位、`quality_multiplier` は失敗率・遅延・証明有無で決定、`stake_multiplier` はステーク量に比例（最大 1.25x）。
- **制裁ポリシー:**
  1. 警告フェーズ（スコアがしきい値 0.6 未満）：通知のみ。
  2.  probation（0.4 未満）：リレーは新規マニフェストの承認が一時停止される。改善報告が無いまま 3 スロット継続すると罰金。
  3. slash（0.2 未満）：ステークの 5% を削減し、さらに 0.1 未満でガバナンス投票により除外。
- **運用:** すべての制裁イベントは `KaigiRelayHealthUpdated` に記録し、ガバナンス審査ログへ送信。異議申し立ては 7 日以内に提出できるよう `relay_penalty_appeals` テーブルを導入する。

これらの決定に従って仕様・実装・運用ドキュメントを更新し、`KaigiPrivacyMode::ZkRosterV1` の本番有効化に必要な未決事項は解消済みとなった。

---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: コンフィデンシャル資産とZK転送
description: shielded circulation、レジストリ、オペレータ制御のPhase Cブループリント。
slug: /nexus/confidential-assets
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# コンフィデンシャル資産とZK転送の設計

## 動機
- ドメインが透明な循環を変えずに取引プライバシーを維持できるよう、opt-inのshielded資産フローを提供する。
- 異なるハードウェアのバリデータ間でも決定論的実行を保ち、Norito/Kotodama ABI v1互換を維持する。
- 監査人とオペレータに回路と暗号パラメータのライフサイクル制御（有効化、ローテーション、失効）を提供する。

## 脅威モデル
- バリデータはhonest-but-curious: コンセンサスは忠実に実行するが、ledger/stateの検査を試みる。
- ネットワーク監視者はブロックデータとgossipされたトランザクションを観測できる。プライベートなgossipチャネルは前提にしない。
- 対象外: off-ledgerトラフィック解析、量子攻撃者（PQ roadmapで別途追跡）、ledger可用性攻撃。

## 設計概要
- 資産は既存の透明残高に加えて*shielded pool*を宣言できる。shielded循環は暗号commitmentsで表現される。
- Notesは `(asset_id, amount, recipient_view_key, blinding, rho)` を内包し、次を持つ:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`（note順序に依存しない）。
  - Encrypted payload: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- トランザクションはNoritoエンコードされた `ConfidentialTransfer` payloadを運び、次を含む:
  - Public inputs: Merkle anchor、nullifiers、新しいcommitments、asset id、circuit version。
  - 受信者と任意監査人向けの暗号化payload。
  - 値保存、ownership、認可を証明するzero-knowledge proof。
- Verifying keysとparameterセットはオンレジャのレジストリで管理され、activation windowを持つ。未知または失効したentryを参照するproofは拒否される。
- コンセンサスヘッダは有効なconfidential feature digestにコミットし、registryとparameter状態が一致する場合のみブロックを受理する。
- Proof生成はtrusted setupなしのHalo2 (Plonkish)スタックを使用する。Groth16などのSNARKバリアントはv1では意図的に非対応。

### Deterministic Fixtures

Confidential memo envelopesは `fixtures/confidential/encrypted_payload_v1.json` にあるカノニカルfixtureを同梱する。データセットは正しいv1エンベロープと破損サンプルを含み、SDKがパース互換性を検証できる。Rustのdata-modelテスト（`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`）とSwift suite（`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`）はfixtureを直接ロードし、Noritoエンコード、エラーサーフェス、回帰カバレッジの整合を保つ。

Swift SDKはカスタムJSONの接着なしにshield命令を送れる。32バイトnote commitment、暗号化payload、debit metadataを持つ `ShieldRequest` を構築し、`IrohaSDK.submit(shield:keypair:)`（または `submitAndWait`）を呼び出して `/v1/pipeline/transactions` へ署名・送信する。ヘルパはcommitment長を検証し、`ConfidentialEncryptedPayload` をNorito encoderに通し、下記の `zk::Shield` レイアウトと一致させることで、walletがRustとロックステップで動作する。

## コンセンサスコミットと能力ゲーティング
- ブロックヘッダは `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` を公開する。digestはコンセンサスハッシュに参加し、ブロック受理にはローカルregistryビューと一致が必要。
- Governanceは将来の `activation_height` を持つ `next_conf_features` を設定してアップグレードを段階化できる。その高さまでは既存digestを継続して出力する。
- バリデータノードは `confidential.enabled = true` と `assume_valid = false` で動作しなければならない。起動時チェックは条件違反またはローカル `conf_features` 不一致でvalidator setへの参加を拒否する。
- P2P handshakeメタデータは `{ enabled, assume_valid, conf_features }` を含む。互換性のない機能を広告するpeerは `HandshakeConfidentialMismatch` で拒否され、コンセンサスローテーションに入らない。
- バリデータ、observer、古いpeer間の互換性結果は [Node Capability Negotiation](#node-capability-negotiation) のhandshakeマトリクスに記録される。handshake失敗は `HandshakeConfidentialMismatch` を返し、digest一致までpeerをローテーション外に保つ。
- 非バリデータobserverは `assume_valid = true` を設定できるが、confidential deltaを盲目的に適用するだけで、コンセンサス安全性には影響しない。

## 資産ポリシー
- 各資産定義は作成者またはgovernanceにより設定された `AssetConfidentialPolicy` を持つ:
  - `TransparentOnly`: デフォルト。透明命令（`MintAsset`, `TransferAsset` など）のみ許可され、shielded操作は拒否される。
  - `ShieldedOnly`: 発行・転送はすべてconfidential命令を使用する。`RevealConfidential` は禁止され、残高は公開されない。
  - `Convertible`: holderは下記のon/off-ramp命令で透明とshieldedの表現間を移動できる。
- ポリシーは資金の取り残しを防ぐため制約FSMに従う:
  - `TransparentOnly → Convertible`（shielded poolを即時有効化）。
  - `TransparentOnly → ShieldedOnly`（pending transitionとconversion windowが必要）。
  - `Convertible → ShieldedOnly`（最小遅延を強制）。
  - `ShieldedOnly → Convertible`（shielded notesが使えるよう移行計画が必要）。
  - `ShieldedOnly → TransparentOnly` はshielded poolが空か、残存notesをunshieldする移行をgovernanceが符号化しない限り禁止。
- Governance命令は ISI `ScheduleConfidentialPolicyTransition` で `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` を設定し、`CancelConfidentialPolicyTransition` で中止できる。Mempool検証は遷移高さをまたぐトランザクションを防ぎ、ブロック途中でポリシー判定が変わる場合は確定的に失敗させる。
- Pending transitionは新ブロック開始時に自動適用される。高さがconversion window（`ShieldedOnly`移行）に入るか `effective_height` に到達すると、runtimeは `AssetConfidentialPolicy` を更新し `zk.policy` metadataを更新、pending entryをクリアする。`ShieldedOnly`移行時に透明サプライが残っていれば、runtimeは変更を中止し警告を記録する。
- `policy_transition_delay_blocks` と `policy_transition_window_blocks` のconfig knobsが最小通知と猶予期間を強制し、walletが切替前後でnotes変換できるようにする。
- `pending_transition.transition_id` は監査ハンドルとして機能する。governanceは遷移の確定または中止時にこれを引用し、オペレータがon/off-ramp報告と紐付けられるようにする。
- `policy_transition_window_blocks` のデフォルトは720（60秒ブロックで約12時間）。ノードはこれより短い通知を要求するgovernanceを制限する。
- Genesis manifestsとCLIフローは現在・保留ポリシーを提示する。Admissionロジックは実行時にポリシーを読み、confidential命令が許可されているか確認する。
- Migration checklist — 下記 “Migration sequencing” にM0マイルストーンに沿った段階的アップグレード計画を示す。

#### Toriiによる遷移の監視

Walletsとauditorsは `GET /v1/confidential/assets/{definition_id}/transitions` をポーリングして `AssetConfidentialPolicy` を確認する。JSON payloadには常に、asset id、最新ブロック高さ、`current_mode`、その高さで有効なモード（conversion window中は一時的に `Convertible`）、および `vk_set_hash`/Poseidon/Pedersen の期待されるパラメータIDが含まれる。governance遷移が保留中の場合、レスポンスは次も含む:

- `transition_id` — `ScheduleConfidentialPolicyTransition` が返す監査ハンドル。
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` と派生した `window_open_height`（ShieldedOnly切替のためwalletが変換を開始すべきブロック）。

Example response:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

`404` は該当するasset定義が存在しないことを示す。遷移がない場合、`pending_transition` は `null` となる。

### ポリシー状態機械

| Current mode       | Next mode        | Prerequisites                                                                 | Effective-height handling                                                                                         | Notes                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | Governanceがverifier/parameter registry entriesを有効化済み。`ScheduleConfidentialPolicyTransition` を `effective_height ≥ current_height + policy_transition_delay_blocks` で提出。 | `effective_height` で正確に実行され、shielded poolが即時利用可能になる。                   | 透明フローを維持しつつconfidentialを有効化するデフォルト経路。               |
| TransparentOnly    | ShieldedOnly     | 上記に加え `policy_transition_window_blocks ≥ 1`。                                                         | `effective_height - policy_transition_window_blocks` で `Convertible` に自動遷移し、`effective_height` で `ShieldedOnly` に切替。 | 透明命令無効化の前に決定論的な変換ウィンドウを提供。   |
| Convertible        | ShieldedOnly     | `effective_height ≥ current_height + policy_transition_delay_blocks` のスケジュール遷移。Governance SHOULDは監査metadataで (`transparent_supply == 0`) を証明し、runtimeがcut-overで検証。 | 上と同じwindowセマンティクス。`effective_height` で透明サプライが残れば `PolicyTransitionPrerequisiteFailed` で中断。 | 資産を完全confidential循環にロックする。                                     |
| ShieldedOnly       | Convertible      | スケジュール遷移。activeなemergency withdrawalがない（`withdraw_height` 未設定）。                                    | `effective_height` で状態が切替わり、shielded notesは有効なままreveal rampsが再開。                           | メンテナンスウィンドウや監査レビューに使用。                                          |
| ShieldedOnly       | TransparentOnly  | Governanceは `shielded_supply == 0` を証明するか、署名済み `EmergencyUnshield` 計画（auditor署名必須）を準備。 | Runtimeは `effective_height` 前に `Convertible` ウィンドウを開き、到達時にconfidential命令をハード失敗させ、資産をtransparent-onlyへ戻す。 | 最終手段。ウィンドウ中にconfidential noteが消費されると自動キャンセル。 |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` がpending変更をクリア。                                                        | `pending_transition` は即時削除。                                                                          | ステータス維持。完全性のため記載。                                             |

上記にない遷移はgovernance提出時に拒否される。Runtimeはスケジュール遷移の適用直前に前提条件を再確認し、失敗時は資産を前のモードへ戻し、`PolicyTransitionPrerequisiteFailed` をテレメトリとブロックイベントで出力する。

### Migration sequencing

1. **Prepare registries:** 対象ポリシーで参照されるverifier/parameter entriesをすべて有効化する。ノードは結果の `conf_features` を広告し、peerが互換性を検証できるようにする。
2. **Stage the transition:** `policy_transition_delay_blocks` を満たす `effective_height` で `ScheduleConfidentialPolicyTransition` を提出。`ShieldedOnly` 方向へ進む場合はconversion window（`window ≥ policy_transition_window_blocks`）を指定する。
3. **Publish operator guidance:** 返された `transition_id` を記録し、on/off-ramp runbookを配布する。walletsとauditorsは `/v1/confidential/assets/{id}/transitions` を購読してwindow open heightを把握する。
4. **Window enforcement:** ウィンドウ開始でruntimeがポリシーを `Convertible` に切り替え、`PolicyTransitionWindowOpened { transition_id }` を発行し、競合するgovernanceリクエストを拒否する。
5. **Finalize or abort:** `effective_height` でruntimeが前提条件（透明サプライゼロ、emergency withdrawalなし等）を検証。成功なら要求モードへ切替、失敗なら `PolicyTransitionPrerequisiteFailed` を出力しpending transitionをクリアしてポリシーを保持する。
6. **Schema upgrades:** 成功後にgovernanceが資産スキーマ版（例 `asset_definition.v2`）を更新し、CLI toolingはmanifestシリアライズ時に `confidential_policy` を要求する。Genesisアップグレード文書はバリデータ再起動前のpolicy設定とregistry fingerprint追加を指示する。

Confidentialityを有効にして開始する新規ネットワークは、希望するpolicyをgenesisに直接エンコードする。それでも起動後のモード変更時は上記チェックリストに従い、conversion windowが決定論的でwalletが調整できるようにする。

### Norito manifestのバージョニングとアクティベーション

- Genesis manifestsはカスタムキー `confidential_registry_root` の `SetParameter` を必ず含む。Payloadは `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` に一致するNorito JSONで、verifier entryが無い場合はフィールドを省略（`null`）し、ある場合は `compute_vk_set_hash` が生成する32バイトhex（`0x…`）を指定する。ノードはパラメータ欠落またはハッシュ不一致の場合に起動を拒否する。
- On-wire `ConfidentialFeatureDigest::conf_rules_version` はmanifest layoutバージョンを埋め込む。v1ネットワークでは `Some(1)` を維持し、`iroha_config::parameters::defaults::confidential::RULES_VERSION` に一致する。ルールが進化したら定数を上げ、manifestを再生成し、バイナリを同期展開する。混在は `ConfidentialFeatureDigestMismatch` でブロック拒否となる。
- Activation manifestsはregistry更新、parameterライフサイクル変更、policy遷移を束ね、digest整合を保つべき:
  1. 予定するregistry変更（`Publish*`, `Set*Lifecycle`）をオフライン状態ビューに適用し、`compute_confidential_feature_digest` でアクティベーション後のdigestを算出する。
  2. 算出したハッシュを使い `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` を発行し、遅延peerが中間registry命令を逃しても正しいdigestを回復できるようにする。
  3. `ScheduleConfidentialPolicyTransition` 命令を追加する。各命令はgovernance発行の `transition_id` を引用しなければならず、欠落するとruntimeが拒否する。
  4. Manifestバイト列、SHA-256 fingerprint、アクティベーションで使ったdigestを保存し、オペレータは投票前に3点を検証して分断を避ける。
- ロールアウトに遅延cut-overが必要な場合、対象高さを補助的なカスタムパラメータ（例 `custom.confidential_upgrade_activation_height`）に記録する。これにより、digest変更が有効になる前に通知ウィンドウが守られたことを監査人がNoritoエンコードの証拠で確認できる。

## Verifierとパラメータのライフサイクル
### ZK Registry
- Ledgerは `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` を保持し、`proving_system` は現在 `Halo2` 固定。
- `(circuit_id, version)` はグローバルにユニークで、registryはcircuit metadataの二次インデックスを維持する。重複ペアの登録はadmissionで拒否される。
- `circuit_id` は空不可、`public_inputs_schema_hash` は必須（通常はverifierのcanonical public-inputエンコードのBlake2b-32ハッシュ）。これらが欠落するレコードはadmissionで拒否される。
- Governance命令は次を含む:
  - `PUBLISH` でmetadataのみの `Proposed` entryを追加。
  - `ACTIVATE { vk_id, activation_height }` でentryのエポック境界アクティベーションをスケジュール。
  - `DEPRECATE { vk_id, deprecation_height }` でproof参照可能な最終高さを指定。
  - `WITHDRAW { vk_id, withdraw_height }` で緊急停止。影響資産はwithdraw height後にconfidential支出が凍結され、新entryが有効化されるまで解除されない。
- Genesis manifestsはactive entriesに一致する `vk_set_hash` を持つ `confidential_registry_root` カスタムパラメータを自動発行する。検証はローカルregistry状態とdigestを照合し、ノード参加前に確認する。
- Verifier登録・更新には `gas_schedule_id` が必要。検証はentryが `Active` であること、`(circuit_id, version)` インデックスに存在すること、Halo2 proofsが `OpenVerifyEnvelope` を提供し `circuit_id`/`vk_hash`/`public_inputs_schema_hash` がregistry記録に一致することを要求する。

### Proving Keys
- Proving keysはoff-ledgerに置かれるが、content-addressed ID（`pk_cid`, `pk_hash`, `pk_len`）がverifier metadataと共に公開される。
- Wallet SDKsはPKデータを取得し、ハッシュを検証してローカルにキャッシュする。

### Pedersen & Poseidon Parameters
- `PedersenParams` と `PoseidonParams` のレジストリはverifierと同様のライフサイクル制御を持ち、`params_id`、generator/constantハッシュ、activation/deprecation/withdraw高さを保持する。
- Commitmentsとハッシュは `params_id` をドメイン分離し、過去のセットのビットパターンを再利用しない。IDはnote commitmentsとnullifierドメインタグに埋め込まれる。
- Circuitsは検証時に複数パラメータ選択をサポートする。`deprecation_height` まで消費可能で、`withdraw_height` で確定的に拒否される。

## Deterministic Ordering & Nullifiers
- 各資産は `CommitmentTree` と `next_leaf_index` を持ち、ブロックは決定論的順序でcommitmentを追加する: ブロック順にトランザクションを走査し、各トランザクション内ではシリアライズ済み `output_idx` 昇順でshielded outputを処理する。
- `note_position` はtree offsetから導かれるがnullifierには含まれず、proof witness内のmembership pathにのみ使われる。
- Reorg時のnullifier安定性はPRF設計により保証される。PRF入力は `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` を結合し、anchorは `max_anchor_age_blocks` に制限された過去のMerkle rootを参照する。

## Ledger Flow
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - `Convertible` または `ShieldedOnly` ポリシーが必要。admissionはasset authorityを確認し、現在の `params_id` を取得し、`rho` をサンプルしてcommitmentを発行、Merkle treeを更新する。
   - `ConfidentialEvent::Shielded` を発行し、新commitment、Merkle root delta、監査用のtransaction call hashを含める。
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - VM syscallがregistry entryでproofを検証し、hostはnullifiers未使用、commitmentsの決定論的追加、anchorの新鮮さを保証する。
   - Ledgerは `NullifierSet` entriesを記録し、受信者/監査人向け暗号化payloadを保存し、`ConfidentialEvent::Transferred` にnullifiers、ordered outputs、proof hash、Merkle rootsを要約する。
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - `Convertible` 資産のみ使用可能。proofはnote値が公開額と一致することを証明し、ledgerは透明残高を加算し、nullifierを消費済みにしてshielded noteを焼却する。
   - `ConfidentialEvent::Unshielded` を発行し、公開額、消費nullifier、proof識別子、transaction call hashを記録する。

## Data Modelの追加
- `ConfidentialConfig`（新configセクション）: enable flag、`assume_valid`、gas/limit knobs、anchor window、verifier backend。
- `ConfidentialNote`/`ConfidentialTransfer`/`ConfidentialMint` のNoritoスキーマは明示的なversion byte（`CONFIDENTIAL_ASSET_V1 = 0x01`）を持つ。
- `ConfidentialEncryptedPayload` はAEAD memo bytesを `{ version, ephemeral_pubkey, nonce, ciphertext }` でラップし、XChaCha20-Poly1305レイアウトの `CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` をデフォルトにする。
- Canonical key-derivationベクトルは `docs/source/confidential_key_vectors.json` にあり、CLIとTorii endpointの回帰テストで使用する。
- `asset::AssetDefinition` に `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` を追加。
- `ZkAssetState` はtransfer/unshield verifiersの `(backend, name, commitment)` を保持し、参照/inline verifying keyが一致しないproofを拒否する。
- `CommitmentTree`（資産ごとのfrontier checkpoints付き）、`NullifierSet`（キー `(chain_id, asset_id, nullifier)`）、`ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` をworld stateに保存。
- Mempoolは重複検出とanchor ageチェックのために一時 `NullifierIndex` と `AnchorIndex` を保持する。
- Noritoスキーマ更新にはpublic inputsのcanonical orderingを含め、round-trip testsでエンコードの決定性を保証する。
- 暗号化payloadのround-tripは単体テスト（`crates/iroha_data_model/src/confidential.rs`）で固定。ウォレット向けベクトルは監査用にcanonical AEAD transcriptを追加予定。`norito.md` はエンベロープのon-wire headerを記載する。

## IVM統合とSyscall
- `VERIFY_CONFIDENTIAL_PROOF` syscallを導入し、次を受け取る:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` と、結果の `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`。
  - Syscallはregistryからverifier metadataを読み込み、サイズ/時間制限を適用し、決定論的gasを課し、proof成功時のみdeltaを適用する。
- HostはMerkle rootスナップショットとnullifier状態を取得するread-only `ConfidentialLedger` traitを公開し、Kotodamaライブラリがwitness組み立てhelpersとschema検証を提供する。
- Pointer-ABIドキュメントはproof bufferレイアウトとregistry handlesを明確化するため更新された。

## Node Capability Negotiation
- Handshakeは `feature_bits.confidential` と `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` を広告する。バリデータ参加には `confidential.enabled=true`, `assume_valid=false`, 同一verifier backend ID、digest一致が必要。不一致は `HandshakeConfidentialMismatch` でhandshake失敗。
- Configはobserverノードに限り `assume_valid` を提供する。無効時はconfidential命令に遭遇するとdeterministic `UnsupportedInstruction` となり、panicしない。有効時はproof検証なしでstate deltaを適用する。
- Mempoolはローカル能力が無効の場合confidentialトランザクションを拒否する。Gossipフィルタは非互換peerへのshieldedトランザクション送信を避け、未知verifier IDはサイズ制限内でブラインド転送する。

### Reveal Pruning & Nullifier Retention Policy

Confidential ledgerはnoteのfreshness証明とgovernance監査の再現に十分な履歴を保持する必要がある。`ConfidentialLedger` によって強制されるデフォルトポリシーは以下:

- **Nullifier retention:** 消費済みnullifierを、消費高さから最低 `730` 日（24か月）保持する。規制要求が長い場合はそちらを優先する。`confidential.retention.nullifier_days` で拡張可能。保持期間内のnullifierはTorii経由で参照可能でなければならず、auditorが二重支出不在を証明できるようにする。
- **Reveal pruning:** 透明reveal（`RevealConfidential`）は関連note commitmentをブロック確定直後に削除するが、消費nullifierは上記retentionルールに従う。`ConfidentialEvent::Unshielded` は公開額、受取人、proof hashを記録し、削除済みciphertextがなくても履歴revealを復元できる。
- **Frontier checkpoints:** commitment frontiersは `max_anchor_age_blocks` とretention windowの大きい方をカバーするrolling checkpointsを維持する。ノードは区間内のnullifierがすべて期限切れになるまで古いcheckpointを圧縮しない。
- **Stale digest remediation:** `HandshakeConfidentialMismatch` がdigest driftで発生した場合、(1)クラスタのnullifier retention windowが一致しているか確認し、(2)`iroha_cli app confidential verify-ledger` で保持済みnullifier集合からdigestを再生成し、(3)更新済みmanifestを再展開する。早期に削除されたnullifierは再参加前にcold storageから復元する必要がある。

ローカル上書きはoperations runbookに記載すること。retention windowを延長するgovernanceポリシーは、ノード設定とアーカイブストレージ計画を同時に更新する必要がある。

### Eviction & Recovery Flow

1. ダイヤル時に `IrohaNetwork` が広告されたcapabilityを比較する。不一致は `HandshakeConfidentialMismatch` を発生させ、接続を閉じ、peerは `Ready` にならずdiscovery queueに留まる。
2. 失敗はネットワークサービスログに記録され（remote digestとbackendを含む）、Sumeragiはproposalやvotingにpeerをスケジュールしない。
3. オペレータはverifier registryとparameterセット（`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`）を揃えるか、合意した `activation_height` で `next_conf_features` を準備する。digestが一致すれば次のhandshakeは自動的に成功する。
4. 古いpeerがブロックを配布できた場合（例: archival replay）、バリデータは `BlockRejectionReason::ConfidentialFeatureDigestMismatch` で確定的に拒否し、ネットワーク全体のledger状態の整合を保つ。

### Replay-safe handshake flow

1. 各アウトバウンド試行は新しいNoise/X25519鍵を生成する。署名されるhandshake payload（`handshake_signature_payload`）はローカル/リモートのephemeral public keys、Noritoエンコードされたadvertised socket address、（`handshake_chain_id` を有効化している場合）chain identifierを連結する。メッセージは送信前にAEAD暗号化される。
2. 応答側はpeer/local順を反転してpayloadを再計算し、`HandshakeHelloV1` に埋め込まれたEd25519署名を検証する。両方のephemeral keysとadvertised addressが署名ドメインに入るため、他peerへのリプレイや古い接続の復元は確定的に失敗する。
3. Confidential capability flagsと `ConfidentialFeatureDigest` は `HandshakeConfidentialMeta` 内で伝達される。受信側は `{ enabled, assume_valid, verifier_backend, digest }` をローカル `ConfidentialHandshakeCaps` と比較し、不一致は `HandshakeConfidentialMismatch` で `Ready` へ移行する前に終了する。
4. オペレータは `compute_confidential_feature_digest` でdigestを再計算し、更新されたregistry/policyでノードを再起動する必要がある。古いdigestを広告するpeerは引き続きhandshake失敗となり、古い状態がvalidator setに戻るのを防ぐ。
5. Handshakeの成功/失敗は標準 `iroha_p2p::peer` カウンタ（`handshake_failure_count` など）を更新し、remote peer IDとdigest fingerprintでタグ付けした構造化ログを出力する。これらの指標でリプレイや設定ミスを監視する。

## Key Management & Payloads
- アカウントごとのキー導出階層:
  - `sk_spend` → `nk` (nullifier key), `ivk` (incoming viewing key), `ovk` (outgoing viewing key), `fvk`.
- 暗号化note payloadはECDH由来の共有鍵でAEADを使用する。必要に応じてauditor view keysをasset policyに従ってoutputsへ付与できる。
- CLI追加: `confidential create-keys`, `confidential send`, `confidential export-view-key`, メモ復号のauditor tooling, オフラインでNorito memo envelopeを生成/検査する `iroha app zk envelope` helper。

## ガス、制限、DoS対策
- 決定論的ガススケジュール:
  - Halo2 (Plonkish): 基本 `250_000` gas + public inputごとに `2_000` gas。
  - proofバイトあたり `5` gas、nullifierあたり (`300`) とcommitmentあたり (`500`) の追加。
  - オペレータはノード設定（`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`）で定数を上書きでき、起動時または設定のホットリロード時にクラスタ全体へ決定論的に適用される。
- ハード制限（設定可能なデフォルト）:
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`。`verify_timeout_ms` を超えるproofは命令を確定的に中断する（governance投票は `proof verification exceeded timeout` を出力し、`VerifyProof` はエラーを返す）。
- 追加クォータはlivenessを確保する: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, `max_public_inputs` がblock buildersを制限し、`reorg_depth_bound`（≥ `max_anchor_age_blocks`）がfrontier checkpoint retentionを制御する。
- Runtimeはこれらのper-transaction/per-block制限を超えるトランザクションを拒否し、決定論的な `InvalidParameter` エラーを出力してledger状態を変更しない。
- Mempoolは `vk_id`、proof長、anchor ageでconfidentialトランザクションを事前フィルタし、verifier実行前にリソース使用を抑制する。
- 検証はtimeoutや制限違反で決定論的に停止し、トランザクションは明示的エラーで失敗する。SIMD backendは任意だがgas会計を変更しない。

### Calibration Baselines & Acceptance Gates
- **Reference platforms.** Calibrationは以下の3つのハードウェアプロファイルを必ず含む。全プロファイルが揃わない場合、レビューで却下する。

  | Profile | Architecture | CPU / Instance | Compiler flags | Purpose |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) または Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | ベクトル命令なしの基準値を確立し、fallback cost tablesを調整する。 |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | default release | AVX2パスを検証し、SIMD高速化がneutral gasの許容範囲内にあることを確認する。 |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | default release | NEON backendが決定論的で、x86スケジュールと整合することを保証する。 |

- **Benchmark harness.** ガス校正レポートは必ず以下で生成する:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` で決定論的fixtureを確認。
  - VM opcodeコスト変更時は `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` を必ず実行。

- **Fixed randomness.** ベンチ前に `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` を設定し、`iroha_test_samples::gen_account_in` が決定論的 `KeyPair::from_seed` パスへ切替わるようにする。Harnessは `IROHA_CONF_GAS_SEED_ACTIVE=…` を一度出力する。変数が無い場合はレビュー失敗とする。新しい校正ユーティリティも補助的ランダム性を導入する際はこのenv varを必ず尊重すること。

- **Result capture.**
  - 各プロファイルのCriterionサマリ（`target/criterion/**/raw.csv`）をrelease artefactに保存する。
  - 派生メトリクス（`ns/op`, `gas/op`, `ns/gas`）を[Confidential Gas Calibration ledger](./confidential-gas-calibration)にgit commitとコンパイラバージョンと共に記録する。
  - 各プロファイルで最新2つのベースラインを保持し、最新が検証されたら古いスナップショットを削除する。

- **Acceptance tolerances.**
  - `baseline-simd-neutral` と `baseline-avx2` のgas差分は ±1.5%以内でなければならない。
  - `baseline-simd-neutral` と `baseline-neon` のgas差分は ±2.0%以内でなければならない。
  - これら閾値を超える校正提案は、スケジュール調整または差分と緩和策を説明するRFCが必要。

- **Review checklist.** 申請者は以下を含める:
  - `uname -a`、`/proc/cpuinfo` 抜粋（model、stepping）、`rustc -Vv` を校正ログに記載。
  - ベンチ出力で `IROHA_CONF_GAS_SEED` が表示されることを確認（ベンチはactive seedを出力）。
  - pacemakerとconfidential verifierのfeature flagsがproductionと一致すること（Telemetry付きでベンチする場合は `--features confidential,telemetry`）。

## Config & Operations
- `iroha_config` に `[confidential]` セクションを追加:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetryは `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, `confidential_policy_transitions_total` を集計し、plaintextは公開しない。
- RPC surfaces:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## テスト戦略
- 決定論: ブロック内トランザクションのランダム並べ替えでも同一Merkle rootsとnullifier setsになる。
- Reorg耐性: anchor付きの複数ブロックreorgをシミュレートし、nullifierは安定し、古いanchorは拒否される。
- Gas不変性: SIMDあり/なしのノードで同一ガス使用を確認する。
- 境界テスト: proofのサイズ/ガス上限、最大入出力数、timeout enforcement。
- ライフサイクル: verifier/parameterのactivation/deprecationガバナンス操作、rotation支出テスト。
- Policy FSM: 許可/禁止遷移、pending transition遅延、effective height周辺のmempool拒否。
- Registry緊急事態: emergency withdrawalが `withdraw_height` で資産を凍結し、それ以降のproofを拒否する。
- Capability gating: `conf_features` が一致しないバリデータはブロックを拒否し、`assume_valid=true` のobserverはコンセンサスに影響せず追従する。
- State equivalence: validator/full/observerノードがカノニカルチェーンで同一state rootsを生成する。
- Negative fuzzing: 破損proof、過大payload、nullifier衝突は確定的に拒否される。

## 移行と互換性
- Feature-gated rollout: Phase C3完了までは `enabled` のデフォルトは `false`。ノードはvalidator set参加前にcapabilitiesを広告する。
- 透明資産は影響なし。confidential命令はregistry entriesとcapability negotiationを必要とする。
- Confidential未対応でビルドされたノードは該当ブロックを確定的に拒否する。validator setには参加できないが、`assume_valid=true` のobserverとして動作可能。
- Genesis manifestsは初期registry entries、parameterセット、資産のconfidential policy、任意のauditor keysを含む。
- オペレータはregistry rotation、policy transition、emergency withdrawalのrunbookに従い、決定論的アップグレードを維持する。

## 未完了の作業
- Halo2パラメータセット（circuitサイズ、lookup戦略）をベンチマークし、結果をcalibration playbookに記録して次回 `confidential_assets_calibration.md` 更新時にgas/timeoutデフォルトを更新する。
- 監査人のdisclosure policyと関連するselective-viewing APIを確定し、governanceドラフト承認後にToriiへワークフローを接続する。
- witness encryptionスキームをmulti-recipient outputsとbatched memosに拡張し、SDK実装者向けにenvelopeフォーマットを文書化する。
- 回路、registry、parameter rotation手順の外部セキュリティレビューを依頼し、内部監査レポートと並べて保存する。
- 監査人のspentness照合APIを定義し、walletベンダが同じアテステーション意味論を実装できるようview-keyスコープのガイダンスを公開する。

## 実装フェーズ
1. **Phase M0 — Stop-Ship Hardening**
   - ✅ Nullifier derivationはPoseidon PRF設計（`nk`, `rho`, `asset_id`, `chain_id`）に従い、ledger更新でdeterministic commitment orderingを強制。
   - ✅ 実行はproofサイズ上限とper-transaction/per-blockのconfidentialクォータを適用し、超過トランザクションを決定論的エラーで拒否。
   - ✅ P2P handshakeは `ConfidentialFeatureDigest`（backend digest + registry fingerprint）を広告し、`HandshakeConfidentialMismatch` で不一致を確定的に拒否。
   - ✅ Confidential実行パスのpanicを除去し、互換性のないノードへのrole gatingを追加。
   - ⚪ Verifier timeout budgetsとfrontier checkpointsのreorg depth boundsを適用。
     - ✅ `verify_timeout_ms` を超えるproofは確定的に失敗するようtimeout budgetsを適用。
     - ✅ `reorg_depth_bound` を尊重し、設定ウィンドウより古いcheckpointを剪定しつつ決定論的スナップショットを維持。
   - `AssetConfidentialPolicy`、policy FSM、mint/transfer/reveal命令へのenforcement gatesを導入。
   - ブロックヘッダで `conf_features` にコミットし、registry/parameter digest不一致ではvalidator参加を拒否。
2. **Phase M1 — Registries & Parameters**
   - `ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` レジストリをgovernance ops、genesis anchoring、キャッシュ管理付きで導入。
   - Syscallにregistry lookup、gas schedule ID、schema hashing、size checksを必須化。
   - encrypted payload format v1、wallet key derivation vectors、confidential key管理のCLIサポートを出荷。
3. **Phase M2 — Gas & Performance**
   - deterministic gas schedule、per-block counters、telemetry付きベンチハーネス（verify latency、proof sizes、mempool rejections）を実装。
   - CommitmentTree checkpoints、LRUロード、nullifier indexを強化してマルチ資産負荷に対応。
4. **Phase M3 — Rotation & Wallet Tooling**
   - multi-parameter/multi-version proof受理を有効化し、governance-driven activation/deprecationとtransition runbooksを提供。
   - wallet SDK/CLI移行フロー、auditorスキャンワークフロー、spentness照合toolingを提供。
5. **Phase M4 — Audit & Ops**
   - auditor keyワークフロー、selective disclosure API、運用runbooksを提供。
   - 外部暗号/セキュリティレビューをスケジュールし、結果を `status.md` に公開。

各フェーズはroadmapマイルストーンと関連テストを更新し、ブロックチェーンネットワークの決定論的実行保証を維持する。

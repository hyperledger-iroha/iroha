---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Activos confidentiales y transferencias ZK
説明: Plano de Phase C para circularacion brandada、registros y controles deoperador。
スラグ: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# 秘密の活動と転送 ZK

## モチベーション
- 活動中の活動をオプトインして、管理者のプライバシーを保護します。
- ハードウェア異種検証と保守 Norito/Kotodama ABI v1 での管理者の排出決定。
- サーキットおよびパラメトロスクリプトグラフィックスの監査およびオペラドールの制御デシクロデヴィダ（アクティベーション、ロータシオン、リボカシオン）を証明します。

## モデロ デ アメナス
- ロス・バリダドレスの息子は正直だが好奇心旺盛: 検査台帳/状態を検査する。
- ブロックや取引の監視員が噂話をする。いいえ、ゴシップのプライベート情報はありません。
- 緊急事態: 台帳外の交通状況の分析、敵対的な状況 (ロードマップ PQ の監視)、台帳の管理。

## 履歴書
- *シールドプール*のバランスを透明に保つための活動を禁止することを宣言します。コミットメントクリプトグラフィックスを介して、目隠しをすることができます。
- ラス ノート カプセル化 `(asset_id, amount, recipient_view_key, blinding, rho)` コン:
  - コミットメント: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`。
  - 無効化: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`、メモは独立しています。
  - ペイロード暗号化: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`。
- ペイロード `ConfidentialTransfer` のコード Norito que contienen のトランザクション転送:
  - publicos の入力: マークル アンカー、ヌルファイア、ヌエボス コミットメント、アセット ID、回路のバージョン。
  - ペイロードは受信者と聴取者の権限で暗号化されます。
  - プルエバは、ゼロ知識で勇気を守り、所有権と権限を保持します。
- 活性化のための台帳管理上の管理対象の管理キーと管理キーを検証します。ロスノドスレチャザンの有効な証明は、参照エントリのデスコノシダスまたはレボカダスを参照します。
- ヘッダーとコンセンサスは、ダイジェスト活動と容量の秘密を保持し、登録とパラメータの一致を確認します。
- 信頼できるセットアップで Halo2 (Plonkish) をスタックし、証明を構築します。 Groth16 では、バージョン 1 の SNARK を考慮したバージョンの機能はありません。

### 試合の決定要因

Los sobres de memo confidentiales ahora incluyen un fixture canonico en `fixtures/confidential/encrypted_payload_v1.json`.このデータセットは、v1 を使用して、SDK の解析を確実に実行し、否定的な不正行為を確実に検出します。 Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) と Swift スイート (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) のデータ モデルの損失テスト、フィクスチャの指示、エンコード Norito の保証、エラーと回帰の永続的なエラーの確認エボルシオナエルコーデック。

Los SDKs de Swift ahora pueden Emiir instrucciones Shield Sin Glue JSON ビスポーク: construye un
`ShieldRequest` 32 バイトのコミットメント、ペイロードの暗号化およびデビットのメタデータ、
ルエゴ・ラマ `IrohaSDK.submit(shield:keypair:)` (o `submitAndWait`) パラ・ファーム・イ・エンヴィア・ラ
トランザクションソブル `/v2/pipeline/transactions`。コミットメントの経度を検証するためのヘルパー、
enhebra `ConfidentialEncryptedPayload` en el エンコーダ Norito、y refleja el レイアウト `zk::Shield`
Rust の財布を説明します。## 合意と機能のゲートに関するコミットメント
- ブロック指数 `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` のヘッダー損失。参加者のダイジェストとコンセンサスのハッシュとブロックのローカル デル レジストリを確認します。
- ガバナンスは、`activation_height` 将来のアップグレード プログラムを準備中です。アルトゥーラの生産物、ブロックの生産品、エミディエンドのダイジェストを前にご覧ください。
- 有効なノードの損失 DEBEN オペラコン `confidential.enabled = true` および `assume_valid = false`。ロスチェックは、初期のレチャザンユニバースアルセットデバリダレスシアルキエラフォールラオシエル`conf_features`ローカルダイバージをチェックします。
- ハンドシェイク P2P アホラのメタデータには `{ enabled, assume_valid, conf_features }` が含まれます。 Peers que anuncien には、`HandshakeConfidentialMismatch` y nunca entran en rotacion de consenso に関する詳細な情報はありません。
- 有効なハンドシェイクの結果が失われ、オブザーバーとピアがハンドシェイクのマトリズをキャプチャする [ノード機能ネゴシエーション](#node-capability-negotiation)。ハンドシェイクの失敗は `HandshakeConfidentialMismatch` と一致し、ダイジェストの一致を確認する必要があります。
- オブザーバーはバリドーレス・プエデン・フィジャール`assume_valid = true`を拒否します。デルタの機密情報は、合意に影響を与えることはありません。

## 資産の政治
- `AssetConfidentialPolicy` フィジャド ポル エル クリエイターまたはガバナンス経由の資産定義:
  - `TransparentOnly`: 欠陥のあるモード;単独で、透明な命令 (`MintAsset`、`TransferAsset` など) を許可し、シールドされたセキュリティを保護します。
  - `ShieldedOnly`: 秘密の使用説明書を提出します。 `RevealConfidential` エスタ・プロヒビド・パラ・ケ・ロス・バランス・ヌンカ・セ・エクスポンガン・パブリケーション。
  - `Convertible`: 損失保持者は、移動者の勇気を示し、透明なシールドを使用して、アバホのオン/オフランプの指示を出します。
- FSM の政治活動は、安全保障とバラドスに影響を与えないようにする必要があります。
  - `TransparentOnly -> Convertible` (シールドされたプールでのリハビリテーション)。
  - `TransparentOnly -> ShieldedOnly` (変換保留と変換が必要です)。
  - `Convertible -> ShieldedOnly` (デモラ・ミニマ・オブリガトリア)。
  - `ShieldedOnly -> Convertible` (必要な移行計画パラケラスノート目隠し、シガンシエンドガステーブルが必要)。
  - `ShieldedOnly -> TransparentOnly` は、シールド付きプールでの一斉射撃を許可しません。また、管理法規、移行中の盲目的なメモを保留します。
- ISI `ScheduleConfidentialPolicyTransition` および `CancelConfidentialPolicyTransition` を介したガバナンスに関する指示。政府の検証は、政府の政治的問題をチェックする必要はありません。
- アプリケーションの自動処理の保留中: 変換中の変換 (アップグレード `ShieldedOnly`) またはアルカンザ `effective_height`、実際のランタイム `AssetConfidentialPolicy`、参照メタデータ `zk.policy` は、ペンディエンテのリンピアです。 `ShieldedOnly` は透明な状態で供給され、ランタイムの中断やカンビオの登録、以前の状態はそのままです。
- ノブの設定 `policy_transition_delay_blocks` と `policy_transition_window_blocks` は、フエルザン アビソ ミニモとウォレットの変換を許可する期間を設定します。
- `pending_transition.transition_id` 聴覚機能を操作します。ガバナンスは、オン/オフランプの最終報告やオペラドールのキャンセルに関する報告を行います。
- `policy_transition_window_blocks` のデフォルトは 720 (ブロック時間で約 12 時間、60 秒)。管理上の意思決定に対するガバナンスの制限に関する制限はありません。
- ジェネシスは、CLI の現実的な政治と懸案を明らかにします。入国審査の論理は、政治的理由からの退去を確認するため、秘密の指示を自分で提出する必要があります。
- 移行チェックリスト - マイルストーン M0 の「移行シーケンス」バージョンのアップグレード計画。

#### Torii 経由の監視

財布と監査は、`GET /v2/confidential/assets/{definition_id}/transitions` パラ検査および `AssetConfidentialPolicy` 活動に相談します。ペイロード JSON には、資産 ID カノニコ、ブロック監視、政治 `current_mode`、有効なモード (一時的な変換レポート `Convertible`)、識別子エスペラードが含まれます。 `vk_set_hash`/ポセイドン/ペダーセン。クアンド・ヘイ・ウナ・トランシオン・ペンディエンテ・ラ・レスプエスタ・タンビエンには以下が含まれます：- `transition_id` - `ScheduleConfidentialPolicyTransition` の聴覚デバイスを処理します。
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` および `window_open_height` デリバド (ブロック ドンデ ウォレット デベン コメンザル変換パラ カットオーバー ShieldedOnly)。

解決策:

```json
{
  "asset_id": "rose#wonderland",
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

解決策 `404` は、資産の定義が一致するという事実が存在しません。クアンドは、干し草トランシオン プログラム、エル カンポ `pending_transition` es `null` を持っていません。

### 政治家のマキナ

|実際のModo |モードシギエンテ |前提条件 |効果的なマネホ デ アルトゥラ |メモ |
|------------------|------------------|---------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
|透明のみ |コンバーチブル |ガバナンスは検証登録/パラメータ管理を有効にします。 Enviar `ScheduleConfidentialPolicyTransition` コン `effective_height >= current_height + policy_transition_delay_blocks`。 | La transicion se ejecuta strictamente en `effective_height`;シールドされたプールは、すぐに使用できます。        |安全性を保証するために欠陥があることを確認します。 |
|透明のみ |シールド付きのみ |イグアル ケ アリバ、マス `policy_transition_window_blocks >= 1`。                                                       |ランタイムは `Convertible` と `effective_height - policy_transition_window_blocks` で自動化されます。カンビア `ShieldedOnly` と `effective_height`。 |変換を決定する前に、安全な命令を実行する必要があります。 |
|コンバーチブル |シールド付きのみ | `effective_height >= current_height + policy_transition_delay_blocks` のトランジション プログラム。監査メタデータによるガバナンス DEBE 認証 (`transparent_supply == 0`)。アプリケーションとカットオーバーのランタイム。 |前もって意味は同一です。 `effective_height` では、シリコン供給は透明であり、`PolicyTransitionPrerequisiteFailed` の移行は中断されません。 |資産の配布は完全に機密です。                                |
|シールド付きのみ |コンバーチブル |移行プログラム。緊急活動の罪 (`withdraw_height` 定義なし)。                            |エル スタド カンビア en `effective_height`;ロスはランプを明らかにし、レアブレン・ミエントラのラス・ノートを目隠しし、シグエン・シエンド・バリダスを記録します。 |定期的なイベントや監査の改訂を行います。                            |
|シールド付きのみ |透明のみ |ガバナンスに関する調査 `shielded_supply == 0` または計画 `EmergencyUnshield` の準備 (監査要求)。 |ランタイムは `Convertible` より前の `effective_height` です。 en esa altura las instrucciones confidentiales fallan duro y elasset vuelve al modo transparent-only。 |サリダ・デ・ウルティモ・リカーソ。ラ・トランシオン・セ・オート・キャンセル・シ・オキュア・カルキエ・ガスト・デ・ノート・コンフィデンシャル・デュランテ・ラ・ベンターナ。 |
|任意 |現在と同じ | `CancelConfidentialPolicyTransition` リンピア エル カンビオ ペンディエンテ。                                                    | `pending_transition` はメディアを削除します。                                                                     |マンティエンは現状維持。ほとんどの完全性。                                          |

Transiciones は、ガバナンスの継続的な提出を確認する必要はありません。前提条件を満たしたランタイム検証は、移行プログラムの実行前に行われます。 si fallan、devuelve elasset al modo previo y は、テレメトリとブロックのイベントを介して `PolicyTransitionPrerequisiteFailed` を発します。

### 移住の手続き1. **登録簿の準備:** 政治目的の検証およびパラメータ参照を有効にします。ロスノードは、`conf_features` の結果として、ピアの一貫性を検証します。
2. **プログラマの変換:** `policy_transition_delay_blocks` に対して `ScheduleConfidentialPolicyTransition` と `effective_height` を接続します。アル・ムーバース・ハシア `ShieldedOnly`、特定のウナ・ベンタナ・デ・コンバージョン (`window >= policy_transition_window_blocks`)。
3. **パブリック ギア パラ オペラドール:** レジストラ el `transition_id` devuelto y circular un runbook de on/off-ramp。財布や監査人は、`/v2/confidential/assets/{id}/transitions` を購入して、イベントの準備を整えています。
4. **Aplicar ventana:** cuando abre la ventana、el runtime cambia la politica a `Convertible`、emite `PolicyTransitionWindowOpened { transition_id }`、y empieza a rechazar が紛争下の統治を要求します。
5. **最終的な中止:** en `effective_height`、ランタイム検証の前提条件 (安全な供給、緊急事態など)。シパサ、カンビア・ラ・ポリティカ・アル・モード・ソリシタド。私は、`PolicyTransitionPrerequisiteFailed`を発し、トランジション・ペンディエンテとデジャ・ラ・ポリティカ・シン・カンビオスをリンピアします。
6. **スキーマのアップグレード:** 移行後の管理、スキーマの資産バージョンのガバナンス (例、`asset_definition.v2`) およびツール CLI には `confidential_policy` およびシリアル マニフェストが必要です。操作、政治、指紋の設定、レジストリの検証を行うための、ジェネシスのアップグレードに関するドキュメントが失われています。

新たな政策は、機密情報を収集し、法的に成り立つ政治的政策を確立するためのものです。チェックリストの前に、発売後のイベントを変更し、財布を変更するかどうかを確認してください。

### マニフェストのバージョン管理 Norito

- Genesis マニフェストには、キー カスタム `confidential_registry_root` に `SetParameter` が含まれています。 El payload es Norito JSON que iguala `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: 省略された el カンポ (`null`) cuando no hay entradas activas, o proveer un string hex de 32 bytes (`0x...`) igual al hash producido por `compute_vk_set_hash` マニフェストの検証に関する指示が表示されます。レジストリ コードのハッシュ ディファレンス パラメータを最初に保存する必要があります。
- オンワイヤ `ConfidentialFeatureDigest::conf_rules_version` のバージョン、レイアウト、マニフェストを埋め込みます。 v1 DEBE 永続化機能 `Some(1)` は、`iroha_config::parameters::defaults::confidential::RULES_VERSION` と同様です。クアンド エル ルールセットの進化、スベ ラ コンスタンテ、リジェネラは、ロック ステップでのデスプリエガ ビナリオを明示します。 mezclar のバージョンは、`ConfidentialFeatureDigestMismatch` を参照してください。
- アクティベーションは、DEBERIAN のレジストリの現実化、パラメトロスと政治の変遷、およびダイジェストの一貫性を示します。
  1. レジストリ計画の変更 (`Publish*`、`Set*Lifecycle`) は、`compute_confidential_feature_digest` でのアクティブ化後の計算をオフラインで確認できます。
  2. `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` を使用して、ピア アトラサドのハッシュ計算を実行し、レジストリの中間指示をダイジェストで修正します。
  3. Anexa las instrucciones `ScheduleConfidentialPolicyTransition`。ガバナンスに関する指示は `transition_id` に送信されます。マニフェストはランタイムを省略します。
  4. マニフェストからバイトを削除し、SHA-256 のフィンガープリントを解除してダイジェストを使用し、アクティベーションを解除します。ロス オペラドールは、安全な証拠を検証するために必要な証拠を作成します。
- カットオーバー ディフェリドのロールアウトに必要なクアンド ロスのロールアウト、カスタム アコンパナンテのパラメトロ オブジェティボの登録 (`custom.confidential_upgrade_activation_height` による)。 Norito は、有効な有効性を確認するために、有効なダイジェストを実行するために必要な情報を提供します。## Ciclo de vida de verificadores y parametros
### レジストロ ZK
- エル・レジャー・アルマセナ `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` ドンデ `proving_system` 実際の状況を把握してください `Halo2`。
- パレス `(circuit_id, version)` 息子グローバルメンテ ユニコス。レジストリ マンティエンは、回路のメタデータを参照してインデックスを作成します。登録者がデュランテ登録を重複して許可する意図。
- `circuit_id` は無効です。`public_inputs_schema_hash` は保証されません (ハッシュ Blake2b-32 の暗号化の正規化と検証の公開入力)。入場料は、レチャザ レジストロス ケ オミテン エストス カンポスです。
- ガバナンスに関する指示には以下が含まれます:
  - `PUBLISH` パラ アグリガー ウナ エントラダ `Proposed` ソロ コン メタデータ。
  - `ACTIVATE { vk_id, activation_height }` の無制限のプログラマ アクティベーション。
  - `DEPRECATE { vk_id, deprecation_height }` パラマルカーラアルトゥーラ最終ドンデ証明プエデン参照ラエントラダ。
  - `WITHDRAW { vk_id, withdraw_height }` 緊急時対応。資産は、安全な秘密を保持し、高さを引き出し、アクティブに保ちます。
- ジェネシスマニフェストは、カスタムパラメータ `confidential_registry_root` が `vk_set_hash` と一致し、アクティバスの自動生成を実行します。有効性を検証し、クルーズ エステのダイジェスト コントラ エル エスケープ ローカル デル レジストリを確認し、合意を確認します。
- レジストラは、`gas_schedule_id` を要求して認証を行います。レジストリの認証情報 `Active`、インデックス `(circuit_id, version)`、y の証明 Halo2 の証明、`OpenVerifyEnvelope` cuyo `circuit_id`、`vk_hash`、y `public_inputs_schema_hash` レジストリの一致。

### 鍵の証明
- 紛失証明キーは、管理対象の管理者からの識別情報 (`pk_cid`、`pk_hash`、`pk_len`) を検証するメタデータとして公開されます。
- PK のウォレット データ、検証ハッシュ、ローカル キャッシュの SDK が失われます。

### パラメトロス・ペデルセンとポセイドン
- レジストロス セパラドス (`PedersenParams`、`PoseidonParams`) 検証対象の自動制御、`params_id` のハッシュ、ジェネラドール/定数のハッシュ、アクティベーション、非推奨、およびレティロの reflejan コントロール。
- `params_id` でのコミットメントのハッシュは、廃止予定のビットとセットのパラメータを変更するために使用されます。 EL ID は、コミットメント、メモ、タグ、ドミニオ、無効化子を埋め込みます。
- ロスサーキットスソポルタン選択マルチパラメータエンティエンポデベリフィカシオン; `deprecation_height` のパラメータを設定し、`deprecation_height` のシグエン シエンド ガステーブルを設定します。`withdraw_height` のレティラドスを設定します。

## 決定的無効化子の順序付け
- `next_leaf_index` と `CommitmentTree` を組み合わせた資産管理; los bloques agregan commits en orden determinista: iterar transacciones en orden de terminista;デントロ デ トランザクション 反復出力は、`output_idx` シリアル化アセンデンテの目隠しを出力します。
- `note_position` 無効化部分に対するオフセットの導出 **なし**。一人で、家族の一員として、プルエバの証人として生きています。
- PRF の無効化に関する規則を確立する。 el input PRF enlaza `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`、Y は `max_anchor_age_blocks` のマークルの歴史的制限を参照するルートをアンカーします。## 帳簿のフルホ
1. **MintConfidential { 資産 ID、金額、受信者ヒント }**
   - 資産 `Convertible` または `ShieldedOnly` の政治的要求。資産の承認認証、実際の `params_id`、実際の `rho`、コミットメントの発行、マークルの取得。
   - `ConfidentialEvent::Shielded` コンエルヌエボコミットメント、デルタマークルルート、ハッシュデラマダデトランザクションパラ監査証跡を発行します。
2. **TransferConfidential {asset_id、proof、circuit_id、version、nullifiers、new_commitments、enc_payloads、anchor_root、memo }**
   - VM 検証のシステムコールは、レジストリの登録を使用します。 el ホスト アセグラ無効化機能は使用せず、コミットメントは形式決定機能、アンカー レシエンテを使用しません。
   - `NullifierSet` の台帳登録、アルマセナ ペイロードは受信者/監査者の暗号化を行い、`ConfidentialEvent::Transferred` を無効化して出力し、オルデナドを出力し、マークル ルートのハッシュ証明を行います。
3. **RevealConfidential {asset_id、proof、circuit_id、version、nullifier、amount、recipient_account、anchor_root }**
   - 適切なソロパラ資産 `Convertible`;ラ・プルーフ・バリダ・ケ・エル・ヴァロール・デ・ラ・ノート・イグアラ・エル・モント・リベラド、エル・レジャー・アクレジット・バランス・トランスペアレント・エ・ケマ・ラ・ノート・ブラインドダダ・マルカンド・エル・ヌリファイア・コモ・ガスタド。
   - `ConfidentialEvent::Unshielded` を公開、無効化コンスミドス、識別子と証明、ハッシュ デ トランザクションを発行します。

## アディシオネス アル データ モデル
- `ConfidentialConfig` (新しい構成) フラグのハビリタシオン、`assume_valid`、ノブのガス/制限、ベンタナのアンカー、バックエンドの検証。
- `ConfidentialNote`、`ConfidentialTransfer`、y `ConfidentialMint` スキーマ Norito バージョン明示的バイト (`CONFIDENTIAL_ASSET_V1 = 0x01`)。
- `{ version, ephemeral_pubkey, nonce, ciphertext }` に関するメモ AEAD の `ConfidentialEncryptedPayload` 環境バイト、XChaCha20-Poly1305 パラメータのデフォルト `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1`。
- `docs/source/confidential_key_vectors.json` でキー導出を実行できるベクトル。タント エル CLI コモ エル エンドポイント Torii レグレサン コントラ エストス フィクスチャ。
- `asset::AssetDefinition` アグレガ `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`。
- `ZkAssetState` 永続的バインディング `(backend, name, commitment)` パラ検証器の転送/シールド解除。取り出した証拠は、キー参照を検証するか、インラインでコミットメント登録と一致することはありません。
- `CommitmentTree` (資産国境チェックポイントによる)、`NullifierSet` は、`(chain_id, asset_id, nullifier)`、`ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` アルマセナドスを世界状態に保ちます。
- 一時的な構造の遷移 `NullifierIndex` および `AnchorIndex` の重複検出およびアンカー チェック。
- スキーマ Norito の実際には、パブリック入力に対する canonico の順序付けが含まれます。エンコーディングの往復アセグラン決定性をテストします。
- 単体テストによるペイロード暗号化のラウンドトリップ (`crates/iroha_data_model/src/confidential.rs`)。ウォレットの副次的トランスクリプトのベクトル AEAD canonicos para audiore。 `norito.md` ドキュメンテーション ヘッダー オンワイヤ パラエル エンベロープ。

## 統合 IVM y システムコール
- システムコール `VERIFY_CONFIDENTIAL_PROOF` の紹介:
  - `circuit_id`、`version`、`scheme`、`public_inputs`、`proof`、および `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` の結果。
  - レジストリの検証用システムコール カーガ メタデータ、タマノ/ティエンポの制限アプリケーション、コブラ ガス決定機能、ソロ アプリケーション エル デルタ シラ プルーフ ティエン 出口。
- ホストは、個別講義 `ConfidentialLedger` のマークル ルートおよび無効化ツールの回復スナップショットを説明します。ライブラリ Kotodama は、アセンブリのヘルパーとスキーマの検証を証明します。
- ドキュメントのポインター - ABI の実際のレイアウト、バッファー、レジストリの処理に関するドキュメントが失われます。

## 容量の交渉
- エルハンドシェイクアヌンシア `feature_bits.confidential` ジュントコン `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`。検証の参加者は `confidential.enabled=true`、`assume_valid=false`、バックエンドの検証の識別子とダイジェストの一致を確認します。 `HandshakeConfidentialMismatch` でのハンドシェイクの矛盾。
- 構成情報 `assume_valid` ソロ パラ ノード オブザーバー: cuando esta deshabilitado、encontrar instrucciones confidentiales が `UnsupportedInstruction` determinista sin パニックを生成します。 cuando esta habilitado、観察者は、罪の証明を申請します。
- 地域の安全性に関する秘密情報を記録します。ゴシップ エビタンの情報を無視して、ピアの罪の容量が偶然に一致することを確認し、タマノの制限を確認することができます。

### マトリス・デ・ハンドシェイク|アヌンシオ・リモト |検証結果の検証 |オペラ ノート |
|---------------------|------------------------------|--------------|
| `enabled=true`、`assume_valid=false`、バックエンド一致、ダイジェスト一致 |アセプタド | `Ready` はプロジェクトに参加し、ファンアウト RBC に投票します。アクションマニュアルは必要ありません。 |
| `enabled=true`、`assume_valid=false`、バックエンドの一致、ダイジェストが古いまたはファルタンテ |レチャザード (`HandshakeConfidentialMismatch`) |レジストリ/パラメータ、または `activation_height` プログラムの保留中のアクティブ化を実行します。ハスタ・コレギル、エル・ノド・シグエが目に見えるペロ・ヌンカ・エントラ・エン・ロータシオン・デ・コンセンサス。 |
| `enabled=true`、`assume_valid=true` |レチャザード (`HandshakeConfidentialMismatch`) |ロスバリダレスは証明の検証を要求します。リモート コンモ オブザーバー コンイングレス ソロ Torii またはカンビア `assume_valid=false` の動作確認が完了しました。 |
| `enabled=false`、カンポス・オミティドス (ビルド・デ・ザ・アクチュアリザド)、検証用バックエンド |レチャザード (`HandshakeConfidentialMismatch`) |ピアは実際の実現を実現するために、コンセンサスを確立する必要はありません。実際のリリースでは、タプル バックエンドとダイジェストが同時にリリースされます。 |

オブザーバーは、能力ゲートのコンセンサスとコンセンサスに基づいて検証を省略し、検証を省略します。 Torii を介して、API がアーカイブされ、コンセンサス ロス レチャザが発生したときに、同時に発生する可能性のある API がブロックされます。

### 政治による剪定と無効化者の保持の暴露

帳簿は機密情報であり、歴史を記録したり、ガバナンスに関する監査の衝動を再現したりするのに十分です。欠陥のある政治、`ConfidentialLedger` による申請、es:

- **無効化者の保持期間:** `730` ディアス (24 メセス) の *minimo* による無効化者の管理は、ガスト、ベンタナ規制の義務を市長に負っています。 `confidential.retention.nullifier_days` 経由のロス オペラドーレス プエデン エクステンダー ラ ベンタナ。 Torii 経由で DEBEN の相談可能性を無効にし、二重支出を防止します。
- **明らかにするプルーニング:** 透明性を明らかにする (`RevealConfidential`) は、関係性に関するコミットメントを仲介し、ブロックの最終決定を行い、事前に保持する権限を無効にします。イベント関連情報 (`ConfidentialEvent::Unshielded`) レジストラン エル モント パブリック、受信者 y ハッシュ デ プルーフ パラ ケ 再構築により、歴史的な要求がないこと、暗号文ポダドが明らかになります。
- **フロンティアチェックポイント:** 国境地帯のチェックポイントは、`max_anchor_age_blocks` および保持期間中の市長のローリングキューにあります。ロス ノードス コンパクタン チェックポイント マス アンティグオス ソロ デ ケ トドス ロス ヌリファイアー デントロ デル インターバル エクスピラン。
- **ダイジェストが古くなった場合の修復:** ダイジェストのドリフト、オペラドールのデベンタ `HandshakeConfidentialMismatch` の修正 (1) クラスターとヌリファイアーの一致検証、(2) ヌルファイアーとの比較 `iroha_cli app confidential verify-ledger` の再生成retenidos, y (3) マニフェストを再展開します。 Cualquier nullifier podado prematurmente deberestaurarse desde almacenamiento frio antes de reingresar a la red.

Documenta は、オペレーションのランブックのロケールをオーバーライドします。政治の統治は、ベンタナの保持期間を延長し、ノードやプレーンの設定をロックステップで実行します。

### 立ち退きと回復のフルホ

1. デュランテ エル ダイヤル、`IrohaNetwork` は、安全性を比較します。 Cualquier の不一致レヴァンタ `HandshakeConfidentialMismatch`;私は、`Ready` のプロモーションを発見するために、永続的な接続を確立します。
2. サービス ログ (ダイジェスト リモート バックエンドを含む) を介して説明が行われると、Sumeragi プログラム ピア パラプロプエスタ ボートが表示されます。
3. オペラドールの修正、検証用のレジストリ、パラメーターのセット (`vk_set_hash`、`pedersen_params_id`、`poseidon_params_id`)、`next_conf_features` と `activation_height` アコルダードのプログラム。ダイジェストが一致すると、ハンドシェイクが自動的に終了します。
4. 古いログラ ディファンディル ウン ブロック (記録のリプレイ経由) を確認し、`BlockRejectionReason::ConfidentialFeatureDigestMismatch` で形式を決定し、一貫性のある帳票を管理します。

### フルホ デ ハンドシェイク セグロ アンティ リプレイ1. クラーベノイズ/X25519 ヌエボの重要な素材を指定します。ハンドシェイクのペイロード (`handshake_signature_payload`) は、ローカルとリモートの接続で、クラベス パブリックの電子メールを接続し、Norito でソケットのコードを指示し、`handshake_chain_id` をコンパイルし、チェーンの識別子を確認します。 AEAD の安全性を暗号化します。
2. 受容体はペイロードを再計算し、クラベス ピア/ローカルの検証を検証します。Ed25519 embebida en `HandshakeHelloV1`。デビド・ア・ケ・アンバス・クラベス・エフィメラスとラ・ディレクション・アヌンシアダ・フォーマン・パート・パルテ・デル・ドミニオ・デ・ファーム、再現性ウン・メンサヘ・キャプチャード・コントラ・オトロ・ピア・オ・レキュペラール・ウナ・コネクシオン・スタレ・フォールラ・ラ・ベリフィカシオン・デ・フォーマ・デターミニスタ。
3. `HandshakeConfidentialMeta` 経由で容量秘密フラグを設定します。 Ｅ１受容体は、タプルＩ１８ＮＩ０００００２６０Ｘと、Ｉ１８ＮＩ０００００２６１Ｘ局所とを比較する。 cualquier 不一致販売 temprano con `HandshakeConfidentialMismatch` antes de que el Transporte transicione a `Ready`。
4. ロス オペラドール DEBEN 再計算のダイジェスト (`compute_confidential_feature_digest` 経由) と、再接続前のレジストリ/政治の実際のノードの詳細。ピアズ・ケ・アヌンシャンは、アンチグオス・シグエン・ファランド・エル・ハンドシェイク、エヴィタンド・ケ・エスタド・スタッド・レイングレス・アル・セット・デ・バリダドレスを消化します。
5. ハンドシェイクを終了し、実際のコンタドール `iroha_p2p::peer` (`handshake_failure_count`、エラー分類法ヘルパー) と、ログ構造体とピア ID リモートとフィンガープリントとダイジェストを出力します。ロールアウト中に検出器のリプレイや設定が正しくないかどうかを監視します。

## クラーベとペイロードのジェスション
- アカウントの派生情報:
  - `sk_spend` -> `nk` (無効化キー)、`ivk` (受信表示キー)、`ovk` (送信表示キー)、`fvk`。
- ECDH から派生した AEAD の共有キーを暗号化するペイロード。補助的な表示キーは監査オプションであり、資産の政治情報を出力します。
- CLI に追加: `confidential create-keys`、`confidential send`、`confidential export-view-key`、説明メモ用の監査ツール、製造者/検査用エンベロープ Norito のヘルパー `iroha app zk envelope` オフライン。 Torii は、`POST /v2/confidential/derive-keyset` 経由でミスモ フルホ デ デリバシオンを説明し、16 進数の Base64 パラケ ウォレットを取得してクラベス プログラムを作成します。

## ガス、DoS 制御の制限
- ガスのスケジュール決定:
  - Halo2 (Plonkish): ベース `250_000` ガス + `2_000` ガス、パブリック入力。
  - `5` ガスポートプルーフバイト、マスカーゴポートヌリファイアー (`300`) とコミットメント (`500`)。
  - 構成デル ノード (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) を介して、オペラドールの操作を実行するための定数を設定します。ロス カンビオスは、クラスタの形式を決定するためのホット リロード構成のプロパガンダを作成します。
- 期間を制限します (デフォルト設定可能):
- `max_proof_size_bytes = 262_144`。
- `max_nullifiers_per_tx = 8`、`max_commitments_per_tx = 8`、`max_confidential_ops_per_block = 256`。
- `verify_timeout_ms = 750`、`max_anchor_age_blocks = 10_000`。 `verify_timeout_ms` 形式決定の指示を中止したことが証明されます (`proof verification exceeded timeout`、`VerifyProof` のレトルナ エラーによるガバナンスの投票用紙)。
- Cuotas adicionales aseguran liveness: `max_proof_bytes_block`、`max_verify_calls_per_tx`、`max_verify_calls_per_block`、y `max_public_inputs` acotan ブロック ビルダー。 `reorg_depth_bound` (>= `max_anchor_age_blocks`) 国境のチェックポイントを保持します。
- ランタイムの実行時に、トランザクションまたはブロックの制限を超えたエラーが発生しました。`InvalidParameter` は、カンビオ上の帳簿上のエラーを決定します。
- `vk_id` による事前フィルター処理の機密情報、長期にわたる証拠、アンカーの事前確認、および再帰的な管理に関する検証。
- タイムアウトまたは制限違反の形式を決定するための検証。ラス・トランザクション・ファラン・コン・エラー・明示的。バックエンド SIMD は、ガスの代替アカウンティングを必要とせずに実行できます。

### ベースラインとキャリブレーションとゲートの受け入れ
- **参照プラットフォーム** ハードウェアの情報を正確に測定するための DEBEN CUBRIR の情報を確認してください。改訂版を再確認する必要はありません。|パーフィル |建築家 | CPU / インスタンス |コンピラドールのフラグ |プロポジト |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) o インテル Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` |最も重要な罪の組み込みベクトルを設定します。米国パラアジュスタータブラスデコストフォールバック。 |
  | `baseline-avx2` | `x86_64` |インテル Xeon ゴールド 6430 (24c) |欠陥のあるリリース |パス AVX2 を検証します。リビザ ケ ロス スピードアップ SIMD セ マンテンガン デントロ デ トレランシア デル ガス ニュートラル。 |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) |欠陥のあるリリース | Asegura は、バックエンド NEON の永続的な決定とスケジュール x86 を実行します。 |

- **ベンチマーク ハーネス** DEBEN ガス校正レポートの作成:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` パラ確認エルフィクスチャ決定者。
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` VM のオペコードのカンビアンコスト。

- **ランダム性は問題ありません。** `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` をエクスポートして、`iroha_test_samples::gen_account_in` を確認して、`KeyPair::from_seed` を確認します。 El ハーネス インプライム `IROHA_CONF_GAS_SEED_ACTIVE=...` ウナベス; si falta la 変数、la リビジョン DEBE fallar。新しい校正機能を使用して、安全な環境を導入したり、補助的に使用したりできます。

- **結果のキャプチャ**
  - リリース基準に関する基準 (`target/criterion/**/raw.csv`) は、成果物に関する情報を提供します。
  - Guardar metricas 派生 (`ns/op`、`gas/op`、`ns/gas`) と [機密ガス校正台帳](./confidential-gas-calibration) は、コンパイラのバージョンを確認します。
  - パーフィルのベースラインを管理します。新しいスナップショットを削除し、新しい検証結果を報告します。

- **許容寛容**
  - デルタガスエントレ `baseline-simd-neutral` y `baseline-avx2` DEBEN 永続性 <= +/-1.5%。
  - デルタガスエントレ `baseline-simd-neutral` y `baseline-neon` DEBEN 永続性 <= +/-2.0%。
  - RFC の矛盾を解消するために、スケジュールを調整するための基準を超えた校正を行います。

- **改訂のチェックリスト** 提出者の息子の責任:
  - `uname -a`、`/proc/cpuinfo` の抽出 (モデル、ステッピング)、`rustc -Vv` のキャリブレーション ログが含まれます。
  - Verificar que `IROHA_CONF_GAS_SEED` se vea en la salida de bench (ベンチはシードアクティバを実行します)。
  - Asegurar que los 機能フラグ デル ペースメーカーおよびデル検証の機密エスペジェン生産 (`--features confidential,telemetry` al correr benches con Telemetry)。

## 設定操作
- `iroha_config` セクション `[confidential]` の集合:
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
- テレメトリアはメトリクスの集合体を発行します: `confidential_proof_verified`、`confidential_verifier_latency_ms`、`confidential_proof_bytes_total`、`confidential_nullifier_spent`、`confidential_commitments_appended`、`confidential_mempool_rejected_total{reason}`、y `confidential_policy_transitions_total`、ヌンカ・エキスポニエンド・ダトス・エン・クラロ。
- 地上権 RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## テスト戦略
- 決定性: トランザクションの実行順序をシャッフルして、マークル ルートを生成し、無効化識別子のセットを作成します。
- Resiliencia a reorg: 複数ブロックのアンカーをシミュレートする reorgs。 nullifiers は mantienen を安定させ、アンカーは古くなり、rechazan になります。
- ガスの不変量: SIMD の高速化に使用されるガスの同一性を検証します。
- 限界テスト: タマノ/ガスのテクノロジの証明、最大入出力カウント、タイムアウトの強制。
- ライフサイクル: ガバナンスに関する活性化/非推奨の検証およびパラメータの運用、ガストトラス回転のテスト。
- FSM ポリシー: 許可のトランジション/許可なし、トランジションの保留と有効性の確認の遅延。
- 緊急レジストリ: `withdraw_height` の緊急コンジェラ資産を確認し、証拠を確認します。
- 機能ゲーティング: validadores con `conf_features` 不一致の rechazan ブロック。オブザーバーズ・コン `assume_valid=true` アヴァンザン・シン・アフェクター・コンセンサス。
- 確立された同等性: ノードのバリデーター/フル/オブザーバーが生成したルートと、カデナの正規化された構造の同一性。
- 否定的なファジング: 不正な形式、ペイロードの無効化と衝突の検証。## マイグラシオン
- ロールアウト機能フラグ: フェーズ C3 が完了しました。`enabled` デフォルトは `false`。ロスノドスは、有効性を設定するために必要な容量を通知します。
- 資産はいかなる影響も透過しません。レジストリと容量の交渉に関する秘密情報を要求します。
- 形式的な決定に関する秘密情報を収集します。 pueden unirse al set validador pero pueden operar como observers con `assume_valid=true` はありません。
- ジェネシスのマニフェストには、登録簿の初期記録、パラメータ設定、資産に関する政治秘密、および監査人の意見の鍵が含まれています。
- レジストリの定期運用、政治の経過、および緊急時の保守管理のアップグレードを決定するための、オペラドールの運用手順書を公開します。

## トラバホ ペンディエンテ
- Halo2 パラメータのベンチマーク (回路のタマノ、ルックアップの戦略) および `confidential_assets_calibration.md` の近傍リフレッシュの実際のガス/タイムアウトのデフォルト設定とプレイブックの調整パラメータのレジストラ結果。
- 選択的閲覧関連の監査機関および API の政治開示の最終決定、Torii のガバナンス管理の安全性確保のための接続に関する最終決定。
- 証人暗号化機能を備えたエクステンダーは、複数の受信者のメモをバッチで出力し、SDK を実装する際に文書化およびエンベロープ形式を実行します。
- 外部回路の改訂、レジストリ、パラメータの回転手順、および公聴会の内部レポートのアーカイブを収集します。
- 監査時の支出の調整と、ウォレットのベンダーに対するビューキーの公開設定に関する特定の API が、セキュリティのセマンティクスを実装します。

## 実装の失敗
1. **フェーズ M0 - ストップシップの継続**
   - [x] ポセイドン PRF (`nk`、`rho`、`asset_id`、`chain_id`) の無効化の派生と実際の台帳に対するコミットメントの決定。
   - [x] トランザクション/ブロックの秘密を保護するためのアプリケーションの制限、エラーの決定に関する事前のトランザクションの再確認。
   - [x] ハンドシェイク P2P 通知 `ConfidentialFeatureDigest` (バックエンドのダイジェスト + レジストリのフィンガープリント) と、`HandshakeConfidentialMismatch` による形式決定の不一致。
   - [x] リムーバーは、移動中に機密情報や集合的な役割をゲートするパスでパニックを起こします。
   - [ ] 辺境チェックポイントの再編成におけるタイムアウト検証の事前準備と制限。
     - [x] 検証アプリケーションのタイムアウトの準備。 `verify_timeout_ms` を超えた証拠。
     - [x] フロンティア チェックポイントは、`reorg_depth_bound` に関する問題であり、決定的なスナップショットを確認するためのチェックポイントです。
   - `AssetConfidentialPolicy` の導入、FSM ポリシーの作成/転送/開示に関する執行のゲート。
   - Comprometer `conf_features` のヘッダー デ ブロックと検証参加のレジストリ/パラメータ分岐のダイジェスト。
2. **フェーズ M1 - レジストリとパラメータ**
   - Entregar レジストリ `ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` ガバナンス管理、生成管理、キャッシュ管理。
   - Conectar システムコールによるレジストリの検索、ガス スケジュールの ID、スキーマのハッシュ、タマノ チェック。
   - ペイロード暗号化 v1 のエンビア形式、ウォレットのキー導出ベクトル、クラベス機密情報の CLI パラメータのサポート。
3. **フェーズ M2 - ガスのパフォーマンス**
   - ガス決定のスケジュール、ブロックのコンタドール、テレメトリのベンチマークのハーネス (検証の遅延、証明のタマノス、メンプールの再確認) を実装します。
   - Endurecer CommitmentTree チェックポイント、carga LRU、マルチアセットのワークロードに対する Nullifier インデックス。
4. **フェーズ M3 - ウォレットのローテーションとツール**
   - マルチパラメータとマルチバージョンの証明の受け入れ。ガバナンスとランブックの移行における活性化/非推奨の推進。
   - SDK/CLI での移行作業、監査のワークフロー、支出の調整のツール。
5. **フェーズ M4 - 業務の監査**
   - 監査人のキーのワークフロー、開示選択の API、ランブックの運用を証明します。
   - プログラムのリビジョンは、暗号文/セキュリティの外部に公開されており、`status.md` で公開されています。ロードマップとテストの実際のマイルストーンは、赤いブロックチェーンの安全性を決定するためのテストです。
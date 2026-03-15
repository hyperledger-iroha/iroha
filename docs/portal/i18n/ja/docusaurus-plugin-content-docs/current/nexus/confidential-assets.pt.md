---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Ativos confidenciais e transferencias ZK
説明: 目隠しのためのブループリント フェーズ C、レジストリとオペレータの制御。
スラグ: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# ZK の秘密と転送のデザイン

## モティバカオ
- オプトイン パラケ ドミニオスを保護するために、透明な環境での安全な管理を行う必要があります。
- ハードウェアの異機種間検証と保存 Norito/Kotodama ABI v1 の実行を決定します。
- サーキットおよびパラメトロ クリプトグラフィーのシステム (アティバカオ、ロタカオ、レボガカオ) を管理する聴衆およびオペラドール。

## 脅威モデル
- Validadores は正直だが好奇心旺盛です: 台帳/状態を検査するためにコンセンサスを実行します。
- 監視員は、ブロックやトランザクションの噂話を聞きました。ゴシップのプライバシーを保護します。
- Fora de escopo: オフレジャーのトラフェゴ分析、敵対的なクォンティコス (ロードマップ PQ なし)、台帳の管理。

## Visao ジェラルドデザイン
- *シールドプール* のアセットポデム宣言は、存在するバランスを透明にします。コミットメントクリプトグラフィックスを介して、目隠しと代表者を循環させます。
- Notes encapsulam `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - コミットメント: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`。
  - 無効化: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`、メモとしては独立しています。
  - ペイロード暗号化: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`。
- Transacoes のトランスポート ペイロード `ConfidentialTransfer` コードと Norito 内容:
  - publicos の入力: マークル アンカー、ヌリファイアー、ノボス コミットメント、アセット ID、回路のバージョン。
  - ペイロードは受信者と監査人の権限を暗号化します。
  - ゼロ知識の証明、勇気の保全、所有権、自己所有権。
- 台帳上のレジストリを介して、SAO 制御のパラメータに含まれるキーを検証します。ノードは有効な証明を参照し、参照内容を確認し、レボガダを確認します。
- ヘッダーのコンセンサスまたはダイジェスト、機密性の高いブロック、つまりレジストリとパラメータの安全性が一致します。
- スタック Halo2 (Plonkish) sem の信頼できるセットアップを使用して証明を構築します。 Groth16 のアウトラス バリアント SNARK は、v1 のサポートを強化します。

### 試合の決定者

封筒はメモの秘密を保持し、固定具のカノニコ em `fixtures/confidential/encrypted_payload_v1.json` を設定します。データセット キャプチャ エンベロープ v1 は、SDK を解析する上で、最も否定的で不正な形式であることを確認しています。オスのテストは、Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) およびスイート Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) のデータ モデルを実行し、Norito をエンコードするためのパラメータを管理し、回帰防止の永久保証として暗号化コーデックを保証します。エボルイ。

SDK Swift アゴラ ポデム エミッター インストルコ シールド セム グルー JSON ビスポーク: construa um
`ShieldRequest` 32 バイトのコミットメント、ペイロード暗号化およびデビットのメタデータ、
e entao Chame `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) para assinar e encaminhar a
`/v2/pipeline/transactions`経由のtransacao。おお、献身的な報酬を認めてくれる助け人よ、
挿入 `ConfidentialEncryptedPayload` エンコーダなし Norito、レイアウトなし `zk::Shield`
Rust の財布の説明。

## コンセンサスコミットメントと機能ゲーティング
- ヘッダー デ ブロックの説明 `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`;ハッシュとコンセンサスの参加をダイジェストし、ブロックの安全性を確認するローカルのレジストリを作成します。
- ガバナンス ノードはアップグレード プログラムを準備し、`next_conf_features` com um `activation_height` futuro; essa alturaを食べ、produtores de bloco devemを継続的に放出し、前方を消化します。
- ノードは DEVEM オペランド com `confidential.enabled = true` e `assume_valid = false` を検証します。 `conf_features` ローカル ダイバーギルをチェックし、スタートアップのエントリが設定されていないことを確認します。
- メタデータは `{ enabled, assume_valid, conf_features }` を含むハンドシェイク P2P アゴラを実行します。ピアは、互換性のない機能を通知します。`HandshakeConfidentialMismatch` は、コンセンサスに関する情報を提供します。
- 有効なハンドシェイクの結果、オブザーバー、ピア、ハンドシェイクのマトリズをキャプチャする [ノード機能ネゴシエーション](#node-capability-negotiation)。ハンドシェイクの説明会 `HandshakeConfidentialMismatch` は、一致した会議とダイジェストの一致を確認するために会議を開催します。
- オブザーバー nao validadores podem definir `assume_valid = true`;デルタは信頼できる証拠を検証し、合意に影響を与えます。## 資産の政治
- ガバナンスによる資産管理 `AssetConfidentialPolicy` の定義:
  - `TransparentOnly`: modo のデフォルト;透明な説明 (`MintAsset`、`TransferAsset` など) は許可され、オペラは保護されます。
  - `ShieldedOnly`: 秘密情報の開示と転送に関する指示。 `RevealConfidential` は、公開された情報のバランスを調整します。
  - `Convertible`: ホルダーはポデム ムーバーの勇気を示し、透明なシールド付きのオン/オフ ランプ アバイソの指示を示します。
- FSM の安全保障政策に関する政策:
  - `TransparentOnly -> Convertible` (シールドされたプールをすぐに利用できる)。
  - `TransparentOnly -> ShieldedOnly` (トランジションペンデンテとジャネラデ会話を要求)。
  - `Convertible -> ShieldedOnly` (遅延最小限義務)。
  - `ShieldedOnly -> Convertible` (Plano de migracao requerido para que Notes Blindadas continuem Gastaveis)。
  - `ShieldedOnly -> TransparentOnly` は、シールド プールのエステジャ ヴァジオとガバナンス法典を管理するために、移民に関するメモを作成する必要があります。
- ISI `ScheduleConfidentialPolicyTransition` および `CancelConfidentialPolicyTransition` のプログラム中止に関するガバナンス定義 `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }`。これは、政治的問題の解決を確実に行うための重要なチェックを含む、重要な取引を保証するものです。
- 自動アプリケーションの自動更新: 高度な対話 (アップグレード `ShieldedOnly`) を実行中 `effective_height`、ランタイム更新 `AssetConfidentialPolicy`、メタデータの参照`zk.policy` リンパ・ア・エントラダ・ペンデンテ。 `ShieldedOnly` は、透明な永続的な転送を提供し、ランタイムの中止と登録の再開、前方への更新を継続します。
- ノブの設定 `policy_transition_delay_blocks` と `policy_transition_window_blocks` は、ミニモと許容範囲の期間を設定し、ウォレットとトルノ ダ ムダンカの会話を許可します。
- `pending_transition.transition_id` 聴覚機能を操作します。ガバナンスは、都市の最終計画とキャンセルのトランジションを開発し、オペラドールとの関係をオン/オフランプに関連付けます。
- `policy_transition_window_blocks` のデフォルトは 720 (~12 時間の com ブロック時間、60 秒)。ノードの制限は、管理上の要求に応じて制限されます。
- ジェネシスは、政治と懸案を明らかにし、CLI を説明します。政治的承認の論理は、秘密保持の自己決定を確認するために一時的に実行されます。
- 移行チェックリスト - バージョン「移行シーケンス」を実行して、マイルストーン M0 をアップグレードします。

#### Monitorando transicoes (Torii 経由)

財布と監査は `GET /v2/confidential/assets/{definition_id}/transitions` の検査を依頼し、`AssetConfidentialPolicy` を検査します。 O ペイロード JSON semper inclui oasset ID canonico、a ultima altura de bloco observada、o `current_mode` da politica、o modo efetivo nessa altura (janelas de conversao reportam Temporariamente `Convertible`)、e os identificadores esperados de `vk_set_hash`/ポセイドン/ペダーセン。政府の方針を決定し、報告書を提出してください:

- `transition_id` - `ScheduleConfidentialPolicyTransition` の聴覚障害を処理します。
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` e o `window_open_height` 派生 (O ブロッコオンデウォレット開発、カットオーバーとの会話、ShieldedOnly)。

レスポスタの例:

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

Uma resposta `404` は資産通信の定義を示しています。クアンド・ナオ・ハ・トランジカオ・アジェンダダ・オ・カンポ `pending_transition` と `null`。

### 政治家のマキナ|モドリアル |プロキシモモード |前提条件 |効果的な治療法 |メモ |
|------------------|------------------|---------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
|透明のみ |コンバーチブル |検証レジストリ/パラメータの管理に関するガバナンス。サブメーター `ScheduleConfidentialPolicyTransition` com `effective_height >= current_height + policy_transition_delay_blocks`。 | `effective_height` でトランザクションを実行します。すぐに対応できるシールド付きプール。               | Caminho デフォルト パラ ハビリタル コンフィデンシャルイダーデ マンテンド フラクソス トランスペアレンス。          |
|透明のみ |シールド付きのみ |メスモ アシマ、`policy_transition_window_blocks >= 1` です。                                                         | O ランタイムの自動調整 em `Convertible` em `effective_height - policy_transition_window_blocks`; `ShieldedOnly` と `effective_height` を比較します。 |会話の内容を決定し、安全性を確保するための指示を出します。   |
|コンバーチブル |シールド付きのみ | Transicao プログラム com `effective_height >= current_height + policy_transition_delay_blocks`。監査メタデータによるガバナンス DEVE 認証 (`transparent_supply == 0`)。ランタイム アプリケーションはカットオーバーなしです。 |同一の意味。 nao-zero em `effective_height`、transicao aborta com `PolicyTransitionPrerequisiteFailed` に透明な情報を提供します。 | Trava は機密情報をすべて公開しています。                                      |
|シールド付きのみ |コンバーチブル | Transicao プログラム。 SEM 緊急撤退 (`withdraw_height` indefinido)。                              | O estado muda em `effective_height`;ランプを明らかにする、リーブレム・エンクアント・ノート、ブラインドダス・パーマネセム・バリダス。             |監査を行ったり、監査を行ったりする必要があります。                                |
|シールド付きのみ |透明のみ |ガバナンスは、`shielded_supply == 0` 計画の準備を行っています。`EmergencyUnshield` assinado (監査要求の監査)。 | O ランタイムは `effective_height` より前の `Convertible` です。アルチュラ、ファルハム デュロ、アセット レポート、およびモードの透明のみを保証します。 |究極の再帰性。ガスタ・デュランテ・ア・ジャネラの機密事項である自動キャンセル通知書。 |
|任意 |現在と同じ | `CancelConfidentialPolicyTransition` リンパ・ア・ムダンカ・ペンデンテ。                                                    | `pending_transition` すぐにビデオを削除します。                                                                        |現状維持。ほとんどの完全性。                                             |

Transicoes nao listadas acima sao rejeitadas durante submissao de Government. O ランタイムの前提条件のロゴを確認し、移行プログラムを適用する必要があります。ファルハスは、テレメトリアとイベント デ ブロコを介して資産を開発し、前方から `PolicyTransitionPrerequisiteFailed` を送信します。

### 移行のシーケンス1. **準備レジストリ:** 検証機関としての活動や政治的参照に関する情報。 `conf_features` の結果としてピアが検証されたノードが通知されます。
2. **トランジションの議題:** サブメーター `ScheduleConfidentialPolicyTransition` が `effective_height` QUE RESPEITE `policy_transition_delay_blocks` に連絡します。 `ShieldedOnly` の青いムーバー、特別な会話 (`window >= policy_transition_window_blocks`)。
3. **パブリック ガイド パラ オペラドール:** レジストラ、`transition_id` の回覧、オン/オフランプのランブック。財布は assinam `/v2/confidential/assets/{id}/transitions` を購入し、さまざまな種類の財布を購入できます。
4. **Aplicar janela:** Quando a janela abre、o runtime muda a politica para `Convertible`、emite `PolicyTransitionWindowOpened { transition_id }`、e Comeca a rejeitar request de Government Conflicantes。
5. **最終的な中止:** em `effective_height`、または実行時の前提条件の検証 (透明なゼロの供給、緊急時の安全確認など)。政治的な問題を解決するために必要なこと。ファルハは `PolicyTransitionPrerequisiteFailed` を発し、政治的混乱を招くことはありません。
6. **スキーマのアップグレード:** トランザクションの管理、スキーマの資産管理のガバナンス強化 (例、`asset_definition.v2`)、ツール CLI の exige `confidential_policy` シリアル化マニフェスト。生成操作のアップグレードに関するドキュメント、政治に関する追加設定、および検証前のレジストリに関する指紋。

政治の起源を明らかにするために、新しい情報を公開します。チェックリストを確認し、スケジュールを調整して、ジャネラスとの会話を決定し、ウォレットを確認して、システムを起動します。

### マニフェストのバージョン管理 Norito

- Genesis マニフェストには、キー カスタム `confidential_registry_root` に `SetParameter` が含まれる DEVEM が含まれています。ペイロード Norito JSON クエリは `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` に対応します: カンポ (`null`) を省略し、32 バイトの 16 進数の文字列 (`0x...`) の初期値ハッシュを生成します。 `compute_vk_set_hash` 検証手順がマニフェストに記載されていないため、地味です。ノードは、レジストリ コードの主要なパラメータやハッシュ ダイバーギルを検索します。
- O オンワイヤー `ConfidentialFeatureDigest::conf_rules_version` は、レイアウトのマニフェストをエミュレートします。 v1 DEVE 永続化 `Some(1)` は、`iroha_config::parameters::defaults::confidential::RULES_VERSION` と同等です。ルールセットを進化させ、定数を増やし、マニフェストを再生成し、バイナリのロールアウトをロックステップで実行します。ミストゥラル ヴァーソス ファズ バリダレス レジェイタレム ブロコス コム `ConfidentialFeatureDigestMismatch`。
- アクティベーション マニフェストは、DEVEM のレジストリ更新、都市政策の管理、政治パラメタの管理、ダイジェストの一貫性を示します。
  1. レジストリの変更点 (`Publish*`、`Set*Lifecycle`) は、オフラインで表示され、計算またはダイジェスト pos-ativacao com `compute_confidential_feature_digest` で表示されます。
  2. Emitir `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` は、ピア アトラサドの回復手段としてハッシュ計算を行い、仲介者による適切な指示を消化します。
  3. アネクサーの説明書 `ScheduleConfidentialPolicyTransition`。 `transition_id` はガバナンスに関する情報を提供します。実行時に必要なマニフェストを作成します。
  4. 永続的な OS バイトは明示されます。UM フィンガープリント SHA-256 は、ダイジェストを使用せず、計画を立てません。オペラドールは、安全な操作を検証し、実際のパーティを明らかにします。
- Quando は、カットオーバー ディフェリドを拡張し、パラメトロ カスタム コンパンヘイロのアルトゥーラを登録します (`custom.confidential_upgrade_activation_height` など)。 Norito は、有効な有効性を確認するために、有効なコードを取得し、ダイジェストを入力します。## 検証者とパラメータの動作確認
### ZK レジストリ
- O レジャー armazena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` および `proving_system` の実際の修正と `Halo2`。
- パレス `(circuit_id, version)` sao グローバルメンテ ユニコス; o 回路上のメタデータを管理するためのレジストリ管理インデックス。登録者が重複して登録できる登録期間の許可。
- `circuit_id` 開発者は、`public_inputs_schema_hash` 開発者です (ハッシュ Blake2b-32 は、公的入力のエンコードを行い、検証を行います)。入場料は、レジストロス ケ オミテム エセス カンポスに登録されます。
- ガバナンスに関する指示には以下が含まれます:
  - `PUBLISH` パラ Adicionar uma entrada `Proposed` apenas com メタデータ。
  - `ACTIVATE { vk_id, activation_height }` は、時代の限界を超えてプログラムを実行します。
  - `DEPRECATE { vk_id, deprecation_height }` パラマーク、アルトゥーラ最終オンデ証明ポデム参照、エントラーダ。
  - `WITHDRAW { vk_id, withdraw_height }` 緊急事態宣言;アフェタドス・コンゲラム・ガストス・コンフィデンシアス・アポス・アポス・撤退高さは、ノバス・エントラダス・アティヴァレムを襲った。
- Genesis マニフェストは、auto-emitem um parametro カスタム `confidential_registry_root` cujo `vk_set_hash` が entradas ativas と一致します。検証済みのクルーズ エッセンシャル ダイジェスト コムを作成し、ローカルのレジストリを確認してから、ノードにアクセスするためのコンセンサスがありません。
- レジストラまたは検証者は `gas_schedule_id` を要求します。認証情報を確認し、登録情報を登録します `Active`、インデックスなし `(circuit_id, version)`、証明する Halo2 fornecam `OpenVerifyEnvelope` cujo `circuit_id`、`vk_hash`、e `public_inputs_schema_hash` はレジストリに対応します。

### 鍵の証明
- コンテンツアドレス指定された識別子 (`pk_cid`、`pk_hash`、`pk_len`) の公開鍵を台帳外で検証し、メタデータを検証します。
- SDK のウォレット バスカム ダドス、PK、検証ハッシュ、ファゼム キャッシュ ローカル。

### パラメトロス・ペデルセンとポセイドン
- レジストリは分離 (`PedersenParams`、`PoseidonParams`) 検証者の自動制御、`params_id` のキャッシュ、ゲラドール/定数のハッシュ、アティバカオ、非プレカカオ、および引き出し高さを制御します。
- コミットメントは、`params_id` のパラメータとドミニオスを分離し、廃止されたパラメータのパラメータを再利用します。 o ID とコミットメントとメモとタグとドミニオと無効化子。
- 検証を行うためのマルチパラメータの選択をサポートする回路。 `deprecation_height` で使用可能なパラメータを非推奨にし、`withdraw_height` を再設定します。

## Ordenacao determinista e nullifiers
- Cada 資産管理 `CommitmentTree` com `next_leaf_index`;ブロコス・アクレッセンタムのコミットメントは、決定的に決定されます。デントロ デ cada transacao iterar 出力シールド ポート `output_idx` シリアル化アセンデンテ。
- `note_position` e デリバド dos オフセット da arvore mas **nao** faz parte do nullifier;会員資格を取得するためのパスは、証拠を証明するために必要です。
- 無効化された不正な組織を確立し、PRF を保証します。 o 入力 PRF vincula `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`、アンカー参照ルート `max_anchor_age_blocks` のマークルの歴史的限界。

## Fluxo の台帳
1. **MintConfidential { 資産 ID、金額、受信者ヒント }**
   - 資産 `Convertible` または `ShieldedOnly` の政治を要求します。入学チェカ・オートリダーデ・ド・アセット、recupera `params_id` atual、amostra `rho`、emit commit、atualiza a Arvore Merkle。
   - `ConfidentialEvent::Shielded` com o novo コミットメント、デルタ デ マークル ルート、ハッシュ デ チャマダ ダ トランザクション パラ監査証跡を発行します。
2. **TransferConfidential {asset_id、proof、circuit_id、version、nullifiers、new_commitments、enc_payloads、anchor_root、memo }**
   - Syscall VM 検証証明は、レジストリを登録します。 o ホストは無効化を保証し、最近のコミットメントは決定性とアンカーを保証します。
   - 台帳登録エントリ `NullifierSet`、armazena ペイロードは受信者/監査者の暗号化と出力 `ConfidentialEvent::Transferred` の無効化子、出力 ordenados、ハッシュ証明、およびマークル ルートを記録します。
3. **RevealConfidential {asset_id、proof、circuit_id、version、nullifier、amount、recipient_account、anchor_root }**
   - 資産`Convertible`の処分。証拠は、イグアラ、モンタンテ レベラド、または元帳信用残高の透明性を証明し、シールドされたマルカンドまたは無効化コモ ガストを記録します。
   - `ConfidentialEvent::Unshielded` com o montante publico、nullifiers consumidos、identificadores deproof、hash de Chamada da transacao を発行します。## Adicoes ao データモデル
- `ConfidentialConfig` (nova secao de config) com flag de habilitacao、`assume_valid`、ノブのガス/制限、ジャネラのアンカー、バックエンドの検証器。
- `ConfidentialNote`、`ConfidentialTransfer`、`ConfidentialMint` スキーマ Norito 明示的なバイト (`CONFIDENTIAL_ASSET_V1 = 0x01`)。
- `ConfidentialEncryptedPayload` は、バイトのメモ AEAD com `{ version, ephemeral_pubkey, nonce, ciphertext }`、com デフォルト `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` パラオ レイアウト XChaCha20-Poly1305 をエンボルブします。
- `docs/source/confidential_key_vectors.json` のキー導出の正規ベクトル。 Tanto CLI quanto エンドポイント Torii リグレッサムはフィクスチャに反します。
- `asset::AssetDefinition` ガンハ `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`。
- `ZkAssetState` は、転送/シールド解除のバインディング `(backend, name, commitment)` パラメータ検証者を保持します。キー参照を検証する証明を実行し、インラインで対応し、コミットメント登録を実行します。
- `CommitmentTree` (資産comフロンティアチェックポイント)、`NullifierSet` comchave `(chain_id, asset_id, nullifier)`、`ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` 世界国家のアルマゼナド。
- 一時的な管理は、`NullifierIndex` と `AnchorIndex` で重複の事前確認とアンカーのチェックを行います。
- スキーマ Norito の更新には、パブリック入力における canonico の順序付けが含まれます。エンコーディングの往復保証の決定性をテストします。
- 単体テストによる暗号化ペイロード ficam 修正のラウンドトリップ (`crates/iroha_data_model/src/confidential.rs`)。ウォレットの付属文書のベクターは、AEAD canonicos para Auditores に保存されています。 `norito.md` ドキュメント ヘッダー オンワイヤ パラオ エンベロープ。

## Integracao IVM システムコール
- システムコール `VERIFY_CONFIDENTIAL_PROOF` の紹介:
  - `circuit_id`、`version`、`scheme`、`public_inputs`、`proof`、または `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` の結果。
  - システムコール カレガ メタデータ、ベリファイア、レジストリ、タマンホ/テンポのアプリケーション制限、コブラ ガス決定機能、アプリケーションまたはデルタ シーの証明が成功します。
- ホスト公開または特性読み取り専用 `ConfidentialLedger` マークル ルートおよびヌルファイアの復元スナップショットのパラメータ。参考資料 Kotodama は、アセンブリの補助者とスキーマの検証を支援します。
- エスクラレーサ レイアウトに関するポインタ - ABI 形式のドキュメントは、バッファの証明とレジストリの削除を処理します。

## ノードの容量をネゴシアカオ
- 握手宣言 `feature_bits.confidential` ジュント コム `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`。検証要求への参加 `confidential.enabled=true`、`assume_valid=false`、バックエンドの識別子は、検証者識別子と対応するダイジェストを実行します。ファルハムとハンドシェイク com `HandshakeConfidentialMismatch` が一致しません。
- 監視者向けの設定サポート `assume_valid` : 監視解除、監視指示、監視 `UnsupportedInstruction` 決定的安全性パニック。 Quando habilitado、オブザーバーはデルタの宣言を検証し、検証を行います。
- 地域住民の安全を守るために、信頼できる情報を提供してください。ゴシップの証拠を監視するフィルタリングは、保護されたパラピアの非互換性を監視し、制限を制限する検証者 ID を確認します。

### マトリス・デ・ハンドシェイク

|アヌンシオ・リモト | Resultado パラ ノードの検証 |オペレーターに関する注意事項 |
|---------------------|------------------------------|--------------|
| `enabled=true`、`assume_valid=false`、バックエンド一致、ダイジェスト一致 |アシート | O ピア チェガ アオ スタド `Ready` が提案に参加し、RBC ファンアウトに投票します。ネンフマアカオマニュアルリケリダ。 |
| `enabled=true`、`assume_valid=false`、バックエンド一致、ダイジェストが古い |レジェイタド (`HandshakeConfidentialMismatch`) | O は、`activation_height` プログラムを保護するレジストリ/パラメータの保留中のアプリケーションを開発します。コリギルを食べました、オーノードセグエデスコブリベルマスヌンカエントラナロータカオデコンセンサス。 |
| `enabled=true`、`assume_valid=true` |レジェイタド (`HandshakeConfidentialMismatch`) |バリダドーレスは証明を要求します。リモート コモ オブザーバー com Torii のみの入力を設定し、`assume_valid=false` アポス ハビリタール検証を完了します。 |
| `enabled=false`、カンポスオミティドス (ビルド解除)、異なる検証者のバックエンド |レジェイタド (`HandshakeConfidentialMismatch`) |ピアは、同意を得て、実際にデータを取得し、登録します。タプル バックエンド + ダイジェストは、再接続する前に、実際にリリースする必要があります。 |

オブザーバーは、機能ゲートのコンセンサスとコンセンサスを確認し、証明を検証します。 Torii を介して API を取得し、コンセンサスを確認して、互換性のある情報を確認してください。

### 政治による剪定と暴露と無効化の保持台帳の機密情報は、ガバナンスの監査を再現するための記録として十分な歴史的情報を提供します。 politica のデフォルト、aplicada por `ConfidentialLedger`、e:

- **無効化リストの保持:** 無効化リストは、`730` ディアス (24 メセス) の *minimo* でガスト ポートを保持し、主要な規制を義務付けます。 Operadores podem estender a janela via `confidential.retention.nullifier_days`。無効化者は、Torii 経由で DEVEM 永続的な相談を行い、二重支出を証明するために監査を行います。
- **プルーニング デ リビール:** は、透明性 (`RevealConfidential`) を明らかにします。 Eventos `ConfidentialEvent::Unshielded` レジストラが公開され、受信者のハッシュ デ プルーフ パラケ再構築により、歴史的な履歴や暗号文が明らかになります。
- **フロンティア チェックポイント:** フロンティア ド コミットメント マンテム チェックポイントは、主要なエントリ `max_anchor_age_blocks` をローリング コブリンドで管理しています。ノード コンパクタム チェックポイント、アンチゴス アペナス デポワ、トドス OS 無効化、インターバル有効期限なし。
- **ダイジェストが古くなった場合の修正:** ダイジェストのドリフト、オペラドールのデベム (1) クラスターなしのジャネラス・デ・リテンカオ・エスタオ・アリンハダとして検証する `HandshakeConfidentialMismatch` の検証、(2) ロダール `iroha_cli app confidential verify-ledger` パラ再生、ダイジェスト・コントラ・コンジュント・デ・ヌリファイアのレチドス、e (3) 再配備またはマニフェストを行います。無効化者は、再導入する前にコールド ストレージを行う前に開発を開始します。

Documente は、オペラの運用手順書を無効にします。管理政治は、管理システムの管理、ノードおよびストレージ管理の計画の設定を管理します。

### Fluxo デエビクションとリカバリ

1. Durante o ダイヤル、`IrohaNetwork` は、無音の容量と比較します。 Qualquer の不一致レヴァンタ `HandshakeConfidentialMismatch`; `Ready` のディスカバリー セム サー プロモビデオでは、ピアの永続的なファイルの検索が行われます。
2. サービスに関するログはありません (ダイジェスト リモートとバックエンドを含む)。Sumeragi の議題は、ピア パラ プロポストと投票に含まれます。
3. オペレータは、レジストリの検証とパラメータの接続 (`vk_set_hash`、`pedersen_params_id`、`poseidon_params_id`) とプログラム、`next_conf_features` および `activation_height` アコルダードの修復を行います。ダイジェストが一致するか、近接ハンドシェイクが自動的に行われるかがわかります。
4. 古いコンセグのディファンディル ウム ブロコ (例として、リプレイ デ アルキーボ経由) を確認し、`BlockRejectionReason::ConfidentialFeatureDigestMismatch` で確認された決定性を確認し、元帳の一貫性を維持する必要があります。

### Fluxo デ ハンドシェイク セグロ コントラ リプレイ

1. Cada tentativa アウトバウンド アロカ マテリアル デ シャーブ ノイズ/X25519 novo。ハンドシェイク アッシナド (`handshake_signature_payload`) のペイロードは、パブリックなローカル電子リモートとして接続され、ソケットのコードは Norito で制御され、`handshake_chain_id` でコンパイルされ、チェーンの識別が行われます。 AEAD ノードを暗号化するためのメッセージが表示されます。
2. レスポンダーはペイロードを再計算し、ピア/ローカルで検証を行い、Ed25519 embutida em `HandshakeHelloV1` を実行します。チャベス・エフェメラスとしてのアンバスと、支配者としてのファゼム・パートを、リプレイ・デ・ウマ・メンセージ・キャプチャー・コントラ・アウトロ・ピア・ウ・レキュペラー・ウマ・コンエクサオ・スタレ・ファルハ決定論。
3. `HandshakeConfidentialMeta` 経由で容量秘密フラグを設定します。 O 受容体比較タプル `{ enabled, assume_valid, verifier_backend, digest }` com seu `ConfidentialHandshakeCaps` local; `Ready` の交通機関へのアクセスが不一致です。`HandshakeConfidentialMismatch`。
4. DEVEM 再計算またはダイジェスト (`compute_confidential_feature_digest` 経由) と、再接続する前に登録されている代表的なノードの操作。仲間の発表は、アンティゴスが継続的にファルハンドと握手を交わし、設定された有効性のない古いものを再確認することを消化します。
5. ハンドシェイクのファルハスとパドラオ `iroha_p2p::peer` (`handshake_failure_count`、エラー分類法のヘルパー) の送信ログ、ピア ID リモート、フィンガープリント ダイジェストの実行。ロールアウト中に設定ミスを再現する検出器の表示を監視します。## 鍵管理とペイロード
- アカウントの派生階層:
  - `sk_spend` -> `nk` (無効化キー)、`ivk` (受信表示キー)、`ovk` (送信表示キー)、`fvk`。
- ECDH から派生した AEAD com 共有キーを暗号化するペイロード。監査人の意見を反映した監査結果のキーを表示し、出力が政治資産に準拠していることを確認します。
- CLI の関連性: `confidential create-keys`、`confidential send`、`confidential export-view-key`、記述メモの監査ツール、製品/検査エンベロープ Norito のヘルパー `iroha app zk envelope` オフライン。 Torii は、`POST /v2/confidential/derive-keyset` 経由での派生メッセージの詳細を説明し、16 進数の Base64 パラケ ウォレット プログラムの階層を表示します。

## ガス、制限、DoS 制御
- ガスのスケジュール決定:
  - Halo2 (Plonkish): ベース `250_000` ガス + `2_000` ガス、パブリック入力。
  - `5` ガスポープルーフバイト、マイスカーゴポーヌリファイア (`300`) ポーコミットメント (`500`)。
  - configuracao do ノードを介した Operadores podem sobrescrever essas 定数 (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`);ムダンカスの宣伝は起動せず、ホットリロードも設定も行われず、アプリケーションの決定性もクラスターがありません。
- 期間の制限 (デフォルト設定):
- `max_proof_size_bytes = 262_144`。
- `max_nullifiers_per_tx = 8`、`max_commitments_per_tx = 8`、`max_confidential_ops_per_block = 256`。
- `verify_timeout_ms = 750`、`max_anchor_age_blocks = 10_000`。 `verify_timeout_ms` が決定的命令を中止したことを証明します (統治投票用紙が `proof verification exceeded timeout`、`VerifyProof` エラーを返す)。
- 活性を保証する割り当て割り当て: `max_proof_bytes_block`、`max_verify_calls_per_tx`、`max_verify_calls_per_block`、`max_public_inputs` 制限ブロック ビルダー。 `reorg_depth_bound` (>= `max_anchor_age_blocks`) は、国境のチェックポイントを管理します。
- 実行ランタイムは、トランザクションの制限を超えてトランザクションを実行し、エラー `InvalidParameter` を決定し、元帳の管理を実行します。
- `vk_id` の事前フィルター処理の秘密保持、再帰的使用制限の管理およびアンカーの呼び出しと検証の実行。
- タイムアウトまたは制限を決定するための検証。ファルハムとの取引は明示的に行われます。バックエンド SIMD は、会計処理を実行するためのオプションを提供します。

### 校正のベースラインと安全性のゲート
- **参照プラットフォーム** 校正済みの DEVEM の情報がすべて公開されています。レビューを読んでください。

  |パーフィル |アーキテトゥーラ | CPU / インスタンス |コンピラドールのフラグ |プロポジト |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) または Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabelece valores の組み込み関数 vetoriais。 usado para ajustar tabelas de custo フォールバック。 |
  | `baseline-avx2` | `x86_64` |インテル Xeon ゴールド 6430 (24c) |リリースデフォルト |パス AVX2 を検証します。 SIMD フィカム デントロ ダ トレランシア ガス ニュートラルを確認してください。 |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) |リリースデフォルト |バックエンド NEON の永続性を決定し、アリンハド AOS スケジュール x86 を保証します。 |

- **ベンチマーク ハーネス** Todos os relatorios de calibracao degas DEVEM ser produzidos com:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` パラ確認またはフィクスチャ決定者。
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` VM の操作コードを管理する必要があります。

- **ランダム性の修正。** `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` をエクスポートして、`iroha_test_samples::gen_account_in` を決定する `KeyPair::from_seed` を選択します。おお、ハーネス・インプライム `IROHA_CONF_GAS_SEED_ACTIVE=...` uma vez;ヴァリアベルファルタール、レビューDEVEファルハールをご覧ください。 Qualquer utilidade nova de calibracao は、継続的な honrando esta env var ao introduzir Randomness auxiliar を開発します。

- **結果のキャプチャ**
  - 基準 (`target/criterion/**/raw.csv`) に関する概要をアップロードし、リリースに関する情報を提供しません。
  - アルマゼナール メトリクス デリバダ (`ns/op`、`gas/op`、`ns/gas`) なし [機密ガス校正台帳](./confidential-gas-calibration) は、米国コンパイラ プログラムをコミットします。
  - パーフィルによる究極のベースラインを管理します。アパガーのスナップショットは、アンチゴス ウマの有効性や新たな関係性を維持します。

- **寛容性の寛容**
  - ガス入力デルタ `baseline-simd-neutral` および `baseline-avx2` DEVEM 永続性 <= +/-1.5%。
  - ガス入力デルタ `baseline-simd-neutral` および `baseline-neon` DEVEM 永続性 <= +/-2.0%。
  - しきい値を超過した場合は、RFC が矛盾を緩和するためにスケジュールを調整するよう要求します。- **レビューのチェックリスト** 提出者の応答は次のとおりです。
  - `uname -a`、`/proc/cpuinfo` のトレコス (モデル、ステッピング)、`rustc -Vv` の校正ログが含まれません。
  - Verificar que `IROHA_CONF_GAS_SEED` aparece na Saida do bench (ベンチがシード アティバを暗示するように)。
  - ギャランティール機能フラグは、ペースメーカーと検証者の機密エスペルヘムの生産を行います (`--features confidential,telemetry` および Rodar Benes com Telemetry)。

## オペラの設定
- `iroha_config` `[confidential]` に注意してください:
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
- テレメトリアはメトリクスの集合体を発行します: `confidential_proof_verified`、`confidential_verifier_latency_ms`、`confidential_proof_bytes_total`、`confidential_nullifier_spent`、`confidential_commitments_appended`、`confidential_mempool_rejected_total{reason}`、e `confidential_policy_transitions_total`、 sem expor ダドス em クラロ。
- 地上権 RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## 精巣戦略
- 決定性: ブロック単位でのトランザクションのランダム化と無効化識別子のセットのマークル ルート。
- Resiliencia a reorg: 複数ブロックの com アンカーをシミュレートする reorg。無効化する永続的なアンカーは古いものです。
- ガスの不変量: SIMD で使用されるガス識別子を検証します。
- 境界テスト: テト デ タマンホ/ガスの証明、最大イン/アウト カウント、タイムアウトの強制。
- ライフサイクル: 政府機関のガバナンス/廃止、検証およびパラメータの運用、定期的なテストの実施。
- FSM ポリシー: トランジション許可/ネガダ、トランジション保留および遅延による遅延。
- 緊急登録: `withdraw_height` の緊急引き出しコンジェラ資産が保管されている証拠。
- 機能ゲーティング: validadores com `conf_features` divergentes rejeitam blocos;オブザーバーズcom `assume_valid=true` acompanham sem afetar consenso。
- 確立された同等性: ノードのバリデーター/フル/オブザーバーの製品ルートは、標準的な確立されたシステムと同一です。
- 否定的なファジング: 不正な形式、ペイロードの超次元と無効化の解決策の決定性を証明します。

## ミグラカオ
- ロールアウト com 機能フラグ: フェーズ C3 端子、`enabled` デフォルトは `false`。ノードは、設定された有効性を設定していないエントリの容量を通知します。
- 資産はすべて透明です。レジストリの登録と容量の変更に関する秘密情報。
- ノードは、決定的な関連性を示す機密情報をサポートします。 nao podem entrar no set validador mas podem operar como Observers com `assume_valid=true`。
- ジェネシスのマニフェストには、レジストリの登録、パラメータ設定、資産に関する政治秘密、監査人の意見の鍵が含まれます。
- 登録簿の公開、政治取引、および緊急撤退の決定事項のアップグレードに関する運用手順書を管理します。

## トラバーリョ・ペンデンテ
- Halo2 パラメータのベンチマーク (サーキットのタマンホ、ルックアップの戦略) レジストラの結果、ガス/タイムアウトのデフォルト設定でのキャリブレーションのプレイブックはなく、`confidential_assets_calibration.md` の近接リフレッシュが行われません。
- 監査人向けの選択的​​閲覧関連の政治情報開示と API の最終化、Torii によるガバナンス草案の承認ワークフローへの接続。
- 証人の暗号化をコブリルで実行すると、複数の受信者の電子メモをバッチで出力し、SDK を実装してエンベロープを作成したり文書化したりフォーマットしたりできます。
- 外部回路、レジストリ、およびパラメトロスと監査の手続きおよび内部関係の見直しを担当する委員。
- 監査時の消費状況を調整するための特定の API と、セキュリティ セマンティクスとしてウォレットを実装するベンダーからのビュー キーの管理に関する公開ガイド。## 実装のフェーズ
1. **フェーズ M0 - 出荷時の硬化**
   - [x] ポセイドン PRF (`nk`、`rho`、`asset_id`、`chain_id`) の設計による無効化セグエの設計は、台帳のコミットメントを決定するための注文を決定します。
   - [x] トランザクション/ポートブロコでの秘密の割り当て制限を実行し、予算委員会の決定のためにトランザクションを再管理します。
   - [x] ハンドシェイク P2P 通知 `ConfidentialFeatureDigest` (バックエンド ダイジェスト + レジストリのフィンガープリント) `HandshakeConfidentialMismatch` によるファルハの不一致が決定的。
   - [x] リムーバーは、ノード間の非互換性のある機密ロール ゲートを実行するパスをパニックに陥らせます。
   - [ ] タイムアウトの予算と検証者、辺境チェックポイントの再編成の制限を提供します。
     - [x] 検証アプリケーションのタイムアウトの予算。証明は `verify_timeout_ms` アゴラ ファルハムの決定性を超えています。
     - [x] `reorg_depth_bound` の前にフロンティア チェックポイントがあり、ポダンド チェックポイントは、古いスナップショットを決定するために必要です。
   - `AssetConfidentialPolicy` の紹介、ポリシー FSM は、ミント/転送/開示に関する執行をゲートします。
   - `conf_features` のヘッダー デ ブロックと検証参加者のレジストリ/パラメータ分岐のダイジェストをコミットします。
2. **フェーズ M1 - レジストリとパラメータ**
   - Entregar レジストリ `ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` のガバナンス構成、生成、キャッシュの作成。
   - Conectar システムコールによるレジストリ検索、ガス スケジュール ID、スキーマ ハッシュ、タマンホのチェック。
   - ペイロード暗号化 v1 の暗号化フォーマット、ウォレットの鍵の暗号化、秘密情報の取得などの CLI のサポート。
3. **フェーズ M2 - ガス性能**
   - ガス決定スケジュール、通信テレメトリーベンチマークハーネス (検証遅延、証明、記録) を実装します。
   - Endurecer CommitmentTree チェックポイント、カーガ LRU インデックス、ワークロード マルチアセットのヌルファイアー インデックス。
4. **フェーズ M3 - Rotacao とウォレットのツール**
   - マルチパラメータおよびマルチバーサオの証明を行うことができます。トランジションのガバナンス com ランブックを支援します。
   - SDK/CLI の移行、ワークフロー、スキャン、監査、ツールの支出調整。
5. **フェーズ M4 - 監査業務**
   - 監査人のワークフロー、選択的開示の API、運用手順書の作成。
   - `status.md` で公開されている、外部暗号化/セグランサの改訂に関する議題。

段階的なマイルストーンでは、ブロックチェーンの実行を決定するためのロードマップとテストが行​​われます。
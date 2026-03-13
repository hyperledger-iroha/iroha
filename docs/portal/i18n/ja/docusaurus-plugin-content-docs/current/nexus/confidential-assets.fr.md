---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 機密情報と転送 ZK の実行
説明: ブループリント フェーズ C は循環を監視し、登録と管理を管理します。
スラグ: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# 機密情報および転送の設計 ZK

## モチベーション
- 流通の透明性を維持するために、プライバシー トランザクションを保護し、変更を加えることなくドメインをオプトインすることで、流動的な活動を遮断します。
- Garder une の実行は、ハードウェアの異種遺伝子の検証と保守 Norito/Kotodama ABI v1 を決定します。
- 回路および暗号パラメータのサイクル管理 (アクティブ化、ローテーション、取り消し) を監査し、管理します。

## 脅威のモデル
- 検証者は正直だが好奇心旺盛ではありません: 検査官の台帳/状態に基づいてコンセンサスを忠実に実行します。
- ブロックや取引のうわさ話を監視する観察者。ゴシップ・プリヴェスの仮説を立てます。
- 対象範囲: 台帳外のトラフィック、量的問題 (ロードマップ PQ の管理)、台帳の管理攻撃を分析します。

## デザインのアンサンブルを確認
- *シールドされたプール*と存在する透明性のバランスを保つための活動宣言者。暗号化のコミットメントを介して、発行者は盲人であり、代表者となります。
- カプセル化されたノート `(asset_id, amount, recipient_view_key, blinding, rho)` avec:
  - コミットメント: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`。
  - 無効化: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`、独立したノート。
  - ペイロード シフレ: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`。
- ペイロードのトランザクション転送 `ConfidentialTransfer` は、Norito コンテンツをエンコードします。
  - 入力パブリック: マークル アンカー、ヌルファイアー、ヌーヴォー コミットメント、アセット ID、回路のバージョン。
  - ペイロードは受信者と監査者のオプションを提供します。
  - Preuve のゼロ知識証明者の保存、所有権、および権限。
- 台帳の有効化を介してキーとパラメータのアンサンブルを検証し、台帳の登録を介して制御します。検証を拒否したり、検証を拒否したり、エントリの参照を取り消したりすることは継続されます。
- ヘッダーとコンセンサスが関連付けられ、機能ダイジェストと機密情報が関連付けられ、ブロック ソエントはレジストリ/パラメータに対応するセキュリティ情報を受け入れます。
- 構築の証明では、信頼できるセットアップなしで Halo2 (Plonkish) の山を利用します。 Groth16 の作者のバリアント SNARK の意図と非サポート対象者用の v1。

### 試合が決定する

機密文書の封筒は `fixtures/confidential/encrypted_payload_v1.json` に準拠しています。データセットは、エンベロープ v1 のポジティブなデータと、SDK の不正な形式をキャプチャし、解析を積極的に肯定します。データモデル Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) と Swift スイート (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) のテスト、フィクスチャーの保証、エンコーディング Norito、表面のエラーと回帰の停止、およびコーデックの進化のテスト。

SDK の Swift のメンテナンス ガイドの説明シールドなしの接着剤 JSON ビスポーク: construire un
`ShieldRequest` 平均コミットメント 32 バイト、ペイロード シフレおよび借方メタデータ、
puis appeler `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) 署名者と中継者を注いでください
`/v2/pipeline/transactions` 経由のトランザクション。 Le helper valide les longueurs de commitment、
`ConfidentialEncryptedPayload` およびエンコード Norito を挿入し、レイアウト `zk::Shield` を参照してください
ci-dessous afin que leswalls は avec Rust と同期します。## コンセンサスコミットメントと機能ゲート
- `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` を公開するブロックのヘッダー。コンセンサスのハッシュに参加するダイジェストは、レジストリのロケールを確認し、アクセプタをブロックに注ぎます。
- 将来の `activation_height` から `next_conf_features` までのアップグレードとプログラムの管理を監視します。 jusqu a cette hauteur、les producteurs de blocs doivent の継続的なダイジェスト先例。
- Les noeuds validateurs DOIVENT fonctionner avec `confidential.enabled = true` et `assume_valid = false`。 `conf_features` ローカル分岐。
- `{ enabled, assume_valid, conf_features }` を含むハンドシェイク P2P のメタデータ。コンセンサスのローテーションを考慮して、価格が設定されていない機能を報告します。
- 検証者、オブザーバー、およびピアのハンドシェイクの結果は、ハンドシェイクのマトリクス [ノード能力ネゴシエーション](#node-capability-negotiation) をキャプチャします。ハンドシェイクの暴露 `HandshakeConfidentialMismatch` とコンセンサスのローテーションでのピア ホールの検査とダイジェストの対応を確認します。
- 監視者が定義を検証できない場合 `assume_valid = true`;デルタの機密情報は、コンセンサスの安全性を維持するために重要な影響力を持ちます。

## 政治と活動
- ガバナンスによる `AssetConfidentialPolicy` 定義の輸送に関する定義:
  - `TransparentOnly`: デフォルトのモード。透明な命令 (`MintAsset`、`TransferAsset` など) は、拒否されたユーザーの操作を許可します。
  - `ShieldedOnly`: 指示の機密情報の送信と転送の利用者を表示します。 `RevealConfidential` est interdit pour que les Balances ne soient jamais は公開を公開します。
  - `Convertible`: 保持者は、オン/オフランプ ci-desous の指示を介して、透明な表現とシールドを保持しています。
- FSM の制約を無視した政治的要求:
  - `TransparentOnly -> Convertible` (シールドされたプールによる即時アクティブ化)。
  - `TransparentOnly -> ShieldedOnly` (変換ペンダントとフェネトルの遷移が必要)。
  - `Convertible -> ShieldedOnly` (デライ最小インポーズ)。
  - `ShieldedOnly -> Convertible` (移行計画には、シールドされた残りのメモが必要です)。
  - `ShieldedOnly -> TransparentOnly` シールド プールのテスト中は、ガバナンス エンコード中の移行を確認し、シールド ファイルの残りを確認してください。
- ISI `ScheduleConfidentialPolicyTransition` を介したガバナンス修正の指示 `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` および変更プログラムの平均 `CancelConfidentialPolicyTransition` の説明。ブロック間の政治的変化をチェックするための、移行などの包括的エコー決定性の検証は、トランザクションの安全性を保証します。
- ヌーボーブロックでのアップリケの自動移行: 変換中の自動移行 (アップグレード `ShieldedOnly`) を監視 `effective_height`、実行時間は `AssetConfidentialPolicy` に達しました、メタデータ `zk.policy` とメインのペンダントが削除されました。透明な供給は非 NULL Quand une 遷移 `ShieldedOnly` に到着し、ランタイムの無効な変更とログの回避は維持され、レザン ル モードの先行はそのまま残ります。
- 設定 `policy_transition_delay_blocks` と `policy_transition_window_blocks` は、自動切り替え用のウォレットを許可するための最小限の制限と期間を課します。
- `pending_transition.transition_id` 監査を処理します。ガバナンスは、最終決定と報告書を作成し、運用担当者がオン/オフランプの関係を集中的に管理します。
- `policy_transition_window_blocks` のデフォルトは 720 (平均ブロック時間約 12 時間、60 秒)。事前審査と法廷でのガバナンスの要求を取り締まります。
- ジェネシスのマニフェストと流動的な CLI は、政治とペンダントを公開します。入学許可は、実行の瞬間の政治政策を決定し、指示の秘密を守ります。
- 移行チェックリスト - 「移行シーケンス」を作成し、マイルストーン M0 のアップグレード プランを作成します。

#### Torii による移行の監視財布と監査人の尋問 `GET /v2/confidential/assets/{definition_id}/transitions` 検査官 l `AssetConfidentialPolicy` 行為。ペイロード JSON には、トゥージュールが含まれ、資産 ID canonique、ブロック監視の詳細、政治政策の詳細 `current_mode`、設定の効果に関するモード効果 (変換の適切なテンポラリメント `Convertible`)、および参加する識別情報が含まれます。 `vk_set_hash`/ポセイドン/ペダーセン。政府への移行はオーストラリアへの対応に注意してください:

- `transition_id` - `ScheduleConfidentialPolicyTransition` の監査レンボイを処理します。
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` および `window_open_height` の派生 (ウォレットブロックの開始、変換、カットオーバー、ShieldedOnly)。

応答例:

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

応答がありません `404` 対応する定義が存在します。 Lorsqu aucune 移行の次のプランニフィエ ル チャンピオン `pending_transition` est `null`。

### 政治の機械論

|モードアクチュエル |モード適性 |前提条件 |効果的なジェスション・ド・ラ・オートール |メモ |
|------------------|------------------|---------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
|透明のみ |コンバーチブル |検証/パラメータの有効な登録をガバナンスします。スメトル `ScheduleConfidentialPolicyTransition` アベック `effective_height >= current_height + policy_transition_delay_blocks`。 |遷移は `effective_height` を実行します。シールドされたプールは、責任を負った即時行為を行います。        | Chemin は、透明性のある機密情報をデフォルトで提供します。 |
|透明のみ |シールド付きのみ | Idem ci-dessus、プラス `policy_transition_window_blocks >= 1`。                                                         | `Convertible` と `effective_height - policy_transition_window_blocks` の自動実行時のランタイム。 `ShieldedOnly` を `effective_height` に渡します。 |フェネートの変換は、事前に非アクティブ化された指示を透過的に決定します。 |
|コンバーチブル |シールド付きのみ |移行プログラムは avec `effective_height >= current_height + policy_transition_delay_blocks` です。メタデータおよび監査によるガバナンス DEVRAIT 認証者 (`transparent_supply == 0`)。ル・ランタイム・アップリケ・オー・カットオーバー。 |フェネートルの同一性の意味。透明な電源供給は `effective_height` ではなく、平均的な遷移は `PolicyTransitionPrerequisiteFailed` です。 | Verrouille l actif encirculation entierement confidentielle。                             |
|シールド付きのみ |コンバーチブル |移行プログラム; aucun retrait durence actif (`withdraw_height` 非定義)。                                  |レターバスキュール `effective_height`;ランプは、シールドされた残りの有効性を明らかにします。       |メンテナンスやレビュー、監査に注力します。                               |
|シールド付きのみ |透明のみ |ガバナンスは、証明者 `shielded_supply == 0` または計画作成者 `EmergencyUnshield` に署名します (署名と監査人が要求します)。 |ランタイムはフェネトレ `Convertible` アバント `effective_height`;オートモード、透明のみの指示、秘密の指示、および有効期限の確認。 |デルニエ出撃。ラ トランジション s 自動環状 si une ノート コンフィデンティエル est depensee ペンダント ラ フェネトル。 |
|任意 |現在と同じ | `CancelConfidentialPolicyTransition` nettoie ファイル変更に注意してください。                                              | `pending_transition` 即時リタイア予定。                                                                       |現状維持。インディケ注ぐ完了。                                         |

非リスト者への移行は、ガバナンスを拒否するものではありません。ランタイムの前提条件は、事前に適用される移行プログラムを検証します。一時的なチェックでは、テレメトリおよびブロック単位のイベントを介して、au モードの先行動作と `PolicyTransitionPrerequisiteFailed` の対応が行われます。

### 移行のシーケンス1. **登録者:** 政治文書の検証とパラメータの参照をアクティブに行います。 Les noeuds annoncent le `conf_features` の結果は、ピアの検証結果を一貫性のあるものに注ぎます。
2. **Planifier の移行:** `policy_transition_delay_blocks` に対する `ScheduleConfidentialPolicyTransition` の平均値。 `ShieldedOnly` よりも正確な変換 (`window >= policy_transition_window_blocks`)。
3. **発行者によるガイダンスの運営者:** 登録者ファイル `transition_id` のリターンとディフューザーのオン/オフランプのランブック。財布や監査人は、`/v2/confidential/assets/{id}/transitions` を精査し、最高の品質を保証します。
4. **フェネットの申請:** ルーバーチャー、`Convertible` のランタイム バスキュール、`PolicyTransitionWindowOpened { transition_id }` のメット、および紛争に対するガバナンス要求の拒否を開始します。
5. **ファイナライザまたは実行者:** `effective_height`、ランタイム検証の前提条件 (透明なゼロの供給、緊急要求など)。成功すれば、時代の要求に応じた政治が行われます。 en echec、`PolicyTransitionPrerequisiteFailed` est emis、la 移行ペンダント est nettoyee および la politiquereste inchangee。
6. **スキーマのアップグレード:** 移行後のガバナンス、スキーマのバージョン (例 `asset_definition.v2`) およびツール CLI の更新 `confidential_policy` マニフェストのシリアル化が行われます。アップグレードの生成を管理するドキュメントと、政治的設定および検証のためのレジストリ管理を管理します。

新たな研究は、機密事項として活動し、政治的欲望の方向性と生成を管理します。チェックリストを作成し、起動後の変更を確認し、変換の残りを決定し、財布の一時的な変更を決定します。

### マニフェストのバージョンとアクティベーション Norito

- ジェネシスのマニフェストには、カスタム `confidential_registry_root` を含む `SetParameter` が含まれています。 Le payload est un Norito JSON 対応 `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omettre le champ (`null`) quand aucune entree nest active, sinon fournir une chaine hex 32-byte (`0x...`) egale au hash produit par `compute_vk_set_hash` マニフェストの検証手順。レジストリのエンコード対象となるセキュリティ パラメータのハッシュを要求する必要はありません。
- オンワイヤー `ConfidentialFeatureDigest::conf_rules_version` マニフェストのレイアウトのバージョンを確認します。 Pour les reseaux v1 il DOIT rester `Some(1)` et egaler `iroha_config::parameters::defaults::confidential::RULES_VERSION`。ルールセットの展開、定数のインクリメンタ、マニフェストの再生成とバイナリのデプロイメントをロックステップで実行します。メランジェのバージョンは既成事実を拒否し、ブロックは平均的な検証を行います `ConfidentialFeatureDigestMismatch`。
- 活性化の兆候として、DEVRAIENT の再グループ化者はレジストリの変更、サイクルの変更、およびダイジェストの修正の一貫性のある政治的移行が示されています。
  1. レジストリ計画のアップリケの突然変異 (`Publish*`、`Set*Lifecycle`) は、オフラインで表示され、アクティブ化後のダイジェストの平均値を計算できます `compute_confidential_feature_digest`。
  2. Emmettre `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` は、ピアのハッシュ計算を実行し、回復手段の回復やダイジェストを正確に実行し、指示の仲介者をサポートします。
  3. 説明書 `ScheduleConfidentialPolicyTransition` をご覧ください。 Chaque 命令 doit citer le `transition_id` emis par ガバナンス。これは、ランタイムに応じて非常に優れたパフォーマンスを発揮します。
  4. マニフェストのバイトを保存し、SHA-256 などのダイジェストを使用して計画をアクティブ化します。操作者は、投票者の前にトロワの成果物を検証し、パーティションを削除するマニフェストを作成します。
- ロールアウトはカットオーバーと異なる必要があり、カスタム コンパニオンのパラメータを登録する必要があります (`custom.confidential_upgrade_activation_height` など)。 Cela fournit aux Auditeurs une preuve Norito encod ee que les validateurs ont respecte la fenetre de preavis avant que le changement de divergement prenne effet.## 検証とパラメータのサイクル
### ZK を登録する
- 帳簿の在庫 `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` または `proving_system` の最新情報は `Halo2` を修正します。
- グローバル固有のペア `(circuit_id, version)` を指定します。レジストリは、回路のメタデータに関する二次的なインデックスのメンテナンスを行います。登録者を二重登録する仮登録者は、登録を拒否する人を拒否します。
- `circuit_id` doit etre non video et `public_inputs_schema_hash` doit etre fourni (典型的なハッシュ Blake2b-32 エンコーディングの標準的な公開入力デュ検証)。チャンピオンの入学を拒否します。
- ガバナンスに関する指示:
  - `PUBLISH` メインディッシュを注ぐ `Proposed` 平均的なメタデータ セクション。
  - `ACTIVATE { vk_id, activation_height }` プログラマ l のアクティベーションが無制限に行われます。
  - `DEPRECATE { vk_id, deprecation_height }` メインディファレンスを参照するために、最高のフィナーレと証明を注ぎます。
  - `WITHDRAW { vk_id, withdraw_height }` シャットダウンの緊急性が発生します。 les actifs touch geleron les depensesconfidentiels apres 引き出しの高さ jusqu a activity de nouvelles entres。
- Les Genesis は、カスタム パラメーター `confidential_registry_root` は、補助メインのアクティブに対応しない `vk_set_hash` を示します。ローカルのレジストリを確認し、コンセンサスを再結合する必要があります。
- 登録者は、`gas_schedule_id` で検証を要求されています。 LA 検証は、クエリのエントリ soit `Active`、インデックス `(circuit_id, version)` の提示、などの証明を Halo2 で 4 つ実行されます `OpenVerifyEnvelope` は `circuit_id`、`vk_hash` などを実行します。 `public_inputs_schema_hash` レジストリの対応者エントリー。

### 鍵の証明
- 台帳外に保持されているキーの証明は、コンテンツの識別情報アドレスを参照し、検証用のメタデータを公開します。
- SDK ウォレット回復ファイル PK、検証ファイル ハッシュ、およびキャッシュ ロケールの管理。

### パラメトレス・ペデルセンとポセイドン
- レジストリは (`PedersenParams`、`PoseidonParams`) ミラーレント サイクルの検証制御、平均 `params_id` の生成、定数のハッシュ、アクティブ化、非推奨および撤回を分離します。
- `params_id` のドメインでのコミットメントとハッシュのフォントの分離は、パラメータの回転とアンサンブルの非推奨のビット パターンの再利用を可能にします。 l ID は、ドメイン無効化者のコミットメントとタグに記録されます。
- 検証による複数パラメータの選択をサポートする回路。パラメータのアンサンブルは、`deprecation_height` の残りの支出を廃止し、アンサンブルは `withdraw_height` を拒否します。

## 順序決定と無効化
- Chaque actif maintient un `CommitmentTree` avec `next_leaf_index`;ブロックに関するコミットメントと順序決定: ブロック内のトランザクションの繰り返し。トランザクションのイテラー レス シールド パー `output_idx` を上位にシリアル化します。
- `note_position` は、無効化された当事者に対する既成のオフセットを導出します。イル・ニー・サート・ク・オ・シュマン・デ・メンバーシップ・ダン・ル・ウィット・ド・プルーヴ。
- 設計 PRF に対する保証を再編成するための無効化の安定化。入力 PRF 嘘 `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`、マークル ルーツ履歴のアンカー参照、`max_anchor_age_blocks` の制限。## 元帳フロー
1. **MintConfidential { 資産 ID、金額、受信者ヒント }**
   - `Convertible` または `ShieldedOnly` の政治的行為を要求します。 l 入院確認 l 自書、回復 `params_id`、echantillonne `rho`、コミットメントを達成、1 日の Arbre Merkle との出会い。
   - Emet `ConfidentialEvent::Shielded` avec le nouveau コミットメント、デルタ マークル ルート、およびトランザクションのハッシュ アプリケーションの監査証跡。
2. **TransferConfidential {asset_id、proof、circuit_id、version、nullifiers、new_commitments、enc_payloads、anchor_root、memo }**
   - システムコール VM は、レジストリのエントリを介して証拠を検証します。ファイル ホストは、無効化されたクエリが使用されないこと、コミットメントが決定性を維持しないこと、および最新のアンカーが存在しないことを保証します。
   - エントリ `NullifierSet` の台帳、受信者/監査者などのペイロードのストック `ConfidentialEvent::Transferred` 無効化子、出力オードンヌ、ハッシュ証明およびマークル ルートの情報。
3. **RevealConfidential {asset_id、proof、circuit_id、version、nullifier、amount、recipient_account、anchor_root }**
   - 活動 `Convertible` の固有の固有性。ラ・プルーフ・バリデ・ケ・ラ・ヴァルール・デ・ラ・ノート・エガーレ・ル・モンタン・レヴェール、ル・レジャー・クレジット・ル・バランス透明、そしてブルール・ラ・ノート・シールド・アン・マルカント・ル・ヌリファイア・コム・デペンス。
   - Emet `ConfidentialEvent::Unshielded` の公開、無効化コンソメ、トランザクションの証明およびハッシュ化のための識別子。

## Ajouts au データモデル
- `ConfidentialConfig` (構成の新規セクション) フラグの有効化、`assume_valid`、ガス/制限のノブ、アンカーのフェネトレ、バックエンドの検証。
- `ConfidentialNote`、`ConfidentialTransfer`、および `ConfidentialMint` スキーマ Norito バージョン明示的な平均バイト (`CONFIDENTIAL_ASSET_V1 = 0x01`)。
- `ConfidentialEncryptedPayload` エンベロープ デ メモ バイト AEAD avec `{ version, ephemeral_pubkey, nonce, ciphertext }`、デフォルト `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` プール レイアウト XChaCha20-Poly1305。
- `docs/source/confidential_key_vectors.json` での派生の規範のベクトル。ファイル CLI およびエンドポイント Torii のリグレッセント表面フィクスチャ。
- `asset::AssetDefinition` ガニ `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`。
- `ZkAssetState` 永続的なファイル バインディング `(backend, name, commitment)` ファイル検証者の転送/シールド解除。実行は拒否され、証明は行われません。キー参照を検証する必要はありません。インラインで対応し、コミットメントを登録します。
- `CommitmentTree` (アベックフロンティアチェックポイントとしての機能)、`NullifierSet`、`(chain_id, asset_id, nullifier)`、`ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` は世界の状態をストックします。
- 構造トランジションのメンプール保守 `NullifierIndex` および `AnchorIndex` は、検出の事前準備とアンカーのチェックを行います。
- パブリック入力の順序付けを解除するためのスキーマ Norito の更新。ラウンドトリップ保証の決定的なエンコーディングのテスト。
- 単体テストを介して、暗号化されたペイロードとベルイユを往復します (`crates/iroha_data_model/src/confidential.rs`)。ウォレットの添付ファイルの転写物は、AEAD の正規の監査人に注がれます。 `norito.md` ドキュメント ファイル ヘッダーがオンワイヤでエンベロープにあります。

## 統合 IVM と syscall
- システムコール `VERIFY_CONFIDENTIAL_PROOF` の紹介:
  - `circuit_id`、`version`、`scheme`、`public_inputs`、`proof`、および `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` の結果。
  - システムコールは、レジストリのメタデータ検証、アップリケの限界値/温度、ガスの決定、およびアップリケのデルタ ケ シラの証明を担当します。
- ホストは、スナップショットの回復者である特徴読み取り専用 `ConfidentialLedger` を公開し、マークル ルートと無効化されたステータスを公開します。ライブラリ Kotodama のヘルパー、アセンブリ、監視、スキーマの検証の 4 つ。
- ドキュメント ポインター - ABI は、バッファーとプルーフのレイアウトおよびレジストリを処理するための明確化ファイルを見逃すことはありません。

## キャパシテス ド ノウドの交渉
- ハンドシェイク アナンス `feature_bits.confidential` avec `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`。検証への参加が必要な `confidential.enabled=true`、`assume_valid=false`、バックエンド検証者の識別子およびダイジェスト通信者の識別子。不一致がエコーエントでハンドシェイクの平均 `HandshakeConfidentialMismatch` に達しました。
- 構成サポート `assume_valid` 監視者が決定する: 停止解除、指示の機密保持、製品 `UnsupportedInstruction` のパニックなしの決定。 Quand はアクティブで、観察者はアプリケント、デルタは検証者、証明なしで宣言します。
- ロケールの上限に基づいてトランザクションの機密情報を削除します。ゴシップの証拠をフィルタリングし、取引の監視者は保護された補助ピアを能力なしで監視し、詳細な制限を無視して検証者 ID を中継します。

### マトリス・デ・ハンドシェイク|遠く離れた場所を知らせる |検証結果を注ぐ | Notes オペレータ |
|---------------------|------------------------------|--------------|
| `enabled=true`、`assume_valid=false`、バックエンド一致、ダイジェスト一致 |同意する | `Ready` を参照して、提案、投票、ファンアウト RBC に参加してください。オーキュヌ・アクション・マヌエル・レクセーズ。 |
| `enabled=true`、`assume_valid=false`、バックエンド一致、ダイジェストが古い、または欠落しています |レジェテ (`HandshakeConfidentialMismatch`)リモートでアップリケのアクティベーション レジストリ/パラメータを監視し、`activation_height` を計画します。タント・ク・コルリッジ、ル・ヌード・レスト・デクーブラブル・メイン・アントレ・ジャメ・アン・ローテーション・デ・コンセンサス。 |
| `enabled=true`、`assume_valid=true` |レジェテ (`HandshakeConfidentialMismatch`)証拠を検証する必要のある検証。構成者ファイル リモート コム オブザーバー avec 入力 Torii のみ、バスキュラー `assume_valid=false` がアクティブ化前の検証を完了しました。 |
| `enabled=false`、チャンピオン オミス (ビルドが廃止されました)、バックエンド検証が異なります |レジェテ (`HandshakeConfidentialMismatch`)ピアは廃止され、部分的なアップグレードは再結合され、コンセンサスが必要になります。 1 時間ごとにリリースの確認と、タプル バックエンド + ダイジェストの事前の再接続を保証します。 |

監視者は、検証活動の検証を行うために、コンセンサスを確立するために自主的に検証を行います。 Torii 経由でブロックを取り込み、API のアーカイブを作成し、コンセンサスを確認して、通信相手の安全性を確認します。

### 無効化するものを明らかにし保持するための枝刈りの政治

台帳の機密情報は、管理者資産の記録と監査管理を保証するものです。デフォルトの政治方針、アップリケ `ConfidentialLedger`、推定値:

- **無効化者の保持期間:** 無効化者の保持は、`730` 時間 (24 分) の *最小* 時間 (24 分) で、規制当局の規則に従う必要があります。 `confidential.retention.nullifier_days` 経由で、フェネトルの操作を実行します。 Les nullifiers plus jeunes que la fenetre DOIVENT rester interrogeables via Torii afin que les Auditeurs prouvent l 不在による二重支出。
- **明らかにするプルーニング:** 透明なものを明らかにする (`RevealConfidential`) プルーニング レス コミットメント アソシエーションは、最終化後の即時ブロック、保持期間の規則に従って無効化コンソメ レストを実行します。イベント `ConfidentialEvent::Unshielded` は、公開者、受信者、および暗号文を削除する必要がある歴史を明らかにするためのハッシュ証明を作成します。
- **フロンティア チェックポイント:** フロンティア チェックポイントのコミットメント メンテナンス チェックポイント ルーラン クーブラン ル プラス グランド `max_anchor_age_blocks` および保持期間。コンパクトなチェックポイントと、無効化された期間の有効期限が切れる前の古いセキュリティ。
- **修復ダイジェストが古い:** si `HandshakeConfidentialMismatch` はダイジェストのドリフトによる原因を生存しており、オペレーターは (1) 検証者がクラスターとアライメントを維持してヌリファイアーを保持し、(2) ランサー `iroha_cli app confidential verify-ledger` がリジェネレーター・ルのダイジェスト・シュール・アンサンブル・デヌリファイアーを注ぎます。 retenus, et (3) 再展開者 le マニフェスト rafraichi。無効化するプルーン トロップを使用して、レゾールの在庫を事前に確認してください。

Documentez は、Runbook の操作で locaux をオーバーライドします。ガバナンスの政治は、定期的に管理され、在庫とアーカイブの計画を段階的に管理します。

### エビクションとリカバリのフロー

1. ペンダント ル ダイヤル、`IrohaNetwork` は les capacites annoncees と比較します。 Tout 不一致レベル `HandshakeConfidentialMismatch`; `Ready` を介して接続を確立し、ピア レストでファイルの発見をサポートします。
2. サービス調査のログ (リモートおよびバックエンドのダイジェストを含む) を介して公開し、Sumeragi を計画し、ピアから提案を投票します。
3. レジストリとパラメータの検証とアンサンブルの調整とパラメータのオペレーターの救済 (`vk_set_hash`、`pedersen_params_id`、`poseidon_params_id`) とプログラムの `next_conf_features` の平均 `activation_height`集まる。ダイジェストを調整し、プロチェーン ハンドシェイクを自動で再利用します。
4. ブロック化されたディフューザーの古い再使用 (リプレイ アーカイブによる例)、妥当性検査の拒否決定の平均 `BlockRejectionReason::ConfidentialFeatureDigestMismatch`、研究の一貫性のある重要な記録。

### リプレイセーフなハンドシェイク フロー1. チャック暫定的なソータンテ・アルーエ・アン・ヌーボー・マテリアル・ド・クレ・ノイズ/X25519。ハンドシェイク署名のペイロード (`handshake_signature_payload`) は、公開されたロケールと距離を連結し、ソケットのアドレスを通知し、エンコード Norito などをコンパイルし、`handshake_chain_id` - チェーンの識別子をコンパイルします。 Le message est chiffre AEAD avant de quitter le noeud。
2. レスポンダは、ペイロードの平均的なペイロードを再計算し、ピア/ローカル インバースを実行し、署名 Ed25519 embarquee dans `HandshakeHelloV1` を検証します。 Parce que les deux cles ephimeres et l adresse annoncee font party du domaine de signed、rejouer un message capture contre un autrepeer une recuperer une connexion stale echoue deterministiquement。
3. `HandshakeConfidentialMeta` に関する情報と `ConfidentialFeatureDigest` の情報。レセプターは、タプル `{ enabled, assume_valid, verifier_backend, digest }` と息子 `ConfidentialHandshakeCaps` ローカルを比較します。不一致ソート avec `HandshakeConfidentialMismatch` の前処理ファイル転送は `Ready` を通過します。
4. DOIVENT の再コンピュータ ファイルのダイジェスト (`compute_confidential_feature_digest` 経由) と、レジストリ/政策の再管理者が、事前に再接続する必要がありました。ハンドシェイクの継続的なダイジェストを報告し、再入者と検証者の設定を無効にします。
5. 標準 `iroha_p2p::peer` (`handshake_failure_count`、エラー分類のヘルパー) と、遠隔のピア ID およびダイジェストの説明のログ構造の成功と実行。監視は、検出器のリプレイやモーベーズ設定ペンダントのロールアウトを示します。

## ジェスチャの説明とペイロード
- 勘定科目ごとの派生階層:
  - `sk_spend` -> `nk` (無効化キー)、`ivk` (受信表示キー)、`ovk` (送信表示キー)、`fvk`。
- ECDH の共有キー導出における AEAD の有効なペイロードの記録。監査のオプションと監査のオプションを表示し、政治活動の補助出力を提供します。
- CLI の調整: `confidential create-keys`、`confidential send`、`confidential export-view-key`、監査ツールのメモの作成、およびヘルパー `iroha app zk envelope` の製造/検査の封筒 Norito オフライン。 Torii は、`POST /v2/confidential/derive-keyset` 経由で派生したファイル ミーム フラックスを公開します。16 進法および Base64 形式の戻り値は、財布、プログラムの階層構造を提供します。

## ガス、制限、および DoS の制御
- ガスのスケジュールを決定します:
  - Halo2 (Plonkish): ベース `250_000` ガス + `2_000` ガス パーパブリック入力。
  - `5` ガスパープルーフバイト、プラス料金パーナリファイアー (`300`) およびパーコミットメント (`500`)。
  - 構成ノード経由でのオペレーター追加料金の定数 (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`);変更内容は、クラスターのホットリロードおよびアップリケの決定を決定するために、設定を変更するためのプロパジェントです。
- 期間の制限 (デフォルト設定可能):
- `max_proof_size_bytes = 262_144`。
- `max_nullifiers_per_tx = 8`、`max_commitments_per_tx = 8`、`max_confidential_ops_per_block = 256`。
- `verify_timeout_ms = 750`、`max_anchor_age_blocks = 10_000`。 `verify_timeout_ms` が指示の決定を中止したことを証明します (投票用紙ガバナンスが `proof verification exceeded timeout`、`VerifyProof` がエラーを返します)。
- 活性を保証する割り当て追加: `max_proof_bytes_block`、`max_verify_calls_per_tx`、`max_verify_calls_per_block`、および `max_public_inputs` 生まれのブロック ビルダー。 `reorg_depth_bound` (>= `max_anchor_age_blocks`) 国境のチェックポイントを保持します。
- L 実行ランタイムは、トランザクションを拒否し、ブロック全体のトランザクションを制限します。また、エラー `InvalidParameter` は、元帳が完全な状態であることを決定します。
- `vk_id` によるトランザクションの機密情報を事前に管理し、リソースの使用状況を事前に確認し、安全性を確保します。
- タイムアウトや違反の有無を確認するための検証が行われます。トランザクションの異常を明示的に反映します。バックエンド SIMD オプションは、互換性を維持するために変更されます。

### ベースラインのキャリブレーションと許容ゲート
- **リファレンスのプレート形状** DOIVENT couvrir les trois profils hardware ci-dessous のキャリブレーションを実行します。レビューを実行し、プロフィールを確認します。|プロフィール |建築 | CPU / インスタンス |フラグのコンパイラ |目的 |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) または Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` |組み込みベクトルのないプランチャーの設定。フォールバックを使用してテーブルを作成します。 |
  | `baseline-avx2` | `x86_64` |インテル Xeon ゴールド 6430 (24c) |デフォルトでリリース |有効なファイル パス AVX2。ニュートラルな耐性による SIMD の高速化を検証します。 |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) |デフォルトでリリース |バックエンドの NEON レストが確実に決定され、x86 の平均スケジュールが調整されます。 |

- **ベンチマーク ハーネス** 校正ガスの関係 DOIVENT の製品平均値:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` 注ぐ確認装置の固定具を決定します。
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` は、オペコード VM の変更を検出します。

- **ランダム性の修正。** エクスポーター `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` 前衛的なベンチが `iroha_test_samples::gen_account_in` を決定する `KeyPair::from_seed`。 Le harness imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` une seule fois;可変マンクを使用し、DOITエコーをレビューします。新しい校正ユーティリティは、継続的に名誉ある環境を設定し、導入と補助を行います。

- **結果をキャプチャします。**
  - リリースの履歴書基準 (`target/criterion/**/raw.csv`) をアップロードして、アーティファクトのチャック プロファイルを作成します。
  - ストッカー les metriques 派生 (`ns/op`、`gas/op`、`ns/gas`) および [機密ガス校正台帳](./confidential-gas-calibration) の avec le commit git およびコンパイルツールのバージョン。
  - Conserver les deux derniers ベースライン パー プロフィール;最高のスナップショットと古い情報、そして最近の有効な情報。

- **許容範囲と許容範囲**
  - ガス エントレのデルタ `baseline-simd-neutral` および `baseline-avx2` DOIVENT レスター <= +/-1.5%。
  - ガスのデルタ エントレ `baseline-simd-neutral` および `baseline-neon` DOIVENT レスター <= +/-2.0%。
  - RFC 明示的なルールと緩和策に基づいて、スケジュールを調整する必要があるため、校正の提案が必要です。

- **レビューのチェックリスト** 提出者には以下の責任があります。
  - `uname -a`、`/proc/cpuinfo` の追加情報 (モデル、ステッピング)、ログ キャリブレーションの `rustc -Vv` を含めます。
  - Verifier que `IROHA_CONF_GAS_SEED` apparait dans la sortie bench (シードアクティブなベンチを実装)。
  - 保証者は、ペースメーカーと製造機密性を検証する機能フラグを立てます (`--features confidential,telemetry` lors des benches avec Telemetry)。

## 設定と操作
- `iroha_config` セクション `[confidential]`:
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
- テレメトリの評価基準: `confidential_proof_verified`、`confidential_verifier_latency_ms`、`confidential_proof_bytes_total`、`confidential_nullifier_spent`、`confidential_commitments_appended`、`confidential_mempool_rejected_total{reason}` など`confidential_policy_transitions_total`、ジャメの暴露者デ・ドニー・アン・クレアなし。
- サーフェス RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## テスト戦略
- 決定主義: マークルの根と無効化セットの同一性をブロックし、トランザクションをシャッフルします。
- Resilience aux reorg: マルチブロック avec アンカーの再構成シミュレータ。無効化されたものは安定したものを保持し、アンカーは古くなったものを拒否します。
- ガスの不変条件: 検証者は、加速 SIMD 以外の平均的なガスの使用法を識別します。
- 限界テスト: テール/ガスのプラフォン、最大入出力数、タイムアウトの強制を証明します。
- ライフサイクル: ガバナンスの運用は、ローテーション後の有効化/非推奨の検証とパラメータ、テストに依存します。
- FSM ポリシー: 自動化/中間移行、移行ペンダント、および効果的な自動化の拒否。
- レジストリの緊急性: `withdraw_height` に関する緊急情報の報告と、その後の証拠の再提出。
- 機能ゲーティング: validateurs avec `conf_features` 不一致の拒否ブロック。オブザーバーは、コンセンサスに影響を与えることなく、`assume_valid=true` を監視します。
- 等価性データ: 検証者/完全/オブザーバーは、正規のチェーン上でのルートの同一性を生成しません。
- 否定的なファジング: 不正な形式、ペイロードの次元の証明、および無効化拒否者の決定的衝突の証明。## 移行
- ロールアウト機能ゲート: フェーズ C3 終了に合わせて、`enabled` のデフォルトは `false`。安全性の高いルールは、事前に確認し、検証を行う必要があります。
- 影響を与えない透明な行為。レジストリとキャパシテスとの交渉に必要な指示の秘密。
- ブロックに関する決定性をサポートする機密情報を無視してコンパイルを実行します。監視者は `assume_valid=true` を使用して検証を行うことができます。
- ジェネシスは、レジストリの初期設定、パラメータのアンサンブル、活動の秘密保持、および監査のオプションを含む政治的秘密を明らかにします。
- 運用担当者は、レジストリのローテーション、政治の移行、およびアップグレードの保守を決定するための緊急の撤回を行うための運用ブックを発行します。

## トラベルレスタント
- Halo2 のパラメータのアンサンブル (回路のタイユ、ルックアップの戦略) と、1 時間ごとのデフォルトのガス/タイムアウトの平均プロチェーン リフレッシュ `confidential_assets_calibration.md` のプレイブックのキャリブレーション結果の登録。
- 開示監査の政治および選択的閲覧関係者の API の最終決定者、Torii ガバナンス草案の署名によるワークフローの承認の確認。
- 証人の暗号化スキーマは、複数の受信者とメモのバッチを出力し、文書形式とエンベロープの実装者 SDK を出力します。
- 委員は、サーキット、レジストリ、およびパラメータおよびアーカイブのローテーションの外部の安全性を審査し、結論を報告し、内部監査を担当します。
- API の指定子は、監査人および発行者による支出の調整と、スコープ ビュー キーに関するガイダンス、ベンダー、ウォレットの実装、ミームの意味を証明します。## 段階的な実装
1. **フェーズ M0 - 出荷時の硬化**
   - [x] ポセイドン PRF (`nk`、`rho`、`asset_id`、`chain_id`) の設計での無効化スーツの保守の導出は、台帳の更新に伴うコミットメントの決定を強制するものではありません。
   - [x] L は、トランザクション/ブロック単位での秘密のプラフォンとクォータの実行のアップリケ、拒否されるトランザクションの予算、およびエラーの上限を決定します。
   - [x] ハンドシェイク P2P 通知 `ConfidentialFeatureDigest` (バックエンド ダイジェスト + フィンガープリント レジストリ) および `HandshakeConfidentialMismatch` を介した不一致の決定をエコーし​​ます。
   - [x] 退職者は、パニックや進路の決定など、職務上の秘密を保持し、非責任者として職務を遂行します。
   - [ ] アップリケは、検証のタイムアウトの予算と、辺境のチェックポイントを再編成するための予算を作成します。
     - [x] 検証アップリケのタイムアウト予算。レ証明デパッサント `verify_timeout_ms` エコーエント保守決定論。
     - [x] メンテナンス `reorg_depth_bound` に関するフロンティア チェックポイント、プルナント チェックポイントとスナップショットの決定に関する古いフェネット構成。
   - `AssetConfidentialPolicy` の導入、ポリシー FSM および施行の指示の作成/転送/公開。
   - `conf_features` をコミットし、ブロックのヘッダーと拒否者が検証に参加し、レジストリ/パラメーターのダイジェストをダイジェストします。
2. **フェーズ M1 - レジストリとパラメータ**
   - レジストリ `ZkVerifierEntry`、`PedersenParams`、および `PoseidonParams` のアベック オペレーション ガバナンス、アンクレイジ ジェネシス、およびキャッシュのキャッシュ。
   - システムコールを実行し、レジストリ、ガス スケジュール ID、スキーマ ハッシュなどを検索し、詳細をチェックします。
   - ペイロード情報 v1 のフォーマット、ウォレットのキー導出ベクトル、および機密情報の提供に関する CLI のサポート。
3. **フェーズ M2 - ガスとパフォーマンス**
   - 実装者によるガス決定のスケジュール、ブロックごとのコンピューティング、およびベンチマークの平均テレメトリの利用 (検証の遅延、証明の遅延、メモリプールの拒否)。
   - Durcir CommitmentTree チェックポイント、チャージ LRU、およびマルチアセットのワークロードを注ぐヌルファイアーのインデックス。
4. **フェーズ M3 - ローテーションとツールウォレット**
   - 複数パラメータと複数バージョンの証明をアクティブに受け入れます。サポーター、アクティベーション/非推奨のパイロット、ガバナンス、移行時のランブックなど。
   - 移行 SDK/CLI のフロー、スキャン監査のワークフロー、支出の調整のツールの説明。
5. **フェーズ M4 - 監査その他**
   - キーと監査人のワークフロー、選択的開示の API、およびランブックの運用に関する情報。
   - 計画者は、外部暗号化/セキュリティと出版者の結論を `status.md` でレビューします。

Chaque フェーズは、ロードマップのマイルストーンと、ブロックチェーンの保証と実行の決定に向けた協会のテストとロードマップのマイルストーンを 1 時間で達成しました。
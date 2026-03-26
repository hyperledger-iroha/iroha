---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 機密資産、ZK の転送
説明: シールドされた循環レジストリ、オペレータ制御、フェーズ C の青写真
スラグ: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# 機密資産 ZK 転送設計

## モチベーション
- オプトインでシールドされたアセット フロー、ドメイン、流通、トランザクション プライバシー、セキュリティ
- 監査人、オペレーター、回路、暗号パラメータ、ライフサイクル制御 (アクティブ化、ローテーション、取り消し)

## 脅威モデル
- 検証者は正直だが好奇心旺盛です: コンセンサスを忠実に把握する 台帳/州を検査する
- ネットワーク監視者がデータをブロックし、うわさ話の取引を阻止プライベート ゴシップ チャンネル
- 範囲外: 台帳外トラフィック分析、量子敵対者 (PQ ロードマップ、台帳可用性攻撃)

## 設計の概要
- 資産の透明な残高 *シールド プール* を宣言します。シールドされた流通暗号化コミットメント سے は ہوتی ہے۔ を表します
- メモ `(asset_id, amount, recipient_view_key, blinding, rho)` は次のようにカプセル化します。
  - コミットメント: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`。
  - 無効化: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`、メモの順序付け、独立したメモ
  - 暗号化されたペイロード: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`。
- トランザクション Norito でエンコードされた `ConfidentialTransfer` ペイロードの内容:
  - パブリック入力: マークル アンカー、ヌリファイア、新しいコミットメント、資産 ID、回路バージョン
  - 受信者とオプションの監査人、および暗号化されたペイロード
  - ゼロ知識証明、価値保全、所有権、認可証明
- 台帳上のレジストリ上のキーとパラメータ セットの検証 アクティベーション ウィンドウの確認ノードが不明 取り消されたエントリ 参照 証明 検証 検証
- コンセンサスヘッダー アクティブな機密機能ダイジェスト コミット ブロック 受け入れ レジストリ パラメーター状態の一致
- 実証済みの構築 Halo2 (Plonkish) スタックの信頼できるセットアップGroth16 の SNARK バリアント v1 のバージョン サポートされていません ہیں۔

### 決定的なフィクスチャ

機密メモ封筒 اب `fixtures/confidential/encrypted_payload_v1.json` میں 正規備品 کے ساتھ آتے ہیں۔データセット v1 エンベロープ 不正なサンプル 不正なサンプル SDK のパリティ解析Rust データ モデル テスト (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) Swift スイート (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) フィクスチャ、Norito エンコード、エラー サーフェス回帰カバレッジ コーデックの調整と整列の調整

Swift SDK は、特注の JSON グルーを使用してシールド命令を発行し、32 バイトのメモコミットメント、暗号化されたペイロード、借方メタデータ、`ShieldRequest` を出力します。 پھر `/v1/pipeline/transactions` پر サイン اور リレー کرنے کے لئے `IrohaSDK.submit(shield:keypair:)` (یا `submitAndWait`) کال کریں۔ヘルパー コミットメントの長さの検証 `ConfidentialEncryptedPayload` Norito エンコーダ スレッドの確認 `zk::Shield` レイアウトの確認ミラー 財布 錆び ロックステップ 財布

## コンセンサスコミットメントとケイパビリティゲーティング
- ブロック ヘッダー `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` が公開されるダイジェストコンセンサスハッシュ حصہ لیتا ہے اور ブロック受け入れ کے لئے ローカルレジストリビュー سے match ہونا چاہئے۔
- ガバナンス `activation_height` レベル `next_conf_features` レベルのアップグレード ステージ高さのブロックプロデューサーのダイジェスト放出の高さ
- 検証ノードは `confidential.enabled = true` `assume_valid = false` を操作する必要があります。スタートアップ チェック バリデータ セットの参加 ٩رنے سے انکار کرتے ہیں اگر کوئی شرط 失敗 ہو یا local `conf_features` diverge ہو۔
- P2P ハンドシェイク メタデータ اب `{ enabled, assume_valid, conf_features }` شامل کرتا ہے۔サポートされていない機能は、ピア `HandshakeConfidentialMismatch` を宣伝します。拒否します。拒否します。コンセンサス ローテーション。
- 非検証オブザーバー `assume_valid = true` سیٹ کر سکتے ہیں؛機密デルタを盲目的に適用する コンセンサスの安全性を確保する## 資産ポリシー
- 資産定義 作成者 ガバナンス 設定 セット `AssetConfidentialPolicy` ہوتا ہے:
  - `TransparentOnly`: デフォルトモード透明な命令 (`MintAsset`、`TransferAsset`) は許可されます。 シールドされた操作は拒否されます。
  - `ShieldedOnly`: 秘密の指示の発行と転送、秘密の指示の送信`RevealConfidential` ممنوع ہے تاکہ 残高 عوامی نہ ہوں۔
  - `Convertible`: 保持者 オン/オフランプ命令 透明なシールド表現 値の移動
- FSM に制約された政策は、資金の滞留に続き、次のようになります。
  - `TransparentOnly → Convertible` (シールドされたプールが有効)
  - `TransparentOnly → ShieldedOnly` (移行保留中、変換ウィンドウ)
  - `Convertible → ShieldedOnly` (最小遅延時間)
  - `ShieldedOnly → Convertible` (移行計画、シールド付きノート、消費可能)
  - `ShieldedOnly → TransparentOnly` は許可されていません シールド プールが空です ガバナンスの未処理のメモ シールドを解除しています 移行エンコード
- ガバナンス指示 `ScheduleConfidentialPolicyTransition` ISI کے ذریعے `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` set کرتی ہیں اور `CancelConfidentialPolicyTransition` سے スケジュールされた変更は中止されます کر سکتی ہیں۔ Mempool の検証 یقینی بناتی ہے کہ کوئی トランザクション遷移の高さ 跨ぎ فوئی 中間ブロック ポリシー チェックの変更 ہونے پر 包含決定的 طور پر 失敗 ہو۔
- 保留中のトランジション ブロックのブロックの適用 ہیں: ブロックの高さの変換ウィンドウ ブロックの高さの変換ウィンドウ (ShieldedOnly のアップグレード) `effective_height` ランタイム `AssetConfidentialPolicy` 更新 `zk.policy` メタデータ更新 保留中のエントリのクリア`ShieldedOnly` 移行の成熟度 透明な供給状態 ランタイム変更の中止 警告ログ 前のモード 状態 ہے۔
- 設定ノブ `policy_transition_delay_blocks` `policy_transition_window_blocks` 最低通知期間 猶予期間の適用 ウォレットの切り替え メモの変換
- `pending_transition.transition_id` 監査ハンドルガバナンス 移行の完了 キャンセル キャンセル 引用 オペレータのオンランプ/オフランプレポートの相関関係
- `policy_transition_window_blocks` デフォルト 720 ہے (60 秒のブロック時間 پر تقریباً 12 گھنٹے)۔ノード ガバナンス リクエストの短縮通知
- ジェネシスは、CLI フローの現在の保留中のポリシーを明らかにします。アドミッション ロジックの実行時間 ポリシー 確認 確認 秘密指示の許可
- 移行チェックリスト — 「移行シーケンス」 マイルストーン M0 طابق 段階的アップグレード計画 دیکھیں۔

#### Torii 遷移監視

ウォレット 監査人 `GET /v1/confidential/assets/{definition_id}/transitions` 投票 アクティブ `AssetConfidentialPolicy` دیکھتے ہیں۔ JSON ペイロード 正規のアセット ID、最新の観測ブロック高さ `current_mode`、高さ、有効モード (変換ウィンドウ、`Convertible` レポート、期待値) `vk_set_hash`/ポセイドン/ペダーセン識別子 شامل کرتا ہے۔ガバナンス移行保留中 回答 بھی ہوتا ہے:

- `transition_id` - `ScheduleConfidentialPolicyTransition` 監査ハンドル
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` 派生 `window_open_height` (ウォレットのブロック、ShieldedOnly のカットオーバー、変換、変換)

応答例:

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

`404` 応答 مطلب ہے کہ 一致する資産定義 موجود نہیں۔移行予定日 `pending_transition` フィールド `null` ہوتا ہے۔

### ポリシーステートマシン|現在のモード |次のモード |前提条件 |有効高さの取り扱い |メモ |
|------------------|------------------|---------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
|透明のみ |コンバーチブル |ガバナンス検証ツール/パラメータ レジストリ エントリがアクティブ化されます`ScheduleConfidentialPolicyTransition` 送信する جس میں `effective_height ≥ current_height + policy_transition_delay_blocks` ہو۔ |遷移 `effective_height` 実行 ہوتا ہے؛シールドプール فوراً دستیاب ہوتا ہے۔                   |フローを確認する 機密性を有効にする デフォルトのパスを設定する               |
|透明のみ |シールド付きのみ | اوپر والی شرائط کے ساتھ `policy_transition_window_blocks ≥ 1` بھی۔                                                         |ランタイム `effective_height - policy_transition_window_blocks` پر `Convertible` میں داخل ہوتا ہے؛ `effective_height` پر `ShieldedOnly` میں بدلتا ہے۔ |命令は無効にする ہونے سے پہلے 確定的変換ウィンドウ دیتا ہے۔   |
|コンバーチブル |シールド付きのみ | `effective_height ≥ current_height + policy_transition_delay_blocks` 移行予定ガバナンスはメタデータを監査する必要があります (`transparent_supply == 0`) を証明する必要がありますランタイムカットオーバーは強制的に実行されます|ウィンドウ セマンティクスの説明`effective_height` 透明な供給がゼロ以外 ہو تو 遷移 `PolicyTransitionPrerequisiteFailed` ٩ے ساتھ 中止 ہو جاتا ہے۔ |資産 秘密の回覧 ロック ہے۔                                     |
|シールド付きのみ |コンバーチブル |スケジュールされた移行Ûアクティブな緊急出金 (`withdraw_height` 未設定)۔                                    | `effective_height` 状態反転 ہوتا ہے؛ランプを明らかにする دوبارہ کھلتی ہیں جبکہ シールドされたノートが有効 رہتے ہیں۔                           |メンテナンスウィンドウ 監査員レビュー ٩ے لئے۔                                          |
|シールド付きのみ |透明のみ |ガバナンス `shielded_supply == 0` 署名 `EmergencyUnshield` 計画段階 (監査人の署名)۔ |ランタイム `effective_height` پہلے `Convertible` ウィンドウ کھولتا ہے؛高さ 機密指示 ハードフェイル 資産透明専用モード جاپس جاتا ہے۔ |最後の手段の出口ウィンドウの表示 機密メモの使用 トランジションの自動キャンセル 表示|
|任意 |現在と同じ | `CancelConfidentialPolicyTransition` 保留中の変更クリア                                                        | `pending_transition` فوراً を削除します ہوتا ہے۔                                                                          |現状維持完全性                                             |

リストの作成 移行 ガバナンス提出 拒否 ہوتے ہیں۔実行時にスケジュールされた移行を適用する 前提条件を確認する障害アセット以前のモードの確認 テレメトリ/ブロック イベントの確認 `PolicyTransitionPrerequisiteFailed` の放出 ہے۔

### 移行のシーケンス

2. **移行を段階的に行う:** `policy_transition_delay_blocks` کو 尊重 کرنے والے `effective_height` کے ساتھ `ScheduleConfidentialPolicyTransition` 提出 کریں۔ `ShieldedOnly` 変換ウィンドウは (`window ≥ policy_transition_window_blocks`) を指定します
3. **オペレータ ガイダンスの公開:** واپس ملنے والا `transition_id` 記録オン/オフランプ ランブックの循環財布 監査人 `/v1/confidential/assets/{id}/transitions` 購読 ウィンドウ オープン高さ جانتے ہیں۔
4. **ウィンドウの強制:** ウィンドウの実行時ポリシー `Convertible` スイッチ `PolicyTransitionWindowOpened { transition_id }` の発行 競合するガバナンス要求の拒否ありがとうございます
5. **終了または中止:** `effective_height` ランタイム移行の前提条件を確認する (透明な供給、緊急撤退、および緊急撤退)۔成功ポリシーとリクエスト モードの反転失敗 `PolicyTransitionPrerequisiteFailed` を発行します 保留中の移行がクリアされます ポリシーは変更されません
6. **スキーマのアップグレード:** 移行 ガバナンス資産スキーマ バージョン バンプ (`asset_definition.v2`) CLI ツール マニフェストのシリアル化 `confidential_policy` が必要ありがとうございますGenesis アップグレード ドキュメント オペレーター バリデータの再起動 ポリシー設定 レジストリ フィンガープリントの追加 ੩ی ہدایت دیتے ہیں۔機密性の有効化 ネットワークの望ましいポリシー ジェネシスのエンコード起動 モード変更 チェックリスト フォロー 変換ウィンドウ 確定的 ウォレット 調整ありがとうございます

### Norito マニフェストのバージョン管理とアクティベーション

- Genesis マニフェストには `confidential_registry_root` カスタム キー `SetParameter` を含める必要があります。ペイロード Norito JSON ہے جو `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` سے match کرتا ہے: 検証者エントリ فعال نہ ہو تو フィールド省略 کریں (`null`) 32 バイトの 16 進文字列 (`0x…`) マニフェストの検証手順 `compute_vk_set_hash` ハッシュの説明こパラメータが欠落しています ハッシュの不一致 ہونے پر ノードの開始 سے انکار کرتے ہیں۔
- オンワイヤー `ConfidentialFeatureDigest::conf_rules_version` マニフェスト レイアウト バージョンの埋め込みv1 ネットワーク `Some(1)` は必ず必要です `iroha_config::parameters::defaults::confidential::RULES_VERSION` は必要ですルールセットの進化 定数バンプ マニフェストの再生成 バイナリの再生成 ロックステップ ロールアウトバージョン ミックス バリデータ `ConfidentialFeatureDigestMismatch` ブロック 拒否 ブロック 拒否
- アクティベーション マニフェスト、レジストリの更新、パラメータのライフサイクルの変更、ポリシーの移行バンドルは、一貫したダイジェストを保持する必要があります。
  1. 計画されたレジストリの変異 (`Publish*`、`Set*Lifecycle`) オフライン状態の表示 適用 `compute_confidential_feature_digest` アクティベーション後のダイジェストの計算
  2. 計算されたハッシュ `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` が放出される 遅れているピア 中間レジストリ命令がミスされる 正しいダイジェストが回復される
  3. `ScheduleConfidentialPolicyTransition` 命令が追加されます政府発行の指示 `transition_id` 引用符マニフェスト ランタイム拒否
  4. マニフェスト バイト、SHA-256 フィンガープリント、アクティベーション プラン、ダイジェスト メッセージ、SHA-256 フィンガープリント、アクティベーション プランオペレーターは、アーティファクトを検証し、投票し、パーティションを検証し、パーティションを検証します。
- ロールアウト 遅延カットオーバー ターゲット高さ コンパニオン カスタム パラメータ レコード (`custom.confidential_upgrade_activation_height`)監査人、Norito でエンコードされた証明、検証者、ダイジェスト変更、通知ウィンドウ、名誉、承認。

## ベリファイアとパラメータのライフサイクル
### ZK レジストリ
- 元帳 `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` 固定 ہہ۔ `proving_system` 固定 `Halo2` 固定 ہے۔
- `(circuit_id, version)` ペアは世界的にユニークですレジストリ回路メタデータ ルックアップ セカンダリ インデックス重複ペア رجسٹر کرنے کی کوشش 入場拒否 ہوتی ہے۔
- `circuit_id` 空でない ہونا چاہئے اور `public_inputs_schema_hash` فراہم کرنا لازم ہے (عام طور پر Verifier کے 正規のパブリック入力エンコーディング کا Blake2b-32ハッシュ)۔入場拒否記録拒否
- ガバナンスに関する指示:
  - `PUBLISH`، メタデータのみ `Proposed` エントリの追加
  - `ACTIVATE { vk_id, activation_height }`エポック境界のアクティベーションスケジュール
  - `DEPRECATE { vk_id, deprecation_height }`، آخری height mark کرنے کیلئے جہاں 証明エントリ کو 参照 کر سکیں۔
  - `WITHDRAW { vk_id, withdraw_height }`緊急シャットダウン資産の引き出し高さ 機密費凍結 ہیں エントリのアクティブ化 نہ ہوں۔
- Genesis マニフェスト `confidential_registry_root` カスタム パラメータの発行 کرتے ہیں جس کا `vk_set_hash` アクティブ エントリ سے match کرتا ہے؛検証ノード コンセンサス参加 ダイジェスト ローカル レジストリ状態 クロスチェック チェック
- 検証者登録の更新 `gas_schedule_id` ضروری ہے؛検証を強制する کرتی ہے کہ レジストリ エントリ `Active` ہو، `(circuit_id, version)` インデックス میں ہو، اور Halo2 証明 میں `OpenVerifyEnvelope` ہو جس کا `circuit_id`、`vk_hash`、`public_inputs_schema_hash` レジストリ レコードが一致しました

### 鍵の証明
- 台帳外のキーの証明 コンテンツアドレス指定識別子 (`pk_cid`、`pk_hash`、`pk_len`) を参照してください。検証者メタデータの公開 فوتے ہیں۔
- ウォレット SDK の PK データの取得、ハッシュの検証、ローカル キャッシュの確認、およびローカル キャッシュの確認

### ペダーセンとポセイドンのパラメータ
- レジストリ (`PedersenParams`、`PoseidonParams`) ベリファイア ライフサイクル コントロール ミラー、ジェネレーター/定数、ハッシュ、アクティブ化/非推奨/撤回高地 ہوتے ہیں۔## 決定的な順序付けと無効化子
- ہر 資産 `CommitmentTree` رکھتا ہے جس میں `next_leaf_index` ہوتا ہے؛コミットメントをブロックする 決定論的な順序を追加する: ブロック順序をブロックする トランザクションを反復するトランザクション シリアル化 `output_idx` 昇順 シールドされた出力の反復
- `note_position` ツリー オフセット سے 導出 ہوتا ہے مگر nullifier کا حصہ **نہیں**؛証拠証人 メンバーシップパス ئے استعمال ہوتا ہے۔
- 無効化剤の安定性を保証する PRF 設計の再編成PRF 入力 `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` バインド アンカー マークル ルート 参照 ہیں جو `max_anchor_age_blocks` سے محدود ہیں۔

## 元帳フロー
1. **MintConfidential { 資産 ID、金額、受信者ヒント }**
   - `Convertible` یا `ShieldedOnly` 資産ポリシー入場資産権限チェック 現在の `params_id` 取得 `rho` サンプル コミットメント 発行 マークル ツリーの更新
   - `ConfidentialEvent::Shielded` は、新しいコミットメント、マークル ルート デルタ、監査証跡、トランザクション呼び出しハッシュを発行します。
2. **TransferConfidential {asset_id、proof、circuit_id、version、nullifiers、new_commitments、enc_payloads、anchor_root、memo }**
   - VM syscall レジストリ エントリの証明検証ホスト یقینی بناتا ہے کہ nullifiers 未使用 ہوں، コミットメント 確定的 طور پر 追加 ہوں، اور アンカー 最近 ہو۔
   - 台帳 `NullifierSet` エントリは、受信者/監査者を記録し、暗号化されたペイロードを保存し、`ConfidentialEvent::Transferred` は、順序付けされた出力を出力し、無効化子を出力し、証明ハッシュを生成します。マークルのルートを要約する
3. **RevealConfidential {asset_id、proof、circuit_id、version、nullifier、amount、recipient_account、anchor_root }**
   - `Convertible` 資産証拠検証 紙幣の価値 明らかにされた金額 紙幣の透明な残高 信用 紙幣の無効化 使用済みマーク 紙幣の保護 書き込み 書き込み
   - `ConfidentialEvent::Unshielded` は、公開量、消費された無効化子、証明識別子、トランザクション呼び出しハッシュ、およびトランザクション呼び出しハッシュを発行します。

## データモデルの追加
- 有効化フラグ付き `ConfidentialConfig` (新しい構成セクション)、`assume_valid`、ガス/制限ノブ、アンカー ウィンドウ、ベリファイアー バックエンド。
- `ConfidentialNote`、`ConfidentialTransfer`、`ConfidentialMint` Norito スキーマ明示バージョン バイト (`CONFIDENTIAL_ASSET_V1 = 0x01`)
- `ConfidentialEncryptedPayload` AEAD メモ バイト `{ version, ephemeral_pubkey, nonce, ciphertext }` ラップ デフォルト `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` XChaCha20-Poly1305 レイアウト
- 正規キー導出ベクトル `docs/source/confidential_key_vectors.json` میں ہیں؛ CLI による Torii エンドポイント フィクスチャのリグレス ہیں۔
- `asset::AssetDefinition` ٩و `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` ملتا ہے۔
- `ZkAssetState` 転送/シールド解除検証機能 `(backend, name, commitment)` バインディングの永続化実行 証明 拒否 拒否 ہے 参照済み キーのインライン検証 登録されたコミットメント 一致 確認
- `CommitmentTree` (フロンティア チェックポイントのある資産ごと)、`(chain_id, asset_id, nullifier)`、`ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` によってキー化された `NullifierSet` 世界状態ストアやあ
- Mempool の早期重複検出、アンカー年齢チェック、一時的な `NullifierIndex`、`AnchorIndex` 構造の維持
- Norito スキーマの更新、パブリック入力、正規の順序付け往復テストによるエンコーディングの決定論により、確実な結果が得られます。
- 暗号化されたペイロードの往復単体テスト (`crates/iroha_data_model/src/confidential.rs`) ロックの解除フォローアップ ウォレット ベクトル監査人 正規 AEAD トランスクリプトを添付`norito.md` エンベロープ オンワイヤ ヘッダー ドキュメント

## IVM 統合とシステムコール
- `VERIFY_CONFIDENTIAL_PROOF` システムコールは、次のように導入します。
  - `circuit_id`、`version`、`scheme`、`public_inputs`、`proof` 結果の `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`。
  - Syscall レジストリ ベリファイア メタデータのロード サイズ/時間制限の適用 確定的なガス料金の適用 証明の成功 デルタ適用
- ホスト読み取り専用 `ConfidentialLedger` 特性の公開 Merkle ルート スナップショットの無効化ステータスの取得Kotodama ライブラリ監視アセンブリ ヘルパー スキーマ検証 فراہم کرتی ہے۔
- Pointer-ABI ドキュメントの証明バッファ レイアウト、レジストリ ハンドル、更新、および更新## ノード機能のネゴシエーション
- ハンドシェイク `feature_bits.confidential` ساتھ `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` アドバタイズ ہے۔検証者の参加状況 `confidential.enabled=true`、`assume_valid=false`、同一の検証者のバックエンド識別子、一致するダイジェスト、不一致 `HandshakeConfidentialMismatch` ハンドシェイクが失敗しました ہیں۔
- オブザーバー ノードの構成 `assume_valid` サポート `assume_valid` サポート: 無効化 機密指示 決定的 `UnsupportedInstruction` パニックパニック有効化 観測者証明検証 状態デルタ適用 ہیں۔
- Mempool の機密トランザクションが拒否されました。ローカル機能が無効になりました。ゴシップは、マッチング機能のないピアをフィルタリングします。 シールドされたトランザクションの監視。 未知の検証者 ID。 サイズの制限。 ブラインドフォワード。

### プルーニングと無効化の保持ポリシーを明らかにする

機密元帳の鮮度を記録する ガバナンス主導の監査を再現する 履歴を保持する`ConfidentialLedger` デフォルト ポリシー 6:

- **無効化保持率:** 使用済み無効化子 کم از کم `730` دن (24 مہینے) 使用高さ کے بعد رکھیں، یا اگر regulator زیادہ مدت مانگے木曜日演算子 `confidential.retention.nullifier_days` ٩ے ذریعے window extend کر سکتے ہیں۔保持期間 無効化 Torii クエリ可能 監査員の二重支出欠勤 監査員の二重支出 欠勤
- **明示的なプルーニング:** 透明なリビール (`RevealConfidential`) コミットメントをメモし、ブロックのファイナライゼーションを確認し、プルーンを実行します。いいえ関連イベントの公開 (`ConfidentialEvent::Unshielded`) 公開金額、受信者、証明ハッシュ レコード、歴史的暴露、再構築、プルーニングされた暗号文、
- **フロンティア チェックポイント:** コミットメント フロンティア ローリング チェックポイント رکھتے ہیں جو `max_anchor_age_blocks` اور 保持期間 میں سے بڑے کو カバー کرتے ہیں۔ノード チェックポイント コンパクト 間隔 無効化子の有効期限 有効期限
- **古いダイジェストの修復:** ダイジェスト ドリフト `HandshakeConfidentialMismatch` 演算子 (1) クラスター ヌリファイア保持ウィンドウの位置合わせ(2) `iroha_cli app confidential verify-ledger` 保持された無効化子セット (3) 更新されたマニフェストの再デプロイ (3) 更新されたマニフェストの再デプロイ時期尚早にプルーニングされたヌリファイアのネットワークへの参加、コールド ストレージの復元、およびリストア

ローカル オーバーライドと運用ランブックのドキュメント保存期間 ガバナンス ポリシー ノード構成 アーカイブ ストレージ プラン ロックステップの更新

### エビクションとリカバリのフロー

1. `IrohaNetwork` にダイヤルして、アドバタイズされた機能を比較します。不一致 `HandshakeConfidentialMismatch` レイズ接続 بند ہوتی ہے اور ピア検出キュー میں رہتا ہے بغیر `Ready` بنے۔
2. 障害ネットワーク サービス ログ (リモート ダイジェスト、バックエンド、リモート ダイジェスト、バックエンド)、Sumeragi ピア、提案、投票、スケジュール
3. オペレータ検証レジストリパラメータセット (`vk_set_hash`、`pedersen_params_id`、`poseidon_params_id`) 整列、合意、`activation_height`、`next_conf_features` ステージ修復 ہیں۔ダイジェストマッチ ہوتے ہی اگلا 握手 خودکار طور پر 成功 کرتا ہے۔
4. 古いピアのブロック ブロードキャスト (アーカイブ リプレイ)、バリデータ `BlockRejectionReason::ConfidentialFeatureDigestMismatch` 決定論的な拒否、拒否ネットワークと台帳の状態の一貫性

### リプレイセーフなハンドシェイク フロー1. アウトバウンド試行の新しいノイズ/X25519 キー素材の割り当て署名付きハンドシェイク ペイロード (`handshake_signature_payload`) ローカル リモート一時公開鍵 Norito でエンコードされたアドバタイズされたソケット アドレス `handshake_chain_id` コンパイル チェーン ID 連結❁❁❁❁メッセージ ノード سے نکلنے سے پہلے AEAD 暗号化 ہوتی ہے۔
2. レスポンダ ピア/ローカル キーの順序の反転、ペイロードの再計算、`HandshakeHelloV1` の組み込み Ed25519 署名の検証、および一時キー アドバタイズされたアドレス署名ドメイン キャプチャされたメッセージ ピア ピア リプレイ 古い接続の回復 確定的失敗 失敗ہوتا ہے۔
3. 機密機能フラグ `ConfidentialFeatureDigest` `HandshakeConfidentialMeta` 旅行 旅行 ہیں۔レシーバー `{ enabled, assume_valid, verifier_backend, digest }` タプル ローカル `ConfidentialHandshakeCaps` 評価 比較 評価不一致 ہونے پر `HandshakeConfidentialMismatch` کے ساتھ 早期終了 ہوتا ہے۔
4. 演算子はダイジェスト (`compute_confidential_feature_digest` 処理) 更新されたレジストリ/ポリシーを再計算します。 ノードは再起動する必要があります。古いダイジェストは、ピアのハンドシェイクの失敗を宣伝します。 古い状態のバリデータ セットを宣伝します。
5. ハンドシェイクの成功/失敗 標準 `iroha_p2p::peer` カウンター (`handshake_failure_count`) 構造化ログ エントリの更新 リモート ピア ID ダイジェスト フィンガープリント タグہوتے ہیں۔インジケーターの監視 ロールアウトの確認 リプレイの試行 設定ミスの確認

## 鍵管理とペイロード
- アカウントごとのキー導出階層:
  - `sk_spend` → `nk` (無効化キー)、`ivk` (受信表示キー)、`ovk` (送信表示キー)、`fvk`。
- 暗号化されたメモのペイロード AEAD 暗号化された ECDH 由来の共有鍵 ہوتے ہیں؛オプションの監査人ビュー キー資産ポリシー 出力 接続 添付
- CLI の追加: `confidential create-keys`、`confidential send`、`confidential export-view-key`、メモ復号化、監査ツール、`iroha app zk envelope` ヘルパー、Norito メモ エンベロープのオフライン生産/検査

## ガス、制限、および DoS 制御
- 決定的なガススケジュール:
  - Halo2 (Plonkish): パブリック入力ごとのベース `250_000` ガス + `2_000` ガス。
  - `5` 証明バイトごとのガス、および無効化器ごと (`300`)、コミットメントごとの料金 (`500`) の料金
  - 演算子定数ノード構成 (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) をオーバーライドします。スタートアップの変更 構成 ホットリロード 伝播 クラスター 確定的 適用 適用
- ハードリミット (設定可能なデフォルト):
- `max_proof_size_bytes = 262_144`。
- `max_nullifiers_per_tx = 8`、`max_commitments_per_tx = 8`、`max_confidential_ops_per_block = 256`。
- `verify_timeout_ms = 750`、`max_anchor_age_blocks = 10_000`。 `verify_timeout_ms` 証明命令 決定論的中止 ہیں (ガバナンス投票 `proof verification exceeded timeout` 発行 کرتے ہیں، `VerifyProof` エラー リターンੁے)۔
- 追加のクォータの活性化により、`max_proof_bytes_block`、`max_verify_calls_per_tx`、`max_verify_calls_per_block`、`max_public_inputs` ブロック ビルダーがバインドされます。 `reorg_depth_bound` (≥ `max_anchor_age_blocks`) フロンティアチェックポイント保持管理
- 実行時、トランザクションごと、ブロックごとの制限を超過する トランザクションが拒否される 確定的 `InvalidParameter` エラーが発生する 台帳の状態が変化しない
- Mempool `vk_id`、証明長さ、アンカー年齢、機密トランザクション、プレフィルター、ベリファイア呼び出し、リソース使用量の制限。
- 検証の確定的タイムアウト、バウンド違反、停止、および停止トランザクションの明示的なエラー 失敗する ہوتی ہیں۔ SIMD バックエンドはオプションです ガス会計を変更します

### キャリブレーションベースラインと許容ゲート
- **リファレンス プラットフォーム。** キャリブレーションを実行する必要があります。必ずハードウェア プロファイルをカバーする必要があります。プロフィールをキャプチャしてレビューを拒否する|プロフィール |建築 | CPU / インスタンス |コンパイラフラグ |目的 |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) インテル Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` |ベクトル組み込み関数 フロア値を確立するフォールバック コスト テーブルの調整|
  | `baseline-avx2` | `x86_64` |インテル Xeon ゴールド 6430 (24c) |デフォルトのリリース | AVX2 パスの検証SIMD の高速化 中性ガス耐性の強化|
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) |デフォルトのリリース | NEON バックエンドと決定的な x86 スケジュールの調整|

- **ベンチマーク ハーネス** ガス校正レポートは次のようにする必要があります:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` 決定的フィクスチャ確認
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` VM オペコードのコストがかかる

- **ランダム性を修正しました。** ベンチ `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` エクスポート `iroha_test_samples::gen_account_in` 決定論的 `KeyPair::from_seed` パス スイッチ ہو۔ハーネス `IROHA_CONF_GAS_SEED_ACTIVE=…` 印刷変数がありません。レビューは失敗する必要があります。キャリブレーション ユーティリティ 補助ランダム性を導入する 環境変数を評価する

- **結果のキャプチャ**
  - 基準概要 (`target/criterion/**/raw.csv`) プロフィール リリース アーティファクト アップロード アップロード
  - 派生メトリクス (`ns/op`、`gas/op`、`ns/gas`) [機密ガス校正台帳](./confidential-gas-calibration) git commit コンパイラのバージョン ストアありがとう
  - プロフィール プロフィール ベースライン ベースラインレポートの検証 チェック スナップショットの削除 スナップショットの削除

- **許容誤差**
  - `baseline-simd-neutral` `baseline-avx2` ガス デルタ ≤ ±1.5%
  - `baseline-simd-neutral` اور `baseline-neon` درمیان ガスデルタ ≤ ±2.0%
  - しきい値の確認 校正提案 スケジュールの調整 不一致/緩和 RFC の確認

- **チェックリストを確認します。** 提出者:
  - `uname -a`、`/proc/cpuinfo` の抜粋 (モデル、ステッピング) `rustc -Vv` キャリブレーション ログ
  - ベンチ出力 میں `IROHA_CONF_GAS_SEED` کے echo ہونے کی تصدیق کریں (ベンチ アクティブ シード プリント کرتے ہیں)۔
  - ペースメーカー、機密検証機能、フラグの生成、一致、 (`--features confidential,telemetry`、テレメトリー、ベンチ、ベンチ)

## 構成と操作
- `iroha_config` 概要 `[confidential]` 概要:
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
- テレメトリ集計メトリクスは次のメッセージを生成します: `confidential_proof_verified`、`confidential_verifier_latency_ms`、`confidential_proof_bytes_total`、`confidential_nullifier_spent`、`confidential_commitments_appended`、`confidential_mempool_rejected_total{reason}`、 `confidential_policy_transitions_total` 平文データが公開される
- RPC サーフェス:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## テスト戦略
- 決定論: 同一のマークル ルートをシャッフルしてランダム化されたトランザクションをブロックし、無効化セットを生成します。
- Reorg の復元力: アンカーのマルチブロック REORG のシミュレートnullifiers 安定した ہیں اور 古いアンカー拒否 ہوتے ہیں۔
- ガス不変条件: SIMD アクセラレーションとノードの同一のガス使用量の検証
- 境界テスト: サイズ/ガスの上限、最大入出力数、タイムアウトの強制などの証明
- ライフサイクル: 検証者、パラメータのアクティブ化/非推奨、ガバナンス運用、ローテーション支出テスト
- ポリシー FSM: 許可/不許可のトランジション、保留中のトランジション遅延、有効な高さ、メンプールの拒否
- レジストリの緊急事態: 緊急引き出し `withdraw_height` 影響を受ける資産の凍結 پر اس کے بعد 証明の拒否 کرتا ہے۔
- 機能ゲーティング: 不一致の `conf_features` バリデーター ブロックが拒否します。 `assume_valid=true` オブザーバーのコンセンサス ٩و متاثر کئے بغیر sync رہتے ہیں۔
- 状態の等価性: バリデータ/フル/オブザーバー ノードの正規チェーンにより、同一の状態ルートが生成されます。
- ネガティブファジング: 不正なプルーフ、特大ペイロード、無効化子の衝突、決定論的拒否、拒否## 優れた作品
- Halo2 パラメータ セット (回路サイズ、ルックアップ戦略) ベンチマーク、キャリブレーション プレイブック、`confidential_assets_calibration.md` リフレッシュ、ガス/タイムアウトのデフォルト更新وں۔
- 監査人の開示ポリシー、選択的閲覧 API の最終決定、ガバナンス草案の承認、ワークフローの承認、Torii ワイヤーの承認
- 暗号化スキームの監視、複数の受信者の出力、バッチ化されたメモ、SDK 実装者の拡張、封筒形式のドキュメントの作成
- 回路、レジストリ、パラメータローテーション手順、外部セキュリティレビュー委員会、調査結果、内部監査報告書、アーカイブ
- 監査人の支出調整 API は、ビューキーのスコープ ガイダンスを指定し、ウォレット ベンダーを発行し、証明セマンティクスを実装します。

## 実装段階
1. **フェーズ M0 — 出荷時の硬化**
   - ✅ ヌリファイアの導出、ポセイドン PRF 設計 (`nk`、`rho`、`asset_id`、`chain_id`) の追跡、元帳の更新、確定的コミットメントの順序付け、強制❁❁❁❁
   - ✅ 実行証明サイズの上限、トランザクションごと/ブロックごとの機密クォータにより、予算超過のトランザクションを強制し、決定論的エラーを拒否します。
   - ✅ P2P ハンドシェイク `ConfidentialFeatureDigest` (バックエンド ダイジェスト + レジストリ フィンガープリント) のアドバタイズ、不一致の通知、`HandshakeConfidentialMismatch` 決定的失敗、決定的な失敗の通知
   - ✅ 機密実行パス パニック 削除 サポートされていないノード ロール ゲーティング 追加 ロール ゲート 追加
   - ⚪ 検証者のタイムアウト バジェット、フロンティア チェックポイント、再編成の深さ境界の強制
     - ✅ 検証タイムアウト予算により ہوئے؛ が適用されます`verify_timeout_ms` 証明 決定論的失敗 ہوتی ہیں۔
     - ✅ フロンティアチェックポイント、`reorg_depth_bound` は、設定されたウィンドウを尊重し、チェックポイントをプルーニングし、決定論的スナップショットを取得します。
   - `AssetConfidentialPolicy`、FSM ポリシー、ミント/転送/開示指示、執行ゲートの導入
   - ブロックヘッダー `conf_features` コミット レジストリ/パラメータダイジェストの分岐 バリデータへの参加拒否
2. **フェーズ M1 — レジストリとパラメータ**
   - `ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` レジストリ ガバナンス オペレーション、ジェネシス アンカリング、キャッシュ管理、土地管理
   - Syscall のレジストリ検索、ガス スケジュール ID、スキーマ ハッシュ、サイズ チェックにはワイヤ接続が必要です。
   - 暗号化ペイロード形式 v1、ウォレット キー導出ベクトル、秘密キー管理、CLI サポート シップ
3. **フェーズ M2 — ガスとパフォーマンス**
   - 決定的なガス スケジュール、ブロックごとのカウンター、テレメトリ、ベンチマーク ハーネスの実装 (レイテンシー、プルーフ サイズ、メモリプール拒否の検証)。
   - CommitmentTree チェックポイント、LRU ロード、ヌルファイア インデックス、マルチアセット ワークロード、強化
4. **フェーズ M3 — ローテーションとウォレット ツール**
   - マルチパラメータ、マルチバージョン証明の受け入れを有効にするガバナンス主導のアクティベーション/非推奨 移行ランブック サポート サポート
   - ウォレット SDK/CLI 移行フロー、監査人スキャン ワークフロー、支出調整ツールを提供
5. **フェーズ M4 — 監査と運用**
   - 監査人の主要なワークフロー、選択的開示 API、運用ランブック
   - 外部暗号化/セキュリティレビュースケジュール 結果 `status.md` 公開 結果

フェーズ ロードマップのマイルストーン テストの更新 ブロックチェーン ネットワーク 決定的な実行保証
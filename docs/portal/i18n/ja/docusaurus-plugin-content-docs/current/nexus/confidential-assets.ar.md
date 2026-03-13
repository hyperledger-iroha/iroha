---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: الاصول السرية وتحويلات ZK
説明: フェーズ C はシールドされています。
スラグ: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# صميم الاصول السرية وتحويلات ZK

## ああ
- シールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きシールド付きああ。
- حفاظ على التنفيذ الحتمي عبر عتاد مدققين غير متجانس مع ابقاء توافق Norito/Kotodama ABI v1。
- تزويد المدققين والمشغلين بضوابط دورة الحياة (تفعيل، تدوير، سحب) للدوائر والمعلماتああ。

## いいえ
- 正直だが好奇心旺盛: ينفذون الاجماع بامانة لكنهم يحاولون فحص 元帳/状態。
- ゴシップニュースゴシップのようなもの。
- 管理: 管理元帳 (PQ ロードマップ)。

## ナオミ
- يمكن للاصول اعلان *シールドプール* الى الارصدة الشفافة؛セキュリティは、セキュリティのコミットメントを保護しました。
- `(asset_id, amount, recipient_view_key, blinding, rho)` のメモ:
  - コミットメント: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`。
  - 無効化: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)` メモ。
  - 暗号化されたペイロード: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`。
- ペイロード `ConfidentialTransfer` のペイロード:
  - パブリック入力: マークル アンカー、無効化子、コミットメント、資産 ID、マークル アンカー。
  - ペイロードは最大です。
  - ゼロ知識証明。
- キーの検証 - キーの検証 - キーの検証 - キーの検証 - キーの検証 - キーの検証証明を証明するために。
- ニュース ダイジェスト ニュース ニュース ニュース ニュース ニュース ニュース ニュース ニュース ダイジェスト ニュースそうです。
- Halo2 (Plonkish) の証明と信頼できるセットアップGroth16 SNARK 日本語版、v1。

### 備品

メモ番号は `fixtures/confidential/encrypted_payload_v1.json` です。エンベロープ v1 のバージョンと、SDK のバージョンを確認します。 Rust データモデル (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) と Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) のフィクスチャと Norito エンコーディングログインしてください。

Swift SDK のセキュリティ シールド グルー JSON のセキュリティ: `ShieldRequest` コミットメント ノート 32 ペイロード セキュリティ デビットメタデータは `IrohaSDK.submit(shield:keypair:)` (`submitAndWait`) および `/v2/pipeline/transactions` です。コミットメント `ConfidentialEncryptedPayload` Norito エンコーダ レイアウト `zk::Shield` エンコーダ錆びた。

## コミットメントとゲートの設定
- 評価 `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` 評価ダイジェスト ハッシュ ハッシュ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ
- يمكن للحوكمة تجهيز الترقيات ببرمجة `next_conf_features` مع `activation_height` مستقبلي؛ビデオは、ダイジェスト版です。
- يجب على عقد المدققين التشغيل مع `confidential.enabled = true` و `assume_valid = false`。重要な情報は、`conf_features` محليا を参照してください。
- P2P ハンドシェイクは `{ enabled, assume_valid, conf_features }` です。ピアは、`HandshakeConfidentialMismatch` の يدخلون ابدافي دوران الاجماع。
- ノード機能ネゴシエーション (#node-capability-negotiation)。ハンドシェイク كـ `HandshakeConfidentialMismatch` وتبقي الـ ピア خارج دوران الاجماع حتى يتطابق ダイジェスト。
- يمكن للمراقبين غير المدققين ضبط `assume_valid = true`؛ طبقون دلتاسرية بشكل اعمى دون التأثير على سلامة الاجماع。## いいえ
- `AssetConfidentialPolicy` يحدده المنشئ الحوكمة:
  - `TransparentOnly`: ログインシールドされています (`MintAsset`、`TransferAsset`、シールド)。
  - `ShieldedOnly`: يجب ان تستخدم كل الاصدارات والتحويلات تعليمات سرية؛ يحظر `RevealConfidential` حتى لا تظهر الارصدة علنا.
  - `Convertible`: シールド付きオン/オフ ランプを備えています。
- FSM の評価:
  - `TransparentOnly → Convertible` (シールド プール)。
  - `TransparentOnly → ShieldedOnly` (يتطلب انتقالا معلقا ونافذة تحويل)。
  - `Convertible → ShieldedOnly` ( تاخير ادنى الزامى)。
  - `ShieldedOnly → Convertible` (يتطلب خطة هجرة لضمان بقاء メモ قابلة للصرف)。
  - `ShieldedOnly → TransparentOnly` シールド プール シールド プール ノートああ。
- 評価 `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` 評価 ISI `ScheduleConfidentialPolicyTransition` 評価 - 評価 - 評価 `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` 評価 ISI `ScheduleConfidentialPolicyTransition` 評価`CancelConfidentialPolicyTransition`。 يضمن تحقق mempool عدم عبور اي معاملة لارتفاع الانتقال، ويفشل الادراج بشكل حتمي اذا تغير فحصありがとうございます。
- يتم تطبيق الانتقالات المعلقة تلقائيا عند فتح كتلة جديدة: عند دخول ارتفاع الكتلة نافذة `ShieldedOnly`) ランタイム `AssetConfidentialPolicy` メタデータ`zk.policy` です。ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイムああ。
- مقابض التهيئة `policy_transition_delay_blocks` و `policy_transition_window_blocks` تفرض اشعارا ادنى وفترات سماح للسماح بتحويل محافظと言うか。
- `pending_transition.transition_id` 監査ハンドルオン/オフランプを確認します。
- `policy_transition_window_blocks` افتراضيه 720 (ブロック時間 60 秒)。最高のパフォーマンスを見せてください。
- ジェネシスは、CLI をマニフェストします。入学許可を取得してください。
- マイルストーン M0 の「移行シーケンス」。

#### 重要な Torii

`GET /v2/confidential/assets/{definition_id}/transitions` と `AssetConfidentialPolicy` を確認してください。ペイロード JSON 認証資産 ID 認証番号 `current_mode` 認証番号 `current_mode` 認証番号عند ذلك الارتفاع (نوافذ التحويل تبلغ مؤقتا `Convertible`)، ومعرفات معلمات `vk_set_hash`/ポセイドン/ペダーセンああ。回答:

- `transition_id` - 監査ハンドル `ScheduleConfidentialPolicyTransition`。
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` و `window_open_height` المشتق (الكتلة التي يجب ان تبدأ فيها المحافظ التحويل لقطع ShieldedOnly)。

メッセージ:

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

يشير رد `404` الى عدم وجود تعريف اصل مطابق。 عند عدم وجود انتقال مجدول يكون الحقل `pending_transition` مساوي لـ `null`。

### آلة حالات السياسة|ログイン | ログインऔर देखें回答 |有効高さ |重要 |
|------------------|------------------|---------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
|透明のみ |コンバーチブル |検証者/パラメータ。 `ScheduleConfidentialPolicyTransition` `effective_height ≥ current_height + policy_transition_delay_blocks`。 | ينفذ الانتقال بالضبط عند `effective_height`؛シールド付きプール。                   |重要な問題は、次のとおりです。               |
|透明のみ |シールド付きのみ | كما سبق، بالاضافة الى `policy_transition_window_blocks ≥ 1`。                                                         |ランタイム `Convertible` `effective_height - policy_transition_window_blocks` `ShieldedOnly` と `effective_height` です。 | حويل حتمية قبل تعطيل التعليمات الشفافة.   |
|コンバーチブル |シールド付きのみ | `effective_height ≥ current_height + policy_transition_delay_blocks` です。監査メタデータ (`transparent_supply == 0`) 監査メタデータランタイムが必要です。 | और देखें `effective_height` は、`PolicyTransitionPrerequisiteFailed` です。 |重要な問題は、次のとおりです。                                     |
|シールド付きのみ |コンバーチブル |ああ、 (`withdraw_height` غير مضبوط)。                                    |セキュリティ `effective_height`ランプを明らかにする、シールドされたノートを明らかにする。                           |あなたのことを忘れないでください。                                          |
|シールド付きのみ |透明のみ | يجب على الحوكمة اثبات `shielded_supply == 0` او تجهيز خطة `EmergencyUnshield` موقعة (تتطلب تاقيع مدققين)。 |ランタイム `Convertible` `effective_height`ログインしてください。 | और देखेंノートに注意してください。 |
|任意 |現在と同じ | `CancelConfidentialPolicyTransition` ログインしてください。                                                        | يتم حذف `pending_transition` فورا。                                                                          | حافظ على الوضع الراهن؛重要です。                                             |

最高のパフォーマンスを見せてください。ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム ランタイム`PolicyTransitionPrerequisiteFailed` を確認してください。

### 移行のシーケンス

1. **レジストリを準備します:** 検証者が検証者を検証します。 `conf_features` は、ピアと同じです。
2. **移行を段階的に行う:** قدّم `ScheduleConfidentialPolicyTransition` مع `effective_height` يراعي `policy_transition_delay_blocks`。 `ShieldedOnly` は、`window ≥ policy_transition_window_blocks` を意味します。
3. **オペレータ ガイダンスの公開:** `transition_id` オン/オフランプのランブック。 `/v2/confidential/assets/{id}/transitions` を確認してください。
4. **ウィンドウの強制:** ランタイム `Convertible`، ويصدر `PolicyTransitionWindowOpened { transition_id }` ويبدأ في رفضありがとうございます。
5. **終了または中止:** عند `effective_height` يتحقق runtime من الشروط المسبقة (عرض شفاف صفر، عدم وجود سحب ()。ログイン して翻訳を追加する`PolicyTransitionPrerequisiteFailed` を確認してください。
6. **スキーマのアップグレード:** 管理者権限 (`asset_definition.v2`) 管理者 CLI `confidential_policy` のマニフェストが表示されます。ジェネシス ジェネシス レジストリ レジストリ レジストリ レジストリ レジストリ レジストリ レジストリ レジストリ レジストリああ。

ジェネシスの起源。 مع ذلك تتبع نفس قائمة التحقق عند تغيير الاوضاع بعد الاطلاق كي تبقى نوافذ التحويل حتمية وتمتلك المحافظ وقتا للتكيف。

### Norito マニフェスト- ジェネシスは `SetParameter` と `confidential_registry_root` を明らかにします。ペイロード Norito JSON `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: 検証 (`null`) 検証検証者数値 16 進数 32 数値 (`0x…`) 数値 `compute_vk_set_hash` 数値検証者マニフェスト。最高のパフォーマンスを見せてください。
- يضمن `ConfidentialFeatureDigest::conf_rules_version` オンワイヤー マニフェスト。 شبكات v1 يجب ان يبقى `Some(1)` ويساوي `iroha_config::parameters::defaults::confidential::RULES_VERSION`。マニフェストをマニフェストに追加する`ConfidentialFeatureDigestMismatch` を参照してください。
- アクティベーション マニフェスト レジストリ レジストリ ダイジェスト ダイジェスト重要:
  1. レジストリ (`Publish*`、`Set*Lifecycle`) オフライン ダイジェスト 分析`compute_confidential_feature_digest`。
  2. صدِر `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` باستخدام الهاش المحسوب حتى يتمكن ピア المتاخرون من استعادة ダイジェスト الصحيح حتىレジストリを登録してください。
  3. `ScheduleConfidentialPolicyTransition`。 يجب على كل تعليمة ان تقتبس `transition_id` الصادر من الحوكمة؛ランタイムをマニフェストします。
  4. マニフェスト SHA-256 ダイジェスト メッセージを表示します。最高のパフォーマンスを見せてください。
- عندما تتطلب عمليات الاطلاق カットオーバー مؤجلا، سجل الارتفاع المستهدف في معامل مخصص مرافق (مثلا `custom.confidential_upgrade_activation_height`)。 هذا يعطي المدققين دليلا مشفرا بنوريتو على ان المدققين احترموا نافذة الاشعار قبل سريانダイジェスト。

## 検証者 دورة حياة 検証者 والمعلمات
### ZK レジストリ
- يخزن レジャー `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` حيث `proving_system` حاليا ثابت على `Halo2`。
- ازواج `(circuit_id, version)` فريدة عالميا؛メタデータを確認してください。入場料を支払う必要があります。
- يجب ان يكون `circuit_id` غير فارغ ويجب توفير `public_inputs_schema_hash` (عادة hash Blake2b-32 لترميز الادخال العام検証者）。入場料は無料です。
- 回答:
  - `PUBLISH` は、`Proposed` メタデータです。
  - `ACTIVATE { vk_id, activation_height }` エポック。
  - `DEPRECATE { vk_id, deprecation_height }` は、証明を証明します。
  - `WITHDRAW { vk_id, withdraw_height }` ログイン高度を撤回してください。
- 創世記は、`confidential_registry_root` `vk_set_hash` المطابقة للادخالات النشطة؛ビデオ ダイジェスト ビデオ ダイジェスト ビデオ ビデオ。
- 検証者 يتطلب `gas_schedule_id`؛ `Active` في فهرس `(circuit_id, version)` Halo2 証明 `OpenVerifyEnvelope` طابق `circuit_id` و`vk_hash` و`public_inputs_schema_hash` في سجل السجل。

### 鍵の証明
- キーの証明 - コンテンツアドレス指定 (`pk_cid`、`pk_hash`、`pk_len`) の証明メタデータ検証者。
- ウォレット SDK と PK をダウンロードします。

### ペダーセンとポセイドンのパラメータ
- 認証 (`PedersenParams`、`PoseidonParams`) 認証、検証者 `params_id` 認証هاشات المولدات/الثوابت، وارتفاعات التفعيل/الاستبدال/السحب。
- コミットメント المجال عبر `params_id` حتى لا تعيد تدوير المعلمات استخدام انماط بت من مجموعات और देखें ID とコミットメントのメモを無効化します。

## 無効化子
- يحافظ كل اصل على `CommitmentTree` مع `next_leaf_index`;約束を守る: 約束を守る: 約束を守るシールド付きシールド `output_idx` です。
- `note_position` مشتق من ازاحات الشجرة لكنه **ليس** جزءا من nullifier؛証拠証人。
- PRF 無効化された再組織化PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` はアンカー、マークル、`max_anchor_age_blocks` を表します。## 台帳
1. **MintConfidential { 資産 ID、金額、受信者ヒント }**
   - `Convertible` と `ShieldedOnly`;入学許可 من سلطة الاصل، يجلب `params_id` الحالي، يعين `rho`، يصدر コミットメント ويحدث マークル ツリー。
   - يصدر `ConfidentialEvent::Shielded` مع commit جديد، فرق Merkle root، وhash استدعاء المعاملة للتدقيق.
2. **TransferConfidential {asset_id、proof、circuit_id、version、nullifiers、new_commitments、enc_payloads、anchor_root、memo }**
   - syscall في VM من 証明 باستخدام مدخل السجل؛ホスト、無効化、コミットメント、アンカー、アンカー。
   - 元帳 `NullifierSet` ペイロード ペイロード `ConfidentialEvent::Transferred` ヌリファイア証明ハッシュ、マークル ルート。
3. **RevealConfidential {asset_id、proof、circuit_id、version、nullifier、amount、recipient_account、anchor_root }**
   - متاحة فقط للاصول `Convertible`;証明、証明、メモ、証明、元帳、シールドされたメモ、無効化。
   - يصدر `ConfidentialEvent::Unshielded` مع المبلغ العلني وnullifiers المستهلكة ومعرفات 証明 وhash استدعاء المعاملة。

## データモデル
- `ConfidentialConfig` (テスト) `assume_valid`、ガス/制限、アンカー、検証バックエンド。
- `ConfidentialNote`、`ConfidentialTransfer`、`ConfidentialMint` 、 Norito 、 بايت نسخة صريح (`CONFIDENTIAL_ASSET_V1 = 0x01`)。
- `ConfidentialEncryptedPayload` يغلف AEAD メモ バイト في `{ version, ephemeral_pubkey, nonce, ciphertext }`، وافتراضيا `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` لتخطيط XChaCha20-Poly1305。
- 評価 `docs/source/confidential_key_vectors.json`; CLI を使用して Torii エンドポイントを確認してください。
- `asset::AssetDefinition` `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`。
- `ZkAssetState` `(backend, name, commitment)` 検証者の転送/シールド解除証明キーの検証 (مرجعا او ضمنيا)。
- `CommitmentTree` (国境検問所) `NullifierSet` `(chain_id, asset_id, nullifier)`، و `ZkVerifierEntry` و `PedersenParams` و `PoseidonParams` في 世界状態。
- メンプール `NullifierIndex` と `AnchorIndex` アンカー。
- 公開 Norito 公開入力往復料金がかかります。
- 往復暗号化ペイロード単体テスト (`crates/iroha_data_model/src/confidential.rs`)。 AEAD のトランスクリプトを作成します。 `norito.md` オンワイヤー エンベロープ。

## IVM とシステムコール
- システムコール `VERIFY_CONFIDENTIAL_PROOF` :
  - `circuit_id`、`version`、`scheme`、`public_inputs`、`proof`、`ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`。
  - syscall メタデータ検証者 من السجل، يفرض حدود الحجم/الوقت، يحسب ガス حتمي، ولا يطبق デルタ الا عند証明。
- ホスト特性読み取り専用 `ConfidentialLedger` スナップショット マークル 無効化子Kotodama ヘルパーはスキーマを目撃します。
- ポインター - ABI レイアウト証明バッファー レジストリ。

## فاوض قدرات العقد
- ハンドシェイク `feature_bits.confidential` と `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`。 `confidential.enabled=true` و`assume_valid=false` 検証検証者バックエンド ダイジェスト متطابق؛ハンドシェイクは `HandshakeConfidentialMismatch` です。
- 構成 `assume_valid` オブザーバー فقط: عند تعطيله، تؤدي تعليمات السرية الى `UnsupportedInstruction` حتمي بلا panic؛監視員は証明を証明します。
- メンプールの名前を入力してください。ゴシップの評価、保護されたピアの評価、評価の検証者、評価者の評価ありがとうございます。

### 握手| और देखेंログイン | ログイン | ログイン重要 |
|---------------------|------------------------------|--------------|
| `enabled=true`、`assume_valid=false`、バックエンド、ダイジェスト、 | और देखेंピア `Ready` 、提案、投票、RBC ファンアウト。と言うのです。 |
| `enabled=true`、`assume_valid=false`、バックエンド متطابق、ダイジェスト قديم او مفقود | (`HandshakeConfidentialMismatch`) | يجب على الطرف البعيد تطبيق تفعيل السجلات/المعلمات المعلقة او انتظار `activation_height` المجدولة。あなたのことを忘れないでください。 |
| `enabled=true`、`assume_valid=true` | (`HandshakeConfidentialMismatch`) |証明を証明するقم بضبط الطرف البعيد كمراقب عبر Torii فقط او اجعل `assume_valid=false` تمكين التحققああ。 |
| `enabled=false`、ハンドシェイク (ハンドシェイク) (認証) 検証者バックエンド | (`HandshakeConfidentialMismatch`) |仲間たちとの関係。バックエンド + ダイジェストを確認してください。 |

証明を証明するために、証明を証明するために、証明を証明する必要があります。 और देखें يمكنها استيعاب الكتل عبر Torii او واجهات الارشفة، لكن شبكة الاجماع ترفضها حتى تعلن قدرات متوافقة.

### 無効化を明らかにする

ノートをノートに記録してください。世界 `ConfidentialLedger` 世界:

- **Nullifier の保持:** الاحتفاظ بـ nullifiers المصروفة لمدة *ادنى* `730` يوما (24 شهرا) بعد ارتفاع الصرف، او助けてください。 يمكن للمشغلين تمديدها عبر `confidential.retention.nullifier_days`。 يجب ان تبقى nullifiers ضمن النافذة قابلة للاستعلام عبر Torii حتى يتمكن المدققون من اثبات二重支出。
- **枝刈りを明らかにする:** تقوم Reveals الشفافة (`RevealConfidential`) بتقليم commits المرتبطة فورا بعد اكتمال الكتلة، لكن nullifierログインしてください。明らかにする (`ConfidentialEvent::Unshielded`) ハッシュ証明を証明する 証明する 証明する 証明する 証明する 証明する暗号文 المقتطع。
- **フロンティアチェックポイント:** フロンティアチェックポイント متحركة تغطي الاكبر من `max_anchor_age_blocks` ونافذة الاحتفاظ。チェックポイントをチェックし、無効化を無効化します。
- **古いダイジェストの修復:** اذا رُفع `HandshakeConfidentialMismatch` بسبب انحراف Digest، يجب على المشغلين (1) التحقق من تطابق نوافذ (2) `iroha_cli app confidential verify-ledger` ダイジェスト مقابل مجموعة nullifier المحتفظ بها، و(3)マニフェストを表示します。 يجب استعادة اي nullifiers حذفت مبكرا من التخزين البارد قبل اعادة الانضمام للشبكة.

運用ランブックを作成するニュース ニュース ニュース ニュース ニュース ニュース ニュース ニュース ニュース ニュース ニュースやあ。

### دفق الاخلاء والاستعادة

1. は、`IrohaNetwork` を意味します。 عدم تطابق يرفع `HandshakeConfidentialMismatch`؛ピア発見キュー、`Ready`。
2. バージョン 18NT00000000X (バージョン ダイジェスト バージョン バックエンド バージョン)ピアをフォローしてください。
3. 検証者 (`vk_set_hash`、`pedersen_params_id`、`poseidon_params_id`)テスト `next_conf_features` テスト `activation_height` テスト。握手会、ダイジェスト、握手会。
4. اذا تمكن ピア قديم من بث كتلة (مثلا عبر アーカイブ リプレイ) ، يرفضها المدققون حتميا بـ `BlockRejectionReason::ConfidentialFeatureDigestMismatch` للحفاظ台帳の管理。

### リプレイセーフなハンドシェイク フロー1. ノイズ/X25519 を確認してください。ハンドシェイク ペイロード (`handshake_signature_payload`) ハンドシェイク ペイロード (`handshake_signature_payload`) ハンドシェイク ペイロード ソケット ソケット`handshake_chain_id` を参照してください。評価はAEAD قبل ارسالها。
2. ペイロード ピア/ローカル アクセス ポイント Ed25519 アクセス `HandshakeHelloV1`。ニュース ニュース ニュース ニュース ニュース ニュース ニュース ニュースピアをフォローしてください。
3. `ConfidentialFeatureDigest` と `HandshakeConfidentialMeta` を確認します。タプル `{ enabled, assume_valid, verifier_backend, digest }` タプル `ConfidentialHandshakeCaps` タプル握手 `HandshakeConfidentialMismatch` と `Ready` を確認してください。
4. يجب على المشغلين اعادة حساب ダイジェスト (`compute_confidential_feature_digest`) واعادة تشغيل السياسات/سجلاتすごいです。ピアは、ダイジェストを分析し、分析を行います。
5. ハンドシェイク تحدث عدادات `iroha_p2p::peer` القياسية (`handshake_failure_count` وغيرها) وتصدر سجلات منظمةピアのダイジェスト。ロールアウト、リプレイ、ロールアウト。

## ペイロード数
- 回答:
  - `sk_spend` → `nk` (無効化キー) `ivk` (受信表示キー) `ovk` (送信表示キー) `fvk`。
- ペイロード メモ、AEAD 、 مفاتيح、 مشتركة、 مشتقة、 من ECDH؛監査人はキーを表示し、出力を表示します。
- CLI: `confidential create-keys`、`confidential send`、`confidential export-view-key`、メモ、`iroha app zk envelope`封筒 Norito です。 Torii 数値 `POST /v2/confidential/derive-keyset` 数値 16 進数 وbase64 数値ありがとうございます。

## セキュリティ DoS
- ガスの状態:
  - Halo2 (Plonkish): `250_000` ガス + `2_000` ガス パブリック入力。
  - `5` ガスは、無効化 (`300`) とコミットメント (`500`) を証明します。
  - يمكن للمشغلين تجاوز هذه الثوابت عبر تهيئة العقد (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`)最高のパフォーマンスを見せてください。
- حدود صارمة (افتراضات قابلة للضبط):
- `max_proof_size_bytes = 262_144`。
- `max_nullifiers_per_tx = 8`、`max_commitments_per_tx = 8`、`max_confidential_ops_per_block = 256`。
- `verify_timeout_ms = 750`、`max_anchor_age_blocks = 10_000`。証明 التي تتجاوز `verify_timeout_ms` تقطع التعليمة حتميا (تصويتات الحوكمة تصدر `proof verification exceeded timeout` و`VerifyProof` يعيد) )。
- ビルダーのバージョン: `max_proof_bytes_block`、`max_verify_calls_per_tx`、`max_verify_calls_per_block`、`max_public_inputs` のビルダー`reorg_depth_bound` (≥ `max_anchor_age_blocks`) 国境の検問所。
- ランタイム プログラム `InvalidParameter`台帳の管理。
- メンプール セキュリティ `vk_id` 証明アンカー 検証者検証すごい。
- タイムアウト時間 - タイムアウト時間ありがとうございます。バックエンド SIMD ガス。

### خطوط اساس المعايرة وبوابات القبول
- **参照プラットフォーム** يجب ان تغطي معايرات الاداء ثلاث ملفات عتاد ادناه。あなたのことを忘れないでください。

  |ああ | और देखें CPU / インスタンス |回答 | 回答ああ |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) と Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | تأسيس قيم ارضية بدون تعليمات متجهة؛フォールバックを実行します。 |
  | `baseline-avx2` | `x86_64` |インテル Xeon ゴールド 6430 (24c) |デフォルトのリリース | AVX2 の評価SIMD とガスを供給します。 |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) |デフォルトのリリース |バックエンド NEON バージョン x86。 |

- **ベンチマーク ハーネス** 評価:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` はフィクスチャです。
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` VM オペコード。

- **ランダム性を修正しました。** `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` テスト、`iroha_test_samples::gen_account_in` テスト、`KeyPair::from_seed` テスト。ハーネス `IROHA_CONF_GAS_SEED_ACTIVE=…` セキュリティあなたのことを忘れないでください。 يجب على اي ادوات معايرة جديدة احترام هذا المتغير عند ادخال عشوائية اضافية。- **結果のキャプチャ**
  - 基準概要 (`target/criterion/**/raw.csv`) は、アーティファクトを示します。
  - خزّن المقاييس المشتقة (`ns/op`, `gas/op`, `ns/gas`) في [機密ガス校正台帳](./confidential-gas-calibration) مع git commitありがとうございます。
  - ベースライン - ベースライン - ベースライン - ベースラインありがとうございます。

- **許容誤差**
  - ±1.5% 以内。
  - ±2.0% 以内。
  - は、RFC يشرح الفجوة وخطة التخفيف を参照してください。

- **チェックリストを確認してください。** 評価:
  - `uname -a` と `/proc/cpuinfo` (モデル、ステッピング) と `rustc -Vv` の情報。
  - التحقق من ظهور `IROHA_CONF_GAS_SEED` في خرج البنش (البنش يطبع シード النشط)。
  - ペースメーカー、機密検証者、生産 (`--features confidential,telemetry` テレメトリ)。

## うーん
- `iroha_config` `[confidential]`:
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
- 評価: `confidential_proof_verified`、`confidential_verifier_latency_ms`、`confidential_proof_bytes_total`、`confidential_nullifier_spent`、`confidential_commitments_appended`、 `confidential_mempool_rejected_total{reason}`、`confidential_policy_transitions_total` 平文。
- RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## いいえ
- マークル ルートと無効化セット。
- 再編成: アンカーの再編成ヌリファイアはアンカーを意味します。
- テスト: SIMD のテスト。
- 証明: 証明タイムアウト/証明証明/証明証明タイムアウト。
- 認証: 認証/認証検証者。
- ポリシー FSM: 移行保留中の有効高さ。
- 認証: 認証 `withdraw_height` 認証。
- 能力ゲーティング: `conf_features` セキュリティ セキュリティ観察者は`assume_valid=true` يواكبون دون التأثير على الاجماع。
- バージョン: バリデーター/フル/オブザーバー、ステート ルート、およびステータス。
- ネガティブファジング: ペイロードの証明と無効化の証明。

## ありがとうございます
- ステータス: フェーズ C3、`enabled`、`false`最高のパフォーマンスを見せてください。
- ニュース ニュース最高のパフォーマンスを見せてください。
- ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュースعرض المزيد عرض المزيد كمراقبين مع `assume_valid=true`。
- 創世記は、次のことを明らかにします。
- ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブック ランブックやあ。

## और देखें
- Halo2 (Halo2 の検索) のキャリブレーション プレイブックガス/タイムアウトは `confidential_assets_calibration.md` です。
- 特別視聴 - 選択的視聴 - Toriiログインしてください。
- 証人の暗号化とセキュリティ SDK の暗号化。
- ログイン アカウント新規登録 ログイン アカウントを作成するありがとうございます。
- API を使用して支出を確認し、ビューキーを確認してください。ああ。## 大事な
1. **フェーズ M0 — 出荷時の硬化**
   - ✅ 無効化子ポセイドン PRF (`nk`、`rho`、`asset_id`、`chain_id`) のコミットメント台帳。
   - ✅ يفرض التنفيذ حدود حجم 証明 وحصص العمليات السرية لكل معاملة/كتلة، المعاملاتありがとうございます。
   - ✅ P2P ハンドシェイク `ConfidentialFeatureDigest` (バックエンド ダイジェスト + ハンドシェイク ハンドシェイク) `HandshakeConfidentialMismatch`。
   - ✅ パニックは、ロールゲートの役割を制限します。
   - ⚪ タイムアウトは、辺境のチェックポイントを検証する検証者を再編成します。
     - ✅ タイムアウト時間証明は `verify_timeout_ms` で証明されています。
     - ✅ `reorg_depth_bound` チェックポイント、スナップショット、スナップショット。
   - ポリシー `AssetConfidentialPolicy` およびポリシー FSM の施行、ミント/転送/公開。
   - `conf_features` レジストリ/パラメータのレジストリ/パラメータ。
2. **フェーズ M1 — レジストリとパラメータ**
   - は、`ZkVerifierEntry` و`PedersenParams` و`PoseidonParams` は、ジェネシス وادارة الكاش を発見しました。
   - システムコール、レジストリ検索、ガス スケジュール ID、スキーマ ハッシュなど。
   - ペイロード バージョン 1 バージョン 1 バージョン 1 バージョン 1 バージョン バージョン 1 バージョン バージョン 1 バージョン バージョン 1 バージョン バージョン 3.0 バージョンの CLI バージョン バージョン。
3. **フェーズ M2 — ガスとパフォーマンス**
   - ガスの分析と分析 (レイテンシの検証、証明、メモリプールの検証)。
   - CommitmentTree チェックポイント、LRU ロード、nullifier インデックスを確認します。
4. **フェーズ M3 — ローテーションとウォレット ツール**
   - 証明証明 متعددة المعلمات والنسخ؛アクティベーション/非推奨の確認と Runbook の確認。
   - ウォレット SDK/CLI を使用して、支出を減らします。
5. **フェーズ M4 — 監査と運用**
   - 選択的開示、runbooks の評価。
   - 国際会議 `status.md`。

ロードマップとロードマップを確認してください。
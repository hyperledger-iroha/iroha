---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Конфиденциальные активы и ZK-переводы
説明: フェーズ C のシールド付きのモジュールです。
スラグ: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Дизайн конфиденциальных активов и ZK-переводов

## Мотивация
- オプトイン シールド付き потоки активов、чтобы домены могли сохранять транзакционную приватность、не изменяя прозрачную циркуляцию。
- Сохранять детерминированное исполнение на разнородном железе валидаторов и совместимость с Norito/Kotodama ABI v1。
- Предоставить аудиторам и операторам контроль жизненного цикла (активация, ротация, отзыв) 回路とАриптографических параметров。

## Модель угроз
- Валидаторы честные-но-любопытные (正直だが好奇心旺盛): исполняют консенсус корректно, но пытаются изучать 台帳/状態。
- Наблюдатели сети видят данные блоков и gossiped транзакции;ゴシップ каналы не предполагаются。
- Вне области: オフレジャー трафика、квантовые противники (отдельно в PQ roadmap)、атаки доступности レジャー。

## Обзор дизайна
- Активы могут объявлять *シールドプール* помимо существующих прозрачных балансов;保護された保護されたコミットメント。
- メモ инкапсулируют `(asset_id, amount, recipient_view_key, blinding, rho)` с:
  - コミットメント: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`。
  - 無効化: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`、независим от порядка メモ。
  - 暗号化されたペイロード: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`。
- Norito-кодированные ペイロード `ConfidentialTransfer`、次のとおりです。
  - パブリック入力: マークル アンカー、ヌリファイア、コミットメント、アセット ID、回路。
  - ペイロードを確認する必要があります。
  - ゼロ知識証明、所有権、авторизацию。
- 台帳レジストリ上のキーを検証します。証明を証明するために、証明する必要があります。
- Заголовки консенсуса коммитятся к активному ダイジェスト конфиденциальных возможностей, поэтому блоки принимаются толькоレジストリとファイルを確認してください。
- Halo2 (Plonkish) の信頼できるセットアップを証明します。 Groth16 と v1 の SNARK バージョン。

### Детерминированные фикстуры

メモ-конверты теперь поставляются с каноническим фикстуром в `fixtures/confidential/encrypted_payload_v1.json`. Набор данных включает положительный v1-конверт и негативные поврежденные образцы, чтобы SDK могли проверятьそうです。 Rust データモデル (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) と Swift スイート (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) の組み合わせNorito-кодирование, поверхности обок и регрессионное покрытие остаются согласованными по мере эволюции кодека。Swift SDK のシールド - カスタム ビスポーク JSON グルー: `ShieldRequest` 32-байтным コミットメント、ペイロード、デビットメタデータ、`IrohaSDK.submit(shield:keypair:)` (`submitAndWait`)、чтобы подписать и отправить транзакцию через `/v1/pipeline/transactions`。コミットメント、`ConfidentialEncryptedPayload`、Norito エンコーダ、レイアウト `zk::Shield`、Хелпер валидирует、пропускает錆びる、錆びる。

## Коммитменты консенсуса и gating возможностей
- Заголовки блоков раскрывают `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`;ダイジェスト консенсуса と должен совпадать с локальным представлением レジストリ для принятия блока。
- ガバナンス、`next_conf_features` с будущим `activation_height`;ビデオのダイジェストをご覧ください。
- `confidential.enabled = true` および `assume_valid = false` を確認してください。 Стартовые проверки не допускают узел в набор валидаторов, если любое условие нарузено или локальные `conf_features` расходятся。
- P2P ハンドシェイクが `{ enabled, assume_valid, conf_features }` を取得しました。ピア、рекламирующие несовместимые возможности, отклоняются с `HandshakeConfidentialMismatch` и никогда не входят в ротацию консенсуса。
- オブザーバーとピアのハンドシェイク [ノードの機能]交渉](#node-capability-negotiation)。ハンドシェイクが `HandshakeConfidentialMismatch` とピアで行われ、ダイジェストが行われます。
- オブザーバー、 не являющиеся валидаторами, могут выставить `assume_valid = true`; они слепо применяют конфиденциальные дельты、но не влияют на безопасность консенсуса.## Политики активов
- `AssetConfidentialPolicy` のガバナンスを確認してください:
  - `TransparentOnly`: режим по умолчанию; (`MintAsset`、`TransferAsset` および т.д.)、シールドされた операции отклоняются。
  - `ShieldedOnly`: эмиссия и переводы должны использовать конфиденциальные инструкции; `RevealConfidential` запрещен, поэтому балансы никогда не становятся публичными.
  - `Convertible`: シールド付きのシールド付きシールド付きシールド付きシールドオン/オフランプ ниже。
- FSM を使用して、次の手順を実行します。
  - `TransparentOnly → Convertible` (シールド プール)。
  - `TransparentOnly → ShieldedOnly` (требует запланированного перехода и окна конверсии)。
  - `Convertible → ShieldedOnly` (обязательная минимальная задержка)。
  - `ShieldedOnly → Convertible` (нужен план миграции、чтобы シールド付きノート оставались тратимыми)。
  - `ShieldedOnly → TransparentOnly` は、シールド プールとガバナンスに関するメモを示します。
- ガバナンスに関する `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` ISI `ScheduleConfidentialPolicyTransition` および могут отменять запланированные изменения через `CancelConfidentialPolicyTransition`。 Валидация mempool гарантирует, что ни одна транзакция не пересекает высоту перехода, а включение падает детерминированно、если проверка политики изменилась бы посреди блока.
- Запланированные переходы применяются автоматически при открытии нового блока: когда высота входит в окно конверсии (для апгрейдов `ShieldedOnly`) или достигает `effective_height`、ランタイム обновляет `AssetConfidentialPolicy`、обновляет メタデータ `zk.policy`および очищает 保留中のエントリ。 `ShieldedOnly` のテスト、ランタイムの実行、実行предупреждение、оставляя прежний режим。
- ノブ `policy_transition_delay_blocks` および `policy_transition_window_blocks` を使用して、猶予期間、猶予期間を設定します。 конвертировать ノート вокруг переключения。
- `pending_transition.transition_id` 監査ハンドル。ガバナンス обязана цитировать его при финализации или отмене переходов, чтобы операторы могли коррелировать отчетыオン/オフランプ。
- `policy_transition_window_blocks` は умолчанию 720 (около 12 часов при времени блока 60 с)。ガバナンスを強化し、統治を強化します。
- Genesis マニフェスト、CLI および ожидающие など。入場料は、Лолитику во время исполнения、подтверждая、что каждая конфиденциальная инструкция разрезенаです。
- チェックリスト миграции — см. 「移行シーケンス」 は、マイルストーン M0 を実行します。

#### Мониторинг переходов через Torii`GET /v1/confidential/assets/{definition_id}/transitions`、чтобы проверить активную `AssetConfidentialPolicy`。 JSON ペイロード всегда включает канонический アセット ID、последнюю наблюдаемую высоту блока、`current_mode` политики、режим、 эффективный на этой высоте (окна конверсии временно отдают `Convertible`)、и ожидаемые идентификаторы параметров `vk_set_hash`/ポセイドン/ペダーセン。ガバナンスを強化する:

- `transition_id` — 監査ハンドル、`ScheduleConfidentialPolicyTransition`。
- `previous_mode`/`new_mode`。
- `effective_height`。
- `conversion_window` および вычисленный `window_open_height` (блок、где колельки должны начать конверсию для カットオーバー ShieldedOnly)。

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

Ответ `404` означает、что соответствующее определение актива не найдено. Когда переход не запланирован, поле `pending_transition` равно `null`.

### Малитина состояний политики| Текущий режим | Следующий режим | Предпосылки |効果的な高さ | Примечания |
|------------------|------------------|---------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
|透明のみ |コンバーチブル |ガバナンス レジストリ検証ツール/パラメータ。 `ScheduleConfidentialPolicyTransition` から `effective_height ≥ current_height + policy_transition_delay_blocks` です。 | Переход выполняется ровно на `effective_height`;シールド付きプールは最高です。                   | Путь по умолчанию для включения конфиденциальности при сохранении прозрачных потоков.               |
|透明のみ |シールド付きのみ | То же、что выге、плюс `policy_transition_window_blocks ≥ 1`。                                                         |ランタイムは `Convertible` または `effective_height - policy_transition_window_blocks` です。 `ShieldedOnly` または `effective_height` です。 | Лает детерминированное окно конверсии до отключения прозрачных инструкций。   |
|コンバーチブル |シールド付きのみ | Запланированный переход с `effective_height ≥ current_height + policy_transition_delay_blocks`。ガバナンスはメタデータを監査する必要があります (`transparent_supply == 0`)。ランタイムはカットオーバーです。 | Та же семантика окна、что выbolе。 `effective_height`、`PolicyTransitionPrerequisiteFailed` を確認してください。 | Закрепляет актив в полностью конфиденциальной циркуляции.                                     |
|シールド付きのみ |コンバーチブル | Запланированный переход;緊急撤退 (`withdraw_height` не задан)。                                    | Состояние переключается на `effective_height`;公開ランプは、シールドされたノートを表示します。                           | Используется для окон обслуживания или аудиторских проверок.                                          |
|シールド付きのみ |透明のみ |ガバナンスは `shielded_supply == 0` または `EmergencyUnshield` (нужны подписи аудиторов) です。 |ランタイムは `Convertible` と `effective_height`; на этой высоте конфиденциальные инструкции жестко проваливаются, и актив возвращается в режим толькоそうですね。 | Выход последней инстанции. Переход автоматически отменяется, если какая-либо конфиденциальная note расходуется в окне. |
|任意 |現在と同じ | `CancelConfidentialPolicyTransition` очищает ожидающее изменение。                                                        | `pending_transition` удаляется немедленно。                                                                          | Сохраняет статус-кво;そうですね。                                             |ガバナンスを強化する必要があります。ランタイムの実行時間は 1 時間です。 Лозвращает актив в предыдущий режим и отправляет `PolicyTransitionPrerequisiteFailed` в телеметрию и события блока.

### Последовательность миграции

1. **テスト結果:** 検証者とテストを実行し、テストを実行します。 `conf_features` は、ピアと同じです。
2. **Спланировать переход:** Отправить `ScheduleConfidentialPolicyTransition` с `effective_height`, учитывающей `policy_transition_delay_blocks`. При движении к `ShieldedOnly` указать окно конверсии (`window ≥ policy_transition_window_blocks`)。
3. **詳細情報:** オンランプ/オフランプの Runbook を確認します。 Кольки и аудиторы подписываются на `/v1/confidential/assets/{id}/transitions`、чтобы узнать высоту открытия окна.
4. **Применение окна:** Когда окно открывается、ランタイム переключает политику в `Convertible`, эмитирует `PolicyTransitionWindowOpened { transition_id }` およびガバナンスを強化する必要があります。
5. ** Финализировать или отменить:** На `effective_height` ランタイム проверяет предпосылки перехода (нулевое прозрачное) предложение、отсутствие 緊急出金、т.п.)。 Успех переключает политику в запроденный режим; `PolicyTransitionPrerequisiteFailed`、保留中の移行と оставляет политику без изменений。
6. **説明:** ガバナンス機能 (например、`asset_definition.v2`)、CLI ツールтребует `confidential_policy` はマニフェストを表示します。ジェネシスとジェネシスの両方を取得し、レジストリを確認してください。 Аалидаторов。

Новые сети, начинающие с включенной конфиденциальностью, кодируют желаемую политику непосредственно в genesis. Они все равно следуют чек-листу выbolе при изменении режимов после запуска, чтобы окна конверсии оставались детерминированными, а кольки успевали адаптироваться.

### Версионирование および активация Norito マニフェスト- 創世記は ДОЛЖНЫ включать `SetParameter` для кастомного ключа `confidential_registry_root` を明らかにします。ペイロード — Norito JSON、соответствующий `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: опускайте поле (`null`)、когда активных 検証者エントリ нет、иначе передайте 32-байтную hex-строку (`0x…`)、равную хэлу、вычисленному `compute_vk_set_hash` はマニフェストの検証者です。レジストリを確認し、レジストリを確認してください。
- オンワイヤー `ConfidentialFeatureDigest::conf_rules_version` のレイアウト マニフェスト。アップデートは、`Some(1)` と `iroha_config::parameters::defaults::confidential::RULES_VERSION` です。ルールセットの定義、説明、マニフェストの作成、およびマニフェストの作成。 `ConfidentialFeatureDigestMismatch` を確認してください。
- アクティブ化マニフェスト ДОЛЖНЫ связывать обновления レジストリ、изменения жизненного цикла параметров и переходы политик、чтобы ダイジェストСонсистентным:
  1. レジストリ (`Publish*`、`Set*Lifecycle`) を参照して、ダイジェストを確認します。 `compute_confidential_feature_digest` です。
  2. Эмитируйте `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})`、используя вычисленный хэл、чтобы отстающие ピア могли воссстановить корректный ダイジェスト、レジストリを確認してください。
  3. `ScheduleConfidentialPolicyTransition` 。ロシア政府の統治 `transition_id`;マニフェスト、カタログ、ランタイム。
  4. マニフェスト、SHA-256 およびダイジェスト、アクティベーション プランを作成します。 Операторы проверяют все три артефакта перед голосованием, чтобы избежать разделения сети.
- ロールアウト ロールアウト отложенного カットオーバー、 записоте целевую высоту в сопутствующий кастомный параметр (например, `custom.confidential_upgrade_activation_height`)。 Это дает аудиторам Norito-кодированное доказательство того, что валидаторы соблюли окно уведомления доダイジェストをご覧ください。## Жизненный цикл verifier и параметров
### ZK レジストリ
- 元帳 `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`、`proving_system` 、`Halo2`。
- Пары `(circuit_id, version)` глобально уникальны;レジストリは、回路を表示します。入場料を支払う必要があります。
- `circuit_id` не может быть пустым, а `public_inputs_schema_hash` обязателен (обычно Blake2b-32 хэл канонического публичного)検証者）。入場料は別途かかります。
- ガバナンスの概要:
  - `PUBLISH` для добавления `Proposed` エントリ только с メタデータ。
  - `ACTIVATE { vk_id, activation_height }` для планирования активации エントリ на границе эпохи。
  - `DEPRECATE { vk_id, deprecation_height }` для отметки последней высоты, где 証明 могут ссылаться на エントリ。
  - `WITHDRAW { vk_id, withdraw_height }` для экстренного отключения;高さを引き出し、エントリを削除します。
- Genesis マニフェスト автоматически эмитируют кастомный параметр `confidential_registry_root` с `vk_set_hash`、совпадающим с активными エントリ。レジストリからダイジェストを取得し、レジストリを確認してください。
- Регистрация или обновление 検証者 требует `gas_schedule_id`;レジストリ `Active`、`(circuit_id, version)`、Halo2 の証明`OpenVerifyEnvelope`、`circuit_id`、`vk_hash`、`public_inputs_schema_hash` のレジストリを参照してください。

### 鍵の証明
- 台帳外のキー、コンテンツアドレス指定されたキーの証明 (`pk_cid`、`pk_hash`、`pk_len`)、メタデータ検証ツールです。
- ウォレット SDK は、PK 日、およびバージョンをサポートしています。

### ペダーセンとポセイドンのパラメータ
- レジストリ (`PedersenParams`、`PoseidonParams`) のレジストリ жизненного цикла Verifier、каждая с `params_id`、 хэзами генераторов/констант、высотами активации、非推奨、撤回。
- コミットメントと、`params_id`、поэтому ротация параметров никогда не переиспользует битовые паттерны из устаревлих наборов; ID は、コミットメントのメモと доменные теги 無効化子です。

## Детерминированный порядок и nullifiers
- Каждый актив поддерживает `CommitmentTree` с `next_leaf_index`;あなたの約束を達成するために、次のことを考えてください。シールド出力は `output_idx` に対応しています。
- `note_position` выводится из оффсетов дерева, но **не** входит в nullifier;証明証人として会員資格を取得してください。
- Стабильность nullifier при reorg обеспечивается дизайном PRF; PRF は `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`、アンカーはマークル ルート、`max_anchor_age_blocks` を示します。## Поток 台帳
1. **MintConfidential { 資産 ID、金額、受信者ヒント }**
   - Требует политику актива `Convertible` または `ShieldedOnly`;入場権限 актива、извлекает текущий `params_id`、сэмплирует `rho`、эмитирует コミットメント、обновляет マークル ツリー。
   - `ConfidentialEvent::Shielded` のコミットメント、マークル ルート デルタ、ハッシュの監査証跡。
2. **TransferConfidential {asset_id、proof、circuit_id、version、nullifiers、new_commitments、enc_payloads、anchor_root、memo }**
   - Syscall VM はレジストリを証明します。ホスト убеждается、無効化子 не использованы、コミットメント добавлены детерминированно、アンカー свежий。
   - 元帳 `NullifierSet` エントリ、ペイロード для получателей/аудиторов и эмитирует `ConfidentialEvent::Transferred`、ヌリファイア、出力、証明ハッシュ、マークル ルートを使用します。
3. **RevealConfidential {asset_id、proof、circuit_id、version、nullifier、amount、recipient_account、anchor_root }**
   - Доступно только для активов `Convertible`;証明 подтверждает, что значение note равно раскрытой сумме, 元帳 начисляет прозрачный баланс и сжигает Shielded note, помечая nullifier как Потраченный。
   - Эмитирует `ConfidentialEvent::Unshielded` с публичной суммой、использованными 無効化子、идентификаторами 証明、ハッシュ вызова транзакции。

## Дополнения データ モデル
- `ConfidentialConfig` (новый раздел конфигурации) Єлагом включения、`assume_valid`、ノブ газа/лимитов、окном アンカーおよびバックエンド検証器。
- `ConfidentialNote`、`ConfidentialTransfer` および `ConfidentialMint` は、Norito および явным байтом версии (`CONFIDENTIAL_ASSET_V1 = 0x01`) です。
- `ConfidentialEncryptedPayload` оборачивает AEAD メモ バイト в `{ version, ephemeral_pubkey, nonce, ciphertext }`、по умолчанию `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` для раскладки XChaCha20-Poly1305。
- `docs/source/confidential_key_vectors.json` のキー導出を実行します。 CLI、Torii エンドポイントが表示されます。
- `asset::AssetDefinition` は `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` です。
- `ZkAssetState` は検証者を転送/シールド解除します。証明、参照、インライン検証キー、コミットメントの確認。
- `CommitmentTree` (フロンティアチェックポイント)、`NullifierSet` と `(chain_id, asset_id, nullifier)`、`ZkVerifierEntry`、`PedersenParams`、`PoseidonParams`世界国家。
- Mempool が、`NullifierIndex` と `AnchorIndex` を表示します。アンカー。
- Обновления схемы Norito включают канонический порядок public input;往復テストが必要です。
- 往復ペイロードの単体テスト (`crates/iroha_data_model/src/confidential.rs`)。 Векторные файлы кобавят канонические AEAD トランスクリプト для аудиторов。 `norito.md` オンワイヤ ヘッダーとエンベロープ。## Интеграция с IVM および syscall
- システムコール `VERIFY_CONFIDENTIAL_PROOF`、例:
  - `circuit_id`、`version`、`scheme`、`public_inputs`、`proof` および результирующий `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`。
  - Syscall のメタデータ ベリファイアとレジストリ、標準のガスとデルタ только の組み合わせ証拠です。
- ホストの読み取り専用特性 `ConfidentialLedger` のマークル ルートと無効化子。 Kotodama は、ヘルパーと目撃者、および監視者をサポートします。
- ポインター - ABI 、レイアウト証明バッファーおよびレジストリ ハンドル。

## Согласование возможностей узлов
- ハンドシェイク `feature_bits.confidential` と `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`。 `confidential.enabled=true`、`assume_valid=false`、バックエンド検証ツールとダイジェストを使用します。ハンドシェイクは `HandshakeConfidentialMismatch` です。
- Конфигурация поддерживает `assume_valid` только для オブザーバー узлов: когда выключено, встреча конфиденциальныхパニック、`UnsupportedInstruction`、パニック。 когда включено、オブザーバーは州のデルタを証明します。
- Mempool は、機能を強化します。ゴシップ フィルター、シールドされたピア、検証者 ID の確認ぱっつん。

### Матрица совместимости 握手

| Объявление удаленной стороны | Итог для валидаторских узлов | Примечания оператора |
|----------------------------|------------------------------|----------------------------|
| `enabled=true`、`assume_valid=false`、バックエンド、ダイジェスト | Принят |ピア `Ready` 、提案、投票、RBC ファンアウトを確認してください。 Ручных действий не требуется。 |
| `enabled=true`、`assume_valid=false`、バックエンド レポート、ダイジェスト отсутствует | Отклонен (`HandshakeConfidentialMismatch`) |リモートのレジストリ/レジストリ/レジストリ/レジストリ/アクセス許可 `activation_height`。 Пока не исправлено, узел остается обнаруживаемым, но не входит в ротацию консенсуса. |
| `enabled=true`、`assume_valid=true` | Отклонен (`HandshakeConfidentialMismatch`) | Валидаторы требуют проверку 証明;リモート ケーブル オブザーバーは、入力側から Torii を受信し、`assume_valid=false` を受信します。 |
| `enabled=false`、ハンドシェイク отсутствуют (старый билд) およびバックエンド ベリファイア отличается | Отклонен (`HandshakeConfidentialMismatch`) | Устаревлие или частично обновленные ピア не могут войти в сеть консенсуса.バックエンド + ダイジェストを確認できます。 |

オブザーバー узлы、намеренно пропускающие проверкуproof、не должны открывать консенсусные соедидаторами、機能ゲートの追加。 Они могут принимать блоки через Torii или архивные API, но сеть консенсуса отвергает их, пока они не объявят совместимые возможности。

### Политика удаления 明らかにする、保持を無効にする台帳の管理、管理、管理、メモの管理、メモの作成、管理ガバナンス-аудитов。 Политика по умолчанию, применяемая `ConfidentialLedger`, такова:

- **無効化子:** 無効化子 минимум `730` дней (24 месяца) после высоты траты или дользе, если требуется регулятором。 Операторы могут увеличить окно через `confidential.retention.nullifier_days`.無効化子、которые моложе окна、ДОЛЖНЫ оставаться доступными через Torii、чтобы аудиторы могли доказать二重支出になります。
- **明らかにする:** 明らかにする (`RevealConfidential`) удаляют соответствующие コミットメント немедленно после финализации блока, но использованный nullifier остается под правилом 保持機能。明らかにする (`ConfidentialEvent::Unshielded`) фиксируют публичную сумму, получателя и hashproof, чтобы реконструкция исторических 明らかにするтребовала удаленного 暗号文。
- **フロンティア チェックポイント:** ローリング チェックポイント、`max_anchor_age_blocks` および окна の保持に関するコミットメント。これは、チェックポイントと無効化子を組み合わせたものです。
- **ダイジェスト:** если `HandshakeConfidentialMismatch` поднят из-за дрейфа ダイジェスト、операторам следует (1) проверить、что окна 保持無効化совпадают по кластеру、(2) запустить `iroha_cli app confidential verify-ledger` для пересчета ダイジェスト по сохраненному набору nullifier、および (3) переразвернуть обновленный マニフェスト。 Любые nullifiers、удаленные раньге срока、должны быть восстановлены из холодного хранения перед повторным входом вさいしょ。

運用ランブックの詳細。ガバナンス - политики、расbolиряющие окно 保持、должны синхронно обновлять конфигурацию узлов и планы архивного хранения。

### Процедура эвикции и восстановления

1. `IrohaNetwork` にダイヤルして、機能を確認します。 Любое несоответствие поднимает `HandshakeConfidentialMismatch`;ピアとディスカバリ キュー、`Ready` を接続します。
2. Ологе сетевого сервиса (включая удаленный ダイジェスト и バックエンド)、Sumeragi никогда не планирует ピア 提案または投票。
3. レジストリとレジストリ (`vk_set_hash`、`pedersen_params_id`、 `poseidon_params_id`) は、`next_conf_features` と согласованной `activation_height` です。ダイジェスト版、ハンドシェイク版などをご覧ください。
4. Если устаревлать блок (например、через архивный 再生)、валидаторы детерминированно отклоняют его `BlockRejectionReason::ConfidentialFeatureDigestMismatch`、元帳を確認してください。

### リプレイセーフなハンドシェイク フロー1. Noise/X25519 を使用してください。ハンドシェイク ペイロード (`handshake_signature_payload`) の一時公開キー、Norito-кодированный アドバタイズされたソケット アドレス — если скомпилировано с `handshake_chain_id` — идентификатор チェーン。 AEAD からは отправкой が見つかります。
2. ペイロードを受信し、ピア/ローカルと接続し、Ed25519、接続します。 `HandshakeHelloV1`。一時的なキーとアドバタイズされたアドレスを確認し、再実行して、ピアを再生します。 устаревливает валидацию をご覧ください。
3. 機密機能フラグ `ConfidentialFeatureDigest` および `HandshakeConfidentialMeta`。タプル `{ enabled, assume_valid, verifier_backend, digest }` を使用して、タプル `ConfidentialHandshakeCaps` を作成します。ハンドシェイクは `HandshakeConfidentialMismatch` と `Ready` で行われます。
4. ダイジェスト (`compute_confidential_feature_digest`) とレジストリ/ポリシーの確認そうです。ピア、ダイジェスト、ハンドシェイク、セッション、コミュニケーションАалидаторов。
5. ハンドシェイク обновляют стандартные счетчики `iroha_p2p::peer` (`handshake_failure_count`、エラー分類ヘルパー) および эмитятリモート ピア ID とフィンガープリント ダイジェストを確認できます。ロールアウトを実行すると、リプレイが表示されます。

## Управление ключами и ペイロード
- 派生クラス:
  - `sk_spend` → `nk` (無効化キー)、`ivk` (受信表示キー)、`ovk` (送信表示キー)、`fvk`。
- ペイロードに関するメモ、AEAD および ECDH 由来の共有キー。監査ビューのキーと出力を確認できます。
- CLI: `confidential create-keys`、`confidential send`、`confidential export-view-key`、аудит-инструменты для расзифровки メモおよびヘルパー `iroha app zk envelope` 日создания/инспекции Norito メモ封筒 офлайн。 Torii フローの導出、`POST /v1/confidential/derive-keyset`、16 進数、base64 の数値、чтобы кольки моглиあなたのことを考えてください。## Газ、лимиты、DoS-контроли
- ロシアのガススケジュール:
  - Halo2 (Plonkish): `250_000` ガス + `2_000` ガス - каждый パブリック入力。
  - `5` ガスの安全性、無効化子ごと (`300`) およびコミットメントごと (`500`) の機能。
  - Операторы могут переопределять эти константы через конфигурацию узла (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`);ホットリロード機能が必要な場合は、ホットリロード機能を使用してください。
- Жесткие лимиты (настраиваемые значения по умолчанию):
- `max_proof_size_bytes = 262_144`。
- `max_nullifiers_per_tx = 8`、`max_commitments_per_tx = 8`、`max_confidential_ops_per_block = 256`。
- `verify_timeout_ms = 750`、`max_anchor_age_blocks = 10_000`。証明、 превылосованно прерывают инструкцию (ガバナンス-голосования эмитят `proof verification exceeded timeout`, `VerifyProof` возвращает озибку)。
- 説明: `max_proof_bytes_block`、`max_verify_calls_per_tx`、`max_verify_calls_per_block`、および `max_public_inputs`ブロックビルダー。 `reorg_depth_bound` (≥ `max_anchor_age_blocks`) 保持フロンティア チェックポイントを確認します。
- ランタイムは、トランザクションごと、またはブロックごとに実行され、実行時に実行されます。 `InvalidParameter` は台帳です。
- Mempool は、`vk_id` を証明し、アンカーを検証し、証明します。 ограничить потребление ресурсов.
- タイムアウトが発生した後、タイムアウトが発生しました。 транзакции падают с явными обками. SIMD バックエンドは会計処理をサポートします。

### Базовые калибровки и критерии приемки
- **参照プラットフォーム** Калибровочные прогоны ДОЛЖНЫ покрывать три профиля оборудования ниже。 Прогоны без всех профилей отклоняются при ревью.

  | Профиль | Архитектура | CPU / インスタンス | Флаги компилятора | Назначение |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) または Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Устанавливает базовые значения без векторных инструкций;フォールバックコストテーブルを参照してください。 |
  | `baseline-avx2` | `x86_64` |インテル Xeon ゴールド 6430 (24c) |デフォルトのリリース | AVX2 をダウンロードします。 проверяет、что SIMD ускорения укладываются в допуск нейтрального ガス。 |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) |デフォルトのリリース | Гарантирует、что バックエンド NEON остается детерминированным и согласован с x86 расписаниями. |

- **ベンチマーク ハーネス** テストを実行:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` は、ждтверждения детерминированной фикстуры です。
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` всякий раз、когда меняются стоимости VM オペコード。

- **ランダム性を修正しました。** Экспортируйте `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` перед запуском бенчей, чтобы `iroha_test_samples::gen_account_in` переключился на детерминированный путь `KeyPair::from_seed`。ハーネス печатает `IROHA_CONF_GAS_SEED_ACTIVE=…` один раз; если переменная отсутствует, ревью ДОЛЖНО провалиться. Любые новые утилиты калибровки должны продолжать учитывать эту env var при введении вспомогательной случайности.- **結果のキャプチャ。**
  - 基準概要 (`target/criterion/**/raw.csv`) は、アーティファクトをリリースします。
  - Храните производные метрики (`ns/op`, `gas/op`, `ns/gas`) в [機密ガス校正台帳](./confidential-gas-calibration) вместе с git commit и версией компилятора。
  - ベースラインを確認します。あなたの人生は、最高です。

- **許容誤差**
  - `baseline-simd-neutral` および `baseline-avx2` ДОЛЖНЫ оставаться ≤ ±1.5%。
  - `baseline-simd-neutral` および `baseline-neon` ДОЛЖНЫ оставаться ≤ ±2.0%。
  - Калибровочные предложения, выходящие за эти пороги, требуют либо корректировки расписания, либо RFC с объяснением расхождения и мерами.

- **チェックリストを確認します。** 手順:
  - Включение `uname -a`、выдержек `/proc/cpuinfo` (モデル、ステップ) および `rustc -Vv` в калибровочный лог.
  - Проверку、что `IROHA_CONF_GAS_SEED` отображается выводе бенчей (бенчи печатают активный シード)。
  - Убедиться、что ペースメーカーおよび機密検証機能フラグ、生産性 (`--features confidential,telemetry` およびテレメトリ)。

## Конфигурация и операции
- `iroha_config` は `[confidential]` を表示します:
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
- 説明: `confidential_proof_verified`、`confidential_verifier_latency_ms`、`confidential_proof_bytes_total`、`confidential_nullifier_spent`、`confidential_commitments_appended`、 `confidential_mempool_rejected_total{reason}`、`confidential_policy_transitions_total`、平文です。
- RPC サーフェス:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## Стратегия тестирования
- 表示: マークル ルートとヌリファイア セットを表示します。
- Устойчивость к reorg: симуляция многоблочных reorg с アンカー;無効化子は стабильными、アンカーは отклоняются です。
- Инварианты газа: одинаковый расход газа на узлах с и без SIMD ускорения.
- 説明: 証明/ガス、強制タイムアウト。
- Жизненный цикл: ガバナンス/検証者、検証者。
- ポリシー FSM: 保留中の移行およびメンプールの有効高さ。
- レジストリ: 緊急出金 замораживает затронутые активы на `withdraw_height` и отклоняет の証明が必要です。
- 能力ゲーティング: валидаторы с несовпадающими `conf_features` отклоняют блоки;観測者は `assume_valid=true` を確認し、観測を開始します。
- Эквивалентность состояния: バリデータ/フル/オブザーバーの状態ルートを確認します。
- Негативный ファジング: 証明、特大ペイロード、衝突ヌルファイア отклоняются。## Миграция и совместимость
- 機能ゲート型ロールアウト: フェーズ C3 `enabled` と `false`。さまざまな機能が備わっています。
- Прозрачные активы не затронуты;レジストリと機能を備えています。
- Узлы、собранные без поддержки confidential、детерминированно отклоняют соответствующие блоки;観測者は `assume_valid=true` です。
- Genesis マニフェストは、レジストリ エントリ、監査キー、監査キーをマニフェストします。
- レジストリのランブック、緊急撤退、緊急撤退のタスクを実行します。 детерминированные апгрейды。

## Незаверсенная работа
- Halo2 のベンチマーク (回路、ルックアップ) とキャリブレーション プレイブック、ガス/タイムアウトの評価`confidential_assets_calibration.md` を参照してください。
- 選択的表示 API の機能、Torii のワークフローの選択統治。
- 複数の受信者の出力とバッチ化されたメモ、SDK 実装者のエンベロープの暗号化を監視します。
- セキュリティ レビュー回路、レジストリ、監査レポートを確認できます。
- API の支出状況、ビュー キー スコープ、ガイダンスなどを確認できます。 семантики аттестации.## Этапы реализации
1. **フェーズ M0 — 出荷時の硬化**
   - ✅ 導出無効化子を使用して、ポセイドン PRF (`nk`、`rho`、`asset_id`、`chain_id`) を使用します。台帳とコミットメント。
   - ✅ トランザクションごと/ブロックごとの証明と証明、トランザクションごと/ブロックごとの証明最高です。
   - ✅ P2P ハンドシェイク объявляет `ConfidentialFeatureDigest` (バックエンド ダイジェスト + отпечатки レジストリ) および детерминированно отклоняет несоответствия через `HandshakeConfidentialMismatch`。
   - ✅ パニックと実行パス、ロール ゲートの制御。
   - ⚪ タイムアウト、検証者、境界、フロンティア チェックポイントの再編成を行います。
     - ✅ タイムアウト時間。証明、`verify_timeout_ms`、теперь детерминированно падают。
     - ✅ フロンティアチェックポイント теперь учитывают `reorg_depth_bound`, удаляя チェックポイント старле заданного окна при сохранении детерминированных снимков.
   - Ввести `AssetConfidentialPolicy`、ポリシー FSM および施行ゲートのミント/転送/公開。
   - `conf_features` とレジストリ/パラメータ ダイジェストを確認します。
2. **フェーズ M1 — レジストリとパラメータ**
   - レジストリ `ZkVerifierEntry`、`PedersenParams`、`PoseidonParams` は、ガバナンス オペレーション、ジェネシス アンカリング、およびそれらを制御します。
   - レジストリ検索、ガス スケジュール ID、スキーマ ハッシュ、サイズ チェックなどのシステムコールを実行します。
   - ペイロード v1 を取得し、機密キーを取得します。
3. **フェーズ M2 — ガスとパフォーマンス**
   - ガス スケジュール、ブロックごとのベンチマーク ハーネスを確認します (レイテンシ、プルーフ サイズ、メモリプールの拒否を確認)。
   - CommitmentTree、LRU ロード、Nullifier インデックスのチェックポイントを確認します。
4. **フェーズ M3 — ローテーションとウォレット ツール**
   - マルチパラメータとマルチバージョンの証明を提供します。ガバナンス主導のアクティブ化/非推奨と移行ランブックを示します。
   - ウォレット SDK/CLI、ワークフロー、ツールの支出を管理します。
5. **フェーズ M4 — 監査と運用**
   - 監査キー、選択的開示 API、運用ランブックなどのワークフローを管理します。
   - Запланировать внезоды в `status.md`.

ロードマップのマイルストーンとブロックチェーンの重要性を確認します。
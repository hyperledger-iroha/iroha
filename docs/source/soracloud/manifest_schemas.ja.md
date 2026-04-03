<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 マニフェスト スキーマ

このページでは、SoraCloud の最初の決定論的な Norito スキーマを定義します
Iroha 3 での展開:

- `SoraContainerManifestV1`
- `SoraServiceManifestV1`
- `SoraStateBindingV1`
- `SoraDeploymentBundleV1`
- `AgentApartmentManifestV1`
- `FheParamSetV1`
- `FheExecutionPolicyV1`
- `FheGovernanceBundleV1`
- `FheJobSpecV1`
- `DecryptionAuthorityPolicyV1`
- `DecryptionRequestV1`
- `CiphertextQuerySpecV1`
- `CiphertextQueryResponseV1`
- `SecretEnvelopeV1`
- `CiphertextStateRecordV1`

Rust の定義は `crates/iroha_data_model/src/soracloud.rs` にあります。

アップロードされたモデルのプライベート ランタイム レコードは、意図的に、
これらの SCR 導入マニフェスト。 Soracloud モデルプレーンを拡張する必要があります
暗号化されたバイトには `SecretEnvelopeV1` / `CiphertextStateRecordV1` を再利用します
新しいサービス/コンテナとしてエンコードされるのではなく、暗号文ネイティブの状態
が現れます。 `uploaded_private_models.md`を参照してください。

## 範囲

これらのマニフェストは、`IVM` + カスタム Sora Container Runtime 用に設計されています。
(SCR) 方向 (WASM なし、ランタイム アドミッションにおける Docker 依存性なし)。- `SoraContainerManifestV1` は、実行可能バンドル ID、ランタイム タイプ、
  機能ポリシー、リソース、ライフサイクル プローブ設定、および明示的
  required-config はランタイム環境またはマウントされたリビジョンにエクスポートします
  木。
- `SoraServiceManifestV1` は展開意図をキャプチャします: サービス ID、
  参照されたコンテナマニフェストのハッシュ/バージョン、ルーティング、ロールアウトポリシー、
  状態バインディング。
- `SoraStateBindingV1` は、確定的な状態書き込みの範囲と制限をキャプチャします。
  (名前空間プレフィックス、可変性モード、暗号化モード、項目/合計クォータ)。
- `SoraDeploymentBundleV1` はコンテナーとサービスのマニフェストを結合して強制します
  決定論的なアドミッションチェック（マニフェストハッシュリンケージ、スキーマアラインメント、
  能力/バインディングの一貫性)。
- `AgentApartmentManifestV1` は永続的なエージェント ランタイム ポリシーをキャプチャします。
  ツールの上限、ポリシーの上限、支出制限、状態クォータ、ネットワーク下り、および
  動作をアップグレードします。
- `FheParamSetV1` は、ガバナンス管理された FHE パラメータ セットをキャプチャします。
  決定論的なバックエンド/スキーム識別子、モジュラスプロファイル、セキュリティ/深さ
  境界、およびライフサイクルの高さ (`activation`/`deprecation`/`withdraw`)。
- `FheExecutionPolicyV1` は、決定論的な暗号文の実行制限をキャプチャします。
  許容ペイロード サイズ、入出力ファンイン、深さ/回転/ブートストラップ キャップ、
  そして正規の丸めモード。
- `FheGovernanceBundleV1` は、決定論的なパラメータ セットとポリシーを結合します。
  入場認証。- `FheJobSpecV1` は、決定論的な暗号文ジョブの受付/実行をキャプチャします。
  リクエスト: オペレーション クラス、順序付けされた入力コミットメント、出力キー、および制限付き
  深さ/回転/ブートストラップの要求は、ポリシー + パラメーター セットにリンクされます。
- `DecryptionAuthorityPolicyV1` は、ガバナンス管理の開示ポリシーをキャプチャします。
  権限モード (クライアント保持 vs しきい値サービス)、承認者のクォーラム/メンバー、
  ブレークガラスの許可、管轄区域のタグ付け、同意の証拠要件、
  TTL 境界と正規の監査タグ付け。
- `DecryptionRequestV1` は、ポリシーにリンクされた開示の試みをキャプチャします。
  暗号文キー参照 (`binding_name` + `state_key` + コミットメント)、
  正当化、管轄タグ、オプションの同意証拠ハッシュ、TTL、
  ブレークグラスの意図/理由、およびガバナンス ハッシュのリンク。
- `CiphertextQuerySpecV1` は、決定論的な暗号文のみのクエリの意図をキャプチャします。
  サービス/バインディング スコープ、キー プレフィックス フィルター、制限された結果制限、メタデータ
  投影レベルと証明の包含の切り替え。
- `CiphertextQueryResponseV1` は、開示を最小限に抑えたクエリ出力をキャプチャします。
  ダイジェスト指向のキー参照、暗号文メタデータ、オプションの包含証明、
  応答レベルの切り捨て/シーケンス コンテキスト。
- `SecretEnvelopeV1` は、暗号化されたペイロード素材自体をキャプチャします。
  暗号化モード、キー識別子/バージョン、ノンス、暗号文バイト、および
  誠実さへの取り組み。
- `CiphertextStateRecordV1` は、暗号文ネイティブ状態のエントリをキャプチャします。パブリック メタデータ (コンテンツ タイプ、ポリシー タグ、コミットメント、ペイロード サイズ) を結合する
  `SecretEnvelopeV1` です。
- ユーザーがアップロードしたプライベート モデル バンドルは、これらの暗号文ネイティブに基づいて構築する必要があります。
  記録:
  暗号化された重み/構成/プロセッサ チャンクは状態に存在し、モデル レジストリは、
  重みリネージ、コンパイルプロファイル、推論セッション、チェックポイントは残ります
  一流のソラクラウドレコード。

## バージョン管理

- `SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
- `SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
- `SORA_STATE_BINDING_VERSION_V1 = 1`
- `SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
- `AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
- `FHE_PARAM_SET_VERSION_V1 = 1`
- `FHE_EXECUTION_POLICY_VERSION_V1 = 1`
- `FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
- `FHE_JOB_SPEC_VERSION_V1 = 1`
- `DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
- `DECRYPTION_REQUEST_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
- `SECRET_ENVELOPE_VERSION_V1 = 1`
- `CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

検証では、サポートされていないバージョンが拒否されます。
`SoraCloudManifestError::UnsupportedVersion`。

## 決定的検証ルール (V1)- コンテナマニフェスト:
  - `bundle_path` および `entrypoint` は空であってはなりません。
  - `healthcheck_path` (設定されている場合) は `/` で始まる必要があります。
  - `config_exports` は、で宣言された構成のみを参照できます。
    `required_config_names`。
  - config-export env ターゲットは正規の環境変数名を使用する必要があります
    (`[A-Za-z_][A-Za-z0-9_]*`)。
  - config-export ファイルのターゲットは相対的なものでなければならず、`/` 区切り文字を使用する必要があります。
    空の `.` または `..` セグメントを含めることはできません。
  - 構成エクスポートは、同じ環境変数または相対ファイル パスをターゲットにしてはなりません。
    一度よりも。
- サービスマニフェスト:
  - `service_version` は空であってはなりません。
  - `container.expected_schema_version` はコンテナー スキーマ v1 と一致する必要があります。
  - `rollout.canary_percent` は `0..=100` である必要があります。
  - `route.path_prefix` (設定されている場合) は `/` で始まる必要があります。
  - 状態バインディング名は一意である必要があります。
- 状態バインディング:
  - `key_prefix` は空ではなく、`/` で始まる必要があります。
  - `max_item_bytes <= max_total_bytes`。
  - `ConfidentialState` バインディングでは平文暗号化を使用できません。
- 導入バンドル:
  - `service.container.manifest_hash` は正規のエンコードされたものと一致する必要があります
    コンテナマニフェストのハッシュ。
  - `service.container.expected_schema_version` はコンテナー スキーマと一致する必要があります。
  - 可変状態バインディングには `container.capabilities.allow_state_writes=true` が必要です。
  - パブリックルートには `container.lifecycle.healthcheck_path` が必要です。
- エージェントのアパートマニフェスト:
  - `container.expected_schema_version` はコンテナー スキーマ v1 と一致する必要があります。
  - ツール機能名は空であってはならず、一意である必要があります。- ポリシー機能名は一意である必要があります。
  - 支出制限アセットは空ではなく、一意である必要があります。
  - 各支出制限の `max_per_tx_nanos <= max_per_day_nanos`。
  - ホワイトリスト ネットワーク ポリシーには、空ではない一意のホストが含まれている必要があります。
- FHEパラメータセット：
  - `backend` および `ciphertext_modulus_bits` は空であってはなりません。
  - 各暗号文モジュラスのビットサイズは `2..=120` 以内である必要があります。
  - 暗号文のモジュラスチェーンの次数は増加しない必要があります。
  - `plaintext_modulus_bits` は、最大の暗号文係数より小さくなければなりません。
  - `slot_count <= polynomial_modulus_degree`。
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`。
  - ライフサイクルの高さの順序は厳密である必要があります。
    `activation < deprecation < withdraw` (存在する場合)。
  - ライフサイクル ステータスの要件:
    - `Proposed` は非推奨/取り消しの高さを許可しません。
    - `Active` には `activation_height` が必要です。
    - `Deprecated` には `activation_height` + `deprecation_height` が必要です。
    - `Withdrawn` には `activation_height` + `withdraw_height` が必要です。
- FHE 実行ポリシー:
  - `max_plaintext_bytes <= max_ciphertext_bytes`。
  - `max_output_ciphertexts <= max_input_ciphertexts`。
  - パラメータセットバインディングは `(param_set, version)` までに一致する必要があります。
  - `max_multiplication_depth` はパラメータセットの深さを超えてはなりません。
  - ポリシーアドミッションは、`Proposed` または `Withdrawn` パラメータセットのライフサイクルを拒否します。
- FHE ガバナンス バンドル:
  - ポリシーとパラメータ セットの互換性を 1 つの決定論的なアドミッション ペイロードとして検証します。
- FHE ジョブ仕様:
  - `job_id` および `output_state_key` は空であってはなりません (`output_state_key` は `/` で始まります)。- 入力セットは空であってはならず、入力キーは一意の正規パスである必要があります。
  - 操作固有の制約が厳しい (`Add`/`Multiply` 複数入力、
    `RotateLeft`/`Bootstrap` シングル入力、相互に排他的な深さ/回転/ブートストラップ ノブ付き)。
  - ポリシーにリンクされたアドミッションでは、以下が強制されます。
    - ポリシー/パラメータ識別子とバージョンが一致する。
    - 入力数/バイト、深さ、回転、およびブートストラップの制限はポリシーの上限内にあります。
    - 決定的に予測される出力バイトがポリシーの暗号文制限に適合します。
- 復号化権限ポリシー:
  - `approver_ids` は空ではなく、一意で、厳密に辞書順にソートされている必要があります。
  - `ClientHeld` モードには、`approver_quorum=1` という 1 人の承認者が必要です。
    そして`allow_break_glass=false`。
  - `ThresholdService` モードでは、少なくとも 2 人の承認者が必要です。
    `approver_quorum <= approver_ids.len()`。
  - `jurisdiction_tag` は空であってはならず、制御文字を含めることはできません。
  - `audit_tag` は空であってはならず、制御文字を含めることはできません。
- 復号化リクエスト:
  - `request_id`、`state_key`、および `justification` は空であってはなりません
    (`state_key` は `/` で始まります)。
  - `jurisdiction_tag` は空であってはならず、制御文字を含めることはできません。
  - `break_glass_reason` は、`break_glass=true` の場合は必須ですが、`break_glass=true` の場合は省略する必要があります。
    `break_glass=false`。
  - ポリシーにリンクされたアドミッションはポリシー名の等価性を強制しますが、要求 TTL は要求しません。`policy.max_ttl_blocks` を超える、管轄タグの平等、ブレークグラス
    ゲーティング、および同意証拠の要件
    `policy.require_consent_evidence=true` (ガラス破り以外のリクエストの場合)。
- 暗号文クエリ仕様:
  - `state_key_prefix` は空ではなく、`/` で始まる必要があります。
  - `max_results` は確定的に制限されています (`<=256`)。
  - メタデータ プロジェクションは明示的です (`Minimal` ダイジェストのみであるのに対し、`Standard` キーは表示されます)。
- 暗号文クエリ応答:
  - `result_count` はシリアル化された行数と等しくなければなりません。
  - `Minimal` 投影は `state_key` を公開してはなりません。 `Standard` はそれを公開する必要があります。
  - 行は平文暗号化モードを決して表面化してはなりません。
  - 包含証明 (存在する場合) には空ではないスキーム ID が含まれている必要があります。
    `anchor_sequence >= event_sequence`。
- 秘密の封筒:
  - `key_id`、`nonce`、および `ciphertext` は空であってはなりません。
  - ノンスの長さは制限されています (`<=256` バイト)。
  - 暗号文の長さは制限されています (`<=33554432` バイト)。
- 暗号文状態レコード:
  - `state_key` は空ではなく、`/` で始まる必要があります。
  - メタデータ コンテンツ タイプは空であってはなりません。タグは空でない一意の文字列である必要があります。
  - `metadata.payload_bytes` は `secret.ciphertext.len()` と等しくなければなりません。
  - `metadata.commitment` は `secret.commitment` と等しくなければなりません。

## 正規のフィクスチャ

正規の JSON フィクスチャは次の場所に保存されます。- `fixtures/soracloud/sora_container_manifest_v1.json`
- `fixtures/soracloud/sora_service_manifest_v1.json`
- `fixtures/soracloud/sora_state_binding_v1.json`
- `fixtures/soracloud/sora_deployment_bundle_v1.json`
- `fixtures/soracloud/agent_apartment_manifest_v1.json`
- `fixtures/soracloud/fhe_param_set_v1.json`
- `fixtures/soracloud/fhe_execution_policy_v1.json`
- `fixtures/soracloud/fhe_governance_bundle_v1.json`
- `fixtures/soracloud/fhe_job_spec_v1.json`
- `fixtures/soracloud/decryption_authority_policy_v1.json`
- `fixtures/soracloud/decryption_request_v1.json`
- `fixtures/soracloud/ciphertext_query_spec_v1.json`
- `fixtures/soracloud/ciphertext_query_response_v1.json`
- `fixtures/soracloud/secret_envelope_v1.json`
- `fixtures/soracloud/ciphertext_state_record_v1.json`

フィクスチャ/ラウンドトリップ テスト:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
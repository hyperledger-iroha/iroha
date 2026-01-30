---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b52a39f15408705f7633f315eb6e5daae049f57ba2a06e908d81551d6fa94a22
source_last_modified: "2025-11-07T10:33:21.926309+00:00"
translation_last_reviewed: 2026-01-30
---

> 次の文書を基に作成: [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# SoraFS プロバイダー受け入れ・アイデンティティポリシー (SF-2b ドラフト)

このノートは **SF-2b** の実行可能な成果物を整理する: SoraFS ストレージプロバイダーの
受け入れワークフロー、アイデンティティ要件、アテステーション・ペイロードを定義して
強制すること。SoraFS Architecture RFC で示した高レベルのプロセスを拡張し、残りの作業を
追跡可能なエンジニアリングタスクに分解する。

## ポリシーの目的

- ネットワークが受け入れる `ProviderAdvertV1` レコードは、審査済みオペレーターのみが公開できるようにする。
- 各広告鍵を、ガバナンス承認の身元文書、アテスト済みエンドポイント、最小ステーク拠出に結び付ける。
- Torii、ゲートウェイ、`sorafs-node` が同じチェックを適用できるよう、決定論的な検証ツールを提供する。
- 決定性やツールの使い勝手を壊さずに更新と緊急失効をサポートする。

## アイデンティティとステーク要件

| 要件 | 説明 | 成果物 |
|------|------|--------|
| 広告鍵の来歴 | プロバイダーは各 advert に署名する Ed25519 鍵ペアを登録する必要がある。受け入れバンドルはガバナンス署名と並べて公開鍵を保存する。 | `ProviderAdmissionProposalV1` スキーマを `advert_key` (32 bytes) で拡張し、レジストリ (`sorafs_manifest::provider_admission`) から参照する。 |
| ステークポインタ | 受け入れには、有効な staking pool を指す非ゼロの `StakePointer` が必要。 | `sorafs_manifest::provider_advert::StakePointer::validate()` に検証を追加し、CLI/テストでエラーを表面化する。 |
| 管轄タグ | プロバイダーは管轄と法的連絡先を宣言する。 | 提案スキーマに `jurisdiction_code` (ISO 3166-1 alpha-2) と任意の `contact_uri` を追加する。 |
| エンドポイント・アテステーション | 各広告エンドポイントは mTLS または QUIC の証明書レポートで裏付ける必要がある。 | Norito ペイロード `EndpointAttestationV1` を定義し、受け入れバンドル内でエンドポイントごとに保存する。 |

## 受け入れワークフロー

1. **提案作成**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     を追加し、`ProviderAdmissionProposalV1` とアテステーション・バンドルを生成する。
   - 検証: 必須フィールド、stake > 0、`profile_id` 内の正規 chunker handle を保証する。
2. **ガバナンス承認**
   - 評議会は既存の envelope ツール (`sorafs_manifest::governance` モジュール) を使って
     `blake3("sorafs-provider-admission-v1" || canonical_bytes)` を署名する。
   - envelope は `governance/providers/<provider_id>/admission.json` に永続化される。
3. **レジストリ取り込み**
   - Torii/ゲートウェイ/CLI が再利用できる共有バリデータ
     (`sorafs_manifest::provider_admission::validate_envelope`) を実装する。
   - Torii の受け入れ経路を更新し、digest または期限が envelope と異なる advert を拒否する。
4. **更新と失効**
   - 任意のエンドポイント/ステーク更新を持つ `ProviderAdmissionRenewalV1` を追加する。
   - 失効理由を記録してガバナンスイベントを送る `--revoke` の CLI 経路を公開する。

## 実装タスク

| 領域 | タスク | Owner(s) | 状態 |
|------|--------|----------|------|
| スキーマ | `crates/sorafs_manifest/src/provider_admission.rs` 配下に `ProviderAdmissionProposalV1`、`ProviderAdmissionEnvelopeV1`、`EndpointAttestationV1` (Norito) を定義する。`sorafs_manifest::provider_admission` に検証ヘルパー付きで実装済み。【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ 完了 |
| CLI ツール | `sorafs_manifest_stub` に `provider-admission proposal`、`provider-admission sign`、`provider-admission verify` のサブコマンドを追加する。 | Tooling WG | ✅ 完了 |

CLI フローは中間証明書バンドル (`--endpoint-attestation-intermediate`) を受け付け、
正規の提案/ envelope bytes を出力し、`sign`/`verify` 中に評議会署名を検証する。運用者は
advert 本文を直接渡すことも署名済み advert を再利用することもでき、署名ファイルは
`--council-signature-public-key` と `--council-signature-file` を組み合わせて指定できるため、
自動化に向いたフローになっている。

### CLI リファレンス

各コマンドは `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` で実行する。

- `proposal`
  - 必須フラグ: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, および少なくとも 1 つの `--endpoint=<kind:host>`.
  - エンドポイントごとのアテステーションには `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, `--endpoint-attestation-leaf=<path>` による証明書
    (各チェーン要素の任意の `--endpoint-attestation-intermediate=<path>` を含む) と、交渉済みの ALPN ID
    (`--endpoint-attestation-alpn=<token>`) が必要。QUIC エンドポイントは
    `--endpoint-attestation-report[-hex]=...` でトランスポートレポートを渡せる。
  - 出力: Norito の正規提案バイト (`--proposal-out`) と JSON 概要
    (デフォルト stdout または `--json-out`).
- `sign`
  - 入力: 提案 (`--proposal`)、署名済み advert (`--advert`)、任意の advert 本文 (`--advert-body`)、保持 epoch、
    そして最低 1 つの評議会署名。署名は inline (`--council-signature=<signer_hex:signature_hex>`)
    でも、`--council-signature-public-key` と `--council-signature-file=<path>` を組み合わせたファイル指定でもよい。
  - 検証済み envelope (`--envelope-out`) と、digest の結び付き、署名者数、入力パスを示す JSON レポートを生成する。
- `verify`
  - 既存の envelope (`--envelope`) を検証し、対応する提案、advert、または advert 本文の一致を任意で確認する。
    JSON レポートは digest 値、署名検証状況、どの任意の成果物が一致したかを示す。
- `renewal`
  - 新たに承認された envelope を、以前に認可された digest に紐付ける。
    `--previous-envelope=<path>` と後継の `--envelope=<path>` (いずれも Norito ペイロード) が必要。
    CLI は profile エイリアス、能力、advert key が不変であることを検証し、stake、エンドポイント、
    metadata の更新のみを許可する。正規の `ProviderAdmissionRenewalV1` bytes (`--renewal-out`) と JSON 概要を出力する。
- `revoke`
  - envelope を撤回すべきプロバイダー向けに緊急の `ProviderAdmissionRevocationV1` バンドルを発行する。
    `--envelope=<path>`, `--reason=<text>`, 最低 1 つの `--council-signature` が必須で、
    `--revoked-at`/`--notes` は任意。CLI は失効 digest を署名・検証し、
    `--revocation-out` で Norito ペイロードを書き出し、digest と署名数を記した JSON レポートを出力する。
| 検証 | Torii、ゲートウェイ、`sorafs-node` が使う共有バリデータを実装し、単体テスト + CLI 結合テストを提供する。【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ 完了 |
| Torii 統合 | Torii の advert 取り込みにバリデータを組み込み、ポリシー外の advert を拒否し、テレメトリを発行する。 | Networking TL | ✅ 完了 | Torii は現在、ガバナンス envelope (`torii.sorafs.admission_envelopes_dir`) を読み込み、取り込み時に digest/署名一致を検証し、受け入れテレメトリを公開する。【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| 更新 | 更新/失効スキーマ + CLI ヘルパーを追加し、ライフサイクルガイドを docs に公開する (下の runbook と `provider-admission renewal`/`revoke` の CLI コマンドを参照)。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ 完了 |
| テレメトリ | `provider_admission` のダッシュボードとアラート (更新漏れ、envelope 期限切れ) を定義する。 | Observability | 🟠 進行中 | カウンタ `torii_sorafs_admission_total{result,reason}` は存在するが、ダッシュボード/アラートは未完。【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### 更新・失効のランブック

#### 定期更新 (ステーク/トポロジー更新)
1. `provider-admission proposal` と `provider-admission sign` で後継の提案/advert ペアを作成し、
   `--retention-epoch` を増やし、必要に応じて stake/エンドポイントを更新する。
2. 実行:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   このコマンドは `AdmissionRecord::apply_renewal` を通じて能力/プロフィールの不変を検証し、
   `ProviderAdmissionRenewalV1` を出力し、ガバナンスログ向けに digests を表示する。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` 内の旧 envelope を置き換え、更新 Norito/JSON をガバナンス
   リポジトリにコミットし、更新 hash + retention epoch を `docs/source/sorafs/migration_ledger.md` に追記する。
4. 新しい envelope が有効になったことを運用者に通知し、
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` を監視して取り込みを確認する。
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` で正規 fixtures を再生成してコミットする。
   CI (`ci/check_sorafs_fixtures.sh`) は Norito 出力が安定していることを検証する。

#### 緊急失効
1. 侵害された envelope を特定し、失効を発行する:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI は `ProviderAdmissionRevocationV1` を署名し、`verify_revocation_signatures` で署名セットを検証し、
   失効 digest を報告する。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. `torii.sorafs.admission_envelopes_dir` から envelope を削除し、失効 Norito/JSON を admission キャッシュに配布し、
   理由 hash をガバナンス議事録に記録する。
3. `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` を確認し、
   キャッシュが失効した advert を破棄したことを確かめる。失効の成果物はインシデント回顧に保管する。

## テストとテレメトリ

- admission 提案と envelope の golden fixtures を `fixtures/sorafs_manifest/provider_admission/` に追加する。
- CI (`ci/check_sorafs_fixtures.sh`) を拡張して提案を再生成し、envelope を検証する。
- 生成された fixtures には canonical digests を持つ `metadata.json` が含まれ、下流のテストは
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936` をアサートする。
- 統合テストを提供する:
  - Torii は admission envelope が欠落または期限切れの advert を拒否する。
  - CLI が proposal → envelope → verification のラウンドトリップを行う。
  - ガバナンス更新が provider ID を変えずに endpoint アテステーションを回転させる。
- テレメトリ要件:
  - Torii で `provider_admission_envelope_{accepted,rejected}` カウンタを出力する。✅ `torii_sorafs_admission_total{result,reason}` が accepted/rejected の結果を公開する。
  - 観測性ダッシュボードに期限切れ警告を追加する (7 日以内に更新が必要な場合)。

## 次のステップ

1. ✅ Norito スキーマ変更を確定し、検証ヘルパーを `sorafs_manifest::provider_admission` に反映した。feature flags は不要。
2. ✅ CLI フロー (`proposal`, `sign`, `verify`, `renewal`, `revoke`) は文書化され、統合テストで実施済み。ガバナンススクリプトは runbook と同期を保つ。
3. ✅ Torii admission/discovery が envelope を取り込み、受け入れ/拒否のテレメトリカウンタを公開する。
4. 観測性に注力: 7 日以内に更新が必要な場合に警告が上がるよう、admission ダッシュボード/アラートを仕上げる (`torii_sorafs_admission_total`, expiry gauges)。

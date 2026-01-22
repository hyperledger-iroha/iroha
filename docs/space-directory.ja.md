---
lang: ja
direction: ltr
source: docs/space-directory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c94fe3437675143bc62c57f9e1eaf27c0ccaae72cea9b6271c9007df19d19136
source_last_modified: "2025-12-06T09:39:38.926713+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/space-directory.md -->

# Space Directory 運用プレイブック

このプレイブックは、Nexus の dataspaces 向けに **Space Directory** の
エントリを作成・公開・監査・ローテーションする方法を説明します。
`docs/source/nexus.md` のアーキテクチャノートと CBDC オンボーディング計画
（`docs/source/cbdc_lane_playbook.md`）を補完し、実践的な手順、fixture、
ガバナンステンプレートを提供します。

> **Scope.** Space Directory は dataspace の manifest、UAID（Universal Account ID）
> の capability ポリシー、および規制当局が参照する監査トレイルの正規レジストリです。
> 背景コントラクトはまだ開発中（NX-15）ですが、以下の fixture とプロセスは
> 既にツールや統合テストに組み込める段階にあります。

## 1. コア概念

| 用語 | 説明 | 参照 |
|------|------|------|
| Dataspace | ガバナンスで承認されたコントラクト集合を実行する実行コンテキスト/Lane。 | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | `UniversalAccountId`（blake2b-32 のハッシュ）で、クロス dataspace 権限を固定する。 | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | UAID/dataspace ペアの決定論的な allow/deny ルールを記述する `AssetPermissionManifest`（deny が優先）。 | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | manifests と一緒に公開されるガバナンス + DA メタデータ。これによりオペレーターは validator セット、コンポーザビリティのホワイトリスト、監査フックを復元できる。 | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | manifest の有効化/失効/取り消し時に発行される Norito エンコードイベント。 | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. Manifest ライフサイクル

Space Directory は **epoch ベースのライフサイクル管理** を強制します。
すべての変更は署名済み manifest バンドルとイベントを生成します。

| イベント | トリガー | 必要アクション |
|---------|----------|----------------|
| `ManifestActivated` | 新しい manifest が `activation_epoch` に到達。 | バンドルを配布、キャッシュ更新、ガバナンス承認をアーカイブ。 |
| `ManifestExpired` | `expiry_epoch` が更新なしで経過。 | オペレーターに通知、UAID ハンドルを掃除、置換 manifest を準備。 |
| `ManifestRevoked` | 期限前の緊急 deny 決定。 | UAID を即時無効化、インシデントレポート発行、フォローアップのガバナンスレビューを計画。 |

特定の dataspace または UAID を監視するには `DataEventFilter::SpaceDirectory`
を使用します。例（Rust）：

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. オペレーターのワークフロー

| フェーズ | オーナー | 手順 | 証跡 |
|---------|----------|------|------|
| Draft | Dataspace オーナー | fixture を複製して許可/ガバナンスを編集し、`cargo test -p iroha_data_model nexus::manifest` を実行。 | Git 差分、テストログ。 |
| Review | Governance WG | manifest JSON + Norito bytes を検証し、決定ログに署名。 | 署名済み議事録、manifest hash（BLAKE3 + Norito `.to`）。 |
| Publish | Lane ops | CLI（`iroha app space-directory manifest publish`）で Norito `.to` もしくは生 JSON を送信 **または** `/v1/space-directory/manifests` に manifest JSON + 任意理由を POST。Torii 応答を確認し `SpaceDirectoryEvent` を取得。 | CLI/Torii レシート、イベントログ。 |
| Expire | Lane ops / Governance | `iroha app space-directory manifest expire`（UAID, dataspace, epoch）を実行し、`SpaceDirectoryEvent::ManifestExpired` を確認、binding 清掃の証跡をアーカイブ。 | CLI 出力、イベントログ。 |
| Revoke | Governance + Lane ops | `iroha app space-directory manifest revoke`（UAID, dataspace, epoch, reason）**または** `/v1/space-directory/manifests/revoke` へ同一 payload を POST。`SpaceDirectoryEvent::ManifestRevoked` を確認し証跡バンドルを更新。 | CLI/Torii レシート、イベントログ、チケットメモ。 |
| Monitor | SRE/Compliance | テレメトリと監査ログを監視し、revocation/expiry のアラートを設定。 | Grafana スクリーンショット、保存ログ。 |
| Rotate/Revoke | Lane ops + Governance | 置換 manifest（新 epoch）を準備し、tabletop を実施。revoke の場合はインシデントを記録。 | ローテーションチケット、ポストモーテム。 |

ロールアウトの成果物は `artifacts/nexus/<dataspace>/<timestamp>/` にまとめ、
チェックサム manifest を添付して規制当局の証跡要求に備えます。

### 3.1 Audit bundle の自動化

`iroha app space-directory manifest audit-bundle` を使用して、各 capability manifest
の証跡パックを組み立てます。JSON または Norito payload に加えて dataspace
profile JSON を受け取り、自己完結のバンドルを生成します。

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z \
  --notes "CBDC -> wholesale rotation drill"
```

このコマンドは `manifest.json`、Norito bytes（`manifest.to`）、hash
（`manifest.hash`）を書き出し、dataspace profile をコピーし、`audit_bundle.json`
を生成します。`audit_bundle.json` には UAID、dataspace id、activation/expiry
epochs、manifest hash、audit hooks、生成タイムスタンプ、任意のメモが含まれます。
`audit_hooks.events` を省略したプロファイルや
`SpaceDirectoryEvent.ManifestActivated` と `SpaceDirectoryEvent.ManifestRevoked`
の購読を忘れたプロファイルは事前に拒否され、コンプライアンスが手動 lint
不要でバンドルを利用できるようにします。生成したバンドルは
`artifacts/nexus/<dataspace>/<stamp>/` に配置し、CLI が出力した bytes と同一の
エビデンスが規制当局パケットに含まれるようにしてください。

### 3.2 Manifest & profile のスキャフォールド

ロードマップ NX-16 は、オペレーターと SDK 自動化が手作業で fixture を編集せずに
UAID capability バンドルをブートストラップできるよう、決定論的な
manifest/profile スキャフォールドを要求します。`iroha app space-directory manifest scaffold`
で、Space Directory スキーマに準拠し必要な audit hooks を登録済みの JSON テンプレート
（manifest + dataspace profile）を生成できます。

```bash
iroha app space-directory manifest scaffold \
  --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
  --dataspace 11 \
  --activation-epoch 4097 \
  --manifest-out artifacts/nexus/cbdc/scaffold/manifest.json \
  --profile-out artifacts/nexus/cbdc/scaffold/profile.json \
  --allow-program cbdc.transfer \
  --allow-method transfer \
  --allow-asset cbdc#centralbank \
  --allow-role initiator \
  --allow-max-amount 500000000 \
  --allow-window per-day \
  --deny-program cbdc.kit \
  --deny-method withdraw \
  --deny-reason "Withdrawals disabled for this UAID." \
  --profile-governance-issuer parliament@cbdc \
  --profile-governance-ticket gov-2026-02-rotation \
  --profile-validator cbdc-validator-1@cbdc \
  --profile-validator cbdc-validator-2@cbdc \
  --profile-da-attester da-attester-1@cbdc
```

このコマンドは `manifest.json` と `profile.json` を書き出し
（デフォルト: `artifacts/space_directory/scaffold/<dataspace>_<activation>/`）、
生成パスを出力するため、リリースプレイブックから直接リンクできます。allow/deny
フラグでルールを事前投入でき、未指定のフィールドはキュレーション済み fixture と
同じプレースホルダを保持します。両方の出力には必須の
`SpaceDirectoryEvent.ManifestActivated/Revoked` の監査フックが含まれるため、本番
manifest と同じ検証を通過します。生成後にファイルを編集し、証跡のために
`manifest encode/audit-bundle` を再実行し、ガバナンス承認後にソース管理へ
コミットしてください。

## 4. Manifest テンプレートと Fixtures

キュレーション済み fixtures を正規のスキーマ参照として使用します。Wholesale CBDC
サンプル（`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`）は
allow と deny の両方を含みます。

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

主要ルール:

- **Deny が優先。** 同じスコープに対する allow の後に明示的な deny を置き、
  優先順位が理解できるようにします。
- **決定論的な金額。** `max_amount` は 10 進文字列として保持し、`Numeric` が
  値を解析する際の浮動小数点の曖昧さを避けます。
- **スケジューラ駆動の失効。** コアランタイムはブロック高が `expiry_epoch` に
  到達すると manifest を自動で失効させ、`SpaceDirectoryEvent::ManifestExpired`
  を発行し、`nexus_space_directory_revision_total` を増やし、Torii/CLI の表示が
  更新される前に UAID を再バインドします。CLI は手動 override や事後の失効記録が
  必要な場合にのみ使用してください。
- **Epoch ゲーティング。** `activation_epoch` と `expiry_epoch` でローテーション
  ケイデンスを表現します。緊急取り消しは `ManifestRevoked` を発行します。

Retail dApp の manifest は
`fixtures/space_directory/capability/retail_dapp_access.manifest.json` にあり、
コンポーザビリティのシナリオを検証できます。

各 capability manifest には同一ディレクトリに Norito エンコード版が存在します
（例: `fixtures/space_directory/capability/cbdc_wholesale.manifest.to`、BLAKE3
ダイジェスト `11a47182ab51e845d53f40f12387caef1e609585a824c0a4feab38f0922859fe`）。
`.to` ファイルは `cargo xtask space-directory encode` で生成され、
`cbdc_capability_manifests_enforce_policy_semantics` 統合テストが JSON と Norito
payload の同期を保証します。

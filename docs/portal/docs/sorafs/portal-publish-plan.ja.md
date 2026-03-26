---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 576e2aa47d5ebf5c11c313549e140bc832d2f666d77fae462b82d714b29e4254
source_last_modified: "2025-11-15T08:41:26.442940+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: portal-publish-plan
title: Docs ポータル → SoraFS 公開計画
sidebar_label: ポータル公開計画
description: docs ポータル、OpenAPI、SBOM バンドルを SoraFS 経由で出荷するための手順書。
---

:::note 正規ソース
このページは `docs/source/sorafs/portal_publish_plan.md` を反映しています。ワークフローが変わったら両方を更新してください。
:::

ロードマップ項目 DOCS-7 では、すべての docs 成果物 (ポータル build、OpenAPI 仕様、
SBOM) を SoraFS の manifest パイプラインに通し、`docs.sora` から `Sora-Proof`
ヘッダ付きで配信することが求められます。このチェックリストは既存の helpers を
つなぎ、Docs/DevRel、Storage、Ops が複数の runbook を探し回らずにリリースできるようにします。

## 1. Build とペイロードのパッケージ化

パッケージング helper を実行します (dry-run 用の skip オプションあり)。

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` は CI が既に生成した `docs/portal/build` を再利用します。
- `syft` が利用できない場合 (例: air-gapped リハーサル) は `--skip-sbom` を追加します。
- スクリプトはポータルのテストを実行し、`portal`、`openapi`、`portal-sbom`、`openapi-sbom`
  の CAR + manifest ペアを出力します。`--proof` が有効な場合は各 CAR を検証し、
  `--sign` が有効な場合は Sigstore bundles を生成します。
- 出力構造:

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

フォルダ全体 (または `artifacts/devportal/sorafs/latest` の symlink) を保持し、
ガバナンスのレビュー担当が build 成果物を追跡できるようにします。

## 2. Manifests と aliases の Pin

`torii` に manifests を送信して aliases を紐付けるために `sorafs_cli manifest submit` を使います。
`${SUBMITTED_EPOCH}` は最新の合意エポックを設定します (`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` またはダッシュボードから取得)。

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="<katakana-i105-account-id>"
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- `openapi.manifest.to` と SBOM manifests も同様に実施します (SBOM bundles の alias フラグは、ガバナンスが namespace を割り当てた場合を除いて省略)。
- 代替手段: `iroha app sorafs pin register` は submit summary の digest があれば利用可能 (バイナリがインストール済みの場合)。
- registry 状態の確認:
  `iroha app sorafs pin list --alias docs:portal --format json | jq`。
- 監視するダッシュボード: `sorafs_pin_registry.json` (`torii_sorafs_replication_*` メトリクス)。

## 3. Gateway Headers と Proofs

HTTP ヘッダブロック + binding メタデータを生成します。

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- テンプレートは `Sora-Name`、`Sora-CID`、`Sora-Proof`、`Sora-Proof-Status` と、
  デフォルトの CSP/HSTS/Permissions-Policy を含みます。
- `--rollback-manifest-json` を使うとロールバック用ヘッダセットを生成できます。

トラフィック公開前に以下を実行:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- probe は GAR 署名の新鮮性、alias ポリシー、TLS 証明書フィンガープリントを検証します。
- self-cert harness は `sorafs_fetch` で manifest を取得し、CAR の replay ログを保存します。監査用の証跡として outputs を保管してください。

## 4. DNS とテレメトリのガードレール

1. governance が binding を証明できるように DNS skeleton を更新します。

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. ロールアウト中に監視:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Dashboards: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` と pin registry ボード。

3. アラートルールの smoke を実施 (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) し、
   リリースアーカイブ用にログ/スクリーンショットを保存します。

## 5. 証跡バンドル

リリースチケットまたはガバナンスパッケージに以下を含めます。

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  Sigstore bundles, submit summaries)。
- Gateway probe + self-cert outputs
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS skeleton + header テンプレート (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- ダッシュボードのスクリーンショット + アラート確認。
- `status.md` の更新 (manifest digest と alias binding 時刻の記録)。

このチェックリストに従うことで DOCS-7 を満たします: portal/OpenAPI/SBOM の
ペイロードは決定論的にパッケージされ、aliases で固定され、`Sora-Proof`
ヘッダで保護され、既存の observability スタックで end-to-end 監視されます。

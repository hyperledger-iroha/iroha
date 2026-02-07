---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#پریویو ہوسٹ ایکسپوژر گائیڈ

DOCS-SORA チェックサム検証済みバンドル チェックサム検証済みバンドルطور پر چلاتے ہیں۔ آن بورڈنگ (اور 招待状 منظوری ٹکٹ) مکمل ہونے کے بعد اس runbook کو استعمال کریں تاکہ بیٹا پریویو ہوسٹ آن لائن لایا جا سکے۔

## پیشگی شرائط

- آن بورڈنگ ویو منظور اور پریویو ٹریکر میں درج ہو چکی ہو۔
- ビルド `docs/portal/build/` チェックサム チェックサム (`build/checksums.sha256`)。
- SoraFS پریویو اسناد (Torii URL、権限、秘密鍵、エポック) ماحولاتی ویری ایبلز میں یا JSON config میں محفوظ ہوں جیسے [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)。
- ホスト名 (`docs-preview.sora.link`、`docs.iroha.tech` وغیرہ) ساتھ DNS 評価 ٹکٹ کھلا ہو ور オンコール رابطے شامل وں۔

## مرحلہ 1 - バンドルを確認する

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

チェックサム マニフェストを検証します。 チェックサム マニフェストを確認します。 پریویو آرٹیفیکٹ آڈٹ میں رہتا ہے۔

## مرحلہ 2 - SoraFS アーティファクト پیک کریں

決定論的 CAR/マニフェストの定義 特定の CAR/マニフェストの定義`ARTIFACT_DIR` کی ڈیفالٹ ویلیو `docs/portal/artifacts/` ہے۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

テスト `portal.car`、`portal.manifest.*`、記述子チェックサム マニフェスト پریویو ویو ٹکٹ کے ساتھ منسلکریں۔

## مرحلہ 3 - پریویو エイリアス شائع کریں

جب ہوسٹ ایکسپوز کرنے لئے تیار ہوں تو ピン ヘルパー کو **بغیر** `--skip-submit` کے دوبارہ چلائیں۔ JSON 構成と CLI フラグの設定:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

یہ کمانڈ `portal.pin.report.json`, `portal.manifest.submit.summary.json` اور `portal.submit.response.json` لکھتی ہے، جو 証拠バンドルを招待する کے ساتھ ہونی چاہئیں۔

## مرحلہ 4 - DNS カットオーバー پلان بنائیں

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

JSON オペレーション オペレーション オペレーション DNS マニフェスト ダイジェスト マニフェスト ダイジェストロールバック 説明 記述子 説明 説明 説明 `--previous-dns-plan path/to/previous.json` 説明 説明 説明

## مرحلہ 5 - ڈیپلائےڈ ہوسٹ کو プローブ کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

プローブ、リリース タグ、CSP ヘッダー、署名メタデータ、テスト、リリース タグ、CSP ヘッダー、署名メタデータ、テストエッジ キャッシュ (カール出力、カール出力) エッジ キャッシュうーん

## 証拠の束

پریویو ویو ٹکٹ میں درج ذیل アーティファクト شامل کریں اور دعوتی ای میل میں ان کا حوالہ دیں:

|アーティファクト |すごい |
|----------|------|
| `build/checksums.sha256` |バンドル CI ビルドを実行する テストを実行する|
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` |正規の SoraFS ペイロード + マニフェスト。 |
| `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` |マニフェストの提出 اور エイリアス バインディング کی کامیابی دکھاتا ہے۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS メタデータ (標準)、ルート プロモーション (`Sora-Route-Binding`)、`route_plan` ポインター (JSON + ヘッダー テンプレート)、キャッシュ パージ、Opsロールバック ロールバック|
| `artifacts/sorafs/preview-descriptor.json` |説明 記述子 アーカイブ + チェックサム جوڑتا ہے۔ |
| `probe` 出力 | تصدیق کرتا ہے کہ لائیو ہوسٹ متوقع release tag دکھا رہا ہے۔ |
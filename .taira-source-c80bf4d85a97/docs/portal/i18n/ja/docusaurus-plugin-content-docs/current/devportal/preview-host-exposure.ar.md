---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تعريض مضيف المعاينة

DOCS-SORA チェックサム チェックサムログインしてください。 استخدم هذا الدليل بعد اكتمال تأهيل المراجعين (وتذكرة اعتماد الدعوات) لوضع مضيف المعاينةありがとうございます。

## ああ、ああ

- عتماد موجة تأهيل المراجعين وتسجيلها في متعقب المعاينة.
- チェックサム `docs/portal/build/` チェックサム (`build/checksums.sha256`)。
- بيانات اعتماد معاينة SoraFS (عنوان Torii، الجهة المخولة، المفتاح الخاص، epoch المرسل) JSON 形式 [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)。
- DNS サーバー (`docs-preview.sora.link`, `docs.iroha.tech`, ...) サーバーああ。

## الخطوة 1 - بناء الحزمة والتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム

## 番号 2 - 番号 SoraFS

CAR/マニフェストを確認してください。 `ARTIFACT_DIR` 、 `docs/portal/artifacts/` 。

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

`portal.car` と `portal.manifest.*` は、チェックサムを検査します。

## الخطوة 3 - 別名 المعاينة

اعد تشغيل مساعد pin **بدون** `--skip-submit` عندما تكون مستعدا لتعريض المضيف. JSON と CLI の組み合わせ:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

يكتب الامر `portal.pin.report.json` و `portal.manifest.submit.summary.json` و `portal.submit.response.json`، والتي يجب ان ترافق حزمة الادلة الخاصةやあ。

## 4 - DNS を使用する

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

JSON 管理、Ops 管理、DNS 管理、ダイジェスト管理、マニフェスト管理。ロールバックを実行してください。`--previous-dns-plan path/to/previous.json`。

## 5 - فحص المضيف المنشور

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

CSP にアクセスしてください。キャッシュをキャッシュします。

## いいえ

重要な情報:

|認証済み |ああ |
|------|------|
| `build/checksums.sha256` | CI。 |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | SoraFS + マニフェスト。 |
| `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` |マニフェスト エイリアス。 |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS (DNS) (`Sora-Route-Binding`) `route_plan` (JSON + ヘッダー) パージ、ロールバック、Ops. |
| `artifacts/sorafs/preview-descriptor.json` | واصف موقع يربط الارشيف + チェックサム。 |
| `probe` |ログインしてください。 |
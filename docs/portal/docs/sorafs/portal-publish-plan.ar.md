---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 576e2aa47d5ebf5c11c313549e140bc832d2f666d77fae462b82d714b29e4254
source_last_modified: "2025-11-15T08:41:26.442940+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: portal-publish-plan
title: خطة نشر بوابة الوثائق → SoraFS
sidebar_label: خطة نشر البوابة
description: قائمة تحقق خطوة بخطوة لشحن بوابة الوثائق و OpenAPI وحزم SBOM عبر SoraFS.
---

:::note المصدر المعتمد
تعكس `docs/source/sorafs/portal_publish_plan.md`. حدّث النسختين عند تغير مسار العمل.
:::

يتطلب بند خارطة الطريق DOCS-7 ان تمر كل artifacts الخاصة بالوثائق (بناء البوابة، مواصفة OpenAPI،
و SBOMs) عبر مسار manifests في SoraFS وان تُخدم عبر `docs.sora` مع رؤوس `Sora-Proof`.
تربط هذه القائمة المساعدات الموجودة حتى تتمكن فرق Docs/DevRel و Storage و Ops من تنفيذ
الاصدار دون البحث بين عدة runbooks.

## 1. البناء وتعبئة الحمولة

شغّل مساعد التعبئة (تتوفر خيارات تخطي لسيناريوهات dry-run):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` يعيد استخدام `docs/portal/build` اذا كان CI قد بناه مسبقا.
- اضف `--skip-sbom` عندما يكون `syft` غير متاح (مثل التدريب في بيئة معزولة).
- يقوم السكربت بتشغيل اختبارات البوابة ويولد ازواج CAR + manifest لـ `portal` و `openapi`
  و `portal-sbom` و `openapi-sbom`، ويتحقق من كل CAR عند تفعيل `--proof`، ويكتب
  حزم Sigstore عند تفعيل `--sign`.
- هيكل المخرجات:

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

احتفظ بالمجلد بالكامل (او اربطه عبر `artifacts/devportal/sorafs/latest`) حتى يتمكن
مراجعو الحوكمة من تتبع artifacts الخاصة بالبناء.

## 2. تثبيت manifests و aliases

استخدم `sorafs_cli manifest submit` لدفع manifests الى Torii وربط aliases.
اضبط `${SUBMITTED_EPOCH}` على اخر ايبوك اجماع (من
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` او لوحة التحكم).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="<i105-account-id>"
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

- كرر ذلك لـ `openapi.manifest.to` و manifests الخاصة بـ SBOM (احذف اعلام alias لحزم SBOM
  الا اذا خصصت الحوكمة namespace).
- بديل: `iroha app sorafs pin register` يعمل باستخدام digest من ملخص الارسال اذا كان
  الثنائي مثبتا بالفعل.
- تحقق من حالة registry عبر
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- لوحات المتابعة: `sorafs_pin_registry.json` (مقاييس `torii_sorafs_replication_*`).

## 3. رؤوس البوابة والـ proofs

ولّد كتلة رؤوس HTTP + بيانات الربط:

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

- القالب يتضمن رؤوس `Sora-Name` و `Sora-CID` و `Sora-Proof` و `Sora-Proof-Status`
  بالاضافة الى CSP/HSTS/Permissions-Policy الافتراضية.
- استخدم `--rollback-manifest-json` لانتاج مجموعة رؤوس للرجوع.

قبل تعريض المرور، شغّل:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- يفرض الـ probe حداثة توقيع GAR وسياسة alias وبصمات TLS.
- يقوم self-cert بتنزيل manifest عبر `sorafs_fetch` ويحتفظ بسجلات replay لـ CAR؛
  احتفظ بالمخرجات لادلة التدقيق.

## 4. حواجز DNS والتليمتري

1. حدّث هيكل DNS حتى تتمكن الحوكمة من اثبات الربط:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. راقب اثناء الاطلاق:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   لوحات المراقبة: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` ولوحة pin registry.

3. نفّذ smoke لقواعد التنبيه (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) واحفظ
   السجلات/الصور للارشيف.

## 5. حزمة الادلة

ضمّن ما يلي في تذكرة الاصدار او حزمة الحوكمة:

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  حزم Sigstore، ملخصات الارسال).
- مخرجات gateway probe + self-cert
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- هيكل DNS + قوالب الرؤوس (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- لقطات لوحات المراقبة + اقرارات التنبيه.
- تحديث `status.md` مع digest الخاص بالـ manifest ووقت ربط alias.

اتباع هذه القائمة يحقق DOCS-7: يتم تعبئة payloads الخاصة بالبوابة/OpenAPI/SBOM بشكل
حتمي، وتثبيتها بالـ aliases، وحمايتها برؤوس `Sora-Proof`، ومراقبتها end-to-end عبر
منظومة الملاحظة الحالية.

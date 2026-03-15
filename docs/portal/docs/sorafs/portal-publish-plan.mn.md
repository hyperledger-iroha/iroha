---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd5fff7302924f71ca19593cbbcc29352c00f286ab5bc555d4654e2dc43c3daa
source_last_modified: "2026-01-22T16:26:46.525444+00:00"
translation_last_reviewed: 2026-02-07
id: portal-publish-plan
title: Docs Portal → SoraFS Publish Plan
sidebar_label: Portal Publish Plan
description: Step-by-step checklist for shipping the docs portal, OpenAPI, and SBOM bundles via SoraFS.
translator: machine-google-reviewed
---

::: Каноник эх сурвалжийг анхаарна уу
`docs/source/sorafs/portal_publish_plan.md` толь. Ажлын урсгал өөрчлөгдөх үед хоёуланг нь шинэчил.
:::

Замын зургийн DOCS-7 зүйлд баримт бичгийн олдвор бүрийг шаарддаг (портал бүтээх, OpenAPI,
SBOMs) SoraFS манифест дамжуулах хоолойгоор урсаж, `docs.sora`-ээр үйлчлэх
`Sora-Proof` толгойтой. Энэхүү хяналтын хуудас нь одоо байгаа туслахуудыг хооронд нь холбож өгдөг
Тиймээс Docs/DevRel, Storage болон Ops нь ямар ч хайлтгүйгээр хувилбарыг ажиллуулах боломжтой
олон runbook.

## 1. Ачааллыг бүтээх, багцлах

Сав баглаа боодлын туслахыг ажиллуул (хуурай гүйлтийн хувьд алгасах сонголтууд байдаг):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- Хэрэв CI аль хэдийн үйлдвэрлэсэн бол `--skip-build` `docs/portal/build`-г дахин ашигладаг.
- `syft` боломжгүй үед `--skip-sbom` нэмнэ (жишээ нь, агаарын завсарлагатай бэлтгэл).
- Скрипт нь портал тестийг ажиллуулж, `portal`-д зориулсан CAR + манифест хосуудыг ялгаруулдаг.
  `openapi`, `portal-sbom`, болон `openapi-sbom` нь машин бүрийг шалгах үед
  `--proof` тохируулагдсан бөгөөд `--sign` тохируулагдсан үед Sigstore багцуудыг хасна.
- Гаралтын бүтэц:

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

Фолдерыг бүхэлд нь (эсвэл `artifacts/devportal/sorafs/latest`-ээр дамжуулан симбол) хадгалаарай
засаглалын тоймчид барилгын олдворуудыг хянах боломжтой.

## 2. Manifests + Aliass зүү

`sorafs_cli manifest submit` ашиглан манифестуудыг Torii руу түлхэж, бусад нэрүүдийг холбоно уу.
`${SUBMITTED_EPOCH}`-г хамгийн сүүлийн үеийн зөвшилцлийн эрин үе (
`curl -s "${TORII_URL}/v2/status" | jq '.sumeragi.epoch'` эсвэл таны хяналтын самбар).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="i105..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v2/status | jq '.sumeragi.epoch')"

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

- `openapi.manifest.to` болон SBOM манифестуудыг давтана уу
  Засаглал нэрийн орон зайг хуваарилаагүй тохиолдолд SBOM багцууд).
- Альтернатив: `iroha app sorafs pin register` нь илгээсэн материалтай ажилладаг
  Хэрэв хоёртын файлыг аль хэдийн суулгасан бол хураангуй.
- Бүртгэлийн төлөвийг баталгаажуулна уу
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Үзэх самбар: `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  хэмжүүр).

## 3. Гарцын толгой ба Баталгаа

HTTP толгой блок + холбох мета өгөгдлийг үүсгэнэ үү:

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

- Загварт `Sora-Name`, `Sora-CID`, `Sora-Proof`, болон
  `Sora-Proof-Status` гарчиг болон үндсэн CSP/HSTS/Зөвшөөрлийн бодлого.
- `--rollback-manifest-json`-г ашиглан хосолсон буцаах толгойн багцыг үзүүлээрэй.

Замын хөдөлгөөнийг нээхээс өмнө гүйх:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- Шинжилгээ нь GAR гарын үсгийн шинэлэг байдал, нэрийн бодлого, TLS сертификатыг мөрддөг
  хурууны хээ.
- Өөрийгөө баталгаажуулах хэрэгсэл нь `sorafs_fetch`-ээр манифестыг татаж аваад хадгалдаг.
  Машины дахин тоглуулах бүртгэл; аудитын нотлох баримтын үр дүнг хадгалах.

## 4. DNS & Telemetry Guardrails

1. DNS араг ясыг шинэчилснээр засаглал нь заавал дагаж мөрдөх ёстойг нотлох болно:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Дамжуулах явцад хяналт тавих:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Хяналтын самбар: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json`, зүү бүртгэлийн самбар.

3. Анхааруулах дүрмийг тамхи татах (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) болон
   хувилбарын архивт зориулсан лог/дэлгэцийн агшин авах.

## 5. Нотлох баримтын багц

Суллах тасалбар эсвэл засаглалын багцад дараахь зүйлийг оруулна уу.

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, нотлох баримтууд,
  Sigstore багцууд, хураангуйг илгээнэ үү).
- Gateway датчик + өөрөө баталгаажуулалтын гаралт
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS араг яс + толгойн загварууд (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Хяналтын самбарын дэлгэцийн агшин + анхааруулга.
- `status.md` шинэчлэлт нь манифест дижест болон бусад нэрээр холбогдох хугацаатай холбоотой.

Энэхүү хяналтын хуудсыг дагаж DOCS-7-г хүргэж байна: портал/OpenAPI/SBOM ачааллыг
тодорхой байдлаар савласан, өөр нэрээр бэхлэгдсэн, `Sora-Proof` хамгаалагдсан
гарчиг, мөн одоо байгаа ажиглалтын стекээр дамжуулан төгсгөлөөс төгсгөл хүртэл хянадаг.
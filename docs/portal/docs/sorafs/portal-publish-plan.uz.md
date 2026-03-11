---
lang: uz
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

::: Eslatma Kanonik manba
Nometall `docs/source/sorafs/portal_publish_plan.md`. Ish jarayoni o'zgarganda ikkala nusxani ham yangilang.
:::

DOCS-7 yoʻl xaritasi bandi har bir hujjat artefaktini talab qiladi (portal tuzilishi, OpenAPI spetsifikatsiyasi,
SBOMs) SoraFS manifest quvur liniyasi orqali oqadi va `docs.sora` orqali xizmat qiladi
`Sora-Proof` sarlavhalari bilan. Ushbu nazorat ro'yxati mavjud yordamchilarni birlashtiradi
shuning uchun Docs/DevRel, Storage va Ops versiyani ov qilmasdan ishga tushirishi mumkin
bir nechta runbook.

## 1. Foydali yuklarni yaratish va paketlash

Qadoqlash yordamchisini ishga tushiring (quruq yugurishlar uchun o'tkazib yuborish opsiyalari mavjud):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- Agar CI allaqachon ishlab chiqargan bo'lsa, `--skip-build` `docs/portal/build` dan qayta foydalanadi.
- `syft` mavjud bo'lmaganda `--skip-sbom` qo'shing (masalan, havo bo'shlig'i bilan mashq qilish).
- Skript portal testlarini o'tkazadi, `portal` uchun CAR + manifest juftlarini chiqaradi,
  `openapi`, `portal-sbom` va `openapi-sbom`, har bir avtomobilni qachon tekshiradi
  `--proof` o'rnatildi va `--sign` o'rnatilganda Sigstore to'plamlari tushadi.
- Chiqish tuzilishi:

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

Butun jildni (yoki `artifacts/devportal/sorafs/latest` orqali symlink) shunday saqlang
boshqaruv tekshiruvchilari qurilish artefaktlarini kuzatishi mumkin.

## 2. PIN manifestlari + taxalluslar

Manifestlarni Torii ichiga surish va taxalluslarni ulash uchun `sorafs_cli manifest submit` dan foydalaning.
`${SUBMITTED_EPOCH}` ni so'nggi konsensus davriga (dan
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` yoki asboblar paneli).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="i105..."
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

- `openapi.manifest.to` va SBOM manifestlari uchun takrorlang (taxallus bayroqlarini qoldirmang
  SBOM to'plamlari, agar boshqaruv nom maydoni tayinlamasa).
- Muqobil: `iroha app sorafs pin register` yuborilgan dayjest bilan ishlaydi
  ikkilik allaqachon o'rnatilgan bo'lsa, xulosa.
- bilan ro'yxatga olish holatini tekshiring
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Ko'rish uchun asboblar paneli: `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  ko'rsatkichlar).

## 3. Gateway sarlavhalari va isbotlari

HTTP sarlavha bloki + bog'lash metama'lumotlarini yarating:

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

- Andoza `Sora-Name`, `Sora-CID`, `Sora-Proof` va
  `Sora-Proof-Status` sarlavhalari va standart CSP/HSTS/Permissions-policy.
- Ulangan orqaga qaytarish sarlavhalari to'plamini ko'rsatish uchun `--rollback-manifest-json` dan foydalaning.

Trafikni ochishdan oldin:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- Tekshiruv GAR imzosining yangiligi, taxallus siyosati va TLS sertifikatini qo'llaydi
  barmoq izlari.
- O'z-o'zini tasdiqlovchi jabduqlar manifestni `sorafs_fetch` bilan yuklaydi va saqlaydi
  CAR takrorlash jurnallari; natijalarni auditorlik dalillari uchun saqlash.

## 4. DNS va telemetriya himoyasi

1. Boshqaruv majburiyligini isbotlashi uchun DNS skeletini yangilang:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Chiqarish paytida kuzating:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Boshqarish paneli: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` va pin ro'yxatga olish kengashi.

3. Ogohlantirish qoidalarini cheking (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) va
   nashr arxivi uchun jurnallar/skrinshotlar olish.

## 5. Dalillar to'plami

Chiqarish chiptasi yoki boshqaruv paketiga quyidagilarni kiriting:

- `artifacts/devportal/sorafs/<stamp>/` (Avtomobillar, manifestlar, SBOMlar, dalillar,
  Sigstore to'plamlari, xulosalarni yuboring).
- Gateway probi + o'z-o'zini sertifikatlash natijalari
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS skeleti + sarlavha shablonlari (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Boshqaruv panelidagi skrinshotlar + ogohlantirishni tasdiqlash.
- `status.md` yangilanishi manifest dayjestiga va taxallusning ulanish vaqtiga tegishli.

Ushbu nazorat roʻyxatidan soʻng DOCS-7 taqdim etiladi: portal/OpenAPI/SBOM foydali yuklari
deterministik tarzda qadoqlangan, taxalluslar bilan qadalgan, `Sora-Proof` tomonidan himoyalangan
sarlavhalar va mavjud kuzatuv stegi orqali oxirigacha nazorat qilinadi.
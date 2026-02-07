---
id: portal-publish-plan
lang: az
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Docs Portal → SoraFS Publish Plan
sidebar_label: Portal Publish Plan
description: Step-by-step checklist for shipping the docs portal, OpenAPI, and SBOM bundles via SoraFS.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
Güzgülər `docs/source/sorafs/portal_publish_plan.md`. İş axını dəyişdikdə hər iki nüsxəni yeniləyin.
:::

Yol xəritəsi elementi DOCS-7 hər bir sənəd artefaktını tələb edir (portal quruluşu, OpenAPI spesifikasiyası,
SBOMs) SoraFS manifest boru kəməri vasitəsilə axmaq və `docs.sora` vasitəsilə xidmət etmək
`Sora-Proof` başlıqları ilə. Bu yoxlama siyahısı mövcud köməkçiləri birləşdirir
beləliklə, Sənədlər/DevRel, Yaddaş və Əməliyyatlar ov etmədən buraxılışı işlədə bilər
çoxlu runbook.

## 1. Faydalı Yükləri Yaradın və Paketləyin

Qablaşdırma köməkçisini işə salın (quru qaçışlar üçün atlama variantları mövcuddur):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- CI artıq istehsal edibsə, `--skip-build` `docs/portal/build`-dən təkrar istifadə edir.
- `syft` əlçatan olmadıqda (məsələn, hava boşluğu olan məşq) `--skip-sbom` əlavə edin.
- Skript portal testlərini həyata keçirir, `portal` üçün CAR + manifest cütlərini yayır,
  `openapi`, `portal-sbom` və `openapi-sbom`, hər CAR-ı yoxlayır
  `--proof` quraşdırılıb və `--sign` təyin edildikdə Sigstore paketləri düşür.
- Çıxış strukturu:

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

Bütün qovluğu (və ya `artifacts/devportal/sorafs/latest` vasitəsilə simvolik əlaqəni) belə saxlayın
idarəetmə rəyçiləri tikinti artefaktlarını izləyə bilərlər.

## 2. Pin Manifestləri + Ləqəblər

Manifestləri Torii-ə köçürmək və ləqəbləri bağlamaq üçün `sorafs_cli manifest submit` istifadə edin.
`${SUBMITTED_EPOCH}`-i ən son konsensus dövrünə təyin edin (dan
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` və ya idarə paneliniz).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="ih58..."
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

- `openapi.manifest.to` və SBOM manifestləri üçün təkrarlayın (ləqəb bayraqlarını buraxın
  İdarəetmə ad sahəsi təyin etmədiyi halda SBOM paketləri).
- Alternativ: `iroha app sorafs pin register` təqdim edilən həzmlə işləyir
  ikili artıq quraşdırılıbsa xülasə.
- Qeydiyyatın vəziyyətini ilə yoxlayın
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Baxılacaq idarə panelləri: `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  ölçülər).

## 3. Gateway Başlıqları və Sübutlar

HTTP başlıq blokunu + bağlama metadatasını yaradın:

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

- Şablonda `Sora-Name`, `Sora-CID`, `Sora-Proof` və
  `Sora-Proof-Status` başlıqları və defolt CSP/HSTS/İcazələr-Siyasəti.
- Cütlənmiş geriyə başlıq dəstini göstərmək üçün `--rollback-manifest-json` istifadə edin.

Trafiki ifşa etməzdən əvvəl qaçın:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- Prob GAR imza təzəliyini, ləqəb siyasətini və TLS sertifikatını tətbiq edir
  barmaq izləri.
- Özünü təsdiq edən qoşqu `sorafs_fetch` ilə manifesti yükləyir və saxlayır
  CAR replay logs; nəticələri audit sübutları üçün saxlamaq.

## 4. DNS & Telemetriya Qoruyucuları

1. DNS skeletini yeniləyin ki, idarəetmə məcburiyyəti sübut etsin:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Yayım zamanı monitorinq edin:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Panellər: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` və pin qeyd lövhəsi.

3. Xəbərdarlıq qaydaları (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) və
   buraxılış arxivi üçün qeydləri/skrinşotları çəkin.

## 5. Evidence Bundle

Buraxılış biletinə və ya idarəetmə paketinə aşağıdakıları daxil edin:

- `artifacts/devportal/sorafs/<stamp>/` (CAR-lar, manifestlər, SBOM-lar, sübutlar,
  Sigstore paketləri, xülasələri təqdim edin).
- Gateway zond + özünü təsdiq çıxışları
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS skeleti + başlıq şablonları (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Dashboard ekran görüntüləri + xəbərdarlıq təsdiqi.
- `status.md` manifest həzminə və ləqəb bağlama vaxtına istinad edən yeniləmə.

Bu yoxlama siyahısından sonra DOCS-7 təqdim olunur: portal/OpenAPI/SBOM yükləri
deterministik şəkildə qablaşdırılır, ləqəblərlə bağlanır, `Sora-Proof` tərəfindən qorunur
başlıqlar və mövcud müşahidələr yığını vasitəsilə başdan sona nəzarət edilir.
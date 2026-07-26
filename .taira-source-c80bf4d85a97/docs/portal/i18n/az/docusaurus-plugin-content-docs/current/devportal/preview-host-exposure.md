---
id: preview-host-exposure
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview host exposure guide
sidebar_label: Preview host exposure
description: Publish and verify the beta preview host before sending invites.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

DOCS‑SORA yol xəritəsi hər bir ictimai önizləmənin eyni yolda getməsini tələb edir
rəyçilərin yerli olaraq istifadə etdiyi yoxlama cəmi ilə təsdiqlənmiş paket. Bu runbook istifadə edin
nəzərdən keçirən onboarding sonra (və dəvət təsdiq bilet) qoymaq üçün tamamlandı
onlayn beta önizləmə hostu.

## İlkin şərtlər

- Rəyçi onboarding dalğası təsdiqləndi və önizləmə izləyicisinə daxil oldu.
- Ən son portal quruluşu `docs/portal/build/` və yoxlama məbləği altında mövcuddur
  yoxlanılıb (`build/checksums.sha256`).
- SoraFS önizləmə etimadnaməsi (Torii URL, səlahiyyət, şəxsi açar, təqdim edilib
  epoch) mühit dəyişənlərində və ya JSON konfiqurasiyasında saxlanılır, məsələn
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- İstədiyiniz host adı ilə açılan DNS dəyişdirmə bileti (`docs-preview.sora.link`,
  `docs.iroha.tech` və s.) üstəgəl zəng üzrə kontaktlar.

## Addım 1 – Paketi qurun və yoxlayın

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Yoxlama skripti yoxlama məbləği manifestinin olmaması və ya olmadıqda davam etməkdən imtina edir
hər bir önizləmə artefaktını yoxlanaraq saxtalaşdırıldı.

## Addım 2 – SoraFS artefaktlarını qablaşdırın

Statik saytı deterministik CAR/manifest cütlüyünə çevirin. `ARTIFACT_DIR`
defolt olaraq `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --skip-submit

node scripts/generate-preview-descriptor.mjs \
  --manifest artifacts/checksums.sha256 \
  --archive artifacts/sorafs/portal.tar.gz \
  --out artifacts/sorafs/preview-descriptor.json
```

Yaradılmış `portal.car`, `portal.manifest.*`, deskriptor və yoxlama məbləğini əlavə edin
önizləmə dalğa biletinə manifest.

## Addım 3 – Önizləmə ləqəbini dərc edin

Açmağa hazır olduqdan sonra pin köməkçisini **siz** `--skip-submit` yenidən işə salın
ev sahibi. JSON konfiqurasiyasını və ya açıq CLI bayraqlarını təmin edin:

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

Komanda `portal.pin.report.json` yazır,
`portal.manifest.submit.summary.json` və `portal.submit.response.json` olan
dəvət sübut paketi ilə göndərilməlidir.

## Addım 4 – DNS kəsmə planını yaradın

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --dns-hostname docs.iroha.tech \
  --dns-zone sora.link \
  --dns-change-ticket DOCS-SORA-Preview \
  --dns-cutover-window "2026-03-05 18:00Z" \
  --dns-ops-contact "pagerduty:sre-docs" \
  --manifest artifacts/sorafs/portal.manifest.to \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --out artifacts/sorafs/portal.dns-cutover.json
```

Yaranan JSON-u Ops ilə paylaşın ki, DNS keçidi dəqiq istinad etsin
aşkar həzm. Əvvəlki təsviri geri qaytarma mənbəyi kimi təkrar istifadə edərkən,
`--previous-dns-plan path/to/previous.json` əlavə edin.

## Addım 5 – Yerləşdirilmiş hostu yoxlayın

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

Prob təqdim edilən buraxılış etiketini, CSP başlıqlarını və imza metadatasını təsdiqləyir.
Auditorların görə bilməsi üçün iki bölgədən əmri təkrarlayın (və ya qıvrım çıxışını əlavə edin).
kənar önbelleğin isti olduğunu.

## Sübut dəsti

Aşağıdakı artefaktları önizləmə dalğa biletinə daxil edin və onlara müraciət edin
dəvət e-poçtu:

| Artefakt | Məqsəd |
|----------|---------|
| `build/checksums.sha256` | Paketin CI quruluşuna uyğun olduğunu sübut edir. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Canonical SoraFS faydalı yük + manifest. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Manifest təqdimini göstərir + ləqəb bağlanması müvəffəqiyyətlidir. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS metadata (bilet, pəncərə, kontaktlar), marşrut təşviqi (`Sora-Route-Binding`) xülasəsi, `route_plan` göstəricisi (plan JSON + başlıq şablonları), keşin təmizlənməsi məlumatı və Əməliyyatlar üçün geri qaytarma təlimatları. |
| `artifacts/sorafs/preview-descriptor.json` | Arxivi + yoxlama məbləğini birləşdirən imzalanmış deskriptor. |
| `probe` çıxış | Canlı ev sahibinin gözlənilən buraxılış etiketini reklam etdiyini təsdiqləyir. |

Ev sahibi canlı olduqdan sonra [Önizləmə dəvətnamə kitabını] izləyin (./public-preview-invite.md)
linki yaymaq, dəvətləri qeyd etmək və telemetriyaya nəzarət etmək.
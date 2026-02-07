---
id: preview-host-exposure
lang: mn
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

DOCS‑SORA замын газрын зураг нь олон нийтийн урьдчилан харах бүрийг ижил замаар явахыг шаарддаг.
Шүүгчдийн орон нутагт хэрэгжүүлдэг шалгах нийлбэрээр баталгаажсан багц. Энэ runbook-г ашигла
Шүүгчийг суулгасны дараа (болон урилгын зөвшөөрлийн тасалбарыг) тавьж дуусна
Онлайнаар урьдчилан үзэх бета хост.

## Урьдчилсан нөхцөл

- Шүүгч эхлэх долгионыг зөвшөөрч, урьдчилан харах трекерд нэвтэрсэн.
- Хамгийн сүүлийн үеийн портал бүтээц нь `docs/portal/build/` болон шалгах нийлбэрийн дагуу байна
  баталгаажуулсан (`build/checksums.sha256`).
- SoraFS урьдчилж харах итгэмжлэлүүд (Torii URL, эрх мэдэл, хувийн түлхүүр, илгээсэн
  epoch) орчны хувьсагчид эсвэл JSON тохиргоонд хадгалагддаг
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Хүссэн хостын нэрээр нээгдсэн DNS өөрчлөлтийн тасалбар (`docs-preview.sora.link`,
  `docs.iroha.tech` гэх мэт) дээр нь дуудлага дээр байгаа харилцагчид.

## Алхам 1 – Багцыг бүрдүүлж, баталгаажуулна уу

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Шалгалтын нийлбэрийн манифест байхгүй эсвэл шалгах скрипт нь үргэлжлүүлэхээс татгалздаг
урьдчилж харах олдвор бүрийг аудитын хяналтанд байлгаж, өөрчилсөн.

## Алхам 2 – SoraFS олдворуудыг багцлана

Статик сайтыг тодорхойлогч CAR/манифест хос болгон хувиргах. `ARTIFACT_DIR`
анхдагч нь `docs/portal/artifacts/`.

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

Үүсгэсэн `portal.car`, `portal.manifest.*`, тодорхойлогч, шалгах нийлбэрийг хавсаргана уу
урьдчилан харах долгионы тасалбарын манифест.

## Алхам 3 – Урьдчилан үзэх өөр нэрийг нийтлэх

Илчлэхэд бэлэн болмогц `--skip-submit` зүү туслагчийг **-гүйгээр** дахин ажиллуулна уу.
хост. JSON тохиргоо эсвэл тодорхой CLI тугуудыг нийлүүлнэ үү:

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

Энэ тушаал нь `portal.pin.report.json` гэж бичнэ.
`portal.manifest.submit.summary.json`, `portal.submit.response.json`,
урилгатай нотлох баримтын хамт илгээх ёстой.

## Алхам 4 – DNS таслах төлөвлөгөөг гарга

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

Үр дүнд нь JSON-г Ops-тэй хуваалцаарай, ингэснээр DNS шилжүүлэгч яг таарч байна
илэрхий задаргаа. Өмнөх тодорхойлогчийг буцаах эх сурвалж болгон дахин ашиглах үед,
`--previous-dns-plan path/to/previous.json` хавсаргана уу.

## Алхам 5 – Байршуулсан хостыг шалгана уу

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

Шинжилгээ нь үйлчилсэн хувилбарын шошго, CSP толгой хэсэг, гарын үсгийн мета өгөгдлийг баталгаажуулдаг.
Аудиторууд харахын тулд хоёр бүсээс тушаалыг давтана уу (эсвэл curl гаралтыг хавсаргана уу).
захын кэш дулаан байна.

## Нотлох баримтын багц

Урьдчилан үзэх долгионы тасалбарт дараах олдворуудыг оруулаад тэдгээрээс лавлана уу
урьсан имэйл:

| Олдвор | Зорилго |
|----------|---------|
| `build/checksums.sha256` | Багц нь CI бүтэцтэй тохирч байгааг нотолж байна. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Каноник SoraFS ачаалал + манифест. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Манифест илгээлт + бусад нэрийн холболт амжилттай болсныг харуулж байна. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS мета өгөгдөл (тасалбар, цонх, харилцагчид), чиглүүлэлтийн сурталчилгаа (`Sora-Route-Binding`) хураангуй, `route_plan` заагч (төлөвлөгөө JSON + толгойн загварууд), кэш цэвэрлэх мэдээлэл болон Ops-д зориулсан буцаах заавар. |
| `artifacts/sorafs/preview-descriptor.json` | Архив + шалгах нийлбэрийг холбосон гарын үсэгтэй тодорхойлогч. |
| `probe` гаралт | Шууд хөтлөгч хүлээгдэж буй хувилбарын шошгыг сурталчилж байгааг баталгаажуулна. |

Хост шууд нэвтэрсний дараа [урьдчилан үзэх урилга тоглуулах номыг] (./public-preview-invite.md) дагаарай.
холбоосыг түгээх, урилгыг бүртгэх, телеметрийг хянах.
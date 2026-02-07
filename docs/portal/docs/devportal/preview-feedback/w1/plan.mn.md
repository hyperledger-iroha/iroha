---
lang: mn
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c3eddff3a7b9b5dc4eac39251c9df72d0533a6e1b5865c716d54dc3b1c5de164
source_last_modified: "2025-12-29T18:16:35.108030+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-plan
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
---

| Зүйл | Дэлгэрэнгүй |
| --- | --- |
| Долгион | W1 — Түншүүд & Torii интеграторууд |
| Зорилтот цонх | 2025 оны 2-р улирал 3 дахь долоо хоног |
| Олдворын шошго (төлөвлөсөн) | `preview-2025-04-12` |
| Tracker асуудал | `DOCS-SORA-Preview-W1` |

## Зорилтууд

1. Түншийн урьдчилан харах нөхцлийн хууль эрх зүйн + засаглалын зөвшөөрлийг баталгаажуулна.
2. Урилгын багцад ашигласан "Try it" прокси болон телеметрийн агшин агшингуудыг үе шаттайгаар тавь.
3. Шалгах нийлбэрээр баталгаажсан урьдчилж харах олдвор болон шалгалтын үр дүнг сэргээнэ үү.
4. Урилгыг илгээхээс өмнө түншийн жагсаалт + хүсэлтийн загваруудыг эцэслэнэ үү.

## Даалгаврын задаргаа

| ID | Даалгавар | Эзэмшигч | Хугацаа | Статус | Тэмдэглэл |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Урьдчилан харах нөхцлийн нэмэлт | хуулийн зөвшөөрөл авах Docs/DevRel тэргүүлэх → Хууль эрх зүйн | 2025-04-05 | ✅ Дууссан | Хуулийн тасалбар `DOCS-SORA-Preview-W1-Legal` 2025‑04‑05-нд гарын үсэг зурсан; Tracker дээр PDF хавсаргасан. |
| W1-P2 | Зураг авах Үүнийг туршаад үзээрэй прокси хувилбарын цонх (2025‑04‑10) ба проксины эрүүл мэндийг баталгаажуулна уу | Docs/DevRel + Ops | 2025-04-06 | ✅ Дууссан | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` гүйцэтгэсэн 2025-04-06; CLI транскрипт + `.env.tryit-proxy.bak` архивлагдсан. |
| W1-P3 | Урьдчилан харах олдворыг бүтээх (`preview-2025-04-12`), `scripts/preview_verify.sh` + `npm run probe:portal` ажиллуулах, архивын тодорхойлогч/шалгах нийлбэр | Портал TL | 2025-04-08 | ✅ Дууссан | `artifacts/docs_preview/W1/preview-2025-04-12/` дор хадгалагдсан Artefact + баталгаажуулалтын бүртгэлүүд; хянагчтай хавсаргасан датчик гаралт. |
| W1-P4 | Түншийн элсэлтийн маягтыг (`DOCS-SORA-Preview-REQ-P01…P08`) хянаж, харилцагчдыг баталгаажуулах + NDA | Засаглалын холбоо | 2025-04-07 | ✅ Дууссан | Бүх найман хүсэлтийг зөвшөөрсөн (сүүлийн хоёр нь 2025-04-11-д батлагдсан); Tracker-д холбогдсон зөвшөөрлүүд. |
| W1-P5 | Урилгын хуулбарын ноорог (`docs/examples/docs_preview_invite_template.md` дээр үндэслэсэн), түнш тус бүрт `<preview_tag>`, `<request_ticket>` багц | Docs/DevRel тэргүүлэх | 2025-04-08 | ✅ Дууссан | Урилгын төслийг 2025‑04‑12 15:00UTC-д олдворын холбоосын хамт илгээсэн. |

## Нислэгийн өмнөх шалгах хуудас

> Зөвлөмж: 1-5-р алхмуудыг автоматаар гүйцэтгэхийн тулд `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`-г ажиллуул (бүтээх, шалгах нийлбэр баталгаажуулах, портал шалгах, холбоос шалгагч, Проксины шинэчлэлтийг туршиж үзэх). Энэ скрипт нь таны tracker асуудалд хавсаргаж болох JSON бүртгэлийг бүртгэдэг.

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12`-тай) `build/checksums.sha256` болон `build/release.json`-ийг сэргээх.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. Тодорхойлогчийн хажууд `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` ба архив `build/link-report.json`.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (эсвэл тохирох зорилтыг `--tryit-target`-ээр дамжуулан өгөх); шинэчлэгдсэн `.env.tryit-proxy`-г амлаж, `.bak`-г буцаахаар үлдээнэ үү.
6. W1 трекерийн асуудлыг бүртгэлийн замуудаар шинэчилнэ үү (тодорхойлогч шалгах нийлбэр, шалгалтын гаралт, прокси солихыг оролдох, Grafana агшин зуурын зураг).

## Нотлох баримт шалгах хуудас

- [x] `DOCS-SORA-Preview-W1` хавсаргасан гарын үсэг зурсан хууль ёсны зөвшөөрөл (PDF эсвэл тасалбарын холбоос).
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`-д зориулсан Grafana дэлгэцийн агшин.
- [x] `preview-2025-04-12` тодорхойлогч + `artifacts/docs_preview/W1/` дор хадгалагдсан шалгах нийлбэрийн бүртгэл.
- [x] `invite_sent_at` цагийн тэмдэглэгээ бүхий урилгын жагсаалтын хүснэгт (W1 трекерийн бүртгэлийг харна уу).
- [x] Санал хүсэлтийн олдворуудыг [`preview-feedback/w1/log.md`](./log.md)-д нэг хамтрагч бүрт нэг эгнээнд тусгав (2025-04-26-нд жагсаалт/телеметрийн/асуудлын өгөгдлийн хамт шинэчлэгдсэн).

Даалгаврууд ахих тусам энэ төлөвлөгөөг шинэчлэх; Замын зургийг хадгалахын тулд мөрдөгч үүнийг иш татдаг
аудит хийх боломжтой.

## Санал хүсэлтийн ажлын урсгал

1. Шүүмжлэгч бүрийн хувьд загварыг хуулбарлана
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   мета өгөгдлийг бөглөж, дууссан хуулбарыг доор хадгална
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Урилга, телеметрийн хяналтын цэгүүд болон шууд бүртгэл доторх нээлттэй асуудлуудыг нэгтгэн бичнэ үү
   [`preview-feedback/w1/log.md`](./log.md) тул засаглалын тоймчид давалгааг бүхэлд нь дахин тоглуулах боломжтой
   агуулахаас гарахгүйгээр.
3. Мэдлэг шалгах эсвэл судалгааны экспорт ирэх үед бүртгэлд тэмдэглэсэн олдворын замд хавсаргана уу.
   болон трекерийн асуудлыг хөндлөн холбоно.
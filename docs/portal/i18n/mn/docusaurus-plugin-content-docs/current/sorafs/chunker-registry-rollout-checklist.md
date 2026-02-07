---
id: chunker-registry-rollout-checklist
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
:::

# SoraFS Бүртгэлийг нэвтрүүлэх хяналтын хуудас

Энэхүү хяналтын хуудас нь шинэ chunker профайлыг сурталчлахад шаардагдах алхмуудыг багтаасан болно
ханган нийлүүлэгчийн элсэлтийн багцыг хянахаас эхлээд засаглалын дараа үйлдвэрлэл
дүрмийг соёрхон баталсан.

> **Хамрах хүрээ:** Өөрчлөгдсөн бүх хувилбарт хамаарна
> `sorafs_manifest::chunker_registry`, үйлчилгээ үзүүлэгчийн элсэлтийн дугтуй, эсвэл
> каноник бэхэлгээний багцууд (`fixtures/sorafs_chunker/*`).

## 1. Нислэгийн өмнөх баталгаажуулалт

1. Бэхэлгээг сэргээж, детерминизмыг баталгаажуулах:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Детерминизмын хэшийг баталгаажуул
   `docs/source/sorafs/reports/sf1_determinism.md` (эсвэл холбогдох профайл
   тайлан) сэргээгдсэн олдворуудтай таарч байна.
3. `sorafs_manifest::chunker_registry` хөрвүүлэлтийг баталгаажуулна уу
   `ensure_charter_compliance()` ажиллуулснаар:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Саналын хавтсыг шинэчлэх:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` дор Зөвлөлийн тэмдэглэл оруулах
   - Детерминизмын тайлан

## 2. Засаглалын гарын үсэг

1. Багаж бэлтгэх ажлын хэсгийн тайлан, саналын тоймыг Сора-д танилцуулна
   УИХ-ын дэд бүтцийн зөвлөл.
2. Зөвшөөрлийн дэлгэрэнгүй мэдээллийг дараах хэсэгт бичнэ үү
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. УИХ-ын гарын үсэгтэй дугтуйг бэхэлгээний хамт хэвлэнэ.
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Дугтуйг удирдах тусламжаар дамжуулан авах боломжтой эсэхийг шалгана уу:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Үзүүлэн гаргах

[Staging manifest playbook](./staging-manifest-playbook) хэсгээс үзнэ үү.
Эдгээр алхмуудын дэлгэрэнгүй танилцуулга.

1. Torii-г `torii.sorafs` илрүүлэлтийг идэвхжүүлж, зөвшөөрнө үү.
   хэрэгжилтийг идэвхжүүлсэн (`enforce_admission = true`).
2. Зөвшөөрөгдсөн үйлчилгээ үзүүлэгчийн элсэлтийн дугтуйг шатлалын бүртгэлд оруулна
   лавлах `torii.sorafs.discovery.admission.envelopes_dir`.
3. Үйлчилгээ үзүүлэгчийн зарыг илрүүлэх API-ээр дамжуулж байгааг баталгаажуулна уу:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Манифест/төлөвлөгөөний төгсгөлийн цэгүүдийг засаглалын гарчигтай дасгал хийх:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Телеметрийн хяналтын самбар (`torii_sorafs_*`) болон дохиоллын дүрмийг баталгаажуулна уу.
   алдаагүй шинэ профайл.

## 4. Үйлдвэрлэлийн эргэлт

1. Үйлдвэрлэлийн Torii зангилааны эсрэг шатлах алхмуудыг давт.
2. Идэвхжүүлэх цонхыг (огноо/цаг, хөнгөлөлтийн хугацаа, буцаах төлөвлөгөө) зарлана.
   оператор болон SDK сувгууд.
3. Дараахыг агуулсан хувилбарын PR-г нэгтгэнэ үү:
   - Шинэчлэгдсэн бэхэлгээ, дугтуй
   - Баримт бичгийн өөрчлөлт (дүрмийн лавлагаа, детерминизмын тайлан)
   - Замын зураг/статусыг шинэчлэх
4. Гарын үсэг зурсан олдворуудыг гарал үүслийн үүднээс архивлана.

## 5. Өргөлтийн дараах аудит

1. Эцсийн хэмжигдэхүүнийг (нээлтийн тоо, дуудах амжилтын хувь, алдаа) авах
   гистограмм) нэвтрүүлсэнээс хойш 24 цагийн дараа.
2. `status.md`-г богино хураангуйгаар шинэчилж, детерминизмын тайлан руу холбоно уу.
3. Дараах даалгавруудыг (жишээ нь, профайл бичих нэмэлт заавар) файлд оруулна уу
   `roadmap.md`.
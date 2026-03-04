---
id: dispute-revocation-runbook
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
:::

## Зорилго

Энэхүү runbook нь SoraFS хүчин чадалтай холбоотой маргааныг гаргах, хүчингүй болгох ажлыг зохицуулах, өгөгдлийг нүүлгэн шилжүүлэх ажлыг тодорхой хэмжээгээр баталгаажуулах замаар засаглалын операторуудыг удирдан чиглүүлдэг.

## 1. Болсон явдлыг үнэл

- **Триггерийн нөхцөл:** SLA зөрчлийг илрүүлэх (ажиллах хугацаа/PoR алдаа), хуулбарлах дутагдал эсвэл төлбөрийн үл ойлголцол.
- **Телеметрийг баталгаажуулах:** үйлчилгээ үзүүлэгчийн хувьд `/v1/sorafs/capacity/state` болон `/v1/sorafs/capacity/telemetry` агшин зуурын зургийг авах.
- **Оролцогч талуудад мэдэгдэх:** Хадгалах баг (үйлүүлэгчийн үйл ажиллагаа), Удирдлагын зөвлөл (шийдвэр гаргах байгууллага), Ажиглах боломжтой байдал (хяналтын самбарын шинэчлэл).

## 2. Нотлох баримтын багц бэлтгэх

1. Түүхий эд өлгийн зүйлс цуглуулах (телеметрийн JSON, CLI бүртгэл, аудиторын тэмдэглэл).
2. Детерминист архив (жишээ нь, tarball) болгон хэвийн болгох; бичлэг:
   - BLAKE3-256 дижест (`evidence_digest`)
   - Хэвлэл мэдээллийн төрөл (`application/zip`, `application/jsonl` гэх мэт)
   - Хостинг URI (объект хадгалах, SoraFS зүү, эсвэл Torii хандалттай төгсгөлийн цэг)
3. Багцыг нэг удаа бичих эрхтэй засаглалын нотлох баримт цуглуулах саванд хадгална.

## 3. Маргаан мэдүүлэх

1. `sorafs_manifest_stub capacity dispute`-д зориулсан тусгай JSON үүсгэх:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI-г ажиллуулна уу:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=ih58... \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json`-г хянан үзэх (төрөл, нотлох баримт, цагийн тэмдэглэгээг баталгаажуулах).
4. JSON хүсэлтийг Torii `/v1/sorafs/capacity/dispute` руу засаглалын гүйлгээний дарааллаар илгээнэ үү. `dispute_id_hex` хариултын утгыг авах; Энэ нь хүчингүй болгох арга хэмжээ болон аудитын тайлангуудыг зангуу болгодог.

## 4. Нүүлгэн шилжүүлэх, хүчингүй болгох

1. **Нөхцөл байдлын цонх:** үйлчилгээ үзүүлэгчид удахгүй цуцлагдах тухай мэдэгдэх; бодлого зөвшөөрвөл тогтоогдсон өгөгдлийг нүүлгэн шилжүүлэхийг зөвшөөрөх.
2. **`ProviderAdmissionRevocationV1` үүсгэх:**
   - Зөвшөөрөгдсөн шалтгаанаар `sorafs_manifest_stub provider-admission revoke` ашиглана уу.
   - Гарын үсэг болон хүчингүй болгох тоймыг шалгах.
3. **Хүчгүй болгохыг нийтлэх:**
   - Хүчингүй болгох хүсэлтээ Torii хаягаар илгээнэ үү.
   - Үйлчилгээ үзүүлэгчийн зарыг хаасан эсэхийг шалгаарай (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` авирах болно).
4. **Хяналтын самбарыг шинэчлэх:** үйлчилгээ үзүүлэгчийг хүчингүй болгосон гэж тэмдэглэж, маргааны ID-г лавлаж, нотлох баримтын багцыг холбоно уу.

## 5. Үхлийн дараах ба хяналт

- Хугацаа, үндсэн шалтгаан, засч залруулах арга хэмжээг засаглалын ослыг хянах төхөөрөмжид тэмдэглэ.
- Нөхөн төлбөрийг тодорхойлох (гадасны бууралт, хураамжийг эргүүлэн авах, харилцагчийн буцаан олголт).
- Суралцсан зүйлээ баримтжуулах; шаардлагатай бол SLA-ийн босго оноо эсвэл хяналтын дохиог шинэчлэх.

## 6. Лавлах материал

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (маргааны хэсэг)
- `docs/source/sorafs/provider_admission_policy.md` (цуцлах ажлын урсгал)
- Ажиглалтын хяналтын самбар: `SoraFS / Capacity Providers`

## Хяналтын хуудас

- [ ] Нотлох баримтын багцыг барьж аваад хэш хийсэн.
- [ ] Маргааны ачааллыг орон нутагт баталгаажуулсан.
- [ ] Torii маргаантай гүйлгээг хүлээн авлаа.
- [ ] Хүчингүй болгох үйлдлийг гүйцэтгэсэн (хэрэв батлагдсан бол).
- [ ] Хяналтын самбар/runbook шинэчлэгдсэн.
- [ ] Захиргааны зөвлөлд нас барсан.
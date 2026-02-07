---
id: chunker-registry-charter
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Charter
sidebar_label: Chunker Registry Charter
description: Governance charter for chunker profile submissions and approvals.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
:::

# SoraFS Chunker Registry Guvernance Charter

> **Сорагийн парламентын дэд бүтцийн зөвлөлөөс 2025-10-29-нд соёрхон баталсан (харна уу).
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Аливаа нэмэлт өөрчлөлтөд a
> албан ёсны засаглалын талаарх санал хураалт; хэрэгжүүлэх багууд энэ баримт бичигт хандах ёстой
> орлох дүрэм батлах хүртэл норматив.

Энэхүү дүрэм нь SoraFS chunker-ийг хөгжүүлэх үйл явц, үүргийг тодорхойлдог.
бүртгэл. Энэ нь [Chunker Profile Authoring Guide](./chunker-profile-authoring.md)-д хэрхэн шинэ болохыг тайлбарласнаар нэмэлт юм.

## Хамрах хүрээ

Дүрэм нь `sorafs_manifest::chunker_registry` болон дээрх бичилт бүрт хамаарна
Бүртгэлийг ашигладаг аливаа хэрэгсэлд (манифест CLI, үйлчилгээ үзүүлэгч-зар сурталчилгааны CLI,
SDK). Энэ нь өөр нэр болон шалгасан хувьсагчдыг хэрэгжүүлдэг
`chunker_registry::ensure_charter_compliance()`:

- Профайлын ID нь нэг хэвийн өсөлттэй эерэг бүхэл тоо юм.
- Каноник бариул `namespace.name@semver` **заавал** эхнийх нь гарч ирнэ.
- Alias мөр нь тайрагдсан, өвөрмөц бөгөөд каноник бариултай мөргөлдөхгүй
  бусад оруулгуудын.

## Дүрүүд

- **Зохиогч(ууд)** – саналыг бэлтгэх, бэхэлгээг сэргээх, цуглуулах
  детерминизмын нотолгоо.
- **Tooling Working Group (TWG)** – нийтлэгдсэнийг ашиглан саналыг баталгаажуулна
  хяналтын хуудсуудыг гаргаж, бүртгэлийн инвариантуудыг баталгаажуулдаг.
- **Засаглалын зөвлөл (ЗЗ)** – ХЗХ-ны тайланг хянаж, саналд гарын үсэг зурна
  дугтуйнд хийж, хэвлэн нийтлэх/хуулалтгүй болгох хугацааг баталдаг.
- **Хадгалах баг** – бүртгэлийн хэрэгжилтийг хөтөлж, нийтэлдэг
  баримт бичгийн шинэчлэлтүүд.

## Амьдралын мөчлөгийн ажлын урсгал

1. **Санал оруулах**
   - Зохиогч нь зохиогчийн гарын авлагаас баталгаажуулалтын хяналтын хуудсыг ажиллуулж, үүсгэнэ
     нь `ChunkerProfileProposalV1` JSON доор байна
     `docs/source/sorafs/proposals/`.
   - CLI гаралтыг оруулах:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Бэхэлгээ, санал, детерминизмын тайлан, бүртгэлийг агуулсан PR илгээнэ үү
     шинэчлэлтүүд.

2. **Хэрэгслийн тойм (TWG)**
   - Баталгаажуулах хяналтын хуудсыг дахин тоглуул (тэхмэлүүд, бүдэг бадаг, манифест/PoR дамжуулах хоолой).
   - `cargo test -p sorafs_car --chunker-registry` ажиллуулаад баталгаажуулна уу
     `ensure_charter_compliance()` шинэ оруулгатай хамт дамждаг.
   - CLI-ийн үйлдлийг шалгах (`--list-profiles`, `--promote-profile`, урсгал
     `--json-out=-`) нь шинэчлэгдсэн нэр болон бариулыг тусгасан.
   - Дүгнэлт болон тэнцсэн/унасан байдлын талаарх товч тайлан гаргах.

3. **Зөвлөлийн зөвшөөрөл (ЗХ)**
   - TWG-ийн тайлан болон саналын мета өгөгдлийг хянана.
   - Саналын хураамжид гарын үсэг зурах (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     мөн зөвлөлийн дугтуйнд гарын үсгээ хавсаргана
     бэхэлгээ.
   - Санал хураалтын дүнг засаглалын протоколд тэмдэглэнэ.

4. **Хэвлэн нийтлэх**
   - PR-г нэгтгэж, шинэчлэх:
     - `sorafs_manifest::chunker_registry_data`.
     - Баримт бичиг (`chunker_registry.md`, зохиох/тохирлын удирдамж).
     - Засвар ба детерминизмын тайлан.
   - Операторууд болон SDK багуудад шинэ профайл болон төлөвлөсөн танилцуулгын талаар мэдэгдэх.

5. **Эсэх / Нар жаргах**
   - Одоо байгаа профайлыг орлох саналууд нь давхар нийтэлсэн байх ёстой
     цонх (хөнгөлөлтийн хугацаа) болон шинэчлэх төлөвлөгөө.
     бүртгэлд оруулж, шилжилт хөдөлгөөний дэвтэрийг шинэчлэх.

6. **Онцгой байдлын өөрчлөлт**
   - Устгах эсвэл засварлахын тулд олонхийн зөвшөөрлөөр зөвлөлийн санал хураалт шаардлагатай.
   - TWG эрсдэлийг бууруулах алхмуудыг баримтжуулж, ослын бүртгэлийг шинэчлэх ёстой.

## Багажны хүлээлт

- `sorafs_manifest_chunk_store` ба `sorafs_manifest_stub` дараахыг илтгэнэ.
  - Бүртгэлийн шалгалтын `--list-profiles`.
  - Ашигласан каноник мета өгөгдлийн блок үүсгэхийн тулд `--promote-profile=<handle>`
    профайлыг сурталчлах үед.
  - `--json-out=-` нь тайланг stdout руу дамжуулж, дахин давтагдах боломжтой
    бүртгэлүүд.
- `ensure_charter_compliance()` нь холбогдох хоёртын файлуудыг эхлүүлэх үед дуудагддаг
  (`manifest_chunk_store`, `provider_advert_stub`). Хэрэв шинэ бол CI тест амжилтгүй болох ёстой
  бичлэгүүд дүрмийг зөрчсөн.

## Албан хэрэг хөтлөлт

- Бүх детерминизмын тайланг `docs/source/sorafs/reports/`-д хадгална.
- Зөрчлийн шийдвэрийг лавлах Зөвлөлийн протокол шууд доор байна
  `docs/source/sorafs/migration_ledger.md`.
- Бүртгэлийн томоохон өөрчлөлт бүрийн дараа `roadmap.md` болон `status.md`-г шинэчилнэ үү.

## Лавлагаа

- Зохиогчийн гарын авлага: [Chunker Profile Authoring Guide](./chunker-profile-authoring.md)
- Тохирлын хяналтын хуудас: `docs/source/sorafs/chunker_conformance.md`
- Бүртгэлийн лавлагаа: [Chunker Profile Registry](./chunker-registry.md)
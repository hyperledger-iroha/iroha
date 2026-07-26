---
slug: /norito/ledger-walkthrough
lang: mn
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Энэ заавар нь [Norito хурдан эхлүүлэх](./quickstart.md)-ийг харуулж байна.
`iroha` CLI ашиглан дэвтэрийн төлөвийг хэрхэн өөрчлөх, шалгах. Та бүртгүүлнэ a
шинэ хөрөнгийн тодорхойлолт, үндсэн оператор данс руу зарим нэгж гаа, шилжүүлэх
үлдэгдлийн хэсгийг өөр данс руу шилжүүлж, гүйлгээг баталгаажуулна уу
болон холдинг. Алхам бүр нь Rust/Python/JavaScript-д тусгагдсан урсгалуудыг тусгадаг
SDK хурдан ажилладаг тул та CLI болон SDK-ийн үйл ажиллагааны тэнцвэрийг баталгаажуулах боломжтой.

## Урьдчилсан нөхцөл

- [Түргэн эхлүүлэх](./quickstart.md)-ыг дагаж нэг үет сүлжээг ачаалах
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- `iroha` (CLI) бүтээгдсэн эсвэл татаж авсан эсэхээ шалгаарай.
  `defaults/client.toml` ашиглан үе тэнгийн.
- Нэмэлт туслахууд: `jq` (JSON хариултуудыг форматлах) ба POSIX бүрхүүл.
  байгаль орчны хувьсагчийн хэсгүүдийг доор ашигласан.

Гарын авлагын туршид `$ADMIN_ACCOUNT` болон `$RECEIVER_ACCOUNT`-г
таны ашиглахаар төлөвлөж буй дансны ID. Өгөгдмөл багцад аль хэдийн хоёр бүртгэл орсон байна
Демо түлхүүрүүдээс гаралтай:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

Эхний хэдэн дансыг жагсааж утгыг баталгаажуулна уу:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Генезисийн төлөв байдлыг шалгах

CLI-ийн зорилтот дэвтэрийг судалж эхэл:

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland.universal"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

Эдгээр командууд нь Norito-ээр дэмжигдсэн хариултууд дээр тулгуурладаг тул шүүлт болон хуудаслах
тодорхойлогч бөгөөд SDK-ийн хүлээн авсан зүйлтэй таарч байна.

## 2. Хөрөнгийн тодорхойлолтыг бүртгэх

`wonderland` дотор `coffee` нэртэй шинэ, хязгааргүй мөнгө үүсгээрэй
домэйн:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI нь илгээсэн гүйлгээний хэшийг хэвлэдэг (жишээлбэл,
`0x5f…`). Дараа нь статусыг асуухын тулд үүнийг хадгалаарай.

## 3. Нэгжийг операторын дансанд оруулна

Хөрөнгийн тоо хэмжээ нь `(asset definition, account)` хосын дор амьдардаг. гаа 250
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` нэгжийг `$ADMIN_ACCOUNT` болгон:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Дахин хэлэхэд, CLI гаралтаас гүйлгээний хэшийг (`$MINT_HASH`) аваарай. руу
үлдэгдлийг давхар шалгаад ажиллуулна уу:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

эсвэл зөвхөн шинэ хөрөнгийг чиглүүлэхийн тулд:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Үлдэгдэлийн хэсгийг өөр данс руу шилжүүлнэ

Операторын данснаас 50 нэгжийг `$RECEIVER_ACCOUNT` руу шилжүүлнэ үү:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Гүйлгээний хэшийг `$TRANSFER_HASH` гэж хадгал. Хоёулангийнх нь хувьцааг асуу
шинэ үлдэгдлийг шалгах дансууд:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Бүртгэлийн нотлох баримтыг баталгаажуулах

Хадгалсан хэшүүдийг ашиглан хоёр гүйлгээ хийгдсэнийг баталгаажуулна уу:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Мөн та аль блок шилжүүлгийг оруулсан болохыг харахын тулд сүүлийн блокуудыг дамжуулж болно:

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Дээрх тушаал бүр нь SDK-тай ижил Norito ачааллыг ашигладаг. Хэрэв та хуулбарлавал
энэ урсгал нь кодоор дамждаг (доорх SDK-н хурдан эхлэлийг харна уу), хэш болон үлдэгдэл болно
Хэрэв та ижил сүлжээ болон өгөгдмөл рүү чиглэж байгаа бол жагсана уу.

## SDK паритын холбоосууд

- [Rust SDK quickstart](../sdks/rust) — бүртгүүлэх зааврыг харуулж байна,
  гүйлгээ илгээх, Rust-аас санал авах статус.
- [Python SDK хурдан эхлүүлэх](../sdks/python) — ижил бүртгэл/минтийг харуулдаг
  Norito-д тулгуурласан JSON туслагчтай үйлдлүүд.
- [JavaScript SDK хурдан эхлүүлэх](../sdks/javascript) — Torii хүсэлтийг хамарна,
  засаглалын туслахууд, шивсэн асуулга.

Эхлээд CLI зааварчилгааг ажиллуулж, дараа нь сонгосон SDK-тэй хувилбарыг давт
гүйлгээний хэш, үлдэгдэл, асуулга дээр хоёр гадаргуу хоёулаа тохирч байгаа эсэхийг шалгах
гаралт.
---
lang: mn
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e39dc94f52395bd9323177df1a7feeb7bbd4f9a3cdea07b02f9d60e7826e199e
source_last_modified: "2026-01-22T16:26:46.506936+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Quickstart
description: Build, validate, and deploy a Kotodama contract with the release tooling and default single-peer network.
slug: /norito/quickstart
translator: machine-google-reviewed
---

Энэхүү танилцуулга нь хөгжүүлэгчид суралцахдаа дагаж мөрдөх ёстой ажлын урсгалыг тусгасан болно
Norito болон Kotodama нь анх удаа: тодорхойлогч нэг үет сүлжээг ачаалах,
гэрээг эмхэтгэж, дотооддоо хатаагаад дараа нь Torii-ээр дамжуулан илгээнэ үү.
лавлагаа CLI.

Гэрээний жишээ нь дуудлага хийгчийн дансанд түлхүүр/утга хос бичдэг тул та боломжтой
`iroha_cli` ашиглан гаж нөлөөг нэн даруй шалгана уу.

## Урьдчилсан нөхцөл

- Compose V2-г идэвхжүүлсэн [Docker](https://docs.docker.com/engine/install/) (ашигласан)
  `defaults/docker-compose.single.yml`-д тодорхойлсон түүврийн үе тэнгийн загварыг эхлүүлэх).
- Хэрэв та татаж авахгүй бол туслах хоёртын файлыг бүтээхэд зориулсан Rust toolchain (1.76+).
  хэвлэгдсэн нь.
- `koto_compile`, `ivm_run`, `iroha_cli` хоёртын файлууд. Та тэдгээрийг дээрээс нь барьж болно
  Доор үзүүлсэн шиг ажлын талбарыг шалгах эсвэл тохирох хувилбарыг татаж авах:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Дээрх хоёртын файлуудыг бусад ажлын талбартай зэрэгцүүлэн суулгахад аюулгүй.
> Тэд хэзээ ч `serde`/`serde_json`-тэй холбогддог; Norito кодекууд нь төгсгөлөөс төгсгөл хүртэл хэрэгждэг.

## 1. Нэг үет хөгжүүлэлтийн сүлжээг эхлүүлэх

Хадгалах газар нь `kagami swarm` үүсгэсэн Docker Compose багцыг агуулдаг.
(`defaults/docker-compose.single.yml`). Энэ нь анхдагч генезийг утсаар холбодог, үйлчлүүлэгч
тохиргоо, эрүүл мэндийн датчикууд, тиймээс Torii `http://127.0.0.1:8080` хаягаар холбогдож болно.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Савыг ажиллуулж орхи (урд талд эсвэл салгасан). Бүгд
дараагийн CLI дуудлагууд нь `defaults/client.toml`-ээр дамжуулан энэ үе тэнгийнхэн рүү чиглэдэг.

## 2. Гэрээг зохиогч

Ажлын лавлах үүсгээд хамгийн бага Kotodama жишээг хадгал:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Kotodama эх сурвалжийг хувилбарын удирдлагад байлгахыг илүүд үзнэ үү. Портал дээр байрлуулсан жишээнүүд
> мөн хэрэв та [Norito жишээ галлерей](./examples/) дээрээс авах боломжтой.
> илүү баялаг эхлэлийн цэгийг хүсч байна.

## 3. IVM ашиглан эмхэтгэж хуурай ажиллуулна

Гэрээг IVM/Norito байт код (`.to`) болгон эмхэтгэж, дотооддоо гүйцэтгэнэ.
Сүлжээнд хүрэхээсээ өмнө хост системийн дуудлагууд амжилттай ажиллаж байгааг баталгаажуулна уу:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Гүйгч нь `info("Hello from Kotodama")` бүртгэлийг хэвлэж, гүйцэтгэнэ
Дооглогдсон хостын эсрэг `SET_ACCOUNT_DETAIL` систем. Хэрэв нэмэлт `ivm_tool`
хоёртын хувилбар боломжтой, `ivm_tool inspect target/quickstart/hello.to`-г харуулна
ABI толгой хэсэг, онцлог битүүд болон экспортлогдсон нэвтрэх цэгүүд.

## 4. Torii-ээр дамжуулан байт кодыг илгээнэ үү

Зангилаа ажиллаж байгаа үед CLI ашиглан эмхэтгэсэн байт кодыг Torii руу илгээнэ үү.
Өгөгдмөл хөгжүүлэлтийн таних тэмдэг нь нийтийн түлхүүрээс үүсэлтэй
`defaults/client.toml` тул дансны ID нь байна
```
soraカタカナ...
```

Torii URL, гинжин ID болон гарын үсэг зурах түлхүүрийг оруулахын тулд тохиргооны файлыг ашиглана уу:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI нь гүйлгээг Norito-ээр кодлож, dev түлхүүрээр гарын үсэг зурж,
гүйж байгаа үе тэнгийнхэнд нь оруулдаг. `set_account_detail`-ийн Docker бүртгэлийг үзээрэй
syscall эсвэл хийсэн гүйлгээний хэшийн CLI гаралтыг хянах.

## 5. Төрийн өөрчлөлтийг баталгаажуулна уу

Гэрээнд бичсэн дансны дэлгэрэнгүй мэдээллийг авахын тулд CLI профайлыг ашиглана уу:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Та Norito дэмждэг JSON ачааллыг харах ёстой:

```json
{
  "hello": "world"
}
```

Хэрэв утга байхгүй бол Docker бичих үйлчилгээ хэвээр байгаа эсэхийг баталгаажуулна уу.
ажиллаж байгаа бөгөөд `iroha`-ийн мэдээлсэн гүйлгээний хэш нь `Committed`-д хүрсэн байна.
муж.

## Дараагийн алхамууд

- Автоматаар үүсгэгдсэн [жишээ галерей](./examples/)-тай танилцана уу.
  Norito системтэй Norito илүү дэвшилтэт хэсгүүдийн зураглал.
- Илүү гүнзгийрүүлэхийн тулд [Norito эхлэх гарын авлагыг](./getting-started) уншина уу.
  хөрвүүлэгч/гүйгч хэрэгслийн тайлбар, манифест байршуулалт, IVM
  мета өгөгдөл.
- Өөрийнхөө гэрээг давтахдаа `npm run sync-norito-snippets`-г ашиглана уу
  ажлын талбар нь татаж авах боломжтой хэсгүүдийг сэргээхийн тулд портал баримт бичиг болон олдворууд үлдэх болно
  `crates/ivm/docs/examples/` доорх эх сурвалжуудтай синхрончлогдсон.
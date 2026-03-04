---
slug: /norito/quickstart
lang: ka
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Quickstart
description: Build, validate, and deploy a Kotodama contract with the release tooling and default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ეს მიმოხილვა ასახავს სამუშაო პროცესს, რომელსაც დეველოპერები სწავლის დროს დაიცავენ
Norito და Kotodama პირველად: ჩატვირთეთ განმსაზღვრელი ერთჯერადი ქსელი,
შეადგინეთ კონტრაქტი, გაუშვით ლოკალურად, შემდეგ გააგზავნეთ Torii-ით
მითითება CLI.

მაგალითი კონტრაქტი წერს გასაღები/მნიშვნელობის წყვილს აბონენტის ანგარიშზე, ასე რომ თქვენ შეგიძლიათ
დაუყოვნებლივ გადაამოწმეთ გვერდითი ეფექტი `iroha_cli`-ით.

## წინაპირობები

- [Docker](https://docs.docker.com/engine/install/) Compose V2 ჩართულია (გამოყენებული
  `defaults/docker-compose.single.yml`-ში განსაზღვრული ნიმუშის თანატოლის დასაწყებად).
- Rust toolchain (1.76+) დამხმარე ბინარების შესაქმნელად, თუ არ ჩამოტვირთავთ
  გამოქვეყნებულები.
- `koto_compile`, `ivm_run` და `iroha_cli` ბინარები. თქვენ შეგიძლიათ ააწყოთ ისინი
  სამუშაო სივრცის გადახდა, როგორც ნაჩვენებია ქვემოთ, ან ჩამოტვირთეთ შესაბამისი გამოშვების არტეფაქტები:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> ზემოთ მოცემული ბინარები უსაფრთხოა დანარჩენი სამუშაო სივრცის გვერდით დასაყენებლად.
> ისინი არასოდეს აკავშირებენ `serde`/`serde_json`-ს; Norito კოდეკები სრულდება ბოლომდე.

## 1. დაიწყეთ ერთი თანატოლი დეველოპერის ქსელი

საცავი მოიცავს Docker Compose პაკეტს, რომელიც გენერირებულია `kagami swarm`-ის მიერ
(`defaults/docker-compose.single.yml`). იგი აკავშირებს ნაგულისხმევი გენეზის, კლიენტს
კონფიგურაცია და ჯანმრთელობის გამოკვლევები, ასე რომ, Torii ხელმისაწვდომია `http://127.0.0.1:8080`-ზე.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

დატოვეთ კონტეინერი გაშვებული (წინა პლანზე ან ცალკე). ყველა
შემდგომი CLI ზარები მიმართულია ამ თანატოლის მეშვეობით `defaults/client.toml`-ის საშუალებით.

## 2. ხელშეკრულების ავტორი

შექმენით სამუშაო დირექტორია და შეინახეთ მინიმალური Kotodama მაგალითი:

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

> უპირატესობა მიანიჭეთ Kotodama წყაროების შენარჩუნებას ვერსიის კონტროლში. პორტალზე განთავსებული მაგალითებია
> ასევე ხელმისაწვდომია [Norito მაგალითების გალერეაში](./examples/) თუ თქვენ
> მინდა უფრო მდიდარი საწყისი წერტილი.

## 3. შედგენა და მშრალი გაშვება IVM-ით

შეადგინეთ კონტრაქტი IVM/Norito ბაიტიკოდზე (`.to`) და შეასრულეთ იგი ადგილობრივად
დაადასტურეთ, რომ მასპინძელი სისტემის ზარები წარმატებულია ქსელთან შეხებამდე:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

მორბენალი ბეჭდავს `info("Hello from Kotodama")` ჟურნალს და ასრულებს
`SET_ACCOUNT_DETAIL` syscall დამცინავი მასპინძლის წინააღმდეგ. თუ სურვილისამებრ `ivm_tool`
ორობითი ხელმისაწვდომია, `ivm_tool inspect target/quickstart/hello.to` აჩვენებს
ABI სათაური, ფუნქციის ბიტები და ექსპორტირებული შესასვლელი წერტილები.

## 4. გაგზავნეთ ბაიტიკოდი Torii-ის საშუალებით

როდესაც კვანძი ჯერ კიდევ მუშაობს, გაგზავნეთ შედგენილი ბაიტეკოდი Torii-ზე CLI-ის გამოყენებით.
განვითარების ნაგულისხმევი იდენტურობა მიღებულია საჯარო გასაღებიდან
`defaults/client.toml`, ასე რომ, ანგარიშის ID არის
```
ih58...
```

გამოიყენეთ კონფიგურაციის ფაილი Torii URL-ის, ჯაჭვის ID-ისა და ხელმოწერის გასაღების მოსაწოდებლად:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI დაშიფვრავს ტრანზაქციას Norito-ით, ხელს აწერს მას dev გასაღებით და
წარუდგენს მას გაშვებულ თანატოლს. უყურეთ Docker ჟურნალებს `set_account_detail`-ისთვის
syscall ან მონიტორინგი CLI გამომავალი ჩადენილი ტრანზაქციის ჰეშისთვის.

## 5. გადაამოწმეთ მდგომარეობის ცვლილება

გამოიყენეთ იგივე CLI პროფილი, რათა მიიღოთ ანგარიშის დეტალები, რომლებიც წერია კონტრაქტში:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

თქვენ უნდა ნახოთ Norito მხარდაჭერილი JSON დატვირთვა:

```json
{
  "hello": "world"
}
```

თუ მნიშვნელობა აკლია, დაადასტურეთ, რომ Docker შედგენის სერვისი ჯერ კიდევ მუშაობს
გაშვებული და `iroha`-ის მიერ მოხსენებული ტრანზაქციის ჰეშმა მიაღწია `Committed`-ს
სახელმწიფო.

## შემდეგი ნაბიჯები

- გამოიკვლიეთ ავტომატურად გენერირებული [მაგალითების გალერეა] (./examples/) სანახავად
  რამდენად უფრო მოწინავე Kotodama ფრაგმენტები ასახავს Norito სისტემას.
- წაიკითხეთ [Norito დაწყების სახელმძღვანელო] (./getting-started) უფრო ღრმად
  შემდგენელის/მწარმოებლის ხელსაწყოების ახსნა, მანიფესტის განლაგება და IVM
  მეტამონაცემები.
- საკუთარ კონტრაქტებზე გამეორებისას გამოიყენეთ `npm run sync-norito-snippets`
  სამუშაო სივრცე ჩამოსატვირთი სნიპეტების რეგენერაციისთვის, რათა დარჩეს პორტალის დოკუმენტები და არტეფაქტები
  სინქრონიზებულია `crates/ivm/docs/examples/` წყაროებთან.
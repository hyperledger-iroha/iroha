---
lang: hy
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

Այս ուղեցույցը արտացոլում է աշխատանքի ընթացքը, որը մենք ակնկալում ենք, որ մշակողները հետևեն սովորելիս
Norito և Kotodama առաջին անգամ. գործարկեք դետերմինիստական մեկ հասակակից ցանց,
կազմել պայմանագիր, չորացնել այն լոկալ, այնուհետև ուղարկել այն Torii-ի միջոցով
հղում CLI.

Օրինակ պայմանագիրը գրում է բանալի/արժեք զույգ զանգահարողի հաշվին, որպեսզի կարողանաք
անմիջապես ստուգեք կողմնակի ազդեցությունը `iroha_cli`-ով:

## Նախադրյալներ

- [Docker](https://docs.docker.com/engine/install/) Compose V2-ով միացված (օգտագործված
  սկսելու համար `defaults/docker-compose.single.yml`-ում սահմանված օրինակելի հավասարումը):
- Rust Toolchain (1.76+) օգնական երկուական սարքեր ստեղծելու համար, եթե չներբեռնեք
  հրապարակվածները։
- `koto_compile`, `ivm_run` և `iroha_cli` երկուականներ: Դուք կարող եք դրանք կառուցել
  Աշխատանքային տարածքի վճարում, ինչպես ցույց է տրված ստորև, կամ ներբեռնեք համապատասխան թողարկման արտեֆակտները.

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Վերևում գտնվող երկուականները ապահով տեղադրվում են մնացած աշխատանքային տարածքի հետ միասին:
> Նրանք երբեք չեն կապում `serde`/`serde_json`-ին; Norito կոդեկները կիրառվում են ծայրից ծայր:

## 1. Սկսեք մեկ հասակակից մշակող ցանց

Պահեստը ներառում է Docker Compose փաթեթ, որը ստեղծվել է `kagami swarm`-ի կողմից
(`defaults/docker-compose.single.yml`): Այն միացնում է լռելյայն ծագումը, հաճախորդ
կոնֆիգուրացիան և առողջական զոնդերը, որպեսզի Torii հասանելի լինի `http://127.0.0.1:8080`-ով:

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Բեռնարկղը թողեք աշխատի (առաջին պլանում կամ անջատված): Բոլորը
CLI-ի հաջորդ զանգերը ուղղված են այս հասակակիցին `defaults/client.toml`-ի միջոցով:

## 2. Հեղինակել պայմանագիրը

Ստեղծեք աշխատանքային գրացուցակ և պահպանեք նվազագույն Kotodama օրինակը.

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

> Նախընտրում են Kotodama աղբյուրները տարբերակի վերահսկման մեջ պահել: Պորտալում տեղակայված օրինակներն են
> հասանելի է նաև [Norito օրինակների պատկերասրահում] (./examples/), եթե դուք
> ցանկանում եք ավելի հարուստ մեկնարկային կետ:

## 3. Կազմել և չորացնել IVM-ով

Կազմեք պայմանագիրը IVM/Norito բայթկոդով (`.to`) և կատարեք այն լոկալ՝
հաստատեք, որ հյուրընկալող համակարգային զանգերը հաջողվում են նախքան ցանցին դիպչելը.

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Վազողը տպում է `info("Hello from Kotodama")` մատյանը և կատարում
`SET_ACCOUNT_DETAIL` syscall ընդդեմ ծաղրված հյուրընկալողի: Եթե ընտրովի `ivm_tool`
Երկուական հասանելի է, `ivm_tool inspect target/quickstart/hello.to` ցուցադրում է
ABI վերնագիր, հատկանիշի բիթեր և արտահանվող մուտքի կետեր:

## 4. Ներկայացրե՛ք բայթ կոդը Torii-ի միջոցով

Երբ հանգույցը դեռ աշխատում է, ուղարկեք կազմված բայթկոդը Torii-ին՝ օգտագործելով CLI:
Լռելյայն զարգացման նույնականացումը բխում է մուտքի հանրային բանալիից
`defaults/client.toml`, ուստի հաշվի ID-ն է
```
ih58...
```

Օգտագործեք կազմաձևման ֆայլը՝ Torii URL-ը, շղթայի ID-ն և ստորագրման բանալի մատակարարելու համար.

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI-ն կոդավորում է գործարքը Norito-ով, ստորագրում է այն մշակող բանալիով և
այն ներկայացնում է վազող հասակակիցին: Դիտեք Docker տեղեկամատյանները `set_account_detail`-ի համար
syscall կամ վերահսկել CLI ելքը կատարված գործարքի հեշի համար:

## 5. Ստուգեք վիճակի փոփոխությունը

Օգտագործեք նույն CLI պրոֆիլը՝ հաշվի մանրամասները ստանալու համար, որոնք գրված են պայմանագրում.

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Դուք պետք է տեսնեք Norito-ով ապահովված JSON ծանրաբեռնվածությունը.

```json
{
  "hello": "world"
}
```

Եթե արժեքը բացակայում է, հաստատեք, որ Docker կոմպոզիցիոն ծառայությունը դեռ գործում է
աշխատում է, և որ `iroha`-ի կողմից ներկայացված գործարքի հեշը հասել է `Committed`-ին
պետություն.

## Հաջորդ քայլերը

- Բացահայտեք ավտոմատ ստեղծվող [օրինակ պատկերասրահը] (./examples/) տեսնելու համար
  որքան ավելի առաջադեմ Kotodama հատվածները քարտեզագրում են Norito syscalls-ին:
- Կարդացեք [Norito մեկնարկի ուղեցույցը] (./getting-started) ավելի խորը
  կոմպիլյատորի/գործարկողի գործիքավորման բացատրություն, մանիֆեստի տեղակայում և IVM
  մետատվյալներ.
- Երբ կրկնում եք ձեր սեփական պայմանագրերը, օգտագործեք `npm run sync-norito-snippets`-ը
  աշխատանքային տարածք՝ ներբեռնվող հատվածները վերականգնելու համար, որպեսզի պորտալի փաստաթղթերն ու արտեֆակտները մնան
  համաժամեցված է `crates/ivm/docs/examples/`-ի աղբյուրների հետ:
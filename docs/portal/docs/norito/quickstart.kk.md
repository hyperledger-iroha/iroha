---
lang: kk
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

Бұл шолу әзірлеушілер үйрену кезінде орындайтын жұмыс үрдісін көрсетеді
Norito және Kotodama бірінші рет: детерминирленген бір деңгейлі желіні жүктеңіз,
келісімшартты құрастырыңыз, оны жергілікті түрде құрғатыңыз, содан кейін оны Torii арқылы жіберіңіз.
сілтеме CLI.

Келісім-шарт мысалында қоңырау шалушының тіркелгісіне кілт/мән жұбын жаза аласыз
жанама әсерді `iroha_cli` арқылы дереу тексеріңіз.

## Алғышарттар

- [Docker](https://docs.docker.com/engine/install/) Compose V2 қосулы (пайдаланылған)
  `defaults/docker-compose.single.yml` ішінде анықталған тең үлгіні бастау үшін).
- Жүктеп алмасаңыз, көмекші екілік файлдарды құруға арналған Rust құралдар тізбегі (1.76+).
  жарияланғандары.
- `koto_compile`, `ivm_run` және `iroha_cli` екілік файлдары. Сіз оларды мына жерден құрастыра аласыз
  төменде көрсетілгендей жұмыс кеңістігін тексеру немесе сәйкес шығарылым артефактілерін жүктеп алыңыз:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Жоғарыдағы екілік файлдарды жұмыс кеңістігінің қалған бөлігімен қатар орнату қауіпсіз.
> Олар ешқашан `serde`/`serde_json`-ге сілтеме жасайды; Norito кодектері басынан аяғына дейін орындалады.

## 1. Бір деңгейлі әзірлеуші ​​желісін іске қосыңыз

Репозиторийде `kagami swarm` арқылы жасалған Docker Compose бумасы бар
(`defaults/docker-compose.single.yml`). Бұл әдепкі генезиске, клиентке сымдар
конфигурациясын және денсаулық зондтарын, сондықтан Torii `http://127.0.0.1:8080` мекенжайында қол жеткізуге болады.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Контейнерді жұмыс істеп тұрған күйде қалдырыңыз (алдыңғы жоспарда немесе бөлек). Барлығы
кейінгі CLI қоңыраулары `defaults/client.toml` арқылы осы теңдесті мақсатты етеді.

## 2. Келісімшарттың авторы

Жұмыс каталогын жасаңыз және минималды Kotodama мысалын сақтаңыз:

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

> Kotodama көздерін нұсқаны басқаруда сақтауды жөн көріңіз. Порталда орналастырылған мысалдар
> [Norito мысалдар галереясы](./examples/) астында да қолжетімді
> бай бастау нүктесін қалайсыз.

## 3. IVM көмегімен құрастырыңыз және құрғатыңыз

IVM/Norito байт кодына (`.to`) келісім-шартты құрастырыңыз және оны жергілікті түрде орындаңыз
желіге қол тигізбес бұрын хост жүйесін шақыруларының сәтті екенін растаңыз:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Жүгіруші `info("Hello from Kotodama")` журналын басып шығарады және орындайды
`SET_ACCOUNT_DETAIL` келекеленген хостқа қарсы жүйе қоңырауы. Қосымша `ivm_tool` болса
екілік қол жетімді, `ivm_tool inspect target/quickstart/hello.to` көрсетеді
ABI тақырыбы, мүмкіндік биттері және экспортталған кіру нүктелері.

## 4. Torii арқылы байт кодты жіберіңіз

Түйін әлі жұмыс істеп тұрғанда, құрастырылған байткодты CLI арқылы Torii жіберіңіз.
Әдепкі әзірлеу идентификаторы ашық кілттен алынған
`defaults/client.toml`, сондықтан тіркелгі идентификаторы
```
<katakana-i105-account-id>
```

Torii URL мекенжайын, тізбек идентификаторын және қол қою кілтін беру үшін конфигурация файлын пайдаланыңыз:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI транзакцияны Norito арқылы кодтайды, оған dev кілтімен қол қояды және
оны жүгіріп келе жатқан теңдесімен ұсынады. `set_account_detail` үшін Docker журналдарын қараңыз
syscall немесе бекітілген транзакция хэшіне арналған CLI шығысын бақылаңыз.

## 5. Күйдің өзгеруін тексеріңіз

Келісімшарт жазған тіркелгі мәліметтерін алу үшін бірдей CLI профилін пайдаланыңыз:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <katakana-i105-account-id> \
  --key example | jq .
```

Norito қолдайтын JSON пайдалы жүктемесін көруіңіз керек:

```json
{
  "hello": "world"
}
```

Мән жоқ болса, Docker құрастыру қызметінің әлі де екенін растаңыз.
іске қосылды және `iroha` хабарлаған транзакция хэші `Committed` мәніне жетті
күй.

## Келесі қадамдар

- Көру үшін автоматты түрде жасалған [мысал галереясын](./examples/) зерттеңіз
  қаншалықты жетілдірілген Kotodama үзінділері Norito жүйе қоңырауларына сәйкес келеді.
- Тереңірек білу үшін [Norito бастау нұсқаулығын](./getting-started) оқыңыз.
  компилятор/жұмыс құралының түсіндірмесі, манифестті орналастыру және IVM
  метадеректер.
- Өз келісім-шарттарыңызды қайталау кезінде `npm run sync-norito-snippets` пайдаланыңыз
  Портал құжаттары мен артефактілер қалуы үшін жүктеп алынатын үзінділерді қалпына келтіруге арналған жұмыс кеңістігі
  `crates/ivm/docs/examples/` астындағы көздермен синхрондалған.
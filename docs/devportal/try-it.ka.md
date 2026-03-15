---
lang: ka
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-12-29T18:16:35.067551+00:00"
translation_last_reviewed: 2026-02-07
title: Try It Sandbox Guide
summary: How to run the Torii staging proxy and developer portal sandbox.
translator: machine-google-reviewed
---

დეველოპერის პორტალი აგზავნის „სცადე“ კონსოლს Torii REST API-სთვის. ეს სახელმძღვანელო
განმარტავს, თუ როგორ უნდა გაუშვათ დამხმარე პროქსი და დააკავშიროთ კონსოლი დადგმას
კარიბჭე რწმუნებათა სიგელების გამოვლენის გარეშე.

## წინაპირობები

- Iroha საცავის შეკვეთა (სამუშაო სივრცის ფესვი).
- Node.js 18.18+ (ემთხვევა პორტალის საწყისს).
- Torii საბოლოო წერტილი ხელმისაწვდომია თქვენი სამუშაო სადგურიდან (დადგმული ან ადგილობრივი).

## 1. შექმენით OpenAPI სნეპშოტი (სურვილისამებრ)

კონსოლი ხელახლა იყენებს იმავე OpenAPI დატვირთვას, როგორც პორტალის საცნობარო გვერდებს. თუ
თქვენ შეცვალეთ Torii მარშრუტები, განაახლეთ სნეპშოტი:

```bash
cargo xtask openapi
```

დავალება წერს `docs/portal/static/openapi/torii.json`.

## 2. დაიწყეთ Try It პროქსი

საცავის ფესვიდან:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### გარემოს ცვლადები

| ცვლადი | აღწერა |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii საბაზისო URL (აუცილებელია). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | მძიმით გამოყოფილი წარმოშობის სია დაშვებულია პროქსის გამოყენებაზე (ნაგულისხმევი `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | არასავალდებულო ნაგულისხმევი მატარებლის ჟეტონი გამოიყენება ყველა პროქსიდულ მოთხოვნაზე. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | დააყენეთ `1` აბონენტის `Authorization` სათაურის სიტყვასიტყვით გადასატანად. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | მეხსიერების სიჩქარის შემზღუდველი პარამეტრები (ნაგულისხმევი: 60 მოთხოვნა 60 წმ-ზე). |
| `TRYIT_PROXY_MAX_BODY` | მოთხოვნის მაქსიმალური დატვირთვა მიღებულია (ბაიტი, ნაგულისხმევი 1 MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii მოთხოვნისთვის (ნაგულისხმევი 10000 ms). |

პროქსი ამხელს:

- `GET /healthz` — მზადყოფნის შემოწმება.
- `/proxy/*` — პროქსიირებული მოთხოვნები, ბილიკისა და მოთხოვნის სტრიქონის შენახვა.

## 3. გაუშვით პორტალი

ცალკე ტერმინალში:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

ეწვიეთ `http://localhost:3000/api/overview` და გამოიყენეთ Try It კონსოლი. იგივე
გარემოს ცვლადები აკონფიგურირებენ Swagger UI და RapiDoc ჩაშენებებს.

## 4. გაშვებული ერთეულის ტესტები

პროქსი ავლენს სწრაფ კვანძზე დაფუძნებულ ტესტის კომპლექტს:

```bash
npm run test:tryit-proxy
```

ტესტები მოიცავს მისამართების ანალიზს, წარმოშობის დამუშავებას, სიჩქარის შეზღუდვას და მატარებელს
ინექცია.

## 5. გამოძიების ავტომატიზაცია და მეტრიკა

გამოიყენეთ შეფუთული ზონდი `/healthz` და ნიმუშის საბოლოო წერტილის დასადასტურებლად:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

გარემოს სახელურები:

- `TRYIT_PROXY_SAMPLE_PATH` — სურვილისამებრ Torii მარშრუტი (`/proxy`-ის გარეშე) ვარჯიშისთვის.
- `TRYIT_PROXY_SAMPLE_METHOD` — ნაგულისხმევად არის `GET`; დაყენებულია `POST`-ზე ჩაწერის მარშრუტებისთვის.
- `TRYIT_PROXY_PROBE_TOKEN` — ახდენს დროებითი გადამტანის ჟეტონს სინჯის ზარისთვის.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — უგულებელყოფს ნაგულისხმევი 5s დროის ამოწურვას.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — Prometheus ტექსტური ფაილის დანიშნულება `probe_success`/`probe_duration_seconds`-ისთვის.
- `TRYIT_PROXY_PROBE_LABELS` — მძიმით გამოყოფილი `key=value` წყვილი დართულია მეტრიკაზე (ნაგულისხმევია `job=tryit-proxy` და `instance=<proxy URL>`).

როდესაც დაყენებულია `TRYIT_PROXY_PROBE_METRICS_FILE`, სკრიპტი ხელახლა წერს ფაილს
ატომურად, ასე რომ, თქვენი node_exporter/textfile კოლექციონერი ყოველთვის ხედავს სრულ
ტვირთამწეობა. მაგალითი:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

გადაიტანეთ მიღებული მეტრიკა Prometheus-ზე და ხელახლა გამოიყენეთ ნიმუშის გაფრთხილება
დეველოპერის-პორტალის დოკუმენტები გვერდზე, როდესაც `probe_success` ჩამოდის `0`-მდე.

## 6. წარმოების გამკვრივების ჩამონათვალი

პროქსის გამოქვეყნებამდე ადგილობრივი განვითარების მიღმა:

- შეწყვიტე TLS პროქსის წინ (უკუ პროქსი ან მართული კარიბჭე).
- სტრუქტურირებული ხე-ტყის კონფიგურაცია და გადამისამართება დაკვირვებადობის მილსადენებზე.
- დაატრიალეთ მატარებლის ნიშნები და შეინახეთ ისინი თქვენს საიდუმლო მენეჯერში.
- თვალყური ადევნეთ პროქსის `/healthz` საბოლოო წერტილს და მთლიანი შეყოვნების მეტრიკას.
- გაასწორეთ განაკვეთის ლიმიტები თქვენს Torii დადგმის კვოტებთან; დაარეგულირეთ `Retry-After`
  ქცევა მომხმარებლებთან კომუნიკაციისთვის.
---
lang: ka
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# სცადეთ ქვიშის ყუთი

დეველოპერის პორტალი აგზავნის არასავალდებულო „სცადე“ კონსოლს, რათა დარეკოთ Torii
საბოლოო წერტილები დოკუმენტაციის დატოვების გარეშე. კონსოლი გადასცემს მოთხოვნებს
შეფუთული პროქსის მეშვეობით, რათა ბრაუზერებს შეეძლოთ CORS-ის ლიმიტების გვერდის ავლით, სანამ ჯერ კიდევ არ იყოთ
განაკვეთის ლიმიტების და ავთენტიფიკაციის აღსრულება.

## წინაპირობები

- Node.js 18.18 ან უფრო ახალი (ემთხვევა პორტალის აგების მოთხოვნებს)
- ქსელის წვდომა Torii დადგმის გარემოზე
- გადამტანი ჟეტონი, რომელსაც შეუძლია დარეკოს Torii მარშრუტებზე, რომელთა განხორციელებასაც აპირებთ

პროქსის ყველა კონფიგურაცია ხდება გარემოს ცვლადების მეშვეობით. ქვემოთ მოყვანილი ცხრილი
ჩამოთვლის ყველაზე მნიშვნელოვან ღილაკებს:

| ცვლადი | დანიშნულება | ნაგულისხმევი |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | საბაზისო Torii URL, რომელსაც პროქსი აგზავნის მოთხოვნას | **აუცილებელი ** |
| `TRYIT_PROXY_LISTEN` | ლოკალური განვითარების მისამართის მოსმენა (ფორმატი `host:port` ან `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | მძიმით გამოყოფილი წარმოშობის სია, რომელსაც შეუძლია გამოიძახოს პროქსი | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` | ნაგულისხმევი მფლობელის ჟეტონი გადაგზავნილია Torii-ზე | _ ცარიელი_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | მიეცით საშუალება საბოლოო მომხმარებლებს მიაწოდონ საკუთარი ტოკენი `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | სხეულის მაქსიმალური მოთხოვნის ზომა (ბაიტი) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ზემოთ ნაკადის დრო მილიწამებში | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | მოთხოვნები დაშვებულია განაკვეთის ფანჯარაზე კლიენტის IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | მოცურების ფანჯარა სიჩქარის შეზღუდვისთვის (ms) | `60000` |

პროქსი ასევე ავლენს `GET /healthz`-ს, აბრუნებს სტრუქტურირებულ JSON შეცდომებს და
ასწორებს მატარებლის ტოკენებს ჟურნალის გამომავალიდან.

## დაიწყეთ პროქსი ადგილობრივად

დააინსტალირეთ დამოკიდებულებები პორტალზე პირველად დაყენებისას:

```bash
cd docs/portal
npm install
```

გაუშვით პროქსი და მიუთითეთ ის თქვენს Torii მაგალითზე:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

სკრიპტი აღრიცხავს შეკრულ მისამართს და აგზავნის მოთხოვნებს `/proxy/*`-დან
კონფიგურირებული Torii წარმოშობა.

## დააკავშირეთ პორტალის ვიჯეტები

როდესაც თქვენ აშენებთ ან ემსახურებით დეველოპერის პორტალს, დააყენეთ ვიჯეტების URL
პროქსისთვის უნდა გამოვიყენოთ:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

შემდეგი კომპონენტები კითხულობენ ამ მნიშვნელობებს `docusaurus.config.js`-დან:

- **Swagger UI** — გაფორმებულია `/reference/torii-swagger`-ზე; იყენებს მოთხოვნას
  ჩამჭრელი, რომელიც ავტომატურად მიამაგრებს მატარებლის ჟეტონებს.
- **RapiDoc** — გაფორმებულია `/reference/torii-rapidoc`-ზე; ასახავს ნიშნის ველს
  და მხარს უჭერს try-it მოთხოვნას პროქსის მიმართ.
- **სცადეთ კონსოლი** — ჩაშენებული API მიმოხილვის გვერდზე; საშუალებას გაძლევთ გაგზავნოთ საბაჟო
  მოთხოვნები, სათაურების ნახვა და რეაგირების ორგანოების შემოწმება.

ჟეტონის შეცვლა ნებისმიერ ვიჯეტში გავლენას ახდენს მხოლოდ მიმდინარე ბრაუზერის სესიაზე; The
პროქსი არასოდეს ნარჩუნდება და არ აღრიცხავს მოწოდებულ ჟეტონს.

## დაკვირვება და ოპერაციები

ყველა მოთხოვნა ერთხელ არის ჩაწერილი მეთოდით, ბილიკით, წარმოშობით, ზემო დინების სტატუსით და
ავთენტიფიკაციის წყარო (`override`, `default`, ან `client`). ჟეტონები არასოდეს არის
შენახულია - ორივე სათაური და `X-TryIt-Auth` მნიშვნელობები შესწორებულია მანამდე
logging — ასე რომ თქვენ შეგიძლიათ გადააგზავნოთ stdout ცენტრალურ კოლექციონერზე ფიქრის გარეშე
საიდუმლოების გაჟონვა.

### ჯანმრთელობის გამოკვლევები და გაფრთხილებაგაუშვით შეფუთული ზონდი განლაგების დროს ან გრაფიკის მიხედვით:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

გარემოს სახელურები:

- `TRYIT_PROXY_SAMPLE_PATH` — სურვილისამებრ Torii მარშრუტი (`/proxy`-ის გარეშე) ვარჯიშისთვის.
- `TRYIT_PROXY_SAMPLE_METHOD` — ნაგულისხმევად არის `GET`; დაყენებულია `POST`-ზე ჩაწერის მარშრუტებისთვის.
- `TRYIT_PROXY_PROBE_TOKEN` — ახდენს დროებითი გადამტანის ჟეტონს ნიმუშის ზარისთვის.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — უგულებელყოფს ნაგულისხმევი 5s ვადას.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — სურვილისამებრ Prometheus ტექსტური ფაილის დანიშნულება `probe_success`/`probe_duration_seconds`-ისთვის.
- `TRYIT_PROXY_PROBE_LABELS` — მძიმით გამოყოფილი `key=value` წყვილი დართულია მეტრიკაზე (ნაგულისხმევია `job=tryit-proxy` და `instance=<proxy URL>`).

შეიტანეთ შედეგები ტექსტის ფაილების კოლექციონერში, ზონდი ჩასაწერად
გზა (მაგალითად, `/var/lib/node_exporter/textfile_collector/tryit.prom`) და
ნებისმიერი მორგებული ეტიკეტის დამატება:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

სკრიპტი ხელახლა წერს მეტრიკის ფაილს ატომურად, ასე რომ თქვენი კოლექციონერი ყოველთვის კითხულობს ა
სრული დატვირთვა.

მსუბუქი გაფრთხილებისთვის, შეაერთეთ ზონდი თქვენს მონიტორინგის დასტაში. A Prometheus
მაგალითი იმისა, რომ გვერდები ზედიზედ ორი წარუმატებლობის შემდეგ:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### დაბრუნების ავტომატიზაცია

გამოიყენეთ მართვის დამხმარე სამიზნე Torii URL-ის განახლებისთვის ან აღდგენისთვის. სცენარი
ინახავს წინა კონფიგურაციას `.env.tryit-proxy.bak`-ში, ასე რომ უკან დაბრუნება არის
ერთი ბრძანება.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

გადააფარეთ env ფაილის გზა `--env` ან `TRYIT_PROXY_ENV`, თუ თქვენი განლაგება
ინახავს კონფიგურაციას სხვაგან.
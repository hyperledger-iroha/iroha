---
lang: ka
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b920e21b96436755f7d37f7b5577465cb3e30016d36340c50f7c6f3a9a46919
source_last_modified: "2025-12-29T18:16:35.116499+00:00"
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
| `TRYIT_PROXY_CLIENT_ID` | იდენტიფიკატორი განთავსებულია `X-TryIt-Client`-ში ყოველი ზემოთ მოთხოვნისთვის | `docs-portal` |
| `TRYIT_PROXY_BEARER` | ნაგულისხმევი მფლობელის ჟეტონი გადაგზავნილია Torii-ზე | _ ცარიელი_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | მიეცით საშუალება საბოლოო მომხმარებლებს მიაწოდონ საკუთარი ტოკენი `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | სხეულის მაქსიმალური მოთხოვნის ზომა (ბაიტი) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ზემოთ ნაკადის დრო მილიწამებში | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | მოთხოვნები დაშვებულია განაკვეთის ფანჯარაზე კლიენტის IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | მოცურების ფანჯარა სიჩქარის შეზღუდვისთვის (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | სურვილისამებრ მოსმენის მისამართი Prometheus სტილის მეტრიკის საბოლოო წერტილისთვის (`host:port` ან `[ipv6]:port`) | _ ცარიელი (გამორთული)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP გზა, რომელსაც ემსახურება მეტრიკის საბოლოო წერტილი | `/metrics` |

პროქსი ასევე ავლენს `GET /healthz`-ს, აბრუნებს სტრუქტურირებულ JSON შეცდომებს და
ასწორებს მატარებლის ტოკენებს ჟურნალის გამომავალიდან.

ჩართეთ `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` პროქსის გამოქვეყნებისას Docs მომხმარებლებისთვის, ასე რომ, Swagger და
RapiDoc პანელებს შეუძლიათ მომხმარებლის მიერ მიწოდებული მატარებლის ნიშნების გაგზავნა. პროქსი კვლავ ახორციელებს განაკვეთის ლიმიტს,
ასწორებს რწმუნებათა სიგელებს და აფიქსირებს, გამოიყენა თუ არა მოთხოვნამ ნაგულისხმევი ჟეტონი თუ თითო მოთხოვნის გადაფარვა.
დააყენეთ `TRYIT_PROXY_CLIENT_ID` იმ ლეიბლზე, რომლის გაგზავნაც გსურთ, როგორც `X-TryIt-Client`
(ნაგულისხმევი `docs-portal`). პროქსი წყვეტს და ამოწმებს აბონენტის მიერ მიწოდებულს
`X-TryIt-Client` მნიშვნელობები, ბრუნდება ამ ნაგულისხმევზე, რათა დადგმულმა კარიბჭეებმა შეძლონ
აუდიტის წარმოშობა ბრაუზერის მეტამონაცემების კორელაციის გარეშე.

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
კონფიგურირებული Torii საწყისი.

სოკეტის დაკავშირებამდე სკრიპტი ამას ადასტურებს
`static/openapi/torii.json` ემთხვევა დაიჯესტს, რომელიც ჩაწერილია
`static/openapi/manifest.json`. თუ ფაილები გადაინაცვლებს, ბრძანება გადის ან
შეცდომა და ავალებს გაშვებას `npm run sync-openapi -- --latest`. ექსპორტი
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` მხოლოდ გადაუდებელი გადაჭარბებისთვის; მარიონეტული ნება
დაარეგისტრირეთ გაფრთხილება და გააგრძელეთ, რათა შეძლოთ აღდგენა ტექნიკური ფანჯრების დროს.

## დააკავშირეთ პორტალის ვიჯეტები

როდესაც თქვენ აშენებთ ან ემსახურებით დეველოპერის პორტალს, დააყენეთ ვიჯეტების URL
პროქსისთვის უნდა გამოვიყენოთ:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

შემდეგი კომპონენტები კითხულობენ ამ მნიშვნელობებს `docusaurus.config.js`-დან:

- **Swagger UI** — გაფორმებულია `/reference/torii-swagger`-ზე; წინასწარ იძლევა ნებართვას
- **MCP reference** - `/reference/torii-mcp`; use this for JSON-RPC `/v1/mcp` agent workflows.
  მატარებლის სქემა, როდესაც ჟეტონი არსებობს, მოთხოვნებს თეგივს `X-TryIt-Client`-ით,
  ინექციებს `X-TryIt-Auth` და ხელახლა წერს ზარებს პროქსის მეშვეობით, როდესაც
  დაყენებულია `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** — გაფორმებულია `/reference/torii-rapidoc`-ზე; ასახავს ნიშნის ველს,
  ხელახლა იყენებს იგივე სათაურებს, როგორც Swagger პანელი და მიზნად ისახავს პროქსი
  ავტომატურად, როდესაც URL კონფიგურირებულია.
- **სცადეთ კონსოლი** — ჩაშენებული API მიმოხილვის გვერდზე; საშუალებას გაძლევთ გაგზავნოთ საბაჟო
  მოთხოვნები, სათაურების ნახვა და რეაგირების ორგანოების შემოწმება.

ორივე პანელს აქვს **სნეპშოტის ამომრჩევი**, რომელიც კითხულობს
`docs/portal/static/openapi/versions.json`. შეავსეთ ეს ინდექსი
`npm run sync-openapi -- --version=<label> --mirror=current --latest` ისე
მიმომხილველებს შეუძლიათ გადახტომა ისტორიულ სპეციფიკაციებს შორის, იხილეთ ჩაწერილი SHA-256 დაიჯესტი,
და დაადასტურეთ, აქვს თუ არა გამოშვების სნეპშოტს ხელმოწერილი მანიფესტი გამოყენებამდე
ინტერაქტიული ვიჯეტები.

ჟეტონის შეცვლა ნებისმიერ ვიჯეტში გავლენას ახდენს მხოლოდ მიმდინარე ბრაუზერის სესიაზე; The
პროქსი არასოდეს ნარჩუნდება და არ აღრიცხავს მოწოდებულ ჟეტონს.

## ხანმოკლე OAuth ჟეტონები

იმისათვის, რომ თავიდან აიცილოთ გრძელვადიანი Torii ჟეტონების მიმომხილველებისთვის გავრცელება, ჩაწერეთ სცადეთ
კონსოლი თქვენს OAuth სერვერზე. როდესაც ქვემოთ მოცემულია გარემოს ცვლადები
პორტალი აწვდის მოწყობილობის კოდის შესვლის ვიჯეტს, ამუშავებს ხანმოკლე მატარებლის ტოკენებს,
და ავტომატურად შეაქვს მათ კონსოლის ფორმაში.

| ცვლადი | დანიშნულება | ნაგულისხმევი |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth მოწყობილობის ავტორიზაციის საბოლოო წერტილი (`/oauth/device/code`) | _ ცარიელი (გამორთული)_ |
| `DOCS_OAUTH_TOKEN_URL` | ტოკენის საბოლოო წერტილი, რომელიც იღებს `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _ ცარიელი_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth კლიენტის იდენტიფიკატორი რეგისტრირებულია დოკუმენტების გადახედვისთვის | _ ცარიელი_ |
| `DOCS_OAUTH_SCOPE` | შესვლისას მოთხოვნილია სივრცით შემოზღუდული ფარგლები | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | არასავალდებულო API აუდიტორია, რომ დააკავშიროს ჟეტონი | _ ცარიელი_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | გამოკითხვის მინიმალური ინტერვალი დამტკიცების მოლოდინში (მმ) | `5000` (მნიშვნელობები <5000ms უარყოფილია) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | სარეზერვო მოწყობილობა-კოდის ვადის გასვლის ფანჯარა (წამი) | `600` (უნდა დარჩეს 300-დან 900-მდე) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | სარეზერვო წვდომის ნიშანი სიცოცხლის ხანგრძლივობა (წამები) | `900` (უნდა დარჩეს 300-დან 900-მდე) |
| `DOCS_OAUTH_ALLOW_INSECURE` | დააყენეთ `1` ლოკალური გადახედვებისთვის, რომლებიც განზრახ გამოტოვებენ OAuth-ის აღსრულებას | _დაუყენებელი_ |

კონფიგურაციის მაგალითი:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

როდესაც თქვენ გაუშვით `npm run start` ან `npm run build`, პორტალი ჩაშენებულია ამ მნიშვნელობებს
`docusaurus.config.js`-ში. ლოკალური გადახედვისას Try it ბარათი აჩვენებს a
ღილაკი "შესვლა მოწყობილობის კოდით". მომხმარებლები შეიყვანენ თქვენს OAuth-ზე გამოჩენილ კოდს
გადამოწმების გვერდი; მას შემდეგ, რაც მოწყობილობის ნაკადი წარმატებით გაივლის ვიჯეტს:

- შეჰყავს გაცემული მატარებლის ჟეტონს Try it კონსოლის ველში,
- ტეგები მოთხოვნებს არსებული `X-TryIt-Client` და `X-TryIt-Auth` სათაურებით,
- აჩვენებს დარჩენილ სიცოცხლეს და
- ავტომატურად ასუფთავებს ჟეტონს, როდესაც ვადა ამოიწურება.

მიმწოდებლის ხელით შეყვანა ხელმისაწვდომი რჩება - გამოტოვეთ OAuth ცვლადები, როდესაც თქვენ
გსურთ აიძულოთ მიმომხილველები თავად ჩასვან დროებითი ჟეტონი ან ექსპორტი
`DOCS_OAUTH_ALLOW_INSECURE=1` იზოლირებული ადგილობრივი გადახედვისთვის, სადაც ანონიმური წვდომა
მისაღებია. OAuth-ის კონფიგურაციის გარეშე ნაგებობები ახლა სწრაფად ვერ აკმაყოფილებენ
DOCS-1b საგზაო რუკის კარიბჭე.

📌 გადახედეთ [უსაფრთხოების გამკვრივების და ტესტის საკონტროლო სიას] (./security-hardening.md)
ლაბორატორიის გარეთ პორტალის გამოვლენამდე; იგი ადასტურებს საფრთხის მოდელს,
CSP/სანდო ტიპების პროფილი და შეღწევადობის ტესტის საფეხურები, რომლებიც ახლა DOCS-1b-ს ხვდება.

## Norito-RPC ნიმუშები

Norito-RPC მოთხოვნები იზიარებს იგივე პროქსისა და OAuth სანტექნიკას, როგორც JSON მარშრუტები,
ისინი უბრალოდ აყენებენ `Content-Type: application/x-norito` და აგზავნიან
წინასწარ დაშიფრული Norito დატვირთვა, რომელიც აღწერილია NRPC სპეციფიკაციაში
(`docs/source/torii/nrpc_spec.md`).
საცავი აგზავნის კანონიკურ დატვირთვას `fixtures/norito_rpc/` ასე პორტალზე
ავტორებს, SDK-ს მფლობელებს და მიმომხილველებს შეუძლიათ გაიმეორონ ზუსტი ბაიტები, რომლებსაც CI იყენებს.

### გაგზავნეთ Norito დატვირთვა Try It კონსოლიდან

1. აირჩიეთ მოწყობილობა, როგორიცაა `fixtures/norito_rpc/transfer_asset.norito`. ესენი
   ფაილები არის დაუმუშავებელი Norito კონვერტები; ** არ ** base64-ის დაშიფვრა მათ.
2. Swagger-ში ან RapiDoc-ში იპოვეთ NRPC საბოლოო წერტილი (მაგალითად
   `POST /v1/pipeline/submit`) და გადართეთ **Content-Type** ამომრჩეველი
   `application/x-norito`.
3. გადართეთ მოთხოვნის სხეულის რედაქტორი **ორობითად** (Swagger-ის "ფაილი" რეჟიმი ან
   RapiDoc-ის „ორობითი/ფაილი“ ამომრჩევი) და ატვირთეთ `.norito` ფაილი. ვიჯეტი
   გადასცემს ბაიტებს პროქსის მეშვეობით ცვლილების გარეშე.
4. გაგზავნეთ მოთხოვნა. თუ Torii დააბრუნებს `X-Iroha-Error-Code: schema_mismatch`,
   გადაამოწმეთ, რომ ურეკავთ საბოლოო წერტილს, რომელიც იღებს ორობით დატვირთვას და
   დაადასტურეთ, რომ სქემის ჰეში ჩაწერილია `fixtures/norito_rpc/schema_hashes.json`-ში
   ემთხვევა Torii კონსტრუქციას, რომელსაც თქვენ ურტყამთ.

კონსოლი ინახავს უახლეს ფაილს მეხსიერებაში, ასე რომ თქვენ შეგიძლიათ ხელახლა გაგზავნოთ იგივე
დატვირთვა სხვადასხვა ავტორიზაციის ნიშნების ან Torii ჰოსტების გამოყენებისას. დამატება
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` თქვენს სამუშაო პროცესს აწარმოებს
მტკიცებულებათა ნაკრები, რომელიც მითითებულია NRPC-4 მიღების გეგმაში (ლოგი + JSON რეზიუმე),
რომელიც შესანიშნავად ერწყმის სკრინშოტით Try It პასუხს მიმოხილვების დროს.

### CLI მაგალითი (დახვევა)

იგივე მოწყობილობების გამეორება შესაძლებელია პორტალის გარეთ `curl`-ის საშუალებით, რაც სასარგებლოა
პროქსის ან გამართვის კარიბჭის პასუხების ვალიდაციისას:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v1/pipeline/submit"
```

შეცვალეთ მოწყობილობა `transaction_fixtures.manifest.json`-ში ჩამოთვლილი ნებისმიერი ჩანაწერისთვის
ან დაშიფვრეთ თქვენი საკუთარი დატვირთვა `cargo xtask norito-rpc-fixtures`-ით. როცა Torii
არის კანარის რეჟიმში, შეგიძლიათ მიუთითოთ `curl` try-it პროქსიზე
(`https://docs.sora.example/proxy/v1/pipeline/submit`) ივარჯიშეთ იგივე
ინფრასტრუქტურა, რომელსაც იყენებენ პორტალის ვიჯეტები.

## დაკვირვება და ოპერაციებიყველა მოთხოვნა ერთხელ არის ჩაწერილი მეთოდით, ბილიკით, წარმოშობით, ზემო დინების სტატუსით და
ავთენტიფიკაციის წყარო (`override`, `default`, ან `client`). ჟეტონები არასოდეს არის
შენახულია - ორივე სათაური და `X-TryIt-Auth` მნიშვნელობები შესწორებულია მანამდე
logging — ასე რომ თქვენ შეგიძლიათ გადააგზავნოთ stdout ცენტრალურ კოლექციონერზე ფიქრის გარეშე
საიდუმლოების გაჟონვა.

### ჯანმრთელობის გამოკვლევები და გაფრთხილება

გაუშვით შეფუთული ზონდი განლაგების დროს ან გრაფიკის მიხედვით:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

გარემოს სახელურები:

- `TRYIT_PROXY_SAMPLE_PATH` — სურვილისამებრ Torii მარშრუტი (`/proxy`-ის გარეშე) ვარჯიშისთვის.
- `TRYIT_PROXY_SAMPLE_METHOD` — ნაგულისხმევად არის `GET`; დაყენებულია `POST` ჩაწერის მარშრუტებისთვის.
- `TRYIT_PROXY_PROBE_TOKEN` — ახდენს დროებითი გადამტანის ჟეტონს სინჯის ზარისთვის.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — უგულებელყოფს ნაგულისხმევი 5s დროის ამოწურვას.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — სურვილისამებრ Prometheus ტექსტური ფაილის დანიშნულება `probe_success`/`probe_duration_seconds`-ისთვის.
- `TRYIT_PROXY_PROBE_LABELS` — მძიმით გამოყოფილი `key=value` წყვილი დართულია მეტრიკაზე (ნაგულისხმევია `job=tryit-proxy` და `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` — არჩევითი მეტრიკის საბოლოო წერტილის URL (მაგალითად, `http://localhost:9798/metrics`), რომელიც წარმატებით უნდა პასუხობდეს `TRYIT_PROXY_METRICS_LISTEN`-ის ჩართვისას.

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

როდესაც `TRYIT_PROXY_METRICS_LISTEN` არის კონფიგურირებული, დააყენეთ
`TRYIT_PROXY_PROBE_METRICS_URL` მეტრიკის ბოლო წერტილამდე, ასე რომ, ზონდი სწრაფად იშლება
თუ ნაკაწრის ზედაპირი გაქრება (მაგალითად, არასწორად კონფიგურირებული შეღწევა ან დაკარგული
firewall-ის წესები). ტიპიური წარმოების პარამეტრია
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

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

### მეტრიკის საბოლოო წერტილი და დაფები

დააყენეთ `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ან ნებისმიერი ჰოსტი/პორტის წყვილი) მანამდე
პროქსის დაწყება Prometheus ფორმატირებული მეტრიკის საბოლოო წერტილის გამოსავლენად. გზა
ნაგულისხმევად არის `/metrics`, მაგრამ მისი გადაფარვა შესაძლებელია
`TRYIT_PROXY_METRICS_PATH=/custom`. თითოეული ნაკაწრი აბრუნებს მრიცხველებს თითოეული მეთოდისთვის
მოთხოვნის ჯამები, განაკვეთის ლიმიტის უარყოფა, ზემოთ ნაკადის შეცდომები/ვაიდები, პროქსის შედეგები,
და ლატენტური შეჯამებები:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

მიუთითეთ თქვენი Prometheus/OTLP კოლექტორები მეტრიკის ბოლო წერტილზე და ხელახლა გამოიყენეთ
არსებული `dashboards/grafana/docs_portal.json` პანელები, რათა SRE-მ შეძლოს კუდის დაკვირვება
შეყოვნება და უარყოფის მწვერვალები ჟურნალების ანალიზის გარეშე. პროქსი ავტომატურად
აქვეყნებს `tryit_proxy_start_timestamp_ms`-ს, რათა დაეხმაროს ოპერატორებს გადატვირთვების აღმოჩენაში.

### დაბრუნების ავტომატიზაცია

გამოიყენეთ მართვის დამხმარე სამიზნე Torii URL-ის განახლებისთვის ან აღდგენისთვის. სცენარი
ინახავს წინა კონფიგურაციას `.env.tryit-proxy.bak`-ში, ამიტომ უკან დაბრუნება არის
ერთი ბრძანება.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

გადააფარეთ env ფაილის გზა `--env` ან `TRYIT_PROXY_ENV`, თუ თქვენი განლაგება
ინახავს კონფიგურაციას სხვაგან.

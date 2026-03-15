---
lang: ka
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

პორტალი აერთიანებს სამ ინტერაქტიულ ზედაპირს, რომელიც გადასცემს ტრაფიკს Torii-ზე:

- **Swagger UI** `/reference/torii-swagger`-ზე გამოაქვს ხელმოწერილი OpenAPI სპეციფიკაცია და ავტომატურად გადაწერს მოთხოვნებს პროქსის მეშვეობით, როდესაც დაყენებულია `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** `/reference/torii-rapidoc`-ზე ასახავს იგივე სქემას ფაილის ატვირთვით და კონტენტის ტიპის ამომრჩევლებით, რომლებიც კარგად მუშაობს `application/x-norito`-ისთვის.
- **სცადეთ sandbox** Norito მიმოხილვის გვერდზე გთავაზობთ მსუბუქ ფორმას ad-hoc REST მოთხოვნებისთვის და OAuth-მოწყობილობის შესვლებისთვის.

სამივე ვიჯეტი აგზავნის მოთხოვნებს ადგილობრივ **Try-It proxy** (`docs/portal/scripts/tryit-proxy.mjs`). პროქსი ადასტურებს, რომ `static/openapi/torii.json` ემთხვევა ხელმოწერილი დაიჯესტს `static/openapi/manifest.json`-ში, ახორციელებს სიჩქარის შემზღუდველს, ახდენს `X-TryIt-Auth` სათაურების რედაქტირებას ჟურნალებში და თეგივს ყველა ზემორე ზარს I18NI0000000029X-ით.

## გაუშვით პროქსი

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` არის Torii საბაზისო URL, რომლის განხორციელებაც გსურთ.
- `TRYIT_PROXY_ALLOWED_ORIGINS` უნდა შეიცავდეს ყველა პორტალის საწყისს (ლოკალური დეველოპერის სერვერი, წარმოების ჰოსტის სახელი, წინასწარი გადახედვის URL), რომელიც უნდა იყოს ჩასმული კონსოლი.
- `TRYIT_PROXY_PUBLIC_URL` მოიხმარს `docusaurus.config.js` და ინექცია ვიჯეტებში `customFields.tryIt`-ის საშუალებით.
- `TRYIT_PROXY_BEARER` იტვირთება მხოლოდ `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`; წინააღმდეგ შემთხვევაში მომხმარებლებმა უნდა მიაწოდონ საკუთარი ჟეტონი კონსოლის ან OAuth მოწყობილობის ნაკადის მეშვეობით.
- `TRYIT_PROXY_CLIENT_ID` აყენებს `X-TryIt-Client` ტეგს, რომელიც შესრულებულია ყოველი მოთხოვნით.
  ბრაუზერიდან `X-TryIt-Client`-ის მიწოდება ნებადართულია, მაგრამ მნიშვნელობები შემცირებულია
  და უარყოფილია, თუ ისინი შეიცავს საკონტროლო სიმბოლოებს.

გაშვებისას პროქსი მუშაობს `verifySpecDigest` და გადის გამოსწორების მინიშნებით, თუ მანიფესტი მოძველებულია. გაუშვით `npm run sync-openapi -- --latest` უახლესი Torii სპეციფიკაციის ჩამოსატვირთად ან გაიარეთ `TRYIT_PROXY_ALLOW_STALE_SPEC=1` გადაუდებელი გადაუდებებისთვის.

პროქსი სამიზნის განახლებისთვის ან უკან დასაბრუნებლად გარემოს ფაილების ხელით რედაქტირების გარეშე, გამოიყენეთ დამხმარე:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## გაატარეთ ვიჯეტები

ემსახურეთ პორტალს პროქსის მოსმენის შემდეგ:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` აჩვენებს შემდეგ ღილაკებს:

| ცვლადი | დანიშნულება |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL შეყვანილია Swagger-ში, RapiDoc-ში და Try it sandbox-ში. დატოვეთ დაუყენებელი ვიჯეტების დასამალად არაავტორიზებული გადახედვისას. |
| `TRYIT_PROXY_DEFAULT_BEARER` | არჩევითი ნაგულისხმევი ჟეტონი ინახება მეხსიერებაში. საჭიროებს `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` და HTTPS-მხოლოდ CSP მცველს (DOCS-1b), თუ ადგილობრივად არ გაივლით `DOCS_SECURITY_ALLOW_INSECURE=1`-ს. |
| `DOCS_OAUTH_*` | ჩართეთ OAuth მოწყობილობის ნაკადი (`OAuthDeviceLogin` კომპონენტი), რათა მიმომხილველებმა შეძლონ ხანმოკლე ჟეტონების მოჭრა პორტალიდან გაუსვლელად. |

როდესაც არსებობს OAuth ცვლადები, sandbox წარმოებს **შესვლა მოწყობილობის კოდით** ღილაკზე, რომელიც გადის კონფიგურირებულ Auth სერვერზე (ზუსტი ფორმისთვის იხილეთ `config/security-helpers.js`). მოწყობილობის ნაკადის მეშვეობით გაცემული ნიშნები ინახება მხოლოდ ბრაუზერის სესიაში.

## Norito-RPC დატვირთვის გაგზავნა

1. შექმენით `.norito` დატვირთვა CLI-ით ან ფრაგმენტებით, რომლებიც აღწერილია [Norito სწრაფ დაწყებაში] (./quickstart.md). პროქსი გადასცემს `application/x-norito` სხეულებს უცვლელად, ასე რომ თქვენ შეგიძლიათ ხელახლა გამოიყენოთ იგივე არტეფაქტი, რომელსაც გამოაქვეყნებდით `curl`-ით.
2. გახსენით `/reference/torii-rapidoc` (სასურველია ორობითი დატვირთვისთვის) ან `/reference/torii-swagger`.
3. ჩამოსაშლელი მენიუდან აირჩიეთ სასურველი Torii სნეპშოტი. Snapshots ხელმოწერილია; პანელი აჩვენებს `static/openapi/manifest.json`-ში ჩაწერილ მანიფესტს.
4. აირჩიეთ `application/x-norito` კონტენტის ტიპი „სცადეთ“ უჯრაში, დააწკაპუნეთ **აირჩიე ფაილი** და აირჩიეთ თქვენი დატვირთვა. პროქსი ხელახლა წერს მოთხოვნას `/proxy/v1/pipeline/submit`-ზე და მონიშნავს მას `X-TryIt-Client=docs-portal-rapidoc`-ით.
5. Norito პასუხების ჩამოსატვირთად დააყენეთ `Accept: application/x-norito`. Swagger/RapiDoc გამოაშკარავებს სათაურის ამომრჩეველს იმავე უჯრაში და აბრუნებს ორობითი პროქსის მეშვეობით.

მხოლოდ JSON მარშრუტებისთვის, ჩაშენებული Try it sandbox ხშირად უფრო სწრაფია: შეიყვანეთ გზა (მაგალითად, `/v1/accounts/i105.../assets`), აირჩიეთ HTTP მეთოდი, ჩასვით JSON ტექსტი საჭიროების შემთხვევაში და დააჭირეთ **Send მოთხოვნა** სათაურების, ხანგრძლივობისა და დატვირთვის ხაზში შესამოწმებლად.

## პრობლემების მოგვარება

| სიმპტომი | სავარაუდო მიზეზი | გამოსწორება |
| --- | --- | --- |
| ბრაუზერის კონსოლი აჩვენებს CORS შეცდომებს ან ქვიშის ყუთი აფრთხილებს, რომ პროქსის URL აკლია. | პროქსი არ მუშაობს ან საწყისი არ არის თეთრ სიაში. | გაუშვით პროქსი, დარწმუნდით, რომ `TRYIT_PROXY_ALLOWED_ORIGINS` ფარავს თქვენს პორტალის ჰოსტს და ხელახლა გაუშვით `npm run start`. |
| `npm run tryit-proxy` გამოდის „დაიჯესტის შეუსაბამობით“. | Torii OpenAPI პაკეტი შეიცვალა ზემოთ. | გაუშვით `npm run sync-openapi -- --latest` (ან `--version=<tag>`) და ხელახლა სცადეთ. |
| ვიჯეტები აბრუნებენ `401` ან `403`. | ჟეტონი აკლია, ვადაგასული ან არასაკმარისი ფარგლები. | გამოიყენეთ OAuth მოწყობილობის ნაკადი ან ჩასვით სწორი გადამტანი ჟეტონი ქვიშის ყუთში. სტატიკური ტოკენებისთვის თქვენ უნდა გაიტანოთ `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` პროქსიდან. | თითო IP ტარიფის ლიმიტი გადაჭარბებულია. | გაზარდეთ `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` სანდო გარემოებისთვის ან დროსელის ტესტის სკრიპტებისთვის. ყველა განაკვეთის ლიმიტის უარყოფა იზრდება `tryit_proxy_rate_limited_total`. |

## დაკვირვებადობა

- `npm run probe:tryit-proxy` (შეფუთვა `scripts/tryit-proxy-probe.mjs`-ის ირგვლივ) იძახებს `/healthz`-ს, სურვილისამებრ ავარჯიშებს მარშრუტის ნიმუშს და გამოსცემს Prometheus ტექსტურ ფაილებს I18NI000000077X/I1807000000000077X/I1807000-ისთვის. დააკონფიგურირეთ `TRYIT_PROXY_PROBE_METRICS_FILE` node_exporter-თან ინტეგრირებისთვის.
- დააყენეთ `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` მრიცხველების გამოსავლენად (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) და ლატენტური ჰისტოგრამები. `dashboards/grafana/docs_portal.json` დაფა კითხულობს ამ მეტრიკას DOCS-SORA SLO-ების აღსასრულებლად.
- Runtime logs პირდაპირ ეთერში stdout-ზე. ყოველი ჩანაწერი მოიცავს მოთხოვნის ID-ს, ზემორედინ სტატუსს, ავტორიზაციის წყაროს (`default`, `override`, ან `client`) და ხანგრძლივობას; საიდუმლოებები გაფრქვევამდე ირკვევა.

თუ თქვენ გჭირდებათ დაადასტუროთ, რომ `application/x-norito` ტვირთამწეობა აღწევს Torii უცვლელად, გაუშვით Jest Suite (`npm test -- tryit-proxy`) ან შეამოწმეთ მოწყობილობები `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` ქვეშ. რეგრესიის ტესტები მოიცავს შეკუმშულ Norito ბინარებს, ხელმოწერილი OpenAPI მანიფესტებს და მარიონეტული ვერსიის დაქვეითების ბილიკებს, რათა NRPC-ის გავრცელებამ შეინარჩუნოს მუდმივი მტკიცებულების კვალი.
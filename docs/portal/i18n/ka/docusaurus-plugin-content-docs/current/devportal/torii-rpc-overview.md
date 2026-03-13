---
lang: ka
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Overview
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC მიმოხილვა

Norito-RPC არის ორობითი ტრანსპორტი Torii API-ებისთვის. ის ხელახლა იყენებს იგივე HTTP ბილიკებს
როგორც `/v2/pipeline`, მაგრამ ცვლის Norito ჩარჩოს მქონე დატვირთვას, რომელიც მოიცავს სქემას
ჰეშები და ჩეკსუმები. გამოიყენეთ იგი, როდესაც გჭირდებათ განმსაზღვრელი, დადასტურებული პასუხები ან
როდესაც მილსადენის JSON პასუხები ხდება ბოთლი.

## რატომ გადართვა?
- დეტერმინისტული კადრირება CRC64-ით და სქემის ჰეშებით ამცირებს დეკოდირების შეცდომებს.
- გაზიარებული Norito დამხმარეები SDK-ებში საშუალებას გაძლევთ ხელახლა გამოიყენოთ არსებული მონაცემთა მოდელის ტიპები.
- Torii უკვე მონიშნავს Norito სესიებს ტელემეტრიაში, ასე რომ ოპერატორებს შეუძლიათ მონიტორინგი
მიღება მოწოდებული დაფებით.

## მოთხოვნის გაკეთება

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v2/transactions/submit
```

1. დაალაგეთ თქვენი დატვირთვა Norito კოდეკით (`iroha_client`, SDK დამხმარეები, ან
   `norito::to_bytes`).
2. გაგზავნეთ მოთხოვნა `Content-Type: application/x-norito`-ით.
3. მოითხოვეთ Norito პასუხი `Accept: application/x-norito`-ის გამოყენებით.
4. პასუხის გაშიფვრა შესაბამისი SDK დამხმარე გამოყენებით.

SDK-ს სპეციფიკური მითითებები:
- **Rust**: `iroha_client::Client` აწარმოებს მოლაპარაკებას Norito ავტომატურად, როცა დააყენებთ
  `Accept` სათაური.
- **პითონი**: გამოიყენეთ `NoritoRpcClient` `iroha_python.norito_rpc`-დან.
- **Android**: გამოიყენეთ `NoritoRpcClient` და `NoritoRpcRequestOptions`
  Android SDK.
- ** JavaScript/Swift**: დამხმარეების თვალყურის დევნება ხდება `docs/source/torii/norito_rpc_tracker.md`-ში
  და დაეშვება NRPC-3-ის შემადგენლობაში.

## სცადეთ კონსოლის ნიმუში

დეველოპერის პორტალი აგზავნის Try It პროქსის, რათა მიმომხილველებმა შეძლონ Norito-ის გამეორება
დატვირთვები შეკვეთილი სკრიპტების დაწერის გარეშე.

1. [გაუშვით პროქსი](./try-it.md#start-the-proxy-locally) და დააყენეთ
   `TRYIT_PROXY_PUBLIC_URL`, რათა ვიჯეტებმა იცოდნენ სად გაგზავნონ ტრაფიკი.
2. გახსენით **სცადეთ** ბარათი ამ გვერდზე ან `/reference/torii-swagger`
   პანელი და აირჩიეთ საბოლოო წერტილი, როგორიცაა `POST /v2/pipeline/submit`.
3. გადართეთ **Content-Type** `application/x-norito`-ზე, აირჩიეთ **ორობითი**
   რედაქტორი და ატვირთეთ `fixtures/norito_rpc/transfer_asset.norito`
   (ან ჩამოთვლილი ნებისმიერი დატვირთვა
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. მიაწოდეთ გადამტანი ჟეტონი OAuth მოწყობილობის კოდის ვიჯეტის ან სახელმძღვანელო ჟეტონის მეშვეობით
   ველი (პროქსი იღებს `X-TryIt-Auth` უგულებელყოფას კონფიგურაციისას
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. გაგზავნეთ მოთხოვნა და შეამოწმეთ, რომ Torii ეხმიანება `schema_hash`-ს, რომელიც ჩამოთვლილია
   `fixtures/norito_rpc/schema_hashes.json`. შესატყვისი ჰეშები ადასტურებს, რომ
   Norito სათაური გადაურჩა ბრაუზერის/პროქსი ჰოპს.

საგზაო რუქის მტკიცებულებისთვის, დააწყვილეთ Try It ეკრანის ანაბეჭდი გაშვებით
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. სცენარი შეფუთულია
`cargo xtask norito-rpc-verify`, წერს JSON-ის რეზიუმეს
`artifacts/norito_rpc/<timestamp>/` და იჭერს იმავე მოწყობილობებს, რასაც
პორტალი მოხმარებული.

## პრობლემების მოგვარება

| სიმპტომი | სად ჩანს | სავარაუდო მიზეზი | შესწორება |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii პასუხი | აკლია ან არასწორია `Content-Type` სათაური | დააყენეთ `Content-Type: application/x-norito` ტვირთის გაგზავნამდე. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii საპასუხო ორგანო/სათაურები | მოწყობილობების სქემის ჰეში განსხვავდება Torii კონსტრუქციისგან | განაახლეთ მოწყობილობები `cargo xtask norito-rpc-fixtures`-ით და დაადასტურეთ ჰეში `fixtures/norito_rpc/schema_hashes.json`-ში; დაუბრუნდით JSON-ს, თუ ბოლო წერტილს ჯერ არ აქვს ჩართული Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | სცადეთ პროქსი პასუხი | მოთხოვნა მოვიდა წარმოშობიდან, რომელიც არ არის ჩამოთვლილი `TRYIT_PROXY_ALLOWED_ORIGINS` | დაამატეთ პორტალის საწყისი (მაგ., `https://docs.devnet.sora.example`) env var-ში და გადატვირთეთ პროქსი. |
| `{"error":"rate_limited"}` (HTTP 429) | სცადეთ პროქსი პასუხი | თითო IP კვოტა გადააჭარბა `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` ბიუჯეტს | გაზარდეთ შიდა დატვირთვის ტესტირების ლიმიტი ან დაელოდეთ ფანჯრის გადატვირთვას (იხილეთ `retryAfterMs` JSON პასუხში). |
| `{"error":"upstream_timeout"}` (HTTP 504) ან `{"error":"upstream_error"}` (HTTP 502) | სცადეთ პროქსი პასუხი | Torii-ის დრო ამოიწურა ან პროქსი ვერ მიაღწია კონფიგურირებულ ბექენდს | შეამოწმეთ, რომ `TRYIT_PROXY_TARGET` ხელმისაწვდომია, შეამოწმეთ Torii სიჯანსაღე ან ხელახლა სცადეთ უფრო დიდი `TRYIT_PROXY_TIMEOUT_MS`. |

მეტი Try It დიაგნოსტიკა და OAuth რჩევები ცოცხალია
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## დამატებითი რესურსები
- ტრანსპორტის RFC: `docs/source/torii/norito_rpc.md`
- რეზიუმე: `docs/source/torii/norito_rpc_brief.md`
- მოქმედების ტრეკერი: `docs/source/torii/norito_rpc_tracker.md`
- Try-It proxy ინსტრუქციები: `docs/portal/docs/devportal/try-it.md`
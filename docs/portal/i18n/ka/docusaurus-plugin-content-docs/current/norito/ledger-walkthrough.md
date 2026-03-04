---
slug: /norito/ledger-walkthrough
lang: ka
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ეს გზამკვლევი ავსებს [Norito სწრაფ დაწყებას](./quickstart.md) ჩვენებით
როგორ მოვახდინოთ მუტაცია და შევამოწმოთ წიგნის მდგომარეობა `iroha` CLI-ით. თქვენ დარეგისტრირდებით ა
ახალი აქტივის განმარტება, რამდენიმე ერთეულის შეტანა ნაგულისხმევი ოპერატორის ანგარიშზე, გადარიცხვა
ბალანსის ნაწილი სხვა ანგარიშზე და გადაამოწმეთ მიღებული ტრანზაქციები
და ჰოლდინგი. თითოეული ნაბიჯი ასახავს Rust/Python/JavaScript-ში დაფარულ ნაკადებს
SDK სწრაფად იწყება, ასე რომ თქვენ შეგიძლიათ დაადასტუროთ პარიტეტი CLI და SDK ქცევას შორის.

## წინაპირობები

- მიჰყევით [სწრაფად დაწყებას] (./quickstart.md) ერთჯერადი ქსელის ჩატვირთვისთვის
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- დარწმუნდით, რომ `iroha` (CLI) არის აშენებული ან ჩამოტვირთული და რომ შეგიძლიათ მიაღწიოთ
  თანატოლების გამოყენებით `defaults/client.toml`.
- არჩევითი დამხმარეები: `jq` (JSON პასუხების ფორმატირება) და POSIX გარსი
  გარემოს ცვლადი ფრაგმენტები გამოყენებული ქვემოთ.

მთელი სახელმძღვანელოს განმავლობაში, შეცვალეთ `$ADMIN_ACCOUNT` და `$RECEIVER_ACCOUNT`
ანგარიშის ID, რომლის გამოყენებასაც აპირებთ. ნაგულისხმევი ნაკრები უკვე შეიცავს ორ ანგარიშს
მიღებული დემო კლავიშებიდან:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

დაადასტურეთ მნიშვნელობები პირველი რამდენიმე ანგარიშის ჩამოთვლით:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. შეამოწმეთ გენეზის მდგომარეობა

დაიწყეთ იმ წიგნის შესწავლით, რომელსაც CLI მიზნად ისახავს:

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

ეს ბრძანებები ეყრდნობა Norito პასუხებს, ამიტომ ფილტრაცია და პაგინაცია ხდება
განმსაზღვრელი და შეესაბამება იმას, რასაც SDK-ები იღებენ.

## 2. დაარეგისტრირეთ აქტივის განმარტება

შექმენით ახალი, უსაზღვროდ დასამუშავებელი აქტივი სახელწოდებით `coffee` `wonderland`-ში
დომენი:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI ბეჭდავს წარდგენილ ტრანზაქციის ჰეშს (მაგალითად,
`0x5f…`). შეინახეთ, რათა მოგვიანებით მოიძიოთ სტატუსი.

## 3. ზარაფხანა ერთეული ოპერატორის ანგარიშზე

აქტივების რაოდენობა ცხოვრობს `(asset definition, account)` წყვილის ქვეშ. ზარაფხანა 250
`coffee#wonderland`-ის ერთეული `$ADMIN_ACCOUNT`-ში:

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --quantity 250
```

კიდევ ერთხელ, აღბეჭდეთ ტრანზაქციის ჰეში (`$MINT_HASH`) CLI გამომავალიდან. რომ
ორჯერ შეამოწმეთ ბალანსი, გაუშვით:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ან უბრალოდ ახალი აქტივის დასამიზნებლად:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4. ბალანსის ნაწილი გადაიტანეთ სხვა ანგარიშზე

გადაიტანეთ 50 ერთეული ოპერატორის ანგარიშიდან `$RECEIVER_ACCOUNT`-ზე:

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

შეინახეთ ტრანზაქციის ჰეში `$TRANSFER_HASH`. გამოკითხეთ ჰოლდინგი ორივეზე
ანგარიშები ახალი ნაშთების შესამოწმებლად:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
```

## 5. გადაამოწმეთ წიგნის მტკიცებულება

გამოიყენეთ შენახული ჰეშები, რათა დაადასტუროთ, რომ ორივე ტრანზაქცია შესრულებულია:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

ასევე შეგიძლიათ უახლესი ბლოკების სტრიმინგი, რომ ნახოთ რომელი ბლოკი მოიცავდა გადაცემას:

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

ზემოთ მოცემული ყველა ბრძანება იყენებს იგივე Norito დატვირთვას, როგორც SDK-ები. თუ იმეორებთ
ეს ნაკადი კოდის საშუალებით (იხილეთ SDK სწრაფი სტარტები ქვემოთ), ჰეშები და ნაშთები იქნება
დაწექით მანამ, სანამ მიზნად ისახავთ იმავე ქსელს და ნაგულისხმევს.

## SDK პარიტეტის ბმულები

- [Rust SDK quickstart](../sdks/rust) — აჩვენებს რეგისტრაციის ინსტრუქციებს,
  ტრანზაქციების წარდგენა და კენჭისყრის სტატუსი Rust-დან.
- [Python SDK სწრაფი დაწყება] (../sdks/python) - აჩვენებს იგივე რეგისტრს/ზარაფხანას
  ოპერაციები Norito მხარდაჭერილი JSON დამხმარეებით.
- [JavaScript SDK სწრაფი დაწყება](../sdks/javascript) — მოიცავს Torii მოთხოვნებს,
  მართვის დამხმარეები და აკრეფილი შეკითხვის შეფუთვები.

ჯერ გაუშვით CLI გზამკვლევი, შემდეგ გაიმეორეთ სცენარი სასურველი SDK-ით
რათა დარწმუნდეთ, რომ ორივე ზედაპირი თანხმდება ტრანზაქციის ჰეშებზე, ნაშთებსა და მოთხოვნაზე
გამოსავლები.
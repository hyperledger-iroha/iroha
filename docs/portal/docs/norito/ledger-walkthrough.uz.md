---
lang: uz
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c61035c0e4b0fd478f08beeef34d7ae41415f55b09dc93dfda9490efe94fb91
source_last_modified: "2026-01-22T16:26:46.505734+00:00"
translation_last_reviewed: 2026-02-07
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
slug: /norito/ledger-walkthrough
translator: machine-google-reviewed
---

Bu koʻrsatma [Norito tezkor boshlash](./quickstart.md) ni koʻrsatish orqali toʻldiradi.
`iroha` CLI bilan kitob holatini qanday mutatsiyalash va tekshirish. Siz ro'yxatdan o'tasiz a
yangi aktiv ta'rifi, ba'zi birliklarni standart operator hisobiga kiritish, uzatish
balansning bir qismini boshqa hisobga o'tkazing va natijada olingan operatsiyalarni tekshiring
va xoldinglar. Har bir qadam Rust/Python/JavaScript-da qamrab olingan oqimlarni aks ettiradi
SDK tez ishga tushadi, shunda siz CLI va SDK xatti-harakatlari o'rtasidagi paritetni tasdiqlashingiz mumkin.

## Old shartlar

- Yagona tarmoqli tarmoqni yuklash uchun [tezkor ishga tushirish](./quickstart.md) ga amal qiling.
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- `iroha` (CLI) qurilgan yoki yuklab olinganligiga ishonch hosil qiling va siz quyidagi manzilga kirishingiz mumkin.
  `defaults/client.toml` yordamida peer.
- Ixtiyoriy yordamchilar: `jq` (JSON javoblarini formatlash) va POSIX qobig'i
  quyida ishlatiladigan muhit o'zgaruvchisi parchalari.

Qo'llanma davomida `$ADMIN_ACCOUNT` va `$RECEIVER_ACCOUNT` ni
foydalanishni rejalashtirgan hisob identifikatorlari. Standartlar to‘plami allaqachon ikkita hisobni o‘z ichiga oladi
demo kalitlardan olingan:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

Birinchi bir nechta hisoblarni sanab, qiymatlarni tasdiqlang:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Genezis holatini tekshiring

CLI maqsad qilgan daftarni o'rganishdan boshlang:

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

Ushbu buyruqlar Norito tomonidan qo'llab-quvvatlanadigan javoblarga tayanadi, shuning uchun filtrlash va sahifalash
deterministik va SDK qabul qilgan narsaga mos keladi.

## 2. Aktiv taʼrifini roʻyxatdan oʻtkazing

`wonderland` ichida `coffee` deb nomlangan yangi, cheksiz zarb qilinadigan aktiv yarating
domen:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI topshirilgan tranzaksiya xeshini chop etadi (masalan,
`0x5f…`). Holatni keyinroq soʻrashingiz uchun uni saqlang.

## 3. Operator hisobiga zarb birliklari

Aktivlar miqdori `(asset definition, account)` juftligi ostida yashaydi. Yalpiz 250
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` birliklari `$ADMIN_ACCOUNT` ga:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Shunga qaramay, CLI chiqishidan tranzaksiya xeshini (`$MINT_HASH`) oling. Kimga
balansni ikki marta tekshiring, ishga tushiring:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

yoki faqat yangi aktivni maqsad qilish uchun:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Balansning bir qismini boshqa hisob raqamiga o'tkazing

Operator hisobidan `$RECEIVER_ACCOUNT` ga 50 birlik o'tkazing:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Tranzaksiya xeshini `$TRANSFER_HASH` sifatida saqlang. Ikkalasida ham xoldinglarni so'rang
yangi qoldiqlarni tekshirish uchun hisoblar:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Buxgalteriya daftarini tasdiqlang

Har ikkala tranzaksiya amalga oshirilganligini tasdiqlash uchun saqlangan xeshlardan foydalaning:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Shuningdek, qaysi blokda transfer kiritilganligini bilish uchun oxirgi bloklarni translatsiya qilishingiz mumkin:

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Yuqoridagi har bir buyruq SDKlar bilan bir xil Norito foydali yuklaridan foydalanadi. Agar takrorlasangiz
bu oqim kod orqali amalga oshiriladi (quyida SDK tezkor ishga tushirishga qarang), xeshlar va balanslar bo'ladi
bir xil tarmoq va standart sozlamalarni maqsad qilgan ekansiz.

## SDK pariteti havolalari

- [Rust SDK quickstart](../sdks/rust) — roʻyxatdan oʻtish yoʻriqnomalarini koʻrsatadi,
  tranzaktsiyalarni yuborish va Rustdan so'rovnoma holati.
- [Python SDK tezkor ishga tushirish](../sdks/python) - bir xil registrni/yalpizni ko'rsatadi
  Norito tomonidan qo'llab-quvvatlanadigan JSON yordamchilari bilan operatsiyalar.
- [JavaScript SDK tezkor ishga tushirish](../sdks/javascript) — Torii soʻrovlarini qamrab oladi,
  boshqaruv yordamchilari va terilgan so‘rovlar o‘ramlari.

Avval CLI-ni ishga tushiring, so'ngra stsenariyni o'zingiz yoqtirgan SDK bilan takrorlang
tranzaksiya xeshlari, balanslari va so'rovlari bo'yicha ikkala sirt ham kelishib olishiga ishonch hosil qilish
chiqishlar.
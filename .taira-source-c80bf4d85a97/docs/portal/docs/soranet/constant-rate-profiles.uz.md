---
lang: uz
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7334c5f2ccfa93c15a0827390e78b6026bb65e80ac9d624321da84f2287ce581
source_last_modified: "2026-01-05T09:28:11.911258+00:00"
translation_last_reviewed: 2026-02-07
id: constant-rate-profiles
title: SoraNet constant-rate profiles
sidebar_label: Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production relays plus the SNNet-17A2 null dogfood profile, with tick->bandwidth math, CLI helpers, and MTU guardrails.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

SNNet-17B qat'iy belgilangan transport yo'laklarini taqdim etadi, shuning uchun relelar 1024 B hujayradagi trafikni harakatga keltiradi.
foydali yuk hajmi. Operatorlar uchta oldindan sozlashdan tanlaydilar:

- **yadro** - ma'lumotlar markazi yoki professional ravishda joylashtirilgan releylar, ular >=30 Mbit/s ni qamrab olishi mumkin.
  tirbandlik.
- **home** - turar joy yoki past ulanish operatorlari uchun hali ham anonim yuklash kerak
  maxfiylik uchun muhim sxemalar.
- **null** - SNNet-17A2 sinov versiyasi oldindan o'rnatilgan. U bir xil TLV/konvertni saqlaydi, lekin uni uzaytiradi
  kam tarmoqli kengligi sahnalashtirish uchun belgi va ship.

## Oldindan o'rnatilgan xulosa

| Profil | Belgilang (ms) | (B) katak | Yo'lak qopqog'i | Soxta qavat | Har bir yo'lda foydali yuk (Mb/s) | Shiftning foydali yuki (Mb/s) | Shift % yuqoriga ulanish | Tavsiya etilgan havola (Mb/s) | Qo'shni qalpoq | Triggerni avtomatik o'chirish (%) |
|---------|-----------|----------|----------|-------------|-------------------------|-------------------|--------------------|--------------------------------|--------------------|
| yadro | 5.0 | 1024 | 12 | 4 | 1.64 | 19.50 | 65 | 30.0 | 8 | 85 |
| uy | 10.0 | 1024 | 4 | 2 | 0,82 | 4.00 | 40 | 10.0 | 2 | 70 |
| null | 20.0 | 1024 | 2 | 1 | 0,41 | 0,75 | 15 | 5.0 | 1 | 55 |

- **Line qopqog'i** - maksimal bir vaqtda doimiy tezlikli qo'shnilar. O'rni qo'shimcha sxemalarni bir marta rad etadi
  qopqoq uriladi va `soranet_handshake_capacity_reject_total` ni oshiradi.
- **Dummy floor** - haqiqiy bo'lsa ham qo'g'irchoq tirbandligida saqlanib qoladigan minimal qatorlar soni
  talab pastroq.
- **Shiftning foydali yuki** - shiftni qo'llaganingizdan so'ng doimiy tezlikli chiziqlarga ajratilgan yuqoriga ulanish byudjeti
  kasr. Qo'shimcha tarmoqli kengligi mavjud bo'lsa ham, operatorlar hech qachon bu byudjetdan oshmasligi kerak.
- **Triggerni avtomatik o'chirish** - barqaror to'yinganlik foizi (oldindan o'rnatilgan o'rtacha), bu
  qo'g'irchoq qavatga tushish uchun ish vaqti. Imkoniyat tiklanish chegarasidan keyin tiklanadi
  (`core` uchun 75%, `home` uchun 60%, `null` uchun 45%).

**Muhim:** `null` oldindan oʻrnatilgani faqat sahnalashtirish va sinovdan oʻtkazish qobiliyati uchun moʻljallangan; ga mos kelmaydi
ishlab chiqarish sxemalari uchun zarur bo'lgan maxfiylik kafolatlari.

## Belgilang -> o'tkazish qobiliyati jadvali

Har bir foydali yuk xujayrasi 1024 B ni tashiydi, shuning uchun KiB/sek ustuni har biriga chiqariladigan hujayralar soniga teng.
ikkinchi. Jadvalni maxsus belgilar bilan kengaytirish uchun yordamchidan foydalaning.

| Belgilang (ms) | Hujayralar/sek | Yuk yuk KiB/sek | Foydali yuk Mb/s |
|----------|-----------|-----------------|--------------|
| 5.0 | 200.00 | 200.00 | 1.64 |
| 7.5 | 133.33 | 133.33 | 1.09 |
| 10.0 | 100.00 | 100.00 | 0,82 |
| 15.0 | 66.67 | 66.67 | 0,55 |
| 20.0 | 50.00 | 50.00 | 0,41 |

Formula:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI yordamchisi:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` GitHub uslubidagi jadvallarni oldindan o'rnatilgan xulosa va ixtiyoriy belgini aldash uchun chiqaradi
varaq orqali deterministik natijani portalga joylashtirishingiz mumkin. Arxivlash uchun uni `--json-out` bilan bog'lang
boshqaruv dalillari uchun taqdim etilgan ma'lumotlar.

## Konfiguratsiya va bekor qilish

`tools/soranet-relay` konfiguratsiya fayllari va ish vaqtini bekor qilishda oldindan o'rnatilgan sozlamalarni ochib beradi:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

Konfiguratsiya kaliti `core`, `home` yoki `null` (standart `core`) ni qabul qiladi. CLI bekor qilish uchun foydalidir
konfiguratsiyalarni qayta yozmasdan ish aylanishini vaqtincha qisqartiruvchi bosqichli mashqlar yoki SOC so'rovlari.

## MTU to'siqlari

- Foydali yuk xujayralari 1024 B plus ~ 96 B Norito + Shovqin ramkasi va minimal QUIC/UDP sarlavhalaridan foydalanadi,
  har bir datagrammani IPv6 1,280 B minimal MTU dan past saqlash.
- Tunnellar (WireGuard/IPsec) qo'shimcha inkapsulyatsiyani qo'shganda, siz **`padding.cell_size` ni kamaytirishingiz kerak**
  shuning uchun `cell_size + framing <= 1,280 B`. O'z o'rni tekshirgichi majbur qiladi
  `padding.cell_size <= 1,136 B` (1280 B - 48 B UDP/IPv6 qo'shimcha yuk - 96 B ramka).
- `core` profillari hatto bo'sh turganda ham >=4 qo'shnini bog'lashi kerak, shuning uchun soxta yo'llar har doim quyidagi qatorlarni qamrab oladi.
  PQ qo'riqchilari. `home` profillari hamyonlar/agregatorlar uchun doimiy tezlikli sxemalarni cheklashi mumkin, ammo qo'llanilishi kerak
  uchta telemetriya oynasi uchun to'yinganlik 70% dan oshganda orqa bosim.

## Telemetriya va ogohlantirishlar

Relelar har bir oldindan o'rnatilgan quyidagi ko'rsatkichlarni eksport qiladi:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Qachon ogohlantirish:

1. Qo'g'irchoq nisbati oldindan o'rnatilgan qavatdan pastda qoladi (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`)
   ikkita deraza.
2. `soranet_constant_rate_ceiling_hits_total` besh daqiqada bir marta urishdan tezroq o'sadi.
3. `soranet_constant_rate_degraded` rejalashtirilgan matkapdan tashqarida `1` ga aylanadi.

Hodisa hisobotlarida oldindan o'rnatilgan yorliq va qo'shnilar ro'yxatini yozib oling, shunda auditorlar doimiy tezlikni isbotlashlari mumkin
siyosatlar yo'l xaritasi talablariga mos keldi.
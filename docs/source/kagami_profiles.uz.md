---
lang: uz
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 profillari

Kagami Iroha 3 tarmoqlari uchun oldindan oʻrnatilgan sozlamalarni joʻnatadi, shunda operatorlar deterministik shtamp qoʻyishi mumkin.
genezis har bir tarmoq tugmachalarini o'ynamasdan namoyon bo'ladi.

- Profillar: `iroha3-dev` (zanjir `iroha3-dev.local`, kollektorlar k=1 r=1, NPoS tanlanganda zanjir identifikatoridan olingan VRF urug'i), `iroha3-testus` (zanjir `iroha3-testus`, kollektorlar k=3r talab qiladi) NPoS tanlanganda `--vrf-seed-hex`), `iroha3-nexus` (zanjir `iroha3-nexus`, kollektorlar k=5 r=3, NPoS tanlanganda `--vrf-seed-hex` talab qilinadi).
- Konsensus: Sora profil tarmoqlari (Nexus + ma'lumotlar maydonlari) NPoSni talab qiladi va bosqichli kesishlarga ruxsat bermaydi; ruxsat etilgan Iroha3 joylashtirishlari Sora profilisiz ishlashi kerak.
- Avlod: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Nexus uchun `--consensus-mode npos` dan foydalaning; `--vrf-seed-hex` faqat NPoS uchun amal qiladi (testus/nexus uchun talab qilinadi). Kagami DA/RBC ni Iroha3 liniyasiga o'rnatadi va xulosa chiqaradi (zanjir, kollektorlar, DA/RBC, VRF urug'i, barmoq izi).
- Tekshirish: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` profil taxminlarini takrorlaydi (zanjir identifikatori, DA/RBC, kollektorlar, PoP qamrovi, konsensus barmoq izi). `--vrf-seed-hex` ni faqat testus/nexus uchun NPoS manifestini tekshirganda taqdim eting.
- Namuna to'plamlari: oldindan yaratilgan to'plamlar `defaults/kagami/iroha3-{dev,testus,nexus}/` ostida yashaydi (genesis.json, config.toml, docker-compose.yml, verify.txt, README). `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` bilan qayta tiklang.
- Mochi: `mochi`/`mochi-genesis` `--genesis-profile <profile>` va `--vrf-seed-hex <hex>` (faqat NPoS) ni qabul qiladi, ularni Kagami ga yuboring va foydalanilganda bir xil Kagami profilini chop eting.

To'plamlar BLS PoP-larini topologiya yozuvlari bilan birga joylashtiradi, shuning uchun `kagami verify` muvaffaqiyatli bo'ladi
qutidan tashqarida; mahalliy uchun kerak bo'lganda konfiguratsiyalardagi ishonchli tengdoshlar/portlarni sozlang
tutun chiqadi.
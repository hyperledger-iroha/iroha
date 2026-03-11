---
lang: uz
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b92bdfc323a4bc031ca7f2237f238d5d515f7238791a6ec9c50b55e361c85560
source_last_modified: "2026-01-28T17:11:30.639071+00:00"
translation_last_reviewed: 2026-02-07
id: account-address-status
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
---

Kanonik ADDR-2 to'plami (`fixtures/account/address_vectors.json`) suratga oladi
I105 (afzal), siqilgan (`sora`, ikkinchi eng yaxshi; yarim/toʻliq kenglik), multisignature va salbiy moslamalar.
Har bir SDK + Torii yuzasi bir xil JSONga tayanadi, shuning uchun biz har qanday kodekni aniqlay olamiz
ishlab chiqarishga yetguncha drift. Ushbu sahifa ichki holat qisqachasini aks ettiradi
(`docs/source/account_address_status.md` ildiz omborida) shuning uchun portal
o'quvchilar mono-repo orqali qazmasdan ish jarayoniga murojaat qilishlari mumkin.

## To'plamni qayta yarating yoki tasdiqlang

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Bayroqlar:

- `--stdout` - maxsus tekshirish uchun JSON-ni stdout-ga chiqaradi.
- `--out <path>` — boshqa yoʻlga yozish (masalan, mahalliy oʻzgarishlarni farqlashda).
- `--verify` - ishchi nusxani yangi yaratilgan tarkib bilan solishtirish (mumkin emas)
  `--stdout` bilan birlashtirilishi mumkin).

CI ish jarayoni **Address Vector Drift** `cargo xtask address-vectors --verify` ishlaydi
moslama, generator yoki hujjatlar o'zgarganda, sharhlovchilarni darhol ogohlantirish uchun.

## Armaturani kim iste'mol qiladi?

| Yuzaki | Tasdiqlash |
|---------|------------|
| Rust ma'lumotlar modeli | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Har bir aylanish uchun kanonik baytlar + I105 + siqilgan (`sora`, ikkinchi eng yaxshi) kodlash va
Norito uslubidagi xato kodlari salbiy holatlar uchun armatura bilan mos kelishini tekshiradi.

## Avtomatlashtirish kerakmi?

Chiqarish asboblari yordamchi bilan armatura yangilanishini skript qilishi mumkin
`scripts/account_fixture_helper.py`, u kanonikni oladi yoki tasdiqlaydi
nusxa ko'chirish/joylashtirish bosqichlarisiz to'plam:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

Yordamchi `--source` bekor qilish yoki `IROHA_ACCOUNT_FIXTURE_URL` ni qabul qiladi
muhit o'zgaruvchisi, shuning uchun SDK CI ishlari o'zlari afzal ko'rgan oynaga ishora qilishi mumkin.
`--metrics-out` berilganda yordamchi yozadi
`account_address_fixture_check_status{target=\"…\"}` kanonik bilan birga
SHA-256 dayjesti (`account_address_fixture_remote_info`) shuning uchun Prometheus matn fayli
kollektorlar va Grafana asboblar paneli `account_address_fixture_status` isbotlashi mumkin
har bir sirt sinxron bo'lib qoladi. Maqsad `0` haqida xabar berganda ogohlantirish. uchun
ko'p sirtli avtomatlashtirish `ci/account_fixture_metrics.sh` o'ramidan foydalaning
(takroriy `--target label=path[::source]` ni qabul qiladi), shuning uchun qo'ng'iroq bo'yicha guruhlar nashr qilishlari mumkin
tugunni eksport qiluvchi matn fayli kollektori uchun bitta konsolidatsiyalangan `.prom` fayli.

## To'liq ma'lumot kerakmi?

To'liq ADDR-2 muvofiqlik holati (egalari, monitoring rejasi, ochiq harakatlar elementlari)
bo'ylab ombor ichida `docs/source/account_address_status.md` da yashaydi
Manzil tuzilmasi RFC (`docs/account_structure.md`) bilan. Ushbu sahifadan a sifatida foydalaning
tezkor eslatma; chuqur yo'l-yo'riq olish uchun repo hujjatlariga murojaat qiling.
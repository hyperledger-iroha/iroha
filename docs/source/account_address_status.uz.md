---
lang: uz
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Hisob manzili muvofiqlik holati (ADDR-2)

Holati: Qabul qilingan 2026-03-30  
Egalari: Data Model Team / QA Guild  
Yo'l xaritasi ma'lumotnomasi: ADDR-2 - Ikki formatli muvofiqlik to'plami

### 1. Umumiy ko'rinish

- Armatura: `fixtures/account/address_vectors.json` (IH58 (afzal) + siqilgan (`sora`, ikkinchi eng yaxshi) + multisig ijobiy/salbiy holatlar).
- Qo'llanish doirasi: noaniq-standart, Local-12, Global registr va to'liq xato taksonomiyasiga ega multisig kontrollerlarini qamrab oluvchi deterministik V1 foydali yuklari.
- Tarqatish: Rust ma'lumotlar modeli, Torii, JS/TS, Swift va Android SDK'lar bo'ylab taqsimlanadi; Har qanday iste'molchi chetga chiqsa, CI muvaffaqiyatsiz tugadi.
- Haqiqat manbai: generator `crates/iroha_data_model/src/account/address/compliance_vectors.rs` da yashaydi va `cargo xtask address-vectors` orqali ochiladi.
### 2. Qayta tiklash va tekshirish

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Bayroqlar:

- `--out <path>` - vaqtinchalik to'plamlarni ishlab chiqarishda ixtiyoriy bekor qilish (birlamchi `fixtures/account/address_vectors.json`).
- `--stdout` - diskka yozish o'rniga stdout-ga JSON-ni chiqaradi.
- `--verify` — joriy faylni yangi yaratilgan kontent bilan solishtiring (driftda tez ishlamayapti; `--stdout` bilan ishlatib boʻlmaydi).

### 3. Artefakt matritsasi

| Yuzaki | Amalga oshirish | Eslatmalar |
|---------|-------------|-------|
| Rust ma'lumotlar modeli | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSONni tahlil qiladi, kanonik foydali yuklarni qayta tiklaydi va IH58 (afzal)/siqilgan (`sora`, ikkinchi eng yaxshi)/kanonik konversiyalarni + tuzilgan xatolarni tekshiradi. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Server tomoni kodeklarini tasdiqlaydi, shuning uchun Torii noto'g'ri tuzilgan IH58 (afzal)/siqilgan (`sora`, ikkinchi eng yaxshi) foydali yuklarni aniq rad etadi. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Mirrors V1 armatura (IH58 afzal/siqilgan (`sora`) ikkinchi eng yaxshi/to'liq kenglik) va har bir salbiy holat uchun Norito uslubidagi xato kodlarini tasdiqlaydi. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Apple platformalarida IH58 (afzal)/siqilgan (`sora`, ikkinchi eng yaxshi) dekodlash, multisig foydali yuklari va xatolarni aniqlash mashqlari. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Kotlin/Java ulanishlarining kanonik moslamaga mos kelishini ta'minlaydi. |

### 4. Monitoring va ajoyib ish- Holat hisoboti: ushbu hujjat `status.md` va yo'l xaritasi bilan bog'langan, shuning uchun haftalik tekshiruvlar armatura sog'lig'ini tekshirishi mumkin.
- Ishlab chiquvchi portali xulosasi: tashqi konspekt uchun hujjatlar portalidagi (`docs/portal/docs/reference/account-address-status.md`) **Maʼlumotnoma → Hisob manzili muvofiqligi** boʻlimiga qarang.
- Prometheus va asboblar paneli: SDK nusxasini tekshirganingizda, `--metrics-out` (va ixtiyoriy ravishda `--metrics-label`) bilan yordamchini ishga tushiring, shunda Prometheus matn fayli kollektori I100NI060. Grafana asboblar paneli **Hisob manzili armatura holati** (`dashboards/grafana/account_address_fixture_status.json`) har bir sirt uchun o‘tish/qobiliyatsizliklar sonini beradi va audit dalillari uchun kanonik SHA-256 dayjestini taqdim etadi. Har qanday maqsad `0` haqida xabar berganida ogohlantirish.
- Torii ko'rsatkichlari: `torii_address_domain_total{endpoint,domain_kind}` endi `torii_address_invalid_total`/`torii_address_local8_total` aks ettirilgan har bir muvaffaqiyatli tahlil qilingan hisob literali uchun chiqaradi. Ishlab chiqarishdagi har qanday `domain_kind="local12"` trafigidan ogohlantirish va hisoblagichlarni SRE `address_ingest` asboblar paneliga aks ettiring, shunda Local-12 pensiya eshigi tekshiriladigan dalillarga ega bo'ladi.
- Yordamchi moslama: `scripts/account_fixture_helper.py` kanonik JSON-ni yuklab oladi yoki tasdiqlaydi, shuning uchun SDK relizlar avtomatizatsiyasi Prometheus ko'rsatkichlarini yozish paytida qo'lda nusxa ko'chirish/joylashsiz to'plamni olish/tekshirish imkonini beradi. Misol:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  Yordamchi maqsad mos kelganda `account_address_fixture_check_status{target="android"} 1`, shuningdek, SHA-256 dayjestlarini ochib beruvchi `account_address_fixture_remote_info` / `account_address_fixture_local_info` o'lchagichlarini yozadi. Yo'qolgan fayllar `account_address_fixture_local_missing` hisoboti.
  Avtomatlashtirish paketi: birlashtirilgan matn faylini chiqarish uchun cron/CI dan `ci/account_fixture_metrics.sh` ga qo'ng'iroq qiling (standart `artifacts/account_fixture/address_fixture.prom`). Takroriy `--target label=path` yozuvlarini o'tkazing (ixtiyoriy ravishda manbani bekor qilish uchun maqsad boshiga `::https://mirror/...` qo'shing), shuning uchun Prometheus har bir SDK/CLI nusxasini qamrab olgan bitta faylni qirib tashlaydi. GitHub ish oqimi `address-vectors-verify.yml` allaqachon kanonik moslamaga qarshi ushbu yordamchini ishga tushiradi va SRE qabul qilish uchun `account-address-fixture-metrics` artefaktini yuklaydi.
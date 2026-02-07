---
lang: uz
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Imkoniyatlarni simulyatsiya qilish asboblar to'plami

Ushbu katalog SF-2c sig'im bozori uchun takrorlanadigan artefaktlarni jo'natadi
simulyatsiya. Asboblar to'plami kvota bo'yicha muzokaralar, nosozliklarni qayta ishlash va kesishni amalga oshiradi
ishlab chiqarish CLI yordamchilari va engil tahlil skripti yordamida tuzatish.

## Old shartlar

- Ish maydoni a'zolari uchun `cargo run`-ni ishga tushirishga qodir Rust asboblar zanjiri.
- Python 3.10+ (faqat standart kutubxona).

## Tez boshlash

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` skripti `sorafs_manifest_stub capacity` ni yaratish uchun chaqiradi:

- Kvota bo'yicha muzokaralar to'plami uchun deterministik provayder deklaratsiyasi.
- Muzokaralar stsenariysiga mos keladigan takrorlash tartibi.
- O'chirish oynasi uchun telemetriya suratlari.
- Kesish so'rovini qamrab oluvchi bahsli yuk.

Skript Norito baytlarini (`*.to`), base64 foydali yuklarni (`*.b64`), Torii so'rovini yozadi.
Tanlangan artefakt ostidagi jismlar va inson o‘qishi mumkin bo‘lgan xulosalar (`*_summary.json`)
katalog.

`analyze.py` yaratilgan xulosalarni iste'mol qiladi, jamlangan hisobot beradi
(`capacity_simulation_report.json`) va Prometheus matn faylini chiqaradi
(`capacity_simulation.prom`) olib yuruvchi:

- `sorafs_simulation_quota_*` o'lchovlari kelishilgan quvvat va taqsimotni tavsiflaydi
  provayder uchun ulush.
- `sorafs_simulation_failover_*` o'lchagichlar ishlamay qolgan deltalarni va tanlanganlarni ta'kidlaydi
  almashtirish provayderi.
- `sorafs_simulation_slash_requested` olingan remediatsiya foizini qayd etadi
  bahsli yukdan.

Grafana paketini `dashboards/grafana/sorafs_capacity_simulation.json` da import qiling
va uni yaratilgan matn faylini qirib tashlaydigan Prometheus ma'lumotlar manbasiga yo'naltiring (uchun
masalan, tugun eksportchisi matn fayli kollektori orqali). Runbook manzili
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` to'liq o'tadi
ish jarayoni, shu jumladan Prometheus konfiguratsiya maslahatlari.

## Armatura

- `scenarios/quota_negotiation/` - Provayder deklaratsiyasining xususiyatlari va replikatsiya tartibi.
- `scenarios/failover/` - Birlamchi uzilishlar va uzilishlar uchun telemetriya oynalari.
- `scenarios/slashing/` - Bir xil replikatsiya tartibiga havola qiluvchi nizo spetsifikatsiyasi.

Ushbu armatura `crates/sorafs_car/tests/capacity_simulation_toolkit.rs` da tasdiqlangan
ular CLI sxemasi bilan sinxron bo'lishini kafolatlash uchun.
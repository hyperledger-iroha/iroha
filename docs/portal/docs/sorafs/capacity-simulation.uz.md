---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: de9c2a162d97ab896e51af36054e0f3342522b241bfba3ded9c1ec764e590159
source_last_modified: "2026-01-22T14:35:36.797990+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-simulation
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

Ushbu runbook SF-2c sig'imli bozor simulyatsiyasi to'plamini qanday ishga tushirish va natijada olingan ko'rsatkichlarni vizualizatsiya qilishni tushuntiradi. U `docs/examples/sorafs_capacity_simulation/` da deterministik moslamalar yordamida kvotalar bo'yicha muzokaralar, nosozliklarni qayta ishlash va tuzatishni oxirigacha tasdiqlaydi. Imkoniyatli foydali yuklar hali ham `sorafs_manifest_stub capacity` dan foydalanadi; manifest/CAR qadoqlash oqimlari uchun `iroha app sorafs toolkit pack` dan foydalaning.

## 1. CLI artefaktlarini yarating

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` `sorafs_manifest_stub capacity` ni oʻrab, Norito foydali yuklarni, base64 bloblarini, Torii soʻrov qismlarini va JSON xulosalarini chiqarish uchun:

- Kvota bo'yicha muzokaralar stsenariysida ishtirok etuvchi uchta provayder deklaratsiyasi.
- Ushbu provayderlar bo'ylab bosqichli manifestni taqsimlovchi takrorlash tartibi.
- Uzilishdan oldingi boshlang'ich chiziq, uzilishlar oralig'i va uzilishlarni tiklash uchun telemetriya suratlari.
- Simulyatsiya qilingan uzilishdan keyin qisqartirishni talab qiladigan nizo.

Barcha artefaktlar `./artifacts` ostida joylashadi (birinchi argument sifatida boshqa katalogni o'tkazish orqali bekor qiling). `_summary.json` fayllarini inson o'qiy oladigan kontekst mavjudligini tekshiring.

## 2. Natijalarni jamlash va ko'rsatkichlarni chiqarish

```bash
./analyze.py --artifacts ./artifacts
```

Analizator ishlab chiqaradi:

- `capacity_simulation_report.json` - yig'ilgan taqsimotlar, o'chirish deltalari va bahsli metama'lumotlar.
- `capacity_simulation.prom` - Prometheus matn fayli ko'rsatkichlari (`sorafs_simulation_*`) tugunni eksport qiluvchi matn fayli yig'uvchisi yoki mustaqil qirqish ishi uchun mos keladi.

Prometheus qirqish konfiguratsiyasiga misol:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

Matn fayli kollektorini `capacity_simulation.prom` ga yo'naltiring (tugun eksportchisidan foydalanganda uni `--collector.textfile.directory` orqali uzatiladigan katalogga nusxalash).

## 3. Grafana asboblar panelini import qiling

1. Grafana da `dashboards/grafana/sorafs_capacity_simulation.json` import qiling.
2. `Prometheus` ma'lumotlar manbai o'zgaruvchisini yuqorida sozlangan qirqish maqsadiga bog'lang.
3. Panellarni tekshiring:
   - **Kvota taqsimoti (GiB)** har bir provayder uchun ajratilgan/tayinlangan qoldiqlarni ko'rsatadi.
   - **To'xtash triggeri** uzilish ko'rsatkichlari oqimga kirganda *Failover Active* rejimiga o'tadi.
   - **Toʻxtash vaqtida ish vaqtining pasayishi** `alpha` provayderi uchun foiz yoʻqotish jadvalini koʻrsatadi.
   - **So'ralgan slash foizi** nizo tuzatmasidan olingan tuzatish nisbatini ko'rsatadi.

## 4. Kutilayotgan tekshiruvlar

- `sorafs_simulation_quota_total_gib{scope="assigned"}` `600` ga teng bo'lib, qabul qilingan jami >=600 bo'lib qoladi.
- `sorafs_simulation_failover_triggered` hisobotlari `1` va almashtiruvchi provayder metrikasi `beta` belgilarini ta'kidlaydi.
- `sorafs_simulation_slash_requested` `alpha` provayder identifikatori uchun `0.15` (15% slash) haqida xabar beradi.

Armatura hali ham CLI sxemasi tomonidan qabul qilinganligini tasdiqlash uchun `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` ni ishga tushiring.
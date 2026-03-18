---
lang: mn
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

::: Каноник эх сурвалжийг анхаарна уу
:::

Энэхүү runbook нь SF-2c багтаамжийн зах зээлийн симуляцийн иж бүрдлийг хэрхэн ажиллуулж, үр дүнгийн хэмжигдэхүүнийг дүрслэн харуулахыг тайлбарладаг. Энэ нь `docs/examples/sorafs_capacity_simulation/` дахь детерминистик бэхэлгээг ашиглан квотын тохиролцоо, бүтэлгүйтлийн зохицуулалт, таслах засварыг төгсгөлөөс нь баталгаажуулдаг. Багтаамжийн ачаалал нь `sorafs_manifest_stub capacity`-г ашигладаг хэвээр байна; manifest/CAR савлагааны урсгалын хувьд `iroha app sorafs toolkit pack` ашиглана уу.

## 1. CLI олдворуудыг үүсгэх

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` нь `sorafs_manifest_stub capacity`-ийг ороож, Norito ачаалал, base64 blob, Torii хүсэлтийн биетүүд болон JSON хураангуйг:

- Квотын хэлэлцээрийн хувилбарт гурван ханган нийлүүлэгчийн мэдүүлэг.
- Үе шаттай манифестийг тэдгээр үйлчилгээ үзүүлэгчдийн дунд хуваарилах хуулбарлах дараалал.
- Тасалдлын өмнөх суурь үзүүлэлт, тасалдалын интервал болон бүтэлгүйтлийг сэргээхэд зориулсан телеметрийн агшин зургууд.
- Загварчилсан тасалдлын дараа таслахыг хүссэн ачааны маргаан.

Бүх олдворууд `./artifacts`-ийн дагуу бууна (эхний аргумент болгон өөр лавлахаар дамжуулж дарна уу). `_summary.json` файлуудыг хүн унших боломжтой эсэхийг шалгана уу.

## 2. Үр дүнг нэгтгэж, хэмжигдэхүүнийг ялгаруулна

```bash
./analyze.py --artifacts ./artifacts
```

Анализатор нь дараахь зүйлийг үүсгэдэг.

- `capacity_simulation_report.json` - нэгтгэсэн хуваарилалт, шилжүүлгийн дельта болон маргааны мета өгөгдөл.
- `capacity_simulation.prom` - Prometheus текст файлын хэмжигдэхүүн (`sorafs_simulation_*`) зангилаа экспортлогч текст файл цуглуулагч эсвэл бие даасан хусах ажилд тохиромжтой.

Жишээ Prometheus хусах тохиргоо:

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

Текст файл цуглуулагчийг `capacity_simulation.prom` руу чиглүүлээрэй (зангилаа экспортлогч ашиглах үед үүнийг `--collector.textfile.directory`-ээр дамжуулсан лавлах руу хуулна уу).

## 3. Grafana хяналтын самбарыг импортлох

1. Grafana, импорт `dashboards/grafana/sorafs_capacity_simulation.json`.
2. `Prometheus` мэдээллийн эх үүсвэрийн хувьсагчийг дээр тохируулсан хусах зорилттой холбоно.
3. Самбаруудыг шалгана уу:
   - **Квотын хуваарилалт (GiB)** нь үйлчилгээ үзүүлэгч бүрийн хувьд хүлээсэн/тогтоосон үлдэгдлийг харуулдаг.
   - **Гүйцэтгэх триггер** нь тасалдалын хэмжигдэхүүн орж ирэхэд *Гэмтлээс идэвхтэй* рүү шилждэг.
   - **Зогслын үед ажиллах хугацаа буурах** нь `alpha` үйлчилгээ үзүүлэгчийн алдагдлын хувийг графикаар харуулав.
   - **Хүссэн налуу зураасны хувь** нь маргааны хэрэглүүрээс гаргаж авсан засварын харьцааг харуулдаг.

## 4. Хүлээгдэж буй шалгалтууд

- `sorafs_simulation_quota_total_gib{scope="assigned"}` нь `600`-тэй тэнцүү байхад хүлээгдэж буй нийт хэмжээ >=600 хэвээр байна.
- `sorafs_simulation_failover_triggered` `1` мэдээлдэг ба орлуулах үйлчилгээ үзүүлэгчийн хэмжүүр нь `beta`-г онцолдог.
- `sorafs_simulation_slash_requested` нь `alpha` үйлчилгээ үзүүлэгчийн танигчийн хувьд `0.15` (15% налуу зураас) гэж мэдээлдэг.

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`-г ажиллуулж бэхэлгээг CLI схемээр хүлээн зөвшөөрсөн хэвээр байгаа эсэхийг баталгаажуулна уу.
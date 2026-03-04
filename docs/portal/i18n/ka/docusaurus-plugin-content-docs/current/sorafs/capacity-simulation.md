---
id: capacity-simulation
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

ეს სახელმძღვანელო განმარტავს, თუ როგორ უნდა გაუშვათ SF-2c სიმძლავრის ბაზრის სიმულაციური ნაკრები და წარმოიდგინოთ მიღებული მეტრიკა. ის ადასტურებს კვოტების მოლაპარაკებას, უკმარისობის დამუშავებას და შემცირების გამოსწორებას ბოლოდან ბოლომდე `docs/examples/sorafs_capacity_simulation/`-ის დეტერმინისტული მოწყობილობების გამოყენებით. სიმძლავრის ტვირთამწეობა კვლავ იყენებს `sorafs_manifest_stub capacity`; გამოიყენეთ `iroha app sorafs toolkit pack` manifest/CAR შეფუთვის ნაკადებისთვის.

## 1. შექმენით CLI არტეფაქტები

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` ახვევს `sorafs_manifest_stub capacity`-ს Norito ტვირთამწეობის, base64 blobs, Torii მოთხოვნის ორგანოებისა და JSON შეჯამებების გამოსაცემად:

- კვოტის მოლაპარაკების სცენარში მონაწილე სამი პროვაიდერის დეკლარაცია.
- რეპლიკაციის შეკვეთა, რომელიც ანაწილებს ეტაპობრივ მანიფესტს ამ პროვაიდერებში.
- ტელემეტრიული კადრები გათიშვამდე საბაზისო ხაზისთვის, გათიშვის ინტერვალისა და უკმარისობის აღდგენისთვის.
- დავის დატვირთვა, რომელიც ითხოვს შემცირებას სიმულირებული გათიშვის შემდეგ.

ყველა არტეფაქტი მოდის `./artifacts`-ის ქვეშ (გადალახვა სხვა დირექტორიაში, როგორც პირველ არგუმენტად). შეამოწმეთ `_summary.json` ფაილები ადამიანის მიერ წასაკითხად კონტექსტში.

## 2. შეაგროვეთ შედეგები და გამოაქვეყნეთ მეტრიკა

```bash
./analyze.py --artifacts ./artifacts
```

ანალიზატორი აწარმოებს:

- `capacity_simulation_report.json` - აგრეგირებული გამოყოფები, დელტა და დავის მეტამონაცემები.
- `capacity_simulation.prom` - Prometheus ტექსტური ფაილის მეტრიკა (`sorafs_simulation_*`) შესაფერისია კვანძის ექსპორტიორის ტექსტური ფაილების კოლექციონერისთვის ან დამოუკიდებლად გაფხეკისთვის.

მაგალითი Prometheus ნაკაწრის კონფიგურაცია:

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

მიუთითეთ ტექსტური ფაილის შემგროვებელი `capacity_simulation.prom`-ზე (კვანძის ექსპორტიორის გამოყენებისას დააკოპირეთ იგი `--collector.textfile.directory`-ის მეშვეობით გადაცემულ დირექტორიაში).

## 3. Grafana დაფის იმპორტი

1. Grafana-ში შემოიტანეთ `dashboards/grafana/sorafs_capacity_simulation.json`.
2. დააკავშირეთ `Prometheus` მონაცემთა წყაროს ცვლადი ზემოთ კონფიგურირებულ scrape სამიზნესთან.
3. შეამოწმეთ პანელები:
   - **კვოტის განაწილება (GiB)** გვიჩვენებს ვალდებულებებს/მიკუთვნებულ ნაშთებს თითოეული პროვაიდერისთვის.
   - **Failover Trigger** გადატრიალდება *Failover Active*-ზე, როდესაც გათიშვის მეტრიკა შემოდის.
   - **Uptime Drop დროს outage** ასახავს პროცენტულ დანაკარგს პროვაიდერის `alpha`.
   - **მოთხოვნილი დახრილობის პროცენტი** ვიზუალურად ასახავს გამოსწორების კოეფიციენტს, რომელიც ამოღებულია დავის წყობიდან.

## 4. მოსალოდნელი შემოწმებები

- `sorafs_simulation_quota_total_gib{scope="assigned"}` უდრის `600` ხოლო ჩადენილი ჯამი რჩება >=600.
- `sorafs_simulation_failover_triggered` იუწყება `1` და შემცვლელი პროვაიდერის მეტრიკა ხაზს უსვამს `beta`-ს.
- `sorafs_simulation_slash_requested` იტყობინება `0.15` (15% დახრილი) `alpha` პროვაიდერის იდენტიფიკატორისთვის.

გაუშვით `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`, რათა დაადასტუროთ, რომ მოწყობილობები კვლავ მიღებულია CLI სქემით.
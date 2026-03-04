---
lang: ka
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ტელემეტრიის მოთხოვნები

## Prometheus მიზნები

გადაფურცლეთ რელე და ორკესტრი შემდეგი ეტიკეტებით:

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## საჭირო დაფები

1. `dashboards/grafana/soranet_testnet_overview.json` *(გამოქვეყნებული)* — ჩატვირთეთ JSON, შემოიტანეთ ცვლადები `region` და `relay_id`.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(არსებული SNNet-8 აქტივი)* — უზრუნველყოს კონფიდენციალურობის თაიგულის პანელები ხარვეზების გარეშე.

## გაფრთხილების წესები

ზღვრები უნდა შეესაბამებოდეს სათამაშო წიგნის მოლოდინს:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` მატება > 0 10 წუთში იწვევს `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 30 წუთში იწვევს `warning`.
- `up{job="soranet-relay"}` == 0 2 წუთის განმავლობაში იწვევს `critical`.

ჩატვირთეთ თქვენი წესები Alertmanager-ში `testnet-t0` მიმღებით; დაადასტურეთ `amtool check-config`-ით.

## მეტრიკის შეფასება

შეაგროვეთ 14 დღიანი სნეპშოტი და მიაწოდეთ SNNet-10 ვალიდატორს:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- შეცვალეთ ნიმუშის ფაილი თქვენი ექსპორტირებული სნეპშოტით, როდესაც მუშაობს პირდაპირ მონაცემებზე.
- `status = fail` შედეგი ბლოკავს აქციას; მოაგვარეთ მონიშნული ჩეკი(ები) ხელახლა ცდამდე.

## მოხსენება

ყოველ კვირას ატვირთეთ:

- შეკითხვის სნეპშოტები (`.png` ან `.pdf`), სადაც ნაჩვენებია PQ თანაფარდობა, მიკროსქემის წარმატების მაჩვენებელი და PoW ამოხსნის ჰისტოგრამა.
- Prometheus ჩაწერის წესის გამომავალი `soranet_privacy_throttles_per_minute`-ისთვის.
- მოკლე ნარატივი, რომელიც აღწერს გაშვებულ ნებისმიერ გაფრთხილებას და შემარბილებელ ნაბიჯებს (შეიცავს დროის ნიშანს).
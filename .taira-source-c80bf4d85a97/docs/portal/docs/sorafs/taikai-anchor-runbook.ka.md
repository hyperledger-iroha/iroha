---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-12-29T18:16:35.204852+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Taikai Anchor Observability Runbook

ეს პორტალი ასახავს კანონიკურ წიგნს
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
გამოიყენეთ იგი SN13-C მარშრუტიზაციის მანიფესტის (TRM) წამყვანების რეპეტიციისას, ასე რომ, SoraFS/SoraNet
ოპერატორებს შეუძლიათ დააკავშირონ კოჭის არტეფაქტები, Prometheus ტელემეტრია და მმართველობა
მტკიცებულება პორტალის წინასწარი გადახედვის კონსტრუქციის დატოვების გარეშე.

## სფერო და მფლობელები

- **პროგრამა:** SN13-C — Taikai manifests & SoraNS წამყვანები.
- **მფლობელები: ** მედია პლატფორმა WG, DA პროგრამა, Networking TL, Docs/DevRel.
- ** მიზანი: ** უზრუნველყოს განმსაზღვრელი სათამაშო წიგნი Sev1/Sev2 გაფრთხილებისთვის, ტელემეტრიისთვის
  ვალიდაცია და მტკიცებულების დაჭერა, სანამ ტაიკაის მარშრუტიზაციის მანიფესტები წინ გადადის
  მეტსახელებს შორის.

## სწრაფი დაწყება (Sev1/Sev2)

1. **გადაიღეთ კოჭის არტეფაქტები** — დააკოპირეთ უახლესი
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` და
   `taikai-lineage-*.json` ფაილები
   `config.da_ingest.manifest_store_dir/taikai/` მუშების გადატვირთვამდე.
2. ** გადაყარეთ `/status` ტელემეტრია** — ჩაწერეთ
   `telemetry.taikai_alias_rotations` მასივი იმის დასამტკიცებლად, თუ რომელია მანიფესტის ფანჯარა
   აქტიური:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. ** შეამოწმეთ დაფები და გაფრთხილებები ** — ჩატვირთვა
   `dashboards/grafana/taikai_viewer.json` (კლასტერი + ნაკადის ფილტრები) და შენიშვნა
   არსებობს თუ არა რაიმე წესები
   `dashboards/alerts/taikai_viewer_rules.yml` გასროლილი (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS მტკიცებულება ჯანმრთელობის მოვლენები).
4. **შეამოწმეთ Prometheus** — გაუშვით მოთხოვნები §„მეტრული მითითება“ დასადასტურებლად
   ჩაყლაპვის შეყოვნება/დრიფტი და ალიას-როტაციის მრიცხველები იქცევიან ისე, როგორც მოსალოდნელია. ესკალაცია
   თუ `taikai_trm_alias_rotations_total` ჩერდება რამდენიმე ფანჯრისთვის ან თუ
   იზრდება შეცდომების მრიცხველები.

## მეტრიკული მითითება

| მეტრული | დანიშნულება |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | CMAF შეყოვნების ჰისტოგრამა თითო კლასტერზე/ნაკადზე (სამიზნე: p95<750ms, p99<900ms). |
| `taikai_ingest_live_edge_drift_ms` | პირდაპირი დრეიფი ენკოდერსა და წამყვანის მუშაკებს შორის (გვერდები p99>1,5 წმ 10 წთ). |
| `taikai_ingest_segment_errors_total{reason}` | შეცდომები ითვლება მიზეზის მიხედვით (`decode`, `manifest_mismatch`, `lineage_replay`,…). ნებისმიერი ზრდა იწვევს `TaikaiIngestFailure`. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | იზრდება, როდესაც `/v1/da/ingest` მიიღებს ახალ TRM-ს მეტსახელისთვის; გამოიყენეთ `rate()` ბრუნვის კადენციის დასადასტურებლად. |
| `/status → telemetry.taikai_alias_rotations[]` | JSON სნეპშოტი `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` და დროის შტამპებით მტკიცებულებათა პაკეტებისთვის. |
| `taikai_viewer_*` (რებუფერი, CEK ბრუნვის ასაკი, PQ ჯანმრთელობა, გაფრთხილებები) | მაყურებლის მხრიდან KPI-ები, რათა უზრუნველყონ CEK ბრუნვა + PQ სქემები ჯანსაღად დარჩეს წამყვანების დროს. |

### PromQL ფრაგმენტები

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## დაფები და გაფრთხილებები

- **Grafana სანახავი დაფა:** `dashboards/grafana/taikai_viewer.json` — p95/p99
  შეყოვნება, პირდაპირი კიდეების დრიფტი, სეგმენტის შეცდომები, CEK ბრუნვის ასაკი, მაყურებლის გაფრთხილებები.
- **Grafana ქეში დაფა:** `dashboards/grafana/taikai_cache.json` — ცხელი/თბილი/ცივი
  აქციები და QoS უარყოფა, როდესაც ფსევდონიმები ბრუნავს.
- **Alertmanager წესები:** `dashboards/alerts/taikai_viewer_rules.yml` — დრიფტი
  პეიჯინგი, გადაყლაპვის გაფრთხილებები, CEK ბრუნვის შეფერხება და SoraFS proof-health
  ჯარიმები/გაგრილება. დარწმუნდით, რომ არსებობს მიმღებები ყველა საწარმოო კლასტერისთვის.

## მტკიცებულებათა ნაკრების საკონტროლო სია

- კოჭის არტეფაქტები (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- გაუშვით `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`, რათა გამოაქვეყნოთ მომლოდინე/მიწოდებული კონვერტების ხელმოწერილი JSON ინვენტარი და დააკოპირეთ მოთხოვნის/SSM/TRM/ხაზოვანი ფაილების საბურღი პაკეტში. ნაგულისხმევი კოჭის გზა არის `storage/da_manifests/taikai` `torii.toml`-დან.
- `/status` სნეპშოტი, რომელიც მოიცავს `telemetry.taikai_alias_rotations`.
- Prometheus ექსპორტი (JSON/CSV) ზემოთ მოცემული მეტრიკებისთვის ინციდენტის ფანჯარაში.
- Grafana ეკრანის ანაბეჭდები ხილული ფილტრებით.
- Alertmanager ID-ები, რომლებიც მიუთითებენ შესაბამის წესზე, ხანძრის.
- ბმული `docs/examples/taikai_anchor_lineage_packet.md`-ზე, რომელიც აღწერს
  კანონიკური მტკიცებულების პაკეტი.

## დაფის ასახვა და საბურღი კადენცია

SN13-C საგზაო რუქის მოთხოვნის დაკმაყოფილება ნიშნავს იმის მტკიცებას, რომ ტაიკაი
მაყურებლის/ქეშის დაფები აისახება პორტალში **და**, რომელიც წამყვანია
მტკიცებულებათა სწავლება პროგნოზირებადი კადენციის მიხედვით მიმდინარეობს.

1. **პორტალის სარკე.** როდესაც `dashboards/grafana/taikai_viewer.json` ან
   `dashboards/grafana/taikai_cache.json` ცვლილებები, შეაჯამეთ დელტაები
   `sorafs/taikai-monitoring-dashboards` (ეს პორტალი) და გაითვალისწინეთ JSON
   ჩეკსუმები პორტალის PR აღწერაში. მონიშნეთ ახალი პანელები/ზღვრები ასე
   მიმომხილველებს შეუძლიათ კორელაცია მართულ Grafana საქაღალდესთან.
2. ** ყოველთვიური საბურღი.**
   - ჩაატარეთ სავარჯიშო ყოველი თვის პირველ სამშაბათს, 15:00 UTC-ზე, ასე რომ დადასტურდება
     მიწები SN13 მმართველობის სინქრონიზაციამდე.
   - გადაიღეთ კოჭის არტეფაქტები, `/status` ტელემეტრია და Grafana ეკრანის ანაბეჭდები შიგნით
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - ჩაწერეთ აღსრულება
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **გადახედეთ და გამოაქვეყნეთ.** 48 საათის განმავლობაში გადახედეთ სიგნალიზაციას/ცრუ პოზიტიურს
   DA პროგრამა + NetOps, ჩაწერეთ შემდგომი ელემენტები საბურღი ჟურნალში და დააკავშირეთ
   მართვის თაიგულის ატვირთვა `docs/source/sorafs/runbooks-index.md`-დან.

თუ დაფა ან საბურღი ჩამორჩება, SN13-C ვერ გადის 🈺; შეინახეთ ეს
განყოფილება განახლებულია, როდესაც იცვლება კადენციის ან მტკიცებულების მოლოდინი.

## სასარგებლო ბრძანებები

```bash
# Snapshot alias rotation telemetry to an artefact directory
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool entries for a specific alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspect TRM mismatch reasons from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

შეინახეთ ეს პორტალი ასლი სინქრონულად კანონიკურ წიგნთან, როდესაც ტაიკაი
ანკორირების ტელემეტრია, დაფები ან მმართველობის მტკიცებულებების მოთხოვნები იცვლება.
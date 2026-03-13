---
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/quickstart/default_lane.md`-i əks etdirir. Hər iki nüsxəni saxlayın
lokalizasiya süpürülməsi portala düşənə qədər düzülür.
:::

# Defolt Zolaqlı Sürətli Başlama (NX-5)

> **Yol xəritəsi konteksti:** NX-5 — defolt ictimai zolaq inteqrasiyası. İndi iş vaxtı
> `nexus.routing_policy.default_lane` geri dönüşünü ortaya qoyur, beləliklə Torii REST/gRPC
> son nöqtələr və hər bir SDK, trafik aid olduqda `lane_id`-i təhlükəsiz şəkildə buraxa bilər
> kanonik ictimai zolaqda. Bu təlimat operatorları konfiqurasiyadan keçir
> kataloq, `/status`-də geri dönüşü yoxlamaq və müştərini həyata keçirmək
> davranış başdan sona.

## İlkin şərtlər

- `irohad` Sora/Nexus quruluşu (`irohad --sora --config ...` ilə işləyir).
- `nexus.*` bölmələrini redaktə etmək üçün konfiqurasiya deposuna daxil olun.
- `iroha_cli` hədəf klasterlə danışmaq üçün konfiqurasiya edilib.
- `curl`/`jq` (və ya ekvivalenti) Torii `/status` faydalı yükünü yoxlamaq üçün.

## 1. Zolaq və məlumat məkanı kataloqunu təsvir edin

Şəbəkədə mövcud olmalı olan zolaqları və məlumat məkanlarını elan edin. Snippet
aşağıda (`defaults/nexus/config.toml`-dən kəsilmiş) üç ictimai zolağı qeyd edir
üstəgəl uyğun verilənlər məkanı ləqəbləri:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Hər bir `index` unikal və bitişik olmalıdır. Məlumat məkanı identifikatorları 64 bitlik dəyərlərdir;
yuxarıdakı nümunələr aydınlıq üçün zolaq indeksləri ilə eyni ədədi dəyərlərdən istifadə edir.

## 2. Marşrutlaşdırma defoltlarını və əlavə qeydləri təyin edin

`nexus.routing_policy` bölməsi ehtiyat zolağına nəzarət edir və sizə imkan verir
xüsusi təlimatlar və ya hesab prefiksləri üçün marşrutlaşdırmanı ləğv edin. Qayda olmasa
uyğun gəlirsə, planlaşdırıcı əməliyyatı konfiqurasiya edilmiş `default_lane`-ə yönləndirir
və `default_dataspace`. Router məntiqi yaşayır
`crates/iroha_core/src/queue/router.rs` və siyasəti şəffaf şəkildə tətbiq edir
Torii REST/gRPC səthləri.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Daha sonra yeni zolaqlar əlavə etdiyiniz zaman əvvəlcə kataloqu yeniləyin, sonra marşrutu genişləndirin
qaydalar. Qaytarma zolağı dayanan ictimai zolağa yönəlməyə davam etməlidir

## 3. Tətbiq edilmiş siyasətlə qovşağı yükləyin

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Node başlanğıc zamanı əldə edilmiş marşrutlaşdırma siyasətini qeyd edir. Hər hansı doğrulama səhvləri
(çatışmayan indekslər, təkrarlanan ləqəblər, etibarsız məlumat məkanı idləri) əvvəl ortaya çıxır
qeybət başlayır.

## 4. Zolaqlı idarəetmə vəziyyətini təsdiqləyin

Düyün onlayn olduqdan sonra standart zolağın olduğunu yoxlamaq üçün CLI köməkçisindən istifadə edin
möhürlənmiş (manifest yüklənmiş) və hərəkətə hazırdır. Xülasə görünüşü bir sıra çap edir
zolaq başına:

```bash
iroha_cli app nexus lane-report --summary
```

Nümunə çıxışı:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Defolt zolaq `sealed`-i göstərirsə, əvvəl zolaq idarəçiliyi kitabına əməl edin.
xarici trafikə imkan verir. `--fail-on-sealed` bayrağı CI üçün əlverişlidir.

## 5. Torii status yüklərini yoxlayın

`/status` cavabı həm marşrut siyasətini, həm də hər zolaqlı planlaşdırıcını ifşa edir.
snapshot. Konfiqurasiya edilmiş standartları təsdiqləmək və yoxlamaq üçün `curl`/`jq` istifadə edin.
ehtiyat zolağı telemetriya istehsal edir:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Nümunə çıxışı:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

`0` zolağı üçün canlı planlaşdırıcı sayğaclarını yoxlamaq üçün:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Bu, TEU snapşotunun, ləqəb metadatasının və manifest bayraqlarının uyğunlaşdığını təsdiqləyir
konfiqurasiya ilə. Eyni faydalı yük Grafana panelləri üçün istifadə etdiyi yükdür
zolaqlı idarəetmə paneli.

## 6. Müştəri standartlarını həyata keçirin

- **Rust/CLI.** `iroha_cli` və Rust müştəri qutusu `lane_id` sahəsini buraxır
  `--lane-id` / `LaneSelector` keçmədikdə. Buna görə də növbə marşrutlaşdırıcısı
  `default_lane`-ə qayıdır. Açıq `--lane-id`/`--dataspace-id` bayraqlarından istifadə edin
  yalnız qeyri-defolt zolağı hədəf aldıqda.
- **JS/Swift/Android.** Ən son SDK buraxılışları `laneId`/`lane_id` isteğe bağlıdır
  və `/status` tərəfindən elan edilən dəyərə qayıdın. Marşrutlaşdırma siyasətini saxlayın
  səhnələşdirmə və istehsal arasında sinxronizasiya edin ki, mobil tətbiqlər fövqəladə vəziyyətə ehtiyac duymasın
  yenidən konfiqurasiyalar.
- **Boru kəməri/SSE testləri.** Əməliyyat hadisəsi filtrləri qəbul edilir
  `tx_lane_id == <u32>` predikatları (bax: `docs/source/pipeline.md`). Abunə olun
  `/v2/pipeline/events/transactions` yazın göndərildiyini sübut etmək üçün həmin filtrlə
  aydın zolaq olmadan geri qaytarma zolağı identifikatorunun altına çatır.

## 7. Müşahidə oluna bilənlik və idarəetmə qarmaqları

- `/status` həmçinin `nexus_lane_governance_sealed_total` nəşr edir və
  `nexus_lane_governance_sealed_aliases` beləliklə Alertmanager istənilən vaxt xəbərdarlıq edə bilər
  zolaq öz manifestini itirir. Hətta devnetlər üçün belə xəbərdarlıqları aktiv edin.
- Planlaşdırıcı telemetriya xəritəsi və zolaq idarəetmə paneli
  (`dashboards/grafana/nexus_lanes.json`) -dən ləqəb/slug sahələrini gözləyirik
  kataloq. Əgər ləqəbin adını dəyişsəniz, müvafiq Kür kataloqlarını belə adlandırın
  auditorlar deterministik yolları saxlayır (NX-1 altında izlənilir).
- Defolt zolaqlar üçün Parlamentin təsdiqləmələrinə geri çəkilmə planı daxil edilməlidir. Qeyd
  açıq-aşkar hash və idarəetmə sübutu sizin bu sürətli başlanğıcla yanaşı
  operator runbook belə gələcək fırlanmalar tələb olunan vəziyyəti təxmin etmir.

Bu yoxlamalar keçdikdən sonra siz `nexus.routing_policy.default_lane` kimi davrana bilərsiniz
şəbəkədə kod yolları.
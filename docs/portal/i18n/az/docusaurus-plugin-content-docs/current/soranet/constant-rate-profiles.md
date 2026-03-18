---
id: constant-rate-profiles
lang: az
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet constant-rate profiles
sidebar_label: Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production relays plus the SNNet-17A2 null dogfood profile, with tick->bandwidth math, CLI helpers, and MTU guardrails.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

SNNet-17B sabit tarifli nəqliyyat zolaqlarını təqdim edir, beləliklə relelər 1024 B hüceyrəsində trafiki hərəkət etdirir
faydalı yük ölçüsü. Operatorlar üç əvvəlcədən təyin edilmiş parametrdən birini seçirlər:

- **nüvə** - əhatə etmək üçün >=30 Mbps ayıra bilən məlumat mərkəzi və ya peşəkar şəkildə yerləşdirilən relelər
  trafik.
- **ev** - hələ də anonim axtarışa ehtiyacı olan yaşayış və ya aşağı bağlantı operatorları
  məxfilik-kritik sxemlər.
- **null** - SNNet-17A2 sınaq versiyasının əvvəlcədən təyini. Eyni TLV-ləri/zərfləri saxlayır, lakin uzanır
  aşağı bant genişliyi səhnələşdirmə üçün gənə və tavan.

## Əvvəlcədən təyin edilmiş xülasə

| Profil | Gənə (ms) | Hüceyrə (B) | Zolaqlı qapağı | Dummy mərtəbə | Hər zolaqlı yük (Mb/s) | Tavan yükü (Mb/s) | Yuxarı bağlantının tavan %-i | Tövsiyə olunan yuxarı keçid (Mb/s) | Qonşu qapağı | Tətiyi avtomatik söndür (%) |
|---------|-----------|----------|----------|-------------|-------------------------|-------------------|--------------------|--------------------------------|--------------------|--------------------------|
| əsas | 5.0 | 1024 | 12 | 4 | 1.64 | 19.50 | 65 | 30.0 | 8 | 85 |
| ev | 10.0 | 1024 | 4 | 2 | 0,82 | 4.00 | 40 | 10.0 | 2 | 70 |
| null | 20.0 | 1024 | 2 | 1 | 0,41 | 0,75 | 15 | 5.0 | 1 | 55 |

- **Lane cap** - maksimum paralel sabit sürət qonşuları. Röle əlavə dövrələri bir dəfə rədd edir
  qapaq vurulur və `soranet_handshake_capacity_reject_total` artır.
- **Dummy mərtəbə** - faktiki olduqda belə dummy trafiklə canlı qalan zolaqların minimum sayı
  tələb aşağıdır.
- **Tavan yükü** - tavan tətbiq edildikdən sonra sabit sürət zolaqlarına həsr olunmuş yuxarı keçid büdcəsi
  kəsir. Əlavə bant genişliyi mövcud olsa belə, operatorlar heç vaxt bu büdcəni aşmamalıdır.
- **Avtomatik söndürmə tetiği** - davamlı doyma faizi (əvvəlcədən təyin edilmiş orta hesabla)
  dummy mərtəbə düşmək üçün runtime. Bərpa həddindən sonra tutum bərpa olunur
  (`core` üçün 75%, `home` üçün 60%, `null` üçün 45%).

**Vacibdir:** `null` ilkin təyini yalnız səhnələşdirmə və imkanların sınaqdan keçirilməsi üçündür; uyğun gəlmir
istehsal sxemləri üçün tələb olunan məxfilik zəmanətləri.

## Gənə -> bant genişliyi cədvəli

Hər bir faydalı yük hüceyrəsi 1,024 B daşıyır, buna görə də KiB/san sütunu hər biri üçün buraxılan hüceyrələrin sayına bərabərdir.
ikinci. Xüsusi gənələrlə masanı genişləndirmək üçün köməkçidən istifadə edin.

| Gənə (ms) | Hüceyrələr/san | Yükləmə KiB/san | Yükləmə Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0 | 200.00 | 200.00 | 1.64 |
| 7.5 | 133.33 | 133.33 | 1.09 |
| 10.0 | 100.00 | 100.00 | 0,82 |
| 15.0 | 66.67 | 66.67 | 0,55 |
| 20.0 | 50.00 | 50.00 | 0,41 |

Formula:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI köməkçisi:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` həm əvvəlcədən təyin edilmiş xülasə, həm də isteğe bağlı gənə fırıldaqları üçün GitHub üslublu cədvəllər yayır
vərəqlə deterministik çıxışı portala yerləşdirə bilərsiniz. Arxivləmək üçün onu `--json-out` ilə birləşdirin
idarəetmə sübutu üçün təqdim edilmiş məlumatlar.

## Konfiqurasiya və ləğvetmələr

`tools/soranet-relay` həm konfiqurasiya fayllarında, həm də icra müddətinin ləğv edilməsində ilkin parametrləri ifşa edir:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

Konfiqurasiya açarı `core`, `home` və ya `null` (defolt `core`) qəbul edir. CLI overrides üçün faydalıdır
konfiqurasiyaları yenidən yazmadan iş dövrünü müvəqqəti azaldan mərhələli təlimlər və ya SOC sorğuları.

## MTU qoruyucuları

- Faydalı yük hüceyrələri 1024 B plus ~96 B Norito+Səs çərçivəsi və minimal QUIC/UDP başlıqlarından istifadə edir,
  hər dataqramı IPv6 1,280 B minimum MTU-dan aşağı saxlamaq.
- Tunellər (WireGuard/IPsec) əlavə enkapsulyasiya əlavə etdikdə siz **`padding.cell_size` azaltmalısınız**
  belə ki, `cell_size + framing <= 1,280 B`. Rele validator tətbiq edir
  `padding.cell_size <= 1,136 B` (1,280 B - 48 B UDP/IPv6 yerüstü - 96 B çərçivə).
- `core` profilləri hətta boş olduqda belə >=4 qonşuya yapışmalıdır, belə ki, dummy zolaqlar həmişə bir alt çoxluğu əhatə edir.
  PQ mühafizəçiləri. `home` profilləri pul kisələri/aqreqatorları üçün sabit sürət sxemlərini məhdudlaşdıra bilər, lakin tətbiq edilməlidir
  üç telemetriya pəncərəsi üçün doyma 70%-dən çox olduqda əks təzyiq.

## Telemetriya və xəbərdarlıqlar

Relelər hər bir əvvəlcədən təyin edilmiş aşağıdakı ölçüləri ixrac edir:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Xəbərdarlıq zamanı:

1. Dummy nisbəti əvvəlcədən təyin edilmiş mərtəbədən (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) aşağıda qalır
   iki pəncərə.
2. `soranet_constant_rate_ceiling_hits_total` beş dəqiqədə bir vuruşdan daha sürətli böyüyür.
3. `soranet_constant_rate_degraded` planlaşdırılmış məşqdən kənarda `1`-ə çevrilir.

Əvvəlcədən təyin edilmiş etiketi və qonşu siyahısını hadisə hesabatlarında qeyd edin ki, auditorlar sabit nisbəti sübut edə bilsinlər
siyasətlər yol xəritəsinin tələblərinə uyğun gəlirdi.
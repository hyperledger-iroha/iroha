---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e41e5ed0d7b74fe8ea1ac5bda290088ebb54572db240ae9c2546d719e0c6815f
source_last_modified: "2025-11-19T13:44:41.615366+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: constant-rate-profiles
title: SoraNet constant-rate profiles
sidebar_label: Constant-rate profiles
description: core/home production relays کے لئے SNNet-17B1 preset catalog اور SNNet-17A2 null dogfood profile، tick->bandwidth math، CLI helpers، اور MTU guardrails کے ساتھ۔
---

:::note Canonical Source
یہ صفحہ `docs/source/soranet/constant_rate_profiles.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retire نہ ہو، دونوں کاپیاں sync رکھیں۔
:::

SNNet-17B فکسڈ ریٹ ٹرانسپورٹ lanes متعارف کراتا ہے تاکہ relays payload سائز سے قطع نظر 1,024 B cells میں traffic منتقل کریں۔ Operators تین presets میں سے انتخاب کرتے ہیں:

- **core** - data-center یا professionally hosted relays جو traffic کور کرنے کے لئے >=30 Mbps وقف کر سکتے ہیں۔
- **home** - residential یا کم uplink والے operators جو privacy-critical circuits کے لئے اب بھی anonymous fetches چاہتے ہیں۔
- **null** - SNNet-17A2 کا dogfood preset۔ یہ وہی TLVs/envelope برقرار رکھتا ہے مگر کم bandwidth staging کے لئے tick اور ceiling کو بڑھا دیتا ہے۔

## Preset خلاصہ

| Profile | Tick (ms) | Cell (B) | Lane cap | Dummy floor | Per-lane payload (Mb/s) | Ceiling payload (Mb/s) | Ceiling % of uplink | Recommended uplink (Mb/s) | Neighbor cap | Auto-disable trigger (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **Lane cap** - constant-rate neighbors کی زیادہ سے زیادہ concurrent تعداد۔ cap لگنے پر relay اضافی circuits reject کرتا ہے اور `soranet_handshake_capacity_reject_total` بڑھاتا ہے۔
- **Dummy floor** - lanes کی کم از کم تعداد جو dummy traffic کے ساتھ زندہ رہتی ہے چاہے اصل demand کم ہو۔
- **Ceiling payload** - uplink budget جو ceiling fraction لاگو کرنے کے بعد constant-rate lanes کے لئے مختص ہوتا ہے۔ operators کو اضافی bandwidth ہونے پر بھی اس بجٹ سے تجاوز نہیں کرنا چاہئے۔
- **Auto-disable trigger** - مسلسل saturation فیصد (ہر preset کے حساب سے اوسط) جو runtime کو dummy floor تک گرا دیتا ہے۔ recovery threshold کے بعد capacity بحال ہوتی ہے (`core` کے لئے 75%، `home` کے لئے 60%، `null` کے لئے 45%).

**اہم:** `null` preset صرف staging اور capability dogfooding کے لئے ہے؛ یہ production circuits کے لئے مطلوبہ privacy guarantees پوری نہیں کرتا۔

## Tick -> bandwidth table

ہر payload cell میں 1,024 B ہوتا ہے، اس لئے KiB/sec کالم فی سیکنڈ بھیجے گئے cells کی تعداد کے برابر ہے۔ custom ticks کے ساتھ جدول بڑھانے کے لئے helper استعمال کریں۔

| Tick (ms) | Cells/sec | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

فارمولا:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI helper:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` GitHub-style tables emit کرتا ہے، چاہے preset summary ہو یا optional tick cheat sheet، تاکہ آپ deterministic output کو portal میں paste کر سکیں۔ rendered data کو governance evidence کے طور پر archive کرنے کے لئے اسے `--json-out` کے ساتھ pair کریں۔

## Configuration اور overrides

`tools/soranet-relay` presets کو config files اور runtime overrides دونوں میں expose کرتا ہے:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

Config key `core`, `home` یا `null` قبول کرتا ہے (default `core`). CLI overrides staging drills یا SOC requests کے لئے مفید ہیں جو configs دوبارہ لکھے بغیر duty cycle عارضی طور پر کم کر دیتی ہیں۔

## MTU guardrails

- Payload cells 1,024 B کے ساتھ ~96 B Norito+Noise framing اور کم از کم QUIC/UDP headers استعمال کرتے ہیں، جس سے ہر datagram IPv6 کے 1,280 B minimum MTU سے نیچے رہتا ہے۔
- جب tunnels (WireGuard/IPsec) اضافی encapsulation شامل کریں تو **ضروری** ہے کہ `padding.cell_size` کم کریں تاکہ `cell_size + framing <= 1,280 B` ہو۔ relay validator `padding.cell_size <= 1,136 B` لاگو کرتا ہے (1,280 B - 48 B UDP/IPv6 overhead - 96 B framing).
- `core` profiles کو idle حالت میں بھی >=4 neighbors pin کرنا چاہئے تاکہ dummy lanes ہمیشہ PQ guards کے ایک subset کو cover کریں۔ `home` profiles constant-rate circuits کو wallets/aggregators تک محدود کر سکتے ہیں مگر saturation 70% سے اوپر تین telemetry windows تک جائے تو back-pressure لازمی ہے۔

## Telemetry اور alerts

Relays ہر preset کے لئے درج ذیل metrics export کرتے ہیں:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Alert کریں جب:

1. Dummy ratio preset floor سے نیچے رہے (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) اور یہ حالت دو سے زیادہ windows تک قائم رہے۔
2. `soranet_constant_rate_ceiling_hits_total` پانچ منٹ میں ایک hit سے تیز بڑھے۔
3. `soranet_constant_rate_degraded` planned drill کے علاوہ `1` پر flip ہو۔

Incident reports میں preset label اور neighbor list درج کریں تاکہ auditors ثابت کر سکیں کہ constant-rate policies نے roadmap requirements پوری کیں۔

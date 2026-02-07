---
lang: am
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 ደረጃ-ጌት ዘገባ (ቲ?_→ቲ?_)

> ከማቅረቡ በፊት እያንዳንዱን ቦታ ያዥ (ዕቃዎችን በማዕዘን ቅንፎች) ይተኩ። አቆይ
> የአስተዳደር አውቶሜሽን ፋይሉን እንዲተነተን የክፍል ራስጌዎች።

## 1. ሜታዳታ

| መስክ | ዋጋ |
|-------|------|
| ማስተዋወቅ | I18NI0000002X |
| የሪፖርት ማድረጊያ መስኮት | `<YYYY-MM-DD → YYYY-MM-DD>` |
| ቅብብሎሽ በስፋት | `<count + IDs or “see appendix A”>` |
| ዋና ግንኙነት | `<name/email/Matrix handle>` |
| የማስረከቢያ ማህደር | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| ማህደር SHA-256 | `<sha256:...>` |

## 2. የመለኪያዎች ማጠቃለያ

| መለኪያ | ተስተውሏል | ገደብ | ማለፍ? | ምንጭ |
|----
| የወረዳ ስኬት ጥምርታ | `<0.000>` | ≥0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| ቡኒ መውጫ ሬሾን አምጣ | `<0.000>` | ≤0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR ድብልቅ ልዩነት | `<+0.0%>` | ≤±10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 ሰከንዶች | `<0.0 s>` | ≤3 ሰ | ☐ / ☑ | `telemetry/pow_window.json` |
| መዘግየት p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ ጥምርታ (አማካይ) | `<0.00>` | ≥ ኢላማ | ☐ / ☑ | `telemetry/pq_summary.json` |

** ትረካ፡** `<summaries of anomalies, mitigations, overrides>`

## 3. ቁፋሮ እና ክስተት መዝገብ

| የጊዜ ማህተም (UTC) | ክልል | አይነት | የማንቂያ መታወቂያ | ቅነሳ ማጠቃለያ |
|-------------|-------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. ማያያዣዎች እና hashes

| Artefact | መንገድ | SHA-256 |
|--------|------|----|
| ሜትሪክስ ቅጽበታዊ እይታ | `reports/metrics-window.json` | `<sha256>` |
| የመለኪያ ሪፖርት | `reports/metrics-report.json` | `<sha256>` |
| የጥበቃ ማዞሪያ ግልባጭ | `evidence/guard_rotation/*.log` | `<sha256>` |
| ውጣ ትስስር ይገለጣል | `evidence/exit_bonds/*.to` | `<sha256>` |
| ቁፋሮ መዝገቦች | `evidence/drills/*.md` | `<sha256>` |
| MASQUE ዝግጁነት (T1→T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| የመመለሻ እቅድ (T1→T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. ማጽደቆች

| ሚና | ስም | የተፈረመ (Y/N) | ማስታወሻ |
|---------|-------------|-------|
| አውታረ መረብ TL | `<name>` | ☐ / ☑ | `<comments>` |
| የአስተዳደር ተወካይ | `<name>` | ☐ / ☑ | `<comments>` |
| SRE ተወካይ | `<name>` | ☐ / ☑ | `<comments>` |

## አባሪ ሀ - ሪሌይ የስም ዝርዝር

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## አባሪ ለ - የክስተት ማጠቃለያዎች

```
<Detailed context for any incidents or overrides referenced above.>
```
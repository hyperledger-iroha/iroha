---
lang: am
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [] የሃርድዌር ዝርዝርን ይገምግሙ፡ 8+ ኮሮች፣ 16 GiB RAM፣ NVMe ማከማቻ ≥ 500 ሚቢ/ሰ።
- [ ] ሁለት IPv4 + IPv6 አድራሻዎችን እና የወዲያኛውን ፈቃድ QUIC/UDP 443 ያረጋግጡ።
- [ ] ኤች.ኤስ.ኤም.ኤም ወይም ለትራፊክ የማንነት ቁልፎች ልዩ የሆነ አስተማማኝ ማቀፊያ ያቅርቡ።
- [ ] ቀኖናዊ መርጦ መውጫ ካታሎግ (`governance/compliance/soranet_opt_outs.json`) አመሳስል።
- [ ] ተገዢነት ብሎክን ወደ ኦርኬስትራ ውቅር አዋህድ (I18NI0000002X ይመልከቱ)።
- [ ] የዳኝነት/የዓለም አቀፍ ተገዢነት ማረጋገጫዎችን ይያዙ እና የI18NI0000003X ዝርዝርን ይሙሉ።
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (ወይም የቀጥታ ቅጽበታዊ ገጽ እይታዎን) ያሂዱ እና የማለፊያ/ያልተሳካ ሪፖርት ይመልከቱ።
- [ ] የቅብብሎሽ መግቢያ CSR ይፍጠሩ እና በአስተዳደር የተፈረመ ፖስታ ያግኙ።
- [ ] የጥበቃ ገላጭ ዘርን አስመጣ እና በታተመ የሃሽ ሰንሰለት አረጋግጥ።
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` አሂድ።
- [ ] በደረቅ የሚሰራ ቴሌሜትሪ ወደ ውጪ መላክ፡ Prometheus መቧጨር በአገር ውስጥ ስኬታማ መሆኑን ያረጋግጡ።
- [ ] ቡኒ አውት መሰርሰሪያ መስኮት መርሐግብር እና ማሳደግ እውቂያዎች መዝግብ.
- [ ] የመሰርሰሪያ ማስረጃ ጥቅል ይፈርሙ፡ `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`።
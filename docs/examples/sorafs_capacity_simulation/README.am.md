---
lang: am
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS የአቅም ማስመሰል መሣሪያ ስብስብ

ይህ ማውጫ ለSF-2c አቅም የገበያ ቦታ ሊባዙ የሚችሉ ቅርሶችን ይልካል።
ማስመሰል. የመሳሪያ ኪቱ የኮታ ድርድርን፣ ያልተሳካ አያያዝን እና መቆራረጥን ይለማመዳል
የምርት CLI አጋዥዎችን እና ቀላል ክብደት ያለው የትንታኔ ስክሪፕት በመጠቀም ማሻሻያ።

## ቅድመ ሁኔታዎች

- Rust toolchain ለስራ ቦታ አባላት `cargo run` ማስኬድ የሚችል።
- Python 3.10+ (መደበኛ ቤተ-መጽሐፍት ብቻ)።

## ፈጣን ጅምር

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

የ`run_cli.sh` ስክሪፕት `sorafs_manifest_stub capacity` እንዲገነባ ጠይቋል፡-

- ቆራጥ አቅራቢዎች ለኮታ ድርድር ማቀፊያ ስብስብ።
- ከድርድር ሁኔታው ​​ጋር የሚዛመድ የማባዛት ትእዛዝ።
- ለተሳካው መስኮት ቴሌሜትሪ ቅጽበተ-ፎቶዎች።
- የመቁረጥ ጥያቄን የሚይዝ የሙግት ጭነት።

ስክሪፕቱ I18NT0000005X ባይት (`*.to`)፣ ቤዝ64 የሚጫኑ ጭነቶች (`*.b64`)፣ Torii ጥያቄን ይጽፋል።
አካላት እና በሰው ሊነበቡ የሚችሉ ማጠቃለያዎች (`*_summary.json`) በተመረጠው ቅርስ ስር
ማውጫ.

`analyze.py` የመነጨውን ማጠቃለያ ይበላል ፣የተጠቃለለ ሪፖርት ያወጣል።
(`capacity_simulation_report.json`)፣ እና Prometheus የጽሑፍ ፋይል ያወጣል።
(`capacity_simulation.prom`) ይዞ፡

- `sorafs_simulation_quota_*` መለኪያዎች የመደራደር አቅም እና ምደባን የሚገልጹ
  በአቅራቢው ያካፍሉ።
- የ `sorafs_simulation_failover_*` መለኪያዎች የመቀነስ ዴልታዎችን እና የተመረጡትን ያደምቃሉ
  ምትክ አቅራቢ.
- `sorafs_simulation_slash_requested` የወጣውን የማገገሚያ መቶኛ በመመዝገብ ላይ
  ከክርክር ጭነት.

የGrafana ጥቅልን በI18NI0000021X አስመጣ
እና የመነጨውን የጽሁፍ ፋይሉን የሚጠርግ I18NT0000002X የመረጃ ምንጭ ላይ ጠቁመው (ለ
ለምሳሌ በመስቀለኛ - ላኪ የጽሑፍ ፋይል ሰብሳቢ)። የ runbook በ
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` ሙሉ በሙሉ ያልፋል
የስራ ሂደት፣ Prometheus ውቅር ምክሮችን ጨምሮ።

## መለዋወጫዎች

- `scenarios/quota_negotiation/` - የአቅራቢ መግለጫ ዝርዝሮች እና የማባዛት ቅደም ተከተል።
- `scenarios/failover/` - የቴሌሜትሪ መስኮቶች ለዋና መጥፋት እና ውድቀት ማንሻ።
- `scenarios/slashing/` - ተመሳሳይ የማባዛት ቅደም ተከተልን የሚያመለክት ክርክር።

እነዚህ ቋሚዎች በ `crates/sorafs_car/tests/capacity_simulation_toolkit.rs` ውስጥ የተረጋገጡ ናቸው።
ከ CLI እቅድ ጋር እንደተመሳሰሉ እንዲቆዩ ዋስትና ለመስጠት።
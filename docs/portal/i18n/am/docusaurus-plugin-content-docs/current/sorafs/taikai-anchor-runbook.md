---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# የታይካይ መልህቅ ታዛቢነት Runbook

ይህ የፖርታል ቅጂ ቀኖናዊውን የሩጫ መጽሐፍን ያንጸባርቃል
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md)።
SoraFS/SoraNet SN13-C routing-manifest (TRM) መልህቆችን ሲለማመዱ ይጠቀሙበት።
ኦፕሬተሮች ስፖል ቅርሶችን፣ Prometheus ቴሌሜትሪ እና አስተዳደርን ማዛመድ ይችላሉ
ከፖርታል ቅድመ እይታ ግንባታ ሳይወጡ ማስረጃ።

## ወሰን እና ባለቤቶች

- ** ፕሮግራም: ** SN13-C - ታይካይ ይገለጣል እና የሶራንስ መልህቆች።
- ** ባለቤቶች፡** የሚዲያ መድረክ WG፣DA Program፣Networking TL፣ Docs/DevRel
- ** ግብ:** ለ Sev1/Sev2 ማንቂያዎች፣ ቴሌሜትሪ የሚወስን የመጫወቻ መጽሐፍ ያቅርቡ
  የታይካይ ማዘዋወር ወደ ፊት በሚገለጥበት ጊዜ ማረጋገጫ እና ማስረጃ መያዝ
  ተለዋጭ ስሞች።

## ፈጣን ጅምር (ሴቭ1/ሴቭ2)

1. ** የስፑል ቅርሶችን ይያዙ *** - የቅርብ ጊዜውን ይቅዱ
   `taikai-anchor-request-*.json`፣ `taikai-trm-state-*.json`፣ እና
   `taikai-lineage-*.json` ፋይሎች ከ
   ሰራተኞችን እንደገና ከመጀመርዎ በፊት `config.da_ingest.manifest_store_dir/taikai/`።
2. ** I18NI0000022X ቴሌሜትሪ ይጥሉ *** - ይመዝግቡ
   የትኛው አንጸባራቂ መስኮት እንደሆነ ለማረጋገጥ `telemetry.taikai_alias_rotations` ድርድር
   ንቁ:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. ** ዳሽቦርዶችን እና ማንቂያዎችን ይመልከቱ *** - ጭነት
   `dashboards/grafana/taikai_viewer.json` (ክላስተር + የዥረት ማጣሪያዎች) እና ማስታወሻ
   ውስጥ ማንኛውም ደንብ እንደሆነ
   `dashboards/alerts/taikai_viewer_rules.yml` ተባረረ (`TaikaiLiveEdgeDrift`፣
   `TaikaiIngestFailure`፣ `TaikaiCekRotationLag`፣ SoraFS ማረጋገጫ-የጤና ክስተቶች)።
4. ** I18NT0000001X ን መርምር *** - ለማረጋገጥ ጥያቄዎቹን በ §“መለኪያ ማጣቀሻ” ውስጥ ያሂዱ።
   መዘግየት/መንሸራተት እና ተለዋጭ መጠሪያ ቆጣሪዎች እንደተጠበቀው ይሠራሉ። ጨምር
   `taikai_trm_alias_rotations_total` ለብዙ መስኮቶች ከቆመ ወይም ከሆነ
   የስህተት ቆጣሪዎች ይጨምራሉ.

## ሜትሪክ ማጣቀሻ

| መለኪያ | ዓላማ |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | CMAF የላቲነት ሂስቶግራምን በክላስተር/ዥረት ይመገባል (ዒላማ፡ p95<750ms፣ p99<900ms)። |
| `taikai_ingest_live_edge_drift_ms` | በመቀየሪያ እና መልህቅ ሰራተኞች መካከል የቀጥታ ጠርዝ መንሸራተት (ገጽ በp99>1.5s ለ10 ደቂቃ)። |
| `taikai_ingest_segment_errors_total{reason}` | ስህተት ቆጣሪዎች በምክንያት (`decode`፣ `manifest_mismatch`፣ `lineage_replay`፣ …)። ማንኛውም ጭማሪ `TaikaiIngestFailure` ቀስቅሴዎች. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | I18NI0000038X ለተለዋጭ ስም አዲስ TRM ሲቀበል ይጨምራል። የማሽከርከር ችሎታን ለማረጋገጥ `rate()` ይጠቀሙ። |
| `/status → telemetry.taikai_alias_rotations[]` | JSON ቅጽበተ-ፎቶ ከI18NI0000041X፣ `window_end_sequence`፣ `manifest_digest_hex`፣ `rotations_total` እና የጊዜ ማህተሞች ለማረጃ ጥቅሎች። |
| `taikai_viewer_*` (ማቋቋሚያ፣ CEK የመዞሪያ ዕድሜ፣ PQ ጤና፣ ማንቂያዎች) | የCEK ሽክርክር + PQ ወረዳዎች በመልህቆች ጊዜ ጤናማ ሆነው መቆየታቸውን ለማረጋገጥ በተመልካች ጎን KPIs። |

### PromQL ቅንጥቦች

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

## ዳሽቦርዶች እና ማንቂያዎች

- **Grafana መመልከቻ ሰሌዳ፡** `dashboards/grafana/taikai_viewer.json` — p95/p99
  መዘግየት፣ የቀጥታ ጠርዝ ተንሳፋፊ፣ የክፍል ስህተቶች፣ የCEK ሽክርክር ዕድሜ፣ የተመልካች ማንቂያዎች።
- ** Grafana መሸጎጫ ሰሌዳ:** `dashboards/grafana/taikai_cache.json` - ሙቅ / ሙቅ / ቅዝቃዜ
  ማስተዋወቂያዎች እና QoS መካድዎች ቅጽል መስኮቶች ሲሽከረከሩ።
- ** የማንቂያ አስተዳዳሪ ህጎች፡** `dashboards/alerts/taikai_viewer_rules.yml` — ተንሸራታች
  ፔጂንግ፣ ወደ ውስጥ መግባት አለመሳካት ማስጠንቀቂያዎች፣ CEK የማዞሪያ መዘግየት፣ እና I18NT0000010X ማረጋገጫ-ጤና
  ቅጣቶች / ቅጣቶች. ለእያንዳንዱ የምርት ክላስተር ተቀባዮች መኖራቸውን ያረጋግጡ።

## የማስረጃ ጥቅል ማረጋገጫ ዝርዝር

- ስፖል ቅርሶች (`taikai-anchor-request-*`፣ `taikai-trm-state-*`፣
  `taikai-lineage-*`)።
- በመጠባበቅ ላይ ያሉ/የሚላኩ ፖስታዎችን እና የተፈረመ JSON ክምችት ለመልቀቅ `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`ን ያሂዱ እና ጥያቄ/SSM/TRM/የመስመር ፋይሎችን ወደ መሰርሰሪያ ጥቅል። ነባሪው የስፑል መንገድ I18NI0000053X ከ `torii.toml` ነው።
- `/status` I18NI0000056X የሚሸፍን ቅጽበታዊ ገጽ እይታ።
- Prometheus ወደ ውጭ መላክ (JSON/CSV) በአደጋው ​​መስኮት ላይ ከላይ ላሉት መለኪያዎች።
- Grafana ቅጽበታዊ ገጽ እይታዎች ከማጣሪያዎች ጋር።
- አግባብነት ያለው ህግን የሚያመለክቱ የማንቂያ አስተዳዳሪ መታወቂያዎች ይቃጠላሉ።
- ወደ `docs/examples/taikai_anchor_lineage_packet.md` የሚገልጽ አገናኝ
  ቀኖናዊ ማስረጃ ፓኬት.

## ዳሽቦርድ ማንጸባረቅ እና መሰርሰሪያ ቁፋሮ

የ SN13-C የመንገድ ካርታ መስፈርትን ማሟላት ማለት ታይካይ መሆኑን ማረጋገጥ ማለት ነው።
ተመልካች/መሸጎጫ ዳሽቦርዶች በፖርታሉ ውስጥ ተንጸባርቀዋል **እና** መልህቁ
የማስረጃ መሰርሰሪያ ሊገመት በሚችል ክዳን ላይ ይሰራል።

1. ** ፖርታል ማንጸባረቅ.** በማንኛውም ጊዜ `dashboards/grafana/taikai_viewer.json` ወይም
   `dashboards/grafana/taikai_cache.json` ይቀየራል፣ በ ውስጥ ያለውን ዴልታ ጠቅለል አድርጉ
   `sorafs/taikai-monitoring-dashboards` (ይህ ፖርታል) እና JSON ን ልብ ይበሉ
   ቼኮች በፖርታል PR መግለጫ። ስለዚህ አዲስ ፓነሎችን/ደረጃዎችን ያድምቁ
   ገምጋሚዎች ከሚተዳደረው Grafana አቃፊ ጋር ማዛመድ ይችላሉ።
2. ** ወርሃዊ ልምምድ.**
   - ልምምዱን በየወሩ የመጀመሪያ ማክሰኞ በ 15:00UTC ስለዚህ ማስረጃ ያካሂዱ
     ከ SN13 አስተዳደር ማመሳሰል በፊት መሬቶች።
   - ስፑል ቅርሶችን፣ `/status` telemetry እና Grafana ቅጽበታዊ ገጽ እይታዎችን ከውስጥ ያንሱ
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - አፈፃፀሙን በ ጋር ይመዝገቡ
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **ይገምግሙ እና አትም
   DA Program + NetOps፣ የክትትል ዕቃዎችን በመሰርሰሪያ መዝገብ ውስጥ ይመዝግቡ እና ያገናኙት።
   የአስተዳደር ባልዲ ጭነት ከ I18NI0000064X.

ዳሽቦርዶች ወይም ልምምዶች ወደ ኋላ ከወደቁ፣ SN13-C መውጣት አይችልም 🈺; ይህን ጠብቅ
የድጋፍ ወይም የማስረጃ ተስፋዎች በሚቀየሩበት ጊዜ ክፍል ወቅታዊ።

## ጠቃሚ ትዕዛዞች

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

ይህንን የፖርታል ቅጂ በማንኛውም ጊዜ ታይካይ ከቀኖናዊው runbook ጋር እንዲመሳሰል ያድርጉት
መልህቅ ቴሌሜትሪ፣ ዳሽቦርድ ወይም የአስተዳደር ማስረጃ መስፈርቶች ይለወጣሉ።
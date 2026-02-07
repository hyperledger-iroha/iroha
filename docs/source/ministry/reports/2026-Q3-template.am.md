---
lang: am
direction: ltr
source: docs/source/ministry/reports/2026-Q3-template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f313d8010f2a7174c90f51dea512bcab6eb4a207df9199f28a7352944cb43c8b
source_last_modified: "2025-12-29T18:16:35.981653+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency Report — 2026 Q3 (Template)
summary: Scaffold for the MINFO-8 quarterly transparency packet; replace all tokens before publication.
quarter: 2026-Q3
translator: machine-google-reviewed
---

<!--
  Usage:
    1. Copy this file when drafting a new quarter (e.g., 2026-Q3 → 2026-Q3.md).
    2. Replace every {{TOKEN}} marker and remove instructional callouts.
    3. Attach supporting artefacts (data appendix, CSVs, manifest, Grafana export) under artifacts/ministry/transparency/<YYYY-Q>/.
-->

# አስፈፃሚ ማጠቃለያ

> ስለ ልከኝነት ትክክለኛነት፣ የይግባኝ ውጤቶች፣ የክህደት ዝርዝር እና የግምጃ ቤት ድምቀቶችን ባለ አንድ አንቀጽ ማጠቃለያ ያቅርቡ። የተለቀቀው የT+14 የመጨረሻ ቀነ-ገደብ መጠናቀቁን ይጥቀሱ።

## በግምገማ ላይ

### ድምቀቶች
- {{HIGHLIGHT_1}}
- {{HIGHLIGHT_2}}
- {{HIGHLIGHT_3}}

### አደጋዎች እና ቅነሳዎች

| ስጋት | ተጽዕኖ | ቅነሳ | ባለቤት | ሁኔታ |
|-------|--------|-----------|-------|
| {{RISK_1}} | {{ተጽዕኖ}} | {{መቀነስ}} | {{ባለቤት}} | {{ሁኔታ}} |
| {{RISK_2}} | {{ተጽዕኖ}} | {{መቀነስ}} | {{ባለቤት}} | {{ሁኔታ}} |

## የመለኪያዎች አጠቃላይ እይታ

ሁሉም መለኪያዎች የሚመነጩት ዲፒ ሳኒታይዘር ካለቀ በኋላ ከ`ministry_transparency_builder` (Norito ጥቅል) ነው። ከዚህ በታች የተጠቀሱትን ተዛማጅ የCSV ቁርጥራጮች ያያይዙ።

### የ AI ልከኝነት ትክክለኛነት

| የሞዴል መገለጫ | ክልል | FP ተመን (ዒላማ) | FN ተመን (ዒላማ) | ድሪፍት vs ካሊብሬሽን | የናሙና መጠን | ማስታወሻ |
-------------|------------|-------------|
| {{መገለጫ}} | {{ክልል}} | {{fp_rate}} ({{fp_ዒላማ}}) | {{fn_rate}} ({{fn_target}}) | {{መንሸራተት}} | {{ናሙናዎች}} | {{ማስታወሻ}} |

### ይግባኝ እና የፓናል እንቅስቃሴ

| መለኪያ | ዋጋ | SLA ዒላማ | አዝማሚያ vs Q-1 | ማስታወሻ |
|--------|--------|------------|-------------|------|
| ይግባኝ ደረሰ | {{ይግባኝ_ተቀበሉ}} | {{sla}} | {{ዴልታ}} | {{ማስታወሻ}} |
| ሚዲያን የመፍትሄ ጊዜ | {{ሚዲያን_ጥራት}} | {{sla}} | {{ዴልታ}} | {{ማስታወሻ}} |
| የተገላቢጦሽ መጠን | {{ተገላቢጦሽ_ደረጃ}} | {{ዒላማ}} | {{ዴልታ}} | {{ማስታወሻ}} |
| የፓነል አጠቃቀም | {{panel_utilization}} | {{ዒላማ}} | {{ዴልታ}} | {{ማስታወሻ}} |

### ውድቅ ዝርዝር እና የድንገተኛ ጊዜ ቀኖና

| መለኪያ | መቁጠር | DP ጫጫታ (ε) | የአደጋ ባንዲራዎች | TTL ተገዢነት | ማስታወሻ |
|--------|-------|------------
| ሃሽ ተጨማሪዎች | {{ተጨማሪ}} | {{epsilon_counts}} | {{ባንዲራ}} | {{ttl_status}} | {{ማስታወሻ}} |
| የሃሽ ማስወገጃዎች | {{ማስወገድ}} | {{epsilon_counts}} | {{ባንዲራ}} | {{ttl_status}} | {{ማስታወሻ}} |
| ቀኖና ጥሪዎች | {{ቀኖና_ጥሪዎች}} | n/a | {{ባንዲራ}} | {{ttl_status}} | {{ማስታወሻ}} |

### የግምጃ ቤት እንቅስቃሴዎች

| ፍሰት | መጠን (MINFO) | ምንጭ ዋቢ | ማስታወሻ |
|-------------|---------|
| ይግባኝ ተቀማጭ | {{መጠን}} | {{tx_ref}} | {{ማስታወሻ}} |
| የፓነል ሽልማቶች | {{መጠን}} | {{tx_ref}} | {{ማስታወሻ}} |
| የስራ ማስኬጃ ወጪ | {{መጠን}} | {{tx_ref}} | {{ማስታወሻ}} |

### የበጎ ፈቃደኞች እና የማድረስ ምልክቶች

| መለኪያ | ዋጋ | ዒላማ | ማስታወሻ |
|--------|-------|--------|-------|
| የበጎ ፈቃደኞች አጭር መግለጫዎች ታትመዋል | {{እሴት}} | {{ዒላማ}} | {{ማስታወሻ}} |
| የተሸፈኑ ቋንቋዎች | {{እሴት}} | {{ዒላማ}} | {{ማስታወሻ}} |
| የአስተዳደር አውደ ጥናቶች ተካሂደዋል | {{እሴት}} | {{ዒላማ}} | {{ማስታወሻ}} |

## ልዩነት ግላዊነት እና ንፅህና

የንፅህና መጠበቂያውን ሩጫ ጠቅለል አድርገው የ RNG ቁርጠኝነትን ያካትቱ።

- የጽዳት ሥራ: `{{CI_JOB_URL}}`
- DP መለኪያዎች፡ ε={{epsilon_total}}፣ δ={{ዴልታ_ቶታል}}
- RNG ቁርጠኝነት: `{{blake3_seed_commitment}}`
- ባልዲዎች የታፈኑ፡ {{የተጨመቁ_ባልዲዎች}}
- QA ገምጋሚ፡ {{ገምጋሚ}}

`artifacts/ministry/transparency/{{Quarter}}/dp_report.json` ያያይዙ እና ማንኛቸውም በእጅ የሚደረጉ ጣልቃ ገብነቶችን ያስተውሉ።## የውሂብ አባሪዎች

| Artefact | መንገድ | SHA-256 | ወደ SoraFS ተሰቅሏል? | ማስታወሻ |
|--------|--------|
| ማጠቃለያ PDF | `artifacts/ministry/transparency/{{Quarter}}/summary.pdf` | {{ሀሽ}} | {{አዎ/አይ}} | {{ማስታወሻ}} |
| Norito ውሂብ አባሪ | `artifacts/ministry/transparency/{{Quarter}}/data/appendix.norito` | {{ሀሽ}} | {{አዎ/አይ}} | {{ማስታወሻ}} |
| መለኪያዎች CSV ጥቅል | `artifacts/ministry/transparency/{{Quarter}}/data/csv/` | {{ሀሽ}} | {{አዎ/አይ}} | {{ማስታወሻ}} |
| Grafana ወደ ውጭ መላክ | `dashboards/grafana/ministry_transparency_overview.json` | {{ሀሽ}} | {{አዎ/አይ}} | {{ማስታወሻ}} |
| የማንቂያ ደንቦች | `dashboards/alerts/ministry_transparency_rules.yml` | {{ሀሽ}} | {{አዎ/አይ}} | {{ማስታወሻ}} |
| Provenance አንጸባራቂ | `artifacts/ministry/transparency/{{Quarter}}/manifest.json` | {{ሀሽ}} | {{አዎ/አይ}} | {{ማስታወሻ}} |
| ገላጭ ፊርማ | `artifacts/ministry/transparency/{{Quarter}}/manifest.json.sig` | {{ሀሽ}} | {{አዎ/አይ}} | {{ማስታወሻ}} |

## የሕትመት ዲበ ውሂብ

| መስክ | ዋጋ |
|-------|------|
| የመልቀቂያ ሩብ | {{ሩብ}} |
| የጊዜ ማህተም (UTC) | {{timestamp}} |
| SoraFS CID | `{{cid}}` |
| የአስተዳደር ድምጽ መታወቂያ | {{የድምጽ_መታወቂያ}} |
| አንጸባራቂ መፍጨት (`blake2b`) | `{{manifest_digest}}` |
| Git መፈጸም / መለያ | `{{git_rev}}` |
| የተለቀቀው ባለቤት | {{ባለቤት}} |

## ማጽደቆች

| ሚና | ስም | ውሳኔ | የጊዜ ማህተም | ማስታወሻ |
|-------------|-------|-----------|-------|
| ሚኒስቴር ታዛቢነት TL | {{ስም}} | ✅/⚠️ | {{timestamp}} | {{ማስታወሻ}} |
| የአስተዳደር ምክር ቤት ግንኙነት | {{ስም}} | ✅/⚠️ | {{timestamp}} | {{ማስታወሻ}} |
| ሰነዶች/Comms መሪ | {{ስም}} | ✅/⚠️ | {{timestamp}} | {{ማስታወሻ}} |

## ለውጥ መዝገብ እና ክትትል

- {{CHANGELOG_ITEM_1}}
- {{CHANGELOG_ITEM_2}}

### የተግባር እቃዎችን ክፈት

| ንጥል | ባለቤት | የሚከፈልበት | ሁኔታ | ማስታወሻ |
|---------
| {{ድርጊት}} | {{ባለቤት}} | {{የተጠናቀቀ}} | {{ሁኔታ}} | {{ማስታወሻ}} |

### ያግኙን።

- ዋና እውቂያ፡ {{contact_name}} (`{{chat_handle}}`)
- መወጣጫ መንገድ፡ {{escalation_details}}
- የስርጭት ዝርዝር፡ {{mailing_list}}

_አብነት ሥሪት፡ 2026-03-25 መዋቅራዊ ለውጦችን ሲያደርጉ የማሻሻያ ቀኑን ያዘምኑ።_
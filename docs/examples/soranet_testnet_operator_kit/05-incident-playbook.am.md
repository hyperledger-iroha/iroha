---
lang: am
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ብራውን መውጣት/የወረደ ምላሽ Playbook

1. ** ፈልግ ***
   - ማንቂያ I18NI0000000X እሳቶች ወይም
     ቡኒ የዌብ መንጠቆ ከአስተዳደር ያስነሳል።
   - በ 5 ደቂቃዎች ውስጥ በI18NI0000001X ወይም በስርዓት የተዘጋጀ ጆርናል ያረጋግጡ።

2. ** ተረጋጋ ***
   - የጥበቃ ማሽከርከርን (`relay guard-rotation disable --ttl 30m`)።
   - ለተጎዱ ደንበኞች በቀጥታ ብቻ መሻርን አንቃ
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`)።
   - የአሁኑን ተገዢነት ማዋቀር ሃሽ (`sha256sum compliance.toml`) ያንሱ።

3. ** ምርመራ ***
   - የቅርብ ጊዜ የማውጫ ቅጽበታዊ ገጽ እይታን ይሰብስቡ እና የመለኪያ ልኬቶችን ጥቅል ያድርጉ።
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - የPoW ወረፋ ጥልቀት ፣ ስሮትል ቆጣሪዎች እና የ GAR ምድብ ስፒሎች ያስተውሉ።
   - የPQ ጉድለት፣ ተገዢነት መሻር ወይም የዝውውር አለመሳካት ክስተቱን ያመጣው እንደሆነ ይለዩ።

4. **አሳድግ**
   - የአስተዳደር ድልድዩን (`#soranet-incident`) በማጠቃለያ እና በጥቅል ሃሽ ያሳውቁ።
   - የጊዜ ማህተሞችን እና የመቀነስ እርምጃዎችን ጨምሮ ከማንቂያው ጋር የሚያገናኝ የክስተቶች ትኬት ይክፈቱ።

5. ** ማገገም ***
   - የስር መንስኤው ከተስተካከለ በኋላ ማሽከርከርን እንደገና አንቃ
     (`relay guard-rotation enable`) እና ቀጥታ-ብቻ መሻሮችን አድህር።
   - ለ 30 ደቂቃዎች KPIs ይቆጣጠሩ; አዲስ ቡኒዎች እንዳይታዩ ያረጋግጡ።

6. **ከሞት በኋላ**
   - የአስተዳደር አብነት በመጠቀም በ 48 ሰዓታት ውስጥ የአደጋ ሪፖርት ያቅርቡ።
   - አዲስ ውድቀት ሁነታ ከተገኘ runbooks ያዘምኑ።
---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ad407370f375e45f0143f082b33a5ea61698825c8cd92dac402f656fb0f61a2
source_last_modified: "2026-01-22T16:26:46.524254+00:00"
translation_last_reviewed: 2026-02-07
id: dispute-revocation-runbook
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

#ዓላማ

ይህ Runbook የአስተዳደር ኦፕሬተሮችን SoraFS የአቅም አለመግባባቶችን በማቅረብ፣ ስረዛዎችን በማስተባበር እና የውሂብ መልቀቅ በቆራጥነት መጠናቀቁን በማረጋገጥ ይመራል።

## 1. ክስተቱን ይገምግሙ

- ** ቀስቅሴ ሁኔታዎች፡** የ SLA ጥሰትን መለየት (የጊዜ/PoR ውድቀት)፣ የማባዛት እጥረት፣ ወይም የሂሳብ አከፋፈል አለመግባባት።
- ** ቴሌሜትሪ አረጋግጥ:** ለአቅራቢው `/v1/sorafs/capacity/state` እና `/v1/sorafs/capacity/telemetry` ቅጽበተ-ፎቶዎችን ያንሱ።
- **ለባለድርሻ አካላት ያሳውቁ፡** የማከማቻ ቡድን (የአቅራቢዎች ስራዎች)፣ የአስተዳደር ምክር ቤት (የውሳኔ አካል)፣ ታዛቢነት (የዳሽቦርድ ዝመናዎች)።

## 2. የማስረጃ ጥቅል ያዘጋጁ

1. ጥሬ እቃዎች (ቴሌሜትሪ JSON, CLI ምዝግብ ማስታወሻዎች, የኦዲተር ማስታወሻዎች) ይሰብስቡ.
2. ወደ መወሰኛ ማህደር (ለምሳሌ ታርቦል) መደበኛ ማድረግ; መዝገብ፡
   - BLAKE3-256 መፍጨት (`evidence_digest`)
   - የሚዲያ ዓይነት (`application/zip`፣ I18NI0000013X፣ እና የመሳሰሉት)
   - URI ማስተናገድ (የነገር ማከማቻ፣ SoraFS ፒን፣ ወይም I18NT0000003X-ተደራሽ የመጨረሻ ነጥብ)
3. ጥቅሉን በአስተዳደር የማስረጃ ማሰባሰቢያ ባልዲ ውስጥ ያከማቹ ከጽሑፍ አንድ ጊዜ ጋር።

## 3. ክርክሩን ያስገቡ

1. ለ`sorafs_manifest_stub capacity dispute` ልዩ JSON ይፍጠሩ፡

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI ን ያሂዱ፡-

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` ይገምግሙ (አይነት፣ የማስረጃ መፍጨት፣ የጊዜ ማህተሞችን ያረጋግጡ)።
4. ጥያቄውን JSON በ Torii `/v1/sorafs/capacity/dispute` በአስተዳደር ግብይት ወረፋ አስረክብ። የ `dispute_id_hex` ምላሽ እሴትን ያንሱ; ተከታታይ የመሻር እርምጃዎችን እና የኦዲት ሪፖርቶችን ያስቀምጣል.

## 4. መልቀቅ እና መሻር

1. **የጸጋ መስኮት፡** ስለሚመጣው መሻር አቅራቢውን ያሳውቁ፤ ፖሊሲ ሲፈቅድ የተሰካውን ውሂብ ለመልቀቅ ፍቀድ።
2. ** `ProviderAdmissionRevocationV1` ፍጠር:**
   - ከተፈቀደው ምክንያት `sorafs_manifest_stub provider-admission revoke` ይጠቀሙ።
   - ፊርማዎችን እና የመሻር ሂደቱን ያረጋግጡ።
3. ** መሻርን አትም::**
   - የመሻር ጥያቄውን ለTorii ያስገቡ።
   - የአቅራቢዎች ማስታወቂያዎች መዘጋታቸውን ያረጋግጡ (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` ለመውጣት ይጠብቁ)።
4. ** ዳሽቦርዶችን ያዘምኑ፡** አቅራቢውን እንደተሻረ ይጠቁሙ፣ የክርክር መታወቂያውን ያጣሩ እና የማስረጃውን ጥቅል ያገናኙ።

## 5. ከሞት በኋላ እና ክትትል

- የጊዜ መስመርን፣ ዋና መንስኤን እና የማስተካከያ እርምጃዎችን በአስተዳደር ክስተት መከታተያ ውስጥ ይመዝግቡ።
- መመለስን ይወስኑ (የካስማ መቆራረጥ ፣ የክፍያ መጨናነቅ ፣ የደንበኛ ተመላሽ ገንዘቦች)።
- የሰነድ ትምህርቶች; አስፈላጊ ከሆነ የ SLA ገደቦችን ያዘምኑ ወይም ማንቂያዎችን ይቆጣጠሩ።

## 6. የማጣቀሻ እቃዎች

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (የክርክር ክፍል)
- `docs/source/sorafs/provider_admission_policy.md` (የመሻሪያ የስራ ፍሰት)
- ታዛቢነት ዳሽቦርድ: `SoraFS / Capacity Providers`

#የማረጋገጫ ዝርዝር

- [ ] ማስረጃ ቅርቅብ ተይዟል እና ሃሽ።
- [ ] የሙግት ጭነት በአገር ውስጥ ተረጋግጧል።
- [ ] Torii የሙግት ግብይት ተቀባይነት አግኝቷል።
- [ ] መሻር ተፈጽሟል (ከተፈቀደ)።
- [ ] ዳሽቦርዶች/ runbooks ተዘምነዋል።
- [ ] የአስከሬን ምርመራ ለአስተዳደር ምክር ቤት ቀረበ።
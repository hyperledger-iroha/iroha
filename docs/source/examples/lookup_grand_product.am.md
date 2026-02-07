---
lang: am
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2025-12-29T18:16:35.953884+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የታላቁን ምርት ምሳሌ ይፈልጉ

ይህ ምሳሌ በ ውስጥ የተጠቀሰውን የFASTPQ ፍቃድ ፍለጋ ክርክርን ያሰፋል
`fastpq_plan.md`.  በ 2 ኛ ደረጃ የቧንቧ መስመር መረጩን ይገመግማል
(`s_perm`) እና ምስክር (`perm_hash`) አምዶች ዝቅተኛ-ዲግሪ ቅጥያ (LDE)
ጎራ፣ እያሄደ ያለ ታላቅ ምርት `Z_i` ያዘምናል፣ እና በመጨረሻም ሙሉውን ፈፅሟል።
ቅደም ተከተል ከ Poseidon ጋር.  የሃሼድ ክምችት ወደ ግልባጩ ተያይዟል።
በ `fastpq:v1:lookup:product` ጎራ ስር፣ የመጨረሻው `Z_i` አሁንም ሲዛመድ
የተሰጠው የፍቃድ ሰንጠረዥ ምርት `T`።

ከሚከተሉት መራጭ እሴቶች ጋር አንድ ትንሽ ስብስብ እንመለከታለን።

| ረድፍ | `s_perm` | `perm_hash` |
| --- | -------- | -------------------------------------------------- |
| 0 | 1 | `0x019a...` (የስጦታ ሚና = ኦዲተር ፣ perm = ማስተላለፍ_ንብረት) |
| 1 | 0 | `0xabcd...` (ምንም የፍቃድ ለውጥ የለም) |
| 2 | 1 | `0x42ff...` (ሚና መሻር = ኦዲተር፣ perm = የተቃጠለ ንብረት) |

`gamma = 0xdead...` የ Fiat-Shamir ፍለጋ ፈተና ይሁን
ግልባጭ  ማረጋገጫው `Z_0 = 1` ይጀምራል እና እያንዳንዱን ረድፍ ያጠፋል፡

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

`s_perm = 0` የሚከማችባቸውን ረድፎች የማይቀይሩት።  ከተሰራ በኋላ
ዱካ፣ ፕሮፌሰሩ ፖሲዶን-ለግልባጩ ቅደም ተከተል `[Z_1, Z_2, ...]` hashes
ሆኖም ከጠረጴዛው ጋር እንዲመሳሰል `Z_final = Z_3` (የመጨረሻው ሩጫ ምርት) ያትማል
የድንበር ሁኔታ.

በጠረጴዛው በኩል ፣የተሰጠው ፈቃድ Merkle ዛፍ ወሳኙን ኮድ ያሳያል
ማስገቢያ የሚሆን ንቁ ፈቃዶች ስብስብ.  አረጋጋጭ (ወይም በወቅት ወቅት
የምሥክር ትውልድ) ያሰላል

```
T = product over entries: (entry.hash + gamma)
```

ፕሮቶኮሉ የድንበር ገደቡን `Z_final / T = 1` ያስፈጽማል።  ዱካው ከሆነ
በሠንጠረዡ ውስጥ የማይገኝ ፈቃድ አስተዋውቋል (ወይም አንዱን የተተወ
ነው)፣ የታላቁ ምርት ጥምርታ ከ1 ይለያል እና አረጋጋጩ ውድቅ ያደርጋል።  ምክንያቱም
ሁለቱም ወገኖች በGoldilocks መስክ ውስጥ በ `(value + gamma)` ይባዛሉ ፣ ጥምርታ
በሲፒዩ/ጂፒዩ ጀርባዎች ላይ የተረጋጋ ሆኖ ይቆያል።

ምሳሌውን እንደ Norito JSON ለቋሚ ዕቃዎች ተከታታይ ለማድረግ፣ የ tuple ይቅዱ
`perm_hash`፣ መራጭ እና ከእያንዳንዱ ረድፍ በኋላ ሰብሳቢ፣ ለምሳሌ፡-

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

የሄክሳዴሲማል ቦታ ያዥዎች (`0x...`) በኮንክሪት ወርቃማው ሊተኩ ይችላሉ
አውቶማቲክ ሙከራዎችን በሚፈጥሩበት ጊዜ የመስክ አካላት.  ደረጃ 2 ተጨማሪ መገልገያዎች
የሩጫ አሰባሳቢውን የPoseidon hash ይመዝግቡ ነገር ግን ተመሳሳይ የJSON ቅርፅ ያስቀምጡ፣
ስለዚህ ምሳሌው ለወደፊቱ የሙከራ ቬክተሮች እንደ አብነት በእጥፍ ሊጨምር ይችላል።
---
lang: am
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2025-12-29T18:16:35.923121+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# የመሣሪያ ቤተ ሙከራ የአደጋ ጊዜ ምዝግብ ማስታወሻ

ሁሉንም የአንድሮይድ መሳሪያ-ላብራቶሪ ድንገተኛ እቅድ እዚህ ይቅዱ።
ለተገዢነት ግምገማዎች እና ለወደፊት ዝግጁነት ኦዲቶች በቂ ዝርዝር ያካትቱ።

| ቀን | ቀስቅሴ | የተወሰዱ እርምጃዎች | ክትትል | ባለቤት |
|-------|---------|----------
| 2026-02-11 | የPixel8 Pro መስመር መቋረጥ እና የPixel8a አቅርቦት ከዘገየ በኋላ አቅሙ ወደ 78% ወድቋል (`android_strongbox_device_matrix.md` ይመልከቱ)። | የPixel7 ሌይን ወደ ዋናው CI ዒላማ ያደገ፣ የተበደረ የተጋራ Pixel6 መርከቦች፣ የታቀዱ የFirebase Test Lab ጭስ ሙከራዎች ለችርቻሮ ቦርሳ ናሙና እና የተሰማራ ውጫዊ StrongBox ቤተ ሙከራ በAND6። | ለPixel8 Pro የተሳሳተ የዩኤስቢ-ሲ ማእከልን ይተኩ (በ2026-02-15 መጨረሻ)። የPixel8a መምጣት እና የዳግም መስመር አቅም ሪፖርት ያረጋግጡ። | የሃርድዌር ላብ አመራር |
| 2026-02-13 | Pixel8 Pro hub ተተክቷል እና GalaxyS24 ጸድቋል፣ አቅምን ወደ 85% ይመልሳል። | የPixel7 ሌይን ወደ ሁለተኛ ደረጃ ተመልሷል፣ `android-strongbox-attestation` Buildkite ስራ በ `pixel8pro-strongbox-a` እና `s24-strongbox-a` መለያዎች፣ የዘመነ ዝግጁነት ማትሪክስ + የማስረጃ መዝገብ። | Pixel8a መላኪያ ETA ተቆጣጠር (አሁንም በመጠባበቅ ላይ); የትርፍ ሃብ ክምችት በሰነድ ያስቀምጡ። | የሃርድዌር ላብ አመራር |
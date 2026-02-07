---
lang: am
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9be80e0138e1e8aa453c703c53069837b24f29f6b463d14c846a01b015918f24
source_last_modified: "2025-12-29T18:16:35.907815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የህትመት ማረጋገጫ ዝርዝር

የገንቢ መግቢያውን ባዘመኑ ቁጥር ይህንን የማረጋገጫ ዝርዝር ይጠቀሙ። መሆኑን ያረጋግጣል
CI ግንባታ፣ የ GitHub ገፆች መዘርጋት እና በእጅ የሚጨሱ ሙከራዎች እያንዳንዱን ክፍል ይሸፍናሉ።
ከመልቀቂያ ወይም የመንገድ ካርታ ወሳኝ መሬት በፊት።

## 1. የአካባቢ ማረጋገጫ

- `npm run sync-openapi` (Torii OpenAPI ሲቀየር)።
- `npm run build` - የ `Build on Iroha with confidence` ጀግና ቅጂ አሁንም ያረጋግጡ
  በ `build/index.html` ውስጥ ይታያል.
- `cd build && sha256sum -c checksums.sha256` - የቼክ ድምር መግለጫውን ያረጋግጡ
  መገንባት የተፈጠረ.
- በ`npm run start` የነካከውን ማርክ እና በቀጥታ ጫን ላይ ምልክት አድርግ
  አገልጋይ.

## 2. የጥያቄ ቼኮችን ይጎትቱ

- በ `.github/workflows/check-docs.yml` ውስጥ የተሳካውን የ `docs-portal-build` ሥራ ያረጋግጡ።
- `ci/check_docs_portal.sh` ሩጫን ያረጋግጡ (CI ምዝግብ ማስታወሻዎች የጀግናውን ጭስ መፈተሽ ያሳያሉ)።
- የቅድመ እይታ የስራ ፍሰት አንጸባራቂ (`build/checksums.sha256`) መሰቀሉን ያረጋግጡ እና
  `sha256sum -c` በ CI ውስጥ ያለፈው.
- የታተመውን ቅድመ እይታ URL ከ GitHub ገጾች አካባቢ ወደ PR ያክሉ
  መግለጫ.

## 3. ክፍል ማቋረጥ

| ክፍል | ባለቤት | የማረጋገጫ ዝርዝር |
|--------|-------|-------|
| መነሻ ገጽ | DevRel | የጀግና ቅጂዎች፣ የፈጣን ጅምር ካርዶች ወደ ትክክለኛ መንገዶች ይገናኛሉ፣ የሲቲኤ አዝራሮች ይፈታሉ። |
| Norito | Norito WG | አጠቃላይ እይታ እና ጅምር መመሪያዎች የቅርብ ጊዜውን የCLI ባንዲራዎች እና Norito schema ሰነዶችን ይጠቅሳሉ። |
| SoraFS | የማከማቻ ቡድን | Quickstart ለማጠናቀቅ ይሰራል፣ የሰነድ የሰነድ ማስረጃዎች ናቸው፣ የማስመሰል መመሪያዎች ተረጋግጠዋል። |
| የኤስዲኬ መመሪያዎች | ኤስዲኬ ይመራል | Rust/Python/JS መመሪያዎች የአሁኑን ምሳሌዎች ያጠናቅራሉ እና ከቀጥታ ሪፖስ ጋር ይገናኛሉ። |
| ዋቢ | ሰነዶች/DevRel | ኢንዴክስ አዲሱን ዝርዝሮች ይዘረዝራል፣ Norito የኮዴክ ማመሳከሪያ ከ`norito.md` ጋር ይዛመዳል። |
| ቅድመ ዕይታ ቅርስ | ሰነዶች/DevRel | `docs-portal-preview` ቅርስ ከ PR ጋር ተያይዟል፣ የጭስ ቼኮች ማለፊያ፣ ከገምጋሚዎች ጋር የተጋራ አገናኝ። |

እያንዳንዱን ረድፍ እንደ የእርስዎ PR ግምገማ አካል ምልክት ያድርጉ ወይም ማንኛውንም የክትትል ተግባራት ሁኔታን ያስታውሱ
መከታተል ትክክለኛ ሆኖ ይቆያል።

## 4. የመልቀቂያ ማስታወሻዎች

- `https://docs.iroha.tech/` (ወይም የአካባቢ ዩአርኤልን) ያካትቱ
  ከማሰማራት ሥራ) በመልቀቂያ ማስታወሻዎች እና በሁኔታ ዝመናዎች ውስጥ።
- የታችኞቹ ቡድኖች የት እንዳሉ እንዲያውቁ ማንኛውንም አዲስ ወይም የተቀየሩ ክፍሎችን በግልፅ ይደውሉ
  የራሳቸውን የጭስ ሙከራዎች እንደገና ለማካሄድ.
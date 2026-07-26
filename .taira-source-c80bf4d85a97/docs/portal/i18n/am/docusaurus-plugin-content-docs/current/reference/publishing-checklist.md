---
lang: am
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# የህትመት ማረጋገጫ ዝርዝር

የገንቢ መግቢያውን ባዘመኑ ቁጥር ይህንን የማረጋገጫ ዝርዝር ይጠቀሙ። መሆኑን ያረጋግጣል
CI ግንባታ፣ የ GitHub ገፆች መዘርጋት እና በእጅ የሚጨሱ ሙከራዎች እያንዳንዱን ክፍል ይሸፍናሉ።
ከመልቀቂያ ወይም የመንገድ ካርታ ወሳኝ መሬት በፊት።

## 1. የአካባቢ ማረጋገጫ

- `npm run sync-openapi -- --version=current --latest` (አንድ ወይም ከዚያ በላይ ይጨምሩ
  `--mirror=<label>` ባንዲራዎች I18NT0000007X OpenAPI ለቀዘቀዘ ቅጽበታዊ እይታ ሲቀየር)።
- `npm run build` - የ `Build on Iroha with confidence` ጀግና ቅጂ አሁንም ያረጋግጡ
  በ `build/index.html` ውስጥ ይታያል.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - ያረጋግጡ
  checksum መግለጫ (የወረደውን CI ሲሞክር `--descriptor`/`--archive` ይጨምሩ
  ቅርሶች)።
- `npm run serve` - የሚያረጋግጥ የቼክ-ጋድ ቅድመ እይታ አጋዥን ይጀምራል
  መግለጫው ወደ `docusaurus serve` ከመደወል በፊት ስለዚህ ገምጋሚዎች በጭራሽ አያስሱም።
  ያልተፈረመ ቅጽበታዊ ገጽ እይታ (የ`serve:verified` ተለዋጭ ስም ለግልጽ ጥሪዎች ይቀራል)።
- በ`npm run start` የነካከውን ምልክት ማድረጊያ እና ቀጥታውን እንደገና መጫን
  አገልጋይ.

## 2. የጥያቄ ቼኮችን ይጎትቱ

- በ `.github/workflows/check-docs.yml` ውስጥ የ I18NI0000021X ሥራ እንደተሳካ ያረጋግጡ።
- I18NI0000023X ሩጫን ያረጋግጡ (CI ምዝግብ ማስታወሻዎች የጀግናውን ጭስ መፈተሽ ያሳያሉ)።
- የቅድመ እይታ የስራ ፍሰት አንጸባራቂ (`build/checksums.sha256`) መሰቀሉን ያረጋግጡ እና
  የቅድመ እይታ ማረጋገጫ ስክሪፕት ተሳክቷል (CI ምዝግብ ማስታወሻዎች የ
  I18NI0000025X ውፅዓት)።
- የታተመውን ቅድመ እይታ URL ከ GitHub ገጾች አካባቢ ወደ PR ያክሉ
  መግለጫ.

## 3. ክፍል ማቋረጥ

| ክፍል | ባለቤት | የማረጋገጫ ዝርዝር |
|--------|-------|-------|
| መነሻ ገጽ | DevRel | የጀግና ቅጂዎች፣ የፈጣን ጅምር ካርዶች ወደ ትክክለኛ መንገዶች ይገናኛሉ፣ የሲቲኤ አዝራሮች ይፈታሉ። |
| Norito | Norito WG | አጠቃላይ እይታ እና ጅምር መመሪያዎች የቅርብ ጊዜውን የCLI ባንዲራዎች እና I18NT0000003X schema ሰነዶችን ይጠቅሳሉ። |
| SoraFS | የማከማቻ ቡድን | Quickstart ለማጠናቀቅ ይሰራል፣ የሰነድ የሰነድ ማስረጃዎች ናቸው፣ የማስመሰል መመሪያዎች ተረጋግጠዋል። |
| የኤስዲኬ መመሪያዎች | ኤስዲኬ ይመራል | Rust/Python/JS መመሪያዎች የአሁኑን ምሳሌዎች ያጠናቅራሉ እና ከቀጥታ ሪፖስ ጋር ይገናኛሉ። |
| ዋቢ | ሰነዶች/DevRel | ማውጫ አዲሱን ዝርዝሮች ይዘረዝራል፣ Norito የኮዴክ ማመሳከሪያ ከ`norito.md` ጋር ይዛመዳል። |
| ቅድመ ዕይታ ቅርስ | ሰነዶች/DevRel | `docs-portal-preview` ቅርስ ከ PR ጋር ተያይዟል፣ የጭስ ቼኮች ማለፊያ፣ አገናኝ ከገምጋሚዎች ጋር ተጋርቷል። |
| ደህንነት እና ይሞክሩት ማጠሪያ | ሰነዶች/DevRel · ደህንነት | OAuth መሣሪያ-የመግቢያ ኮድ ተዋቅሯል (`DOCS_OAUTH_*`)፣ `security-hardening.md` ማረጋገጫ ዝርዝር ተፈፅሟል፣ CSP/የታመኑ ዓይነቶች ራስጌዎች በ`npm run build` ወይም I18NI0000031X ተረጋግጠዋል። |

እያንዳንዱን ረድፍ እንደ የእርስዎ PR ግምገማ አካል ምልክት ያድርጉ ወይም ማንኛውንም የክትትል ተግባራት ሁኔታን ያስታውሱ
መከታተል ትክክለኛ ሆኖ ይቆያል።

## 4. የመልቀቂያ ማስታወሻዎች

- `https://docs.iroha.tech/` (ወይም የአካባቢ ዩአርኤልን) ያካትቱ
  ከማሰማራት ሥራ) በመልቀቂያ ማስታወሻዎች እና በሁኔታ ዝመናዎች ውስጥ።
- የታችኞቹ ቡድኖች የት እንዳሉ እንዲያውቁ ማንኛውንም አዲስ ወይም የተቀየሩ ክፍሎችን በግልፅ ይደውሉ
  የራሳቸውን የጭስ ሙከራዎች እንደገና ለማካሄድ.
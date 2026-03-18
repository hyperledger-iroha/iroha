---
lang: am
direction: ltr
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-11T04:52:11.136647+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የስህተት የካርታ መመሪያ

ለመጨረሻ ጊዜ የዘመነው፡ 2025-08-21

ይህ መመሪያ በ Iroha ውስጥ በመረጃ ሞዴሉ ብቅ ያሉ የተረጋጉ የስህተት ምድቦችን የጋራ ውድቀት ሁነታዎችን ያዘጋጃል። ፈተናዎችን ለመንደፍ እና የደንበኛ ስህተት አያያዝ ሊተነበይ የሚችል ለማድረግ ይጠቀሙበት።

መርሆዎች
- የመመሪያ እና የመጠይቅ መንገዶች የተዋቀሩ ቁጥሮችን ያስወጣሉ። ድንጋጤን ያስወግዱ; በተቻለ መጠን አንድ የተወሰነ ምድብ ሪፖርት ያድርጉ።
- ምድቦች የተረጋጉ ናቸው, መልዕክቶች ሊሻሻሉ ይችላሉ. ደንበኞች በነጻ ቅፅ ሕብረቁምፊዎች ላይ ሳይሆን በምድብ ላይ መመሳሰል አለባቸው።

ምድቦች
- InstructionExecutionError :: አግኝ: የጠፋ አካል (ንብረት, መለያ, ጎራ, NFT, ሚና, ቀስቅሴ, ፈቃድ, የህዝብ ቁልፍ, እገዳ, ግብይት). ምሳሌ፡- የሌለ የሜታዳታ ቁልፍን በማስወገድ አግኝ (ሜታዳታ ቁልፍ)።
- InstructionExecutionError:: መደጋገም: የተባዛ ምዝገባ ወይም የሚጋጭ መታወቂያ። የመመሪያውን አይነት እና ተደጋጋሚውን IdBox ይዟል።
- InstructionExecutionError:: Mintability: Mintability invariant ተጥሷል (`Once` ሁለት ጊዜ ተዳክሟል፣ `Limited(n)` ከልክ በላይ ተሳጥቷል፣ ወይም `Infinitely`ን ለማሰናከል ሙከራዎች)። ምሳሌዎች፡- እንደ `Once` ሁለት ጊዜ የተገለጸውን ንብረት መፍጠር `Mintability(MintUnmintable)`; `Limited(0)` ማዋቀር `Mintability(InvalidMintabilityTokens)` ያስገኛል.
- InstructionExecutionError::ሒሳብ: የቁጥር ጎራ ስህተቶች (ትርፍ፣ በዜሮ መከፋፈል፣ አሉታዊ እሴት፣ በቂ ያልሆነ መጠን)። ምሳሌ፡ ካለው መጠን በላይ ማቃጠል ሒሳብ (NotEnoughQuantity) ያስገኛል።
- InstructionExecutionError:: ልክ ያልሆነ ፓራሜትር፡ ልክ ያልሆነ የትምህርት መለኪያ ወይም ውቅር (ለምሳሌ፡ ያለፈው ጊዜ ቀስቅሴ)። ለተበላሸ የኮንትራት ጭነት ተጠቀም።
- InstructionExecutionError:: ይገምግሙ: DSL/የመመሪያ ቅርጽ ወይም ዓይነቶች ጋር አለመዛመድ. ምሳሌ፡ የተሳሳተ የቁጥር ዝርዝር ለንብረት እሴት ያስገኛል ይገምግሙ(አይነት(AssetNumericSpec(..))))።
- InstructionExecutionError::ኢንቫሪንት ጥሰት: በሌሎች ምድቦች ውስጥ ሊገለጽ የማይችል የሥርዓት የማይለዋወጥ ጥሰት. ምሳሌ፡ የመጨረሻውን ፈራሚ ለማስወገድ መሞከር።
- InstructionExecutionError::ጥያቄ፡- በመመሪያ አፈጻጸም ወቅት መጠይቁ ሳይሳካ ሲቀር የQueryExecutionFail መጠቅለል።

QueryExecutionFail
- አግኝ፡ በመጠይቅ አውድ ውስጥ የጎደለ አካል።
ልወጣ፡- በጥያቄ የሚጠበቀው የተሳሳተ ዓይነት።
- አልተገኘም፡ የቀጥታ መጠይቅ ጠቋሚ ጠፍቷል።
- CursorMismatch / CursorDone: የጠቋሚ ፕሮቶኮል ስህተቶች።
- FetchSizeTooBig፡ በአገልጋይ የተገደበ ገደብ ታልፏል።
- GasBudget ታልፏል፡ የጥያቄ አፈጻጸም ከጋዝ/ቁሳቁስ በጀት አልፏል።
- ልክ ያልሆኑ ነጠላ መለኪያዎች፡ ለነጠላ መጠይቆች የማይደገፉ መለኪያዎች።
- የአቅም ገደብ፡ የቀጥታ መጠይቅ ማከማቻ አቅም ላይ ደርሷል።

የሙከራ ምክሮች
- ለስህተት መነሻ ቅርብ የሆኑ የክፍል ሙከራዎችን ይምረጡ። ለምሳሌ፣ የንብረት ቁጥራዊ ዝርዝር አለመመጣጠን በውሂብ-ሞዴል ሙከራዎች ውስጥ ሊፈጠር ይችላል።
- የውህደት ሙከራዎች ለወኪል ጉዳዮች ከመጨረሻ እስከ መጨረሻ ካርታ ስራን መሸፈን አለባቸው (ለምሳሌ፡ የተባዛ መዝገብ፣ የማስወገድ ቁልፍ ጠፍቷል፣ ያለ ባለቤትነት ማስተላለፍ)።
- ከመልእክት ንዑስ ሕብረቁምፊዎች ይልቅ የኢነም ተለዋጮችን በማዛመድ ፅንሰ-ሀሳቦችን ጠንካራ ይሁኑ።
---
lang: am
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 465d8cf704022986b169ab93133517428f8cf2ffe01a498cbda458f4a5b2e69b
source_last_modified: "2026-07-11"
translation_last_reviewed: 2026-07-11
translator: machine-assisted
---

> ይህ ገጽ አጭር የተተረጎመ ማጠቃለያ እንጂ ሙሉ ትርጉም አይደለም።
> ለትክክለኛው የደንብ፣ API፣ የማስረጃ እና የልቀት መስፈርት
> [የእንግሊዝኛውን ዋና ገጽ](bridge_proofs.md) ይጠቀሙ።

# SCCP V1 የድልድይ ማስረጃዎች — አጭር ማጠቃለያ

## የመጀመሪያው ልቀት ወሰን

SCCP V1 የተዘጋ የመጀመሪያ-ልቀት ፕሮቶኮል ነው። የተደገፉት ውጫዊ
ምንጮች `ethereum-mainnet`፣ `bsc-mainnet` እና `tron-mainnet` ብቻ ሲሆኑ
ብቸኛው የSORA መዳረሻ `sora-taira` ነው። Solana፣ TON፣ ብጁ ኔትወርኮች
ወይም ሌላ የSORA መዳረሻ አይደገፉም እና ዝግ ሆነው ውድቅ ይደረጋሉ።

`SubmitBridgeProof` በአሁኑ ልቀት የተተየቡትን `NativeProtocol` እና
`SccpDestination` ማስረጃዎች ብቻ ይቀበላል። አጠቃላይ `Ics` እና
`TransparentZk` ማቅረብ የሚፈቀድ አይደለም፤ በሰንሰለቱ ላይ ሥልጣን
ያለው ማረጋገጫ እስኪኖር ድረስ ውድቅ ይደረጋል።

## የተተየበ መዝገብ እና የመልሶ-ማጫወት ጥበቃ

`SccpRegistryV1` በlane የተያዘ፣ የተተየበ እና መዝገቦች ብቻ የሚጨመሩበት
(append-only) መዝገብ ነው። እያንዳንዱ lane ቢበዛ 64 የተቀመጡ route
revision-ዎችን እና 4,096 የተቀመጡ native trust anchor-ዎችን ይይዛል።
መዝገቦች በስውር አይወገዱም፤ ገደቡን የሚያልፍ ቀጣይ ለውጥ ምንም
ሁኔታ ሳይቀይር ውድቅ ይደረጋል።

የanchor ክፍተት በተረጋገጠ የconsensus እድገት ይለካል፤ Ethereum
የተጠናቀቀውን beacon slot፣ BSC እና TRON ደግሞ የተጠናቀቀውን native
block height ይጠቀማሉ። አሮጌ anchor እስከ ተተኪው checkpoint ድረስ
ያንን checkpoint ጨምሮ ትክክለኛ ነው፤ የመጨረሻው anchor ክፍት-መጨረሻ
አለው። የተቋረጠ route የfinality cutoff ከታሪካዊ anchor ተተኪ
checkpoint ጋር በትክክል መዛመድ አለበት።

ቋሚው የinbound መዝገብ የevent/source finality height እና የተረጋገጠውን
`anchor_interval_height` ሁለቱንም ይይዛል። በlane እና በanchor hash የተያዘ
high-water ኢንዴክስ ቀደም ሲል ከተቀበለ ከፍታ በታች ያለ ተተኪ
checkpoint እንዳይመረጥ ያደርጋል። Snapshot hydration ኢንዴክሱን ከቋሚ
መዝገቦች እንደገና አስልቶ ትክክለኛ እኩልነትን ይጠይቃል፤ የጠፋ፣
ያረጀ፣ የተበላሸ ወይም መሠረት የሌለው ኢንዴክስ ውድቅ ይደረጋል።

## አንድ-ጊዜ ማረጋገጥ እና የሥራ ገደቦች

የdestination እና native ማስረጃዎች አንድ ጊዜ ይፈታሉ፣ አንድ ጊዜ
ይታሰራሉ እና የውድ ክሪፕቶግራፊ ሥራ ከመጀመሩ በፊት የdeterministic
ሥራ መጠን ይያዛል። Destination መንገዱ BN254 pairing-product እና የአካባቢ
BLS finality እያንዳንዱን አንድ ጊዜ ብቻ ያረጋግጣል። Native መንገዶች
canonical shortest-prefix ይጠይቃሉ፤ BSC ቢበዛ 1,004 headers፣ TRON
ቢበዛ 54 headers ይፈቅዳሉ።

`[zk.sccp]` የማስረጃ ቁጥር/bytes፣ native headers/bytes፣ Ethereum
light-client updates፣ secp256k1 recoveries፣ BLS aggregate checks/key
contributions እና BN254 pairing checks ላይ ከዜሮ በላይ የtransaction እና
block ገደቦችን ያስገድዳል። እነዚህ የተቀባይነት ገደቦች
consensus-bound ናቸው፤ በሁሉም validators ውስጥ ከconfig file ተመሳሳይ
መሆን አለባቸው እና environment-variable override የላቸውም።

የመጀመሪያ ልቀት ነባሪ ገደቦች እነዚህ ናቸው፦

| የሥራ መለኪያ | Transaction | Block |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1,004 | 4,016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1,005 | 4,020 |
| BLS aggregate checks | 1,004 | 4,016 |
| BLS key/contribution work items | 131,713 | 526,852 |
| BN254 pairing-product checks | 1 | 4 |

አንድ proof ቢበዛ 8 MiB canonical bytes ሊይዝ ይችላል። የተቋረጠ ወይም
ውድቅ የተደረገ transaction የተዘጋጀውን ሥራ ወደ block አያፈስስም።

## Torii እና HTTP ገደቦች

Torii ለእያንዳንዱ SCCP endpoint የተለየ የJSON body ገደብ ከbody ንባብ፣
allocation ወይም cryptographic verification በፊት ያስገድዳል። በጣም ትልቅ
`Content-Length` ወይም chunked body በHTTP `413` ውድቅ ይደረጋል። ደንበኞች
የተፈታውን HTTP response በተወሰነ ገደብ ውስጥ ብቻ ያነባሉ፣ ስለዚህ
የጎደለ ወይም የሐሰት `Content-Length` ገደቡን ማለፍ አይችልም።

ሁሉም JSON፣ base64 እና Norito ግብዓቶች canonical መሆን አለባቸው።
ያልታወቁ fields፣ የተባዙ keys፣ የተሳሳተ route/anchor/network፣ replay፣
የሥራ ገደብ ማለፍ ወይም ማረጋገጫ አለመሳካት ሁኔታን ሳይቀይር
ውድቅ ይደረጋል።

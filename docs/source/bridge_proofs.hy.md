---
lang: hy
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 69c9a740261d0c367d52870fc1f48775ae48307056ba9b79d2f811e0c0849f20
source_last_modified: "2026-07-11"
translation_last_reviewed: 2026-07-11
translator: machine-assisted
---

> Այս էջը թարգմանված համառոտագիր է, ոչ թե ամբողջական թարգմանություն։
> Կառավարման, API-ների, ապացույցների իմաստաբանության և թողարկման պահանջների
> ճշգրիտ նորմատիվ աղբյուրը [անգլերեն կանոնական էջն է](bridge_proofs.md)։

# SCCP V1 կամրջի ապացույցներ — համառոտագիր

## Առաջին թողարկման շրջանակը

SCCP V1-ը առաջին թողարկման փակ արձանագրություն է։ Աջակցվող միակ արտաքին
աղբյուրներն են `ethereum-mainnet`, `bsc-mainnet` և `tron-mainnet`, իսկ SORA-ի
միակ նպատակակետը `sora-taira`-ն է։ Solana-ն, TON-ը, հատուկ ցանցերը և SORA-ի
այլ նպատակակետերը չեն աջակցվում և անվտանգ կերպով մերժվում են։

Այս թողարկման մեջ `SubmitBridgeProof`-ն ընդունում է միայն տիպավորված
`NativeProtocol` և `SccpDestination` ապացույցները։ Ընդհանուր `Ics` կամ
`TransparentZk` ներկայացումը հասանելի չէ և մերժվում է, մինչև շղթայում
հեղինակավոր ստուգիչ չլինի։

## Տիպավորված գրանցամատյան և replay-ի պաշտպանություն

`SccpRegistryV1`-ը lane-ին կապված, տիպավորված և միայն հավելվող (append-only)
գրանցամատյան է։ Յուրաքանչյուր lane պահպանում է առավելագույնը 64 route revision
և 4,096 native trust anchor։ Պատմական գրառումները լռելյայն չեն հեռացվում․
սահմանին հասնելուց հետո հաջորդ հավելումը ատոմապես մերժվում է՝ առանց վիճակի
փոփոխության։

Anchor interval-ները չափվում են վավերացված consensus առաջընթացի կոորդինատով․
Ethereum-ը կիրառում է finalized beacon slot-ը, իսկ BSC-ն և TRON-ը՝ finalized
native block height-ը։ Հին anchor-ը վավեր է մինչև հաջորդ checkpoint-ը՝ ներառյալ
սահմանային կետը, իսկ վերջին ընթացիկ anchor-ը բաց վերջ ունի։ Ավարտված route-ի
finality cutoff-ը պետք է ճշգրտորեն հավասար լինի պատմական anchor-ի հաջորդ
checkpoint-ին։

Մշտական inbound գրառումն առանձին պահպանում է event/source finality height-ը և
ստուգված `anchor_interval_height`-ը։ Lane-ով և anchor hash-ով բանալված մշտական
high-water ինդեքսը թույլ չի տալիս կառավարմանը ընտրել արդեն ընդունված
կոորդինատից ցածր հաջորդ checkpoint։ Snapshot hydration-ը ինդեքսը վերահաշվում
է մշտական գրառումներից և պահանջում ճշգրիտ հավասարություն՝ մերժելով բացակայող,
հնացած, սխալ կամ չհիմնավորված ինդեքսը։ Օգտագործված message id-ները նույնպես
մշտապես պահվում են replay-ը կանխելու համար։

## Մեկ անցումով ստուգում և աշխատանքի սահմաններ

Destination և native ապացույցները կառուցվածքավորվում են մեկ անգամ, կապվում են
մեկ անգամ, և թանկ կրիպտոգրաֆիայից առաջ պահուստավորվում է դետերմինիստական
աշխատանքը։ Destination ուղին մեկ անգամ ստուգում է BN254 pairing-product-ը և
մեկ անգամ՝ տեղային BLS finality-ն։ Native ուղիները պահանջում են canonical
shortest-prefix. առավելագույնը 1,004 header BSC-ի և 54՝ TRON-ի համար։

`[zk.sccp]`-ն սահմանում է ոչ զրոյական transaction և block սահմաններ proof-երի
քանակի/bytes-ի, native headers/bytes-ի, Ethereum light-client updates-ի,
secp256k1 recoveries-ի, BLS aggregate checks/key contributions-ի և BN254
pairing checks-ի համար։ Ընդունման այս սահմանները consensus-bound են․ բոլոր
validator-ները պետք է օգտագործեն config file-ի նույն արժեքները, և environment
variable override գոյություն չունի։

Առաջին թողարկման լռելյայն սահմաններն են․

| Աշխատանքի չափում | Transaction | Block |
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

Մեկ proof-ը կարող է պարունակել առավելագույնը 8 MiB canonical bytes։ Լքված կամ
մերժված transaction-ի համար պահուստավորված աշխատանքը block չի անցնում։

## Torii և HTTP սահմաններ

Torii-ն յուրաքանչյուր SCCP endpoint-ի համար կիրառում է առանձին JSON body
սահման՝ նախքան body-ն կարդալը, հիշողություն հատկացնելը կամ կրիպտոգրաֆիկ
ստուգումը։ Չափազանց մեծ `Content-Length` կամ chunked body-ն մերժվում է HTTP
`413`-ով։ Հաճախորդը նաև decoded HTTP response-ը կարդում է հաստատուն սահմանի
ներքո, ուստի բացակայող կամ կեղծ `Content-Length`-ը չի կարող շրջանցել այն։

Բոլոր JSON, base64 և Norito մուտքերը պետք է canonical լինեն։ Անհայտ fields-ը,
կրկնվող keys-ը, սխալ network/route/anchor-ը, replay-ը, աշխատանքի քվոտայի
գերազանցումը կամ ստուգման ձախողումը մերժվում են՝ առանց վիճակի մասնակի փոփոխության։

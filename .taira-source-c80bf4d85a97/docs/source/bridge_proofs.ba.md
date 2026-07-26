---
lang: ba
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> Был бит — ҡыҫҡартылған тәржемә йомғағы, тулы тәржемә түгел. Идара итеү,
> API, иҫбатлау мәғәнәһе һәм релиз талаптары өсөн аныҡ норматив сығанаҡ —
> [инглизсә төп бит](bridge_proofs.md).

# SCCP V1 күпер иҫбатлауҙары — ҡыҫҡаса йомғаҡ

## Беренсе релиз сиктәре

SCCP V1 — беренсе релиз өсөн ябыҡ протокол. Тышҡы сығанаҡтарҙан тик
`ethereum-mainnet`, `bsc-mainnet` һәм `tron-mainnet` ҡына хуплана; берҙән-бер
SORA тәғәйенләнеше — `sora-taira`. Solana, TON, махсус селтәрҙәр һәм башҡа SORA
тәғәйенләнештәре хупланмай һәм хәүефһеҙ рәүештә кире ҡағыла.

Был релизда `SubmitBridgeProof` тик типланған `NativeProtocol` һәм
`SccpDestination` иҫбатлауҙарын ҡабул итә. Дөйөм `Ics` йәки `TransparentZk`
тапшырыу мөмкин түгел һәм абруйлы on-chain verifier барлыҡҡа килгәнсе кире
ҡағыла.

## Типланған реестр һәм replay-ҙан һаҡлау

`SccpRegistryV1` lane-ға бәйләнгән, типланған һәм тик өҫтәлә торған (append-only)
реестр. Һәр lane иң күбе 64 һаҡланған route revision һәм 4,096 һаҡланған native
trust anchor тота. Тарихи яҙмалар йәшерен рәүештә юйылмай; сиккә еткәс, киләһе
өҫтәү хәлде үҙгәртмәйенсә атомар кире ҡағыла.

Anchor интервалы раҫланған consensus үҫеш координатаһы менән үлсәнә: Ethereum
finalized beacon slot ҡуллана, BSC һәм TRON finalized native block height
ҡуллана. Иҫке anchor алмашсының checkpoint-ы ингән сиккә тиклем ғәмәлдә ҡала;
һуңғы ағымдағы anchor-ҙың осо асыҡ. Тамамланған route-тың finality cutoff-ы
тарихи anchor-ҙың алмашсы checkpoint-ына теүәл тиң булырға тейеш.

Даими inbound яҙма event/source finality height менән раҫланған
`anchor_interval_height`-ты айырым һаҡлай. Lane һәм anchor hash буйынса асҡыслы
high-water индексы элек ҡабул ителгән координатанан түбән алмашсы checkpoint
һайларға бирмәй. Snapshot hydration индексты даими яҙмаларҙан яңынан иҫәпләй һәм
теүәл тигеҙлек талап итә; юғалған, иҫкергән, боҙолған йәки нигеҙһеҙ индекс кире
ҡағыла. Ҡулланылған message id-ҙар replay-ҙы туҡтатыу өсөн даими һаҡлана.

TRON сығанаҡ route-ы теүәл
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI-һын ҡуллана. Уңышлы
башҡарыу өсөн `expectedNonce == transferNonce` булырға тейеш; шунан storage
арттырылғанға тиклем шул уҡ ҡиммәт canonical payload-ҡа яҙыла. Native admission
тулы ABI саҡырыуын payload recipient-ы, масштабланған сумма һәм nonce буйынса
яңынан төҙөй; шуға күрә иҫке ике-argument selector, stale йәки future nonce һәм
сигенә еткән `uint64` nonce хәүефһеҙ рәүештә кире ҡағыла.

## Бер үтеүле тикшереү һәм эш сиктәре

Destination һәм native иҫбатлауҙары бер тапҡыр структуралана, бер тапҡыр
бәйләнә, ә ҡиммәтле криптографияға тиклем детерминистик эш резервы алына.
Destination юлы BN254 pairing-product менән урындағы BLS finality-ҙы һәр береһен
тик бер тапҡыр тикшерә. Native юлдар canonical shortest-prefix талап итә: BSC
өсөн иң күбе 1,004 header, TRON өсөн 54 header.

`[zk.sccp]` proof һаны/bytes, native headers/bytes, Ethereum light-client
updates, secp256k1 recoveries, BLS aggregate checks/key contributions һәм BN254
pairing checks өсөн нулдән ҙур transaction һәм block лимиттары ҡуя. Был ҡабул
итеү лимиттары consensus-bound; бөтә validator-ҙарҙа config file ҡиммәттәре бер
иш булырға тейеш һәм environment-variable алмаштырыуы юҡ.

Беренсе релиздың ғәҙәти лимиттары:

| Эш үлсәме | Transaction | Block |
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

Бер proof иң күбе 8 MiB canonical bytes тота ала. Туҡтатылған йәки кире ҡағылған
transaction өсөн резервланған эш block эсенә үтмәй.

## Outbound йөкләмәһе, һаҡлау һәм табыу

Һәр уңышлы outbound message block execution order буйынса тығыҙ
`commitment_index` (`0..=511`) ала. V1 өсөн үҙгәрмәҫ сиктәр: бер block-та 512
message һәм бер message-та 4,096 canonical payload byte. `[zk.sccp]` pending
payload state-ты `max_pending_outbound_messages` (default `65536`) һәм
`max_pending_outbound_payload_bytes` (default `268435456`) менән бергә сикләй.

Kura finality баҫтырылғанға йәки block body сығарылғанға тиклем теүәл canonical
header һәм root-authenticated SCCP archive-ты immutable һаҡлай. Proof, bundle,
proof request һәм recent history өсөн тарихи block body йәки mutable WSV payload
күсермәһе кәрәкмәй. Destination proof ҡабул ителһә, pending payload һәм уның
charge-ы atomically юйыла, ә fixed terminal descriptor locator/index менән ҡала.
Pending state сикләнгән; terminal records һәм immutable Kura history даими replay
һағы өсөн аңлы рәүештә үҫә. `GET /v1/sccp/messages/recent` составлы
`{ from, after_index }` cursor ҡуллана. Immutable evidence total/operator disk
usage эсенә инә, әммә evictable-body budget-тан сығарыла.

## Torii һәм HTTP сиктәре

Torii һәр SCCP endpoint өсөн JSON body лимитын body уҡылғанға, хәтер бүленгәнгә
йәки криптографик тикшереү башланғанға тиклем ҡуллана. Артыҡ ҙур
`Content-Length` йәки chunked body HTTP `413` менән кире ҡағыла. Клиент асылған
HTTP яуабын да сикләнгән күләмдә генә уҡый; юҡ йәки ялған `Content-Length`
лимитты уҙа алмай.

JSON, base64 һәм Norito индереүҙәре canonical булырға тейеш. Билдәһеҙ fields,
ҡабатланған keys, тап килмәгән network/route/anchor, replay, эш квотаһын арттырыу
йәки тикшереү хатаһы хәлде өлөшләтә үҙгәртмәйенсә кире ҡағыла.

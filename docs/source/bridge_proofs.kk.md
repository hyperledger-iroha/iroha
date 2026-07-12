---
lang: kk
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> Бұл — 2026-07-11 күнгі қысқартылған жергіліктендірілген шолу, толық
> нормативтік аударма емес. Нақты типтер, API келісімдері және релиз талаптары
> үшін [ағылшын тіліндегі канондық бетті](bridge_proofs.md) пайдаланыңыз.

# SCCP V1 көпір дәлелдері — қысқаша шолу

## Бірінші релиз шекарасы

- SCCP V1 — жабық бет: тек Ethereum mainnet, BSC mainnet және TRON mainnet
  қолданылады; SORA жағындағы жалғыз нүкте — `sora-taira`. Кез келген өзге
  желі профилі немесе SORA идентификаторы қабылданбайды.
- `SubmitBridgeProof` тек маршрутпен байланыстырылған типтелген
  `NativeProtocol` және `SccpDestination` дәлелдерін қабылдайды. Жалпы `Ics`
  және `TransparentZk` payload жіберу қолжетімсіз және fail-closed тәртібімен
  кері қайтарылады.

## Типтелген реестр және тарих

- `SccpRegistryV1` типтелген әрі append-only. Әр lane ең көбі 64 маршрут
  ревизиясын және 4 096 native trust anchor сақтайды. Жазбалар жасырын
  шығарылмайды; шектен кейінгі қосу атомарлық түрде қабылданбайды.
- Anchor аралығы расталған консенсус координатын қолданады: Ethereum үшін
  finalized beacon slot, BSC/TRON үшін finalized native block height. Ескі
  anchor келесі checkpoint-ті қоса алғанда жарамды, одан кейін жарамсыз.
- Тұрақты inbound жазбасы event/finality height пен `anchor_interval_height`
  мәндерін бөлек сақтайды. lane+anchor high-water тек өседі; келесі checkpoint
  одан төмен бола алмайды. Snapshot hydration индексті толық қайта есептеп,
  жетіспейтін, ескірген немесе артық мәнді қабылдамайды. Message id қайталау
  мен replay де кері қайтарылады.

TRON бастапқы route-ы дәл
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI-сын қолданады. Орындау
сәтті болуы үшін `expectedNonce == transferNonce` болуы керек; одан кейін storage
ұлғайтылмай тұрып сол мән canonical payload-қа жазылады. Native admission толық
ABI call-ды payload recipient-і, масштабталған сома және nonce арқылы қайта құрады.
Сондықтан ескірген екі-argument selector, stale немесе future nonce және шегіне
жеткен `uint64` nonce қауіпсіз түрде қабылданбайды.

## Бір рет тексеру және детерминдік лимиттер

- Native және destination дәлелдері канондық түрде бір рет декодталып, қымбат
  криптографиялық тексеру бір рет қана орындалады. Оған дейін консенсус
  консервативті, hardware-independent жұмыс бағасын резервтейді.
- `[zk.sccp]` proof саны/байты, native headers, Ethereum light-client updates,
  header bytes, secp256k1 recoveries, BLS aggregate checks/signing contributions
  және BN254 pairing-product checks үшін міндетті нөлден үлкен per-proof,
  per-transaction және per-block шектерін қояды. Бұл қабылдау шектері
  консенсусқа байланыстырылған және барлық валидаторда бірдей болуы тиіс.

## Outbound commitment, сақтау және табу

Әрбір сәтті outbound message block execution order бойынша тығыз
`commitment_index` (`0..=511`) алады. V1 тұрақты шектері: бір block-та 512 message
және бір message-та 4,096 canonical payload byte. `[zk.sccp]` pending payload
state-ті `max_pending_outbound_messages` (default `65536`) және
`max_pending_outbound_payload_bytes` (default `268435456`) арқылы бірге шектейді.

Kura finality жарияланғанға немесе block body шығарылғанға дейін нақты canonical
header мен root-authenticated SCCP archive-ті immutable түрде сақтайды. Proof,
bundle, proof request және recent history тарихи block body немесе mutable WSV
payload көшірмесін оқымайды. Destination proof қабылданғанда pending payload пен
оның charge-ы atomically жойылып, locator/index сақталған fixed terminal descriptor
қалады. Pending state шектеулі; terminal records пен immutable Kura history тұрақты
replay protection үшін әдейі өседі. `GET /v1/sccp/messages/recent` құрама
`{ from, after_index }` cursor қолданады. Immutable evidence total/operator disk
usage-қа кіреді, бірақ evictable-body budget-тен шығарылады.

## Torii шектері

`/v1/bridge/proofs/submit` және `/v1/bridge/messages` endpoint-specific HTTP
body шектерін қолданады. Аутентификация, rate limit және `Content-Length` body
оқылмай тұрып тексеріледі; chunked body тек қатаң шекке дейін оқылады. Өлшемі
артық сұрау `413`, malformed transport/JSON бөлек `400` қайтарады. Detached
transaction payload шегі — 16 MiB, signature payload шегі — 16 KiB.

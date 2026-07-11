---
lang: kk
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 69c9a740261d0c367d52870fc1f48775ae48307056ba9b79d2f811e0c0849f20
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
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

## Бір рет тексеру және детерминдік лимиттер

- Native және destination дәлелдері канондық түрде бір рет декодталып, қымбат
  криптографиялық тексеру бір рет қана орындалады. Оған дейін консенсус
  консервативті, hardware-independent жұмыс бағасын резервтейді.
- `[zk.sccp]` proof саны/байты, native headers, Ethereum light-client updates,
  header bytes, secp256k1 recoveries, BLS aggregate checks/signing contributions
  және BN254 pairing-product checks үшін міндетті нөлден үлкен per-proof,
  per-transaction және per-block шектерін қояды. Бұл қабылдау шектері
  консенсусқа байланыстырылған және барлық валидаторда бірдей болуы тиіс.

## Torii шектері

`/v1/bridge/proofs/submit` және `/v1/bridge/messages` endpoint-specific HTTP
body шектерін қолданады. Аутентификация, rate limit және `Content-Length` body
оқылмай тұрып тексеріледі; chunked body тек қатаң шекке дейін оқылады. Өлшемі
артық сұрау `413`, malformed transport/JSON бөлек `400` қайтарады. Detached
transaction payload шегі — 16 MiB, signature payload шегі — 16 KiB.

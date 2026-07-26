---
lang: ka
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> ეს არის 2026-07-11-ის შემოკლებული ლოკალიზებული მიმოხილვა და არა სრული
> ნორმატიული თარგმანი. ზუსტი ტიპების, API კონტრაქტებისა და გამოშვების
> მოთხოვნებისთვის გამოიყენეთ [ინგლისური კანონიკური გვერდი](bridge_proofs.md).

# SCCP V1 ხიდის მტკიცებულებები — შემოკლებული მიმოხილვა

## პირველი გამოშვების საზღვარი

- SCCP V1 დახურული ზედაპირია: მხარდაჭერილია მხოლოდ Ethereum mainnet, BSC
  mainnet და TRON mainnet, ხოლო SORA-ს ერთადერთი ბოლო წერტილია `sora-taira`.
  სხვა ქსელის პროფილი ან SORA-ს სხვა იდენტობა უარყოფილია.
- `SubmitBridgeProof` იღებს მხოლოდ მარშრუტზე მიბმულ ტიპიზებულ
  `NativeProtocol` და `SccpDestination` მტკიცებულებებს. ზოგადი `Ics` და
  `TransparentZk` payload-ების წარდგენა მიუწვდომელია და fail-closed წესით
  უარყოფილია.

## ტიპიზებული რეესტრი და ისტორია

- `SccpRegistryV1` არის ტიპიზებული და append-only. თითო lane ინარჩუნებს
  მაქსიმუმ 64 მარშრუტის რევიზიას და 4,096 native trust anchor-ს. ჩანაწერები
  ფარულად არ იშლება; ზღვრის შემდეგი დამატება ატომურად უარყოფილია.
- Anchor ინტერვალი იყენებს დამოწმებულ კონსენსუსის კოორდინატს: Ethereum-ზე
  finalized beacon slot-ს, BSC/TRON-ზე finalized native block height-ს.
  ძველი anchor ძალაშია მომდევნო checkpoint-ის ჩათვლით და შემდეგ აღარ.
- მდგრადი inbound ჩანაწერი ცალ-ცალკე ინახავს event/finality height-სა და
  `anchor_interval_height`-ს. lane+anchor high-water მნიშვნელობა მხოლოდ
  იზრდება; ახალი checkpoint მასზე დაბალი ვერ იქნება. Snapshot hydration ამ
  ინდექსს სრულად ხელახლა ითვლის და აკლებული, მოძველებული ან ზედმეტი
  მნიშვნელობა უარყოფილია. message id-ის ხელახალი გამოყენება ან replay ასევე
  უარყოფილია.

TRON-ის წყაროს route იყენებს ზუსტ
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI-ს. წარმატებული
შესრულებისთვის აუცილებელია `expectedNonce == transferNonce`; შემდეგ storage-ის
გაზრდამდე იგივე მნიშვნელობა canonical payload-ში იწერება. Native admission
payload-ის recipient-ისგან, მასშტაბირებული თანხისა და nonce-ისგან სრულ ABI call-ს
აღადგენს. ამიტომ მოძველებული ორ-argumentიანი selector, ძველი ან მომავალი nonce და
ამოწურული `uint64` nonce უსაფრთხოდ უარყოფილია.

## ერთჯერადი შემოწმება და დეტერმინისტული ლიმიტები

- Native და destination მტკიცებულება კანონიკურად იშიფრება ერთხელ და ძვირი
  კრიპტოგრაფიული შემოწმება ერთხელ სრულდება. მანამდე კონსენსუსი ჯავშნის
  კონსერვატიულ, hardware-independent სამუშაოს შეფასებას.
- `[zk.sccp]` ადგენს სავალდებულო არანულოვან per-proof, per-transaction და
  per-block ლიმიტებს: მტკიცებულებების რაოდენობა/ბაიტები, native headers,
  Ethereum light-client updates, header bytes, secp256k1 recoveries, BLS
  aggregate checks/signing contributions და BN254 pairing-product checks.
  ეს მიღების ლიმიტები კონსენსუსზეა მიბმული და ყველა ვალიდატორზე ერთნაირი
  უნდა იყოს.

## Outbound commitment, შენახვა და აღმოჩენა

ყოველი წარმატებული outbound message იღებს მკვრივ `commitment_index`-ს block-ის
execution order-ით (`0..=511`). V1-ის ფიქსირებული ზღვარია 512 message თითო block-ზე და
4,096 canonical payload byte თითო message-ზე. `[zk.sccp]` pending payload state-ს
ერთობლივად ზღუდავს `max_pending_outbound_messages` (default `65536`) და
`max_pending_outbound_payload_bytes` (default `268435456`).

Finality-ის გამოქვეყნებამდე ან block body-ის eviction-მდე Kura immutable ფორმით
ინახავს ზუსტ canonical header-ს და root-authenticated SCCP archive-ს. Proof, bundle,
proof request და recent history-ის აღდგენა არ კითხულობს ისტორიულ block body-ს ან
mutable WSV payload copy-ს. Destination proof-ის მიღებისას pending payload და მისი
charge atomically იშლება და რჩება fixed terminal descriptor locator/index-თან ერთად.
Pending state შეზღუდულია; terminal records და immutable Kura history მუდმივი replay
protection-ისთვის განზრახ იზრდება. `GET /v1/sccp/messages/recent` იყენებს compound
cursor-ს `{ from, after_index }`. Immutable evidence ითვლება total/operator disk
usage-ში, მაგრამ გამორიცხულია evictable-body budget-იდან.

## Torii-ის საზღვრები

`/v1/bridge/proofs/submit` და `/v1/bridge/messages` იყენებს endpoint-specific
HTTP body ლიმიტებს. ავთენტიკაცია, rate limit და `Content-Length` მოწმდება body-ს
წაკითხვამდე; chunked body იკითხება მხოლოდ მკაცრ ზღვრამდე. ზედმეტად დიდი
მოთხოვნა აბრუნებს `413`, ხოლო malformed transport/JSON — განცალკევებულ `400`-ს.
Detached transaction payload მაქსიმუმ 16 MiB-ია, signature payload — 16 KiB.

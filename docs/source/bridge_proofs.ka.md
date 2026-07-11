---
lang: ka
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 69c9a740261d0c367d52870fc1f48775ae48307056ba9b79d2f811e0c0849f20
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
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

## Torii-ის საზღვრები

`/v1/bridge/proofs/submit` და `/v1/bridge/messages` იყენებს endpoint-specific
HTTP body ლიმიტებს. ავთენტიკაცია, rate limit და `Content-Length` მოწმდება body-ს
წაკითხვამდე; chunked body იკითხება მხოლოდ მკაცრ ზღვრამდე. ზედმეტად დიდი
მოთხოვნა აბრუნებს `413`, ხოლო malformed transport/JSON — განცალკევებულ `400`-ს.
Detached transaction payload მაქსიმუმ 16 MiB-ია, signature payload — 16 KiB.

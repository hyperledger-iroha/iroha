---
lang: ka
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraFS CI ნიმუშები

ეს დირექტორიაში შეფუთულია ნიმუშიდან გამომუშავებული დეტერმინისტული არტეფაქტები
ტვირთამწეობა `fixtures/sorafs_manifest/ci_sample/` ქვეშ. პაკეტი აჩვენებს
ბოლოდან ბოლომდე SoraFS შეფუთვისა და ხელმოწერის მილსადენი, რომელსაც CI workflows ახორციელებს.

## არტეფაქტის ინვენტარი

| ფაილი | აღწერა |
|------|-------------|
| `payload.txt` | წყაროს დატვირთვა, რომელსაც იყენებენ მოწყობილობების სკრიპტები (უბრალო ტექსტის ნიმუში). |
| `payload.car` | მანქანის არქივი გამოშვებული `sorafs_cli car pack`-ის მიერ. |
| `car_summary.json` | `car pack`-ის მიერ გენერირებული რეზიუმე, რომელიც აღწერს ნაწილაკებს და მეტამონაცემებს. |
| `chunk_plan.json` | Fetch-plan JSON, რომელიც აღწერს ნაჭრის დიაპაზონს და პროვაიდერის მოლოდინებს. |
| `manifest.to` | Norito მანიფესტი დამზადებულია `sorafs_cli manifest build`-ის მიერ. |
| `manifest.json` | ადამიანის მიერ წაკითხვადი მანიფესტის რენდერი გამართვისთვის. |
| `proof.json` | PoR შეჯამება გამოშვებული `sorafs_cli proof verify`-ის მიერ. |
| `manifest.bundle.json` | `sorafs_cli manifest sign`-ის მიერ გენერირებული ხელმოწერის გარეშე გასაღების ნაკრები. |
| `manifest.sig` | მოშორებული Ed25519 ხელმოწერა, რომელიც შეესაბამება manifest-ს. |
| `manifest.sign.summary.json` | ხელმოწერის დროს გამოქვეყნებული CLI შეჯამება (ჰეშები, ნაკრების მეტამონაცემები). |
| `manifest.verify.summary.json` | CLI რეზიუმე `manifest verify-signature`-დან. |

ყველა დაიჯესტი, რომელიც მითითებულია გამოშვების შენიშვნებში და დოკუმენტაციაში, წყაროდან იყო
ამ ფაილებს. `ci/check_sorafs_cli_release.sh` სამუშაო პროცესი იგივეს აღადგენს
არტეფაქტები და განასხვავებს მათ ჩადენილი ვერსიებისგან.

## მოწყობილობების რეგენერაცია

განახორციელეთ ქვემოთ მოცემული ბრძანებები საცავის ფესვიდან, რათა აღადგინოთ მოწყობილობების ნაკრები.
ისინი ასახავს `sorafs-cli-fixture` სამუშაო ნაკადის მიერ გამოყენებულ ნაბიჯებს:

```bash
sorafs_cli car pack \
  --input fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)" \
  --issued-at 1700000000 \
  > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
  > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

თუ რომელიმე ნაბიჯი აწარმოებს სხვადასხვა ჰეშებს, გამოიკვლიეთ მოწყობილობების განახლებამდე.
CI სამუშაო ნაკადები ეყრდნობა დეტერმინისტულ გამომავალს რეგრესიების გამოსავლენად.

## მომავალი გაშუქება

როგორც დამატებითი chunker პროფილები და მტკიცებულების ფორმატები სრულდება საგზაო რუქიდან,
მათი კანონიკური მოწყობილობები დაემატება ამ დირექტორიაში (მაგ.
`sorafs.sf2@1.0.0` (იხ. `fixtures/sorafs_manifest/ci_sample_sf2/`) ან PDP
ნაკადი მტკიცებულებები). ყოველი ახალი პროფილი მიჰყვება იმავე სტრუქტურას - payload, CAR,
გეგმა, მანიფესტი, მტკიცებულებები და ხელმოწერის არტეფაქტები - ასე რომ, ქვემოთ ავტომატიზაციას შეუძლია
განსხვავებული რელიზები პერსონალური სკრიპტის გარეშე.
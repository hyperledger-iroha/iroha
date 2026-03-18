---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> ადაპტირებულია [`docs/source/sorafs/provider_admission_policy.md`]-დან (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# SoraFS პროვაიდერის დაშვებისა და პირადობის პოლიტიკა (SF-2b პროექტი)

ეს ჩანაწერი ასახავს **SF-2b**-ის ქმედითუნარიან მიწოდებებს: განმსაზღვრელი და
მისაღები სამუშაო პროცესის, პირადობის მოთხოვნების და ატესტაციის აღსრულება
დატვირთვა SoraFS შენახვის პროვაიდერებისთვის. ის აფართოებს მაღალი დონის პროცესს
ასახულია SoraFS Architecture RFC-ში და ყოფს დარჩენილ სამუშაოს
თვალყურის დევნებადი საინჟინრო ამოცანები.

## პოლიტიკის მიზნები

- დარწმუნდით, რომ მხოლოდ შემოწმებულ ოპერატორებს შეუძლიათ გამოაქვეყნონ `ProviderAdvertV1` ჩანაწერები, რომ
  ქსელი მიიღებს.
- მიამაგრეთ ყველა სარეკლამო გასაღები მმართველობის მიერ დამტკიცებულ პირადობის დამადასტურებელ დოკუმენტთან,
  დამოწმებული საბოლოო წერტილები და მინიმალური ფსონის წვლილი.
- უზრუნველყოს განმსაზღვრელი გადამოწმების ხელსაწყოები, რათა Torii, კარიბჭეები და
  `sorafs-node` ახორციელებს იგივე შემოწმებებს.
- მხარი დაუჭირეთ განახლებას და საგანგებო გაუქმებას დეტერმინიზმის დარღვევის გარეშე ან
  ხელსაწყოების ერგონომიკა.

## პირადობის და ფსონის მოთხოვნები

| მოთხოვნა | აღწერა | მიწოდება |
|-------------|-------------|-------------|
| სარეკლამო გასაღები წარმოშობის | პროვაიდერებმა უნდა დაარეგისტრირონ Ed25519 გასაღებების წყვილი, რომელიც ხელს აწერს ყველა რეკლამას. დაშვების პაკეტი ინახავს საჯარო გასაღებს მმართველობის ხელმოწერასთან ერთად. | გააფართოვეთ `ProviderAdmissionProposalV1` სქემა `advert_key`-ით (32 ბაიტი) და მიმართეთ მას რეესტრიდან (`sorafs_manifest::provider_admission`). |
| ფსონების მაჩვენებელი | დასაშვებად საჭიროა არანულოვანი `StakePointer`, რომელიც მიუთითებს აქტიური ფსონის აუზზე. | დაამატეთ ვალიდაცია `sorafs_manifest::provider_advert::StakePointer::validate()`-ში და ზედაპირული შეცდომები CLI/ტესტებში. |
| იურისდიქციის ტეგები | პროვაიდერები აცხადებენ იურისდიქციას + იურიდიულ კონტაქტს. | გააფართოვეთ წინადადების სქემა `jurisdiction_code` (ISO 3166-1 alpha-2) და სურვილისამებრ `contact_uri`. |
| ბოლო წერტილის ატესტაცია | თითოეული რეკლამირებული საბოლოო წერტილი უნდა იყოს მხარდაჭერილი mTLS ან QUIC სერტიფიკატის ანგარიშით. | განსაზღვრეთ `EndpointAttestationV1` Norito ტვირთამწეობა და შეინახეთ თითო საბოლოო წერტილის დაშვების პაკეტში. |

## მისაღები სამუშაო პროცესი

1. **წინადადების შექმნა **
   - CLI: დაამატეთ `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …`
     აწარმოებს `ProviderAdmissionProposalV1` + საატესტაციო პაკეტს.
   - ვალიდაცია: უზრუნველყოს საჭირო ველები, ფსონი > 0, კანონიკური ცუნკერის სახელური `profile_id`-ში.
2. **მმართველობის მოწონება **
   - საბჭო ხელს აწერს `blake3("sorafs-provider-admission-v1" || canonical_bytes)` არსებულის გამოყენებით
     კონვერტის ხელსაწყოები (`sorafs_manifest::governance` მოდული).
   - კონვერტი შენარჩუნებულია `governance/providers/<provider_id>/admission.json`-მდე.
3. ** რეესტრის გადაყლაპვა **
   - გაზიარებული ვერიფიკატორის დანერგვა (`sorafs_manifest::provider_admission::validate_envelope`)
     რომ Torii/Gateways/CLI ხელახლა გამოყენება.
   - განაახლეთ Torii დაშვების გზა, რათა უარყოთ რეკლამები, რომელთა დაიჯესტი ან ვადა განსხვავდება კონვერტისგან.
4. ** განახლება და გაუქმება **
   - დაამატეთ `ProviderAdmissionRenewalV1` არჩევითი საბოლოო წერტილის/ფსონის განახლებით.
   - გამოავლინეთ `--revoke` CLI ბილიკი, რომელიც ჩაწერს გაუქმების მიზეზს და უბიძგებს მმართველობით მოვლენას.

## განხორციელების ამოცანები

| ფართობი | ამოცანა | მფლობელ(ებ)ი | სტატუსი |
|------|------|----------|--------|
| სქემა | განსაზღვრეთ `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) `crates/sorafs_manifest/src/provider_admission.rs`-ში. დანერგილია `sorafs_manifest::provider_admission`-ში ვალიდაციის დამხმარეებით.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | შენახვა / მმართველობა | ✅ დასრულებული |
| CLI ხელსაწყოები | გააფართოვეთ `sorafs_manifest_stub` ქვებრძანებებით: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | ინსტრუმენტები WG | ✅ |

CLI ნაკადი ახლა იღებს სერთიფიკატების შუალედურ პაკეტებს (`--endpoint-attestation-intermediate`), ასხივებს
კანონიკური წინადადება/კონვერტის ბაიტი და ამოწმებს საბჭოს ხელმოწერებს `sign`/`verify`-ის დროს. ოპერატორებს შეუძლიათ
პირდაპირ მიაწოდეთ რეკლამის ტექსტები, ან ხელახლა გამოიყენოთ ხელმოწერილი რეკლამები და ხელმოწერის ფაილები შეიძლება მიწოდებული იყოს დაწყვილებით
`--council-signature-public-key` `--council-signature-file`-ით ავტომატიზაციის კეთილგანწყობისთვის.

### CLI მითითება

გაუშვით თითოეული ბრძანება `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …`-ის საშუალებით.

- `proposal`
  - საჭირო დროშები: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>` და მინიმუმ ერთი `--endpoint=<kind:host>`.
  - საბოლოო წერტილის ატესტაცია მოელის `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, სერტიფიკატი მეშვეობით
    `--endpoint-attestation-leaf=<path>` (პლუს სურვილისამებრ `--endpoint-attestation-intermediate=<path>`
    თითოეული ჯაჭვის ელემენტისთვის) და ნებისმიერი შეთანხმებული ALPN ID
    (`--endpoint-attestation-alpn=<token>`). QUIC საბოლოო წერტილებმა შეიძლება მიაწოდოს ტრანსპორტის ანგარიშები
    `--endpoint-attestation-report[-hex]=…`.
  - გამომავალი: კანონიკური Norito წინადადების ბაიტი (`--proposal-out`) და JSON რეზიუმე
    (ნაგულისხმევი stdout ან `--json-out`).
- `sign`
  - შეყვანები: წინადადება (`--proposal`), ხელმოწერილი რეკლამა (`--advert`), სურვილისამებრ რეკლამის ტექსტი
    (`--advert-body`), შეკავების ეპოქა და მინიმუმ ერთი საბჭოს ხელმოწერა. ხელმოწერების მიწოდება შესაძლებელია
    inline (`--council-signature=<signer_hex:signature_hex>`) ან ფაილების საშუალებით კომბინირებით
    `--council-signature-public-key` `--council-signature-file=<path>`-თან ერთად.
  - აწარმოებს დადასტურებულ კონვერტს (`--envelope-out`) და JSON ანგარიშს, რომელიც მიუთითებს დაიჯესტის შეკვრაზე,
    ხელმომწერთა რაოდენობა და შეყვანის ბილიკები.
- `verify`
  - ამოწმებს არსებულ კონვერტს (`--envelope`), სურვილისამებრ ამოწმებს შესატყვის წინადადებას,
    რეკლამა, ან რეკლამის სხეული. JSON ანგარიში ხაზს უსვამს დაიჯესტის მნიშვნელობებს, ხელმოწერის დადასტურების სტატუსს,
    და რომელი არჩევითი არტეფაქტები ემთხვეოდა.
- `renewal`
  - აკავშირებს ახლად დამტკიცებულ კონვერტს ადრე რატიფიცირებულ დაიჯესტთან. მოითხოვს
    `--previous-envelope=<path>` და მემკვიდრე `--envelope=<path>` (ორივე Norito დატვირთვა).
    CLI ადასტურებს, რომ პროფილის მეტსახელები, შესაძლებლობები და რეკლამის გასაღებები უცვლელი რჩება, სანამ
    ფსონის, საბოლოო წერტილებისა და მეტამონაცემების განახლების დაშვების საშუალებას. გამოაქვს კანონიკური
    `ProviderAdmissionRenewalV1` ბაიტი (`--renewal-out`) პლუს JSON რეზიუმე.
- `revoke`
  - გასცემს გადაუდებელ `ProviderAdmissionRevocationV1` პაკეტს პროვაიდერისთვის, რომლის კონვერტიც უნდა
    ამოღებული იყოს. მოითხოვს `--envelope=<path>`, `--reason=<text>`, მინიმუმ ერთი
    `--council-signature` და სურვილისამებრ `--revoked-at`/`--notes`. CLI ხელს აწერს და ადასტურებს
    გაუქმების დაიჯესტი, წერს Norito დატვირთვას `--revocation-out`-ით და ბეჭდავს JSON ანგარიშს
    დაიჯესტისა და ხელმოწერების რაოდენობის აღება.
| გადამოწმება | Torii, კარიბჭეების და `sorafs-node`-ის მიერ გამოყენებული გაზიარებული ვერიფიკატორის დანერგვა. უზრუნველყოს ერთეული + CLI ინტეგრაციის ტესტები.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | ქსელის TL / შენახვა | ✅ დასრულებული |
| Torii ინტეგრაცია | თემის დამადასტურებელი Torii რეკლამის გადაღებაში, უარი თქვით პოლიტიკის მიღმა რეკლამებზე, გამოუშვით ტელემეტრია. | ქსელის TL | ✅ დასრულებული | Torii ახლა იტვირთება მართვის კონვერტები (`torii.sorafs.admission_envelopes_dir`), ამოწმებს დაიჯესტს/ხელმოწერის შესაბამისობას გადაყლაპვისას და ზედაპირების დაშვებას ტელემეტრია.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs
| განახლება | დაამატეთ განახლების / გაუქმების სქემები + CLI დამხმარეები, გამოაქვეყნეთ სასიცოცხლო ციკლის სახელმძღვანელო დოკუმენტებში (იხილეთ ქვემოთ მოცემული Runbook და CLI ბრძანებები `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy:120m | შენახვა / მმართველობა | ✅ დასრულებული |
| ტელემეტრია | განსაზღვრეთ `provider_admission` დაფები და გაფრთხილებები (გამოტოვებული განახლება, კონვერტის ვადის გასვლა). | დაკვირვებადობა | 🟠 მიმდინარეობს | მრიცხველი `torii_sorafs_admission_total{result,reason}` არსებობს; დაფები/გაფრთხილებები ელოდება.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### განახლება და გაუქმება Runbook

#### დაგეგმილი განახლება (ფსონის/ტოპოლოგიის განახლებები)
1. შექმენით მემკვიდრე წინადადება/რეკლამის წყვილი `provider-admission proposal`-ით და `provider-admission sign`-ით, გაზარდეთ `--retention-epoch` და განაახლეთ ფსონი/ბოლო წერტილები საჭიროებისამებრ.
2. შეასრულეთ  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   ბრძანება ამოწმებს უცვლელი შესაძლებლობების/პროფილის ველებს მეშვეობით
   `AdmissionRecord::apply_renewal`, გამოსცემს `ProviderAdmissionRenewalV1` და ბეჭდავს დაიჯესტს
   მმართველობის ჟურნალი.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. შეცვალეთ წინა კონვერტი `torii.sorafs.admission_envelopes_dir`-ში, განაახლეთ Norito/JSON მმართველობის საცავში და დაუმატეთ განახლების ჰეში + შეკავების ეპოქა `docs/source/sorafs/migration_ledger.md`-ს.
4. შეატყობინეთ ოპერატორებს, რომ ახალი კონვერტი ცოცხალია და დააკვირდით `torii_sorafs_admission_total{result="accepted",reason="stored"}` გადაყლაპვის დასადასტურებლად.
5. აღადგინეთ და ჩაატარეთ კანონიკური მოწყობილობები `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`-ის მეშვეობით; CI (`ci/check_sorafs_fixtures.sh`) ადასტურებს Norito გამომავლების სტაბილურობას.

#### გადაუდებელი გაუქმება
1. დაადგინეთ კომპრომეტირებული კონვერტი და გააუქმეთ:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI ხელს აწერს `ProviderAdmissionRevocationV1`-ს, ამოწმებს ხელმოწერის კომპლექტს
   `verify_revocation_signatures` და აცნობებს გაუქმების შეჯამებას.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L
2. ამოიღეთ კონვერტი `torii.sorafs.admission_envelopes_dir`-დან, გაავრცელეთ Norito/JSON გაუქმება დაშვების ქეშებში და ჩაწერეთ მიზეზი ჰეშის მართვის ოქმებში.
3. უყურეთ `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`, რათა დაადასტუროთ, რომ ქეშმა გააუქმა გაუქმებული რეკლამა; შეინახეთ გაუქმების არტეფაქტები ინციდენტების რეტროსპექტებში.

## ტესტირება და ტელემეტრია- დაამატეთ ოქროს მოწყობილობები მისაღები წინადადებებისთვის და კონვერტებისთვის
  `fixtures/sorafs_manifest/provider_admission/`.
- გააფართოვეთ CI (`ci/check_sorafs_fixtures.sh`) წინადადებების რეგენერაციისა და კონვერტების შესამოწმებლად.
- გენერირებული მოწყობილობები მოიცავს `metadata.json` კანონიკური დიჯესტებით; ქვედა დინების ტესტები ამტკიცებს
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- მიაწოდეთ ინტეგრაციის ტესტები:
  - Torii უარყოფს რეკლამებს დაკარგული ან ვადაგასული დაშვების კონვერტებით.
  - CLI ორმხრივი მოგზაურობის შეთავაზებას → კონვერტს → გადამოწმებას.
  - მმართველობის განახლება ახდენს საბოლოო წერტილის ატესტაციის როტაციას პროვაიდერის ID-ის შეცვლის გარეშე.
- ტელემეტრიის მოთხოვნები:
  - გამოუშვით `provider_admission_envelope_{accepted,rejected}` მრიცხველები Torii-ში. ✅ `torii_sorafs_admission_total{result,reason}` ახლა ასახავს მიღებულ/უარყოფილ შედეგებს.
  - დაამატეთ ვადის გასვლის გაფრთხილებები დაკვირვებადობის დაფებს (განახლება 7 დღის განმავლობაში).

## შემდეგი ნაბიჯები

1. ✅ დაასრულა Norito სქემის ცვლილებები და დაეშვა ვალიდაციის დამხმარეები
   `sorafs_manifest::provider_admission`. არ არის საჭირო ფუნქციის დროშები.
2. ✅ CLI სამუშაო ნაკადები (`proposal`, `sign`, `verify`, `renewal`, `revoke`) დოკუმენტირებულია და ხორციელდება ინტეგრაციის ტესტების საშუალებით; შეინახეთ მმართველობის სკრიპტები რენტაბელთან სინქრონში.
3. ✅ Torii დაშვება/აღმოჩენა ჭამს კონვერტებს და გამოაშკარავებს ტელემეტრიის მრიცხველებს მიღება/უარყოფისთვის.
4. ფოკუსირება დაკვირვებადობაზე: დაასრულეთ დაშვების დაფები/გაფრთხილებები, რათა შვიდი დღის განმავლობაში განახლებები გაფრთხილებდეს (`torii_sorafs_admission_total`, ვადის გასვლის ლიანდაგები).
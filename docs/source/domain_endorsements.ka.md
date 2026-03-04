---
lang: ka
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# დომენის დადასტურება

დომენის მოწონება ოპერატორებს უფლებას აძლევს შეზღუდონ დომენის შექმნა და ხელახალი გამოყენება კომიტეტის მიერ ხელმოწერილი განცხადების მიხედვით. დადასტურების ტვირთამწეობა არის Norito ობიექტი, რომელიც ჩაწერილია ჯაჭვზე, რათა კლიენტებმა შეძლონ აუდიტი, ვინ რომელ დომენზე და როდის დაამოწმა.

## დატვირთვის ფორმა

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: კანონიკური დომენის იდენტიფიკატორი
- `committee_id`: ადამიანის მიერ წაკითხული კომიტეტის ეტიკეტი
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: ბლოკის სიმაღლის შეზღუდვის ვალიდობა
- `scope`: სურვილისამებრ მონაცემთა სივრცე პლუს არჩევითი `[block_start, block_end]` ფანჯარა (მათ შორის), რომელიც **უნდა** ფარავდეს მიმღები ბლოკის სიმაღლეს
- `signatures`: ხელმოწერები `body_hash()`-ზე (მოწონება `signatures = []`-ით)
- `metadata`: სურვილისამებრ Norito მეტამონაცემები (წინადადების ID, აუდიტის ბმულები და ა.შ.)

## აღსრულება

- რეკომენდაციები საჭიროა, როდესაც ჩართულია Nexus და `nexus.endorsement.quorum > 0`, ან როდესაც დომენის პოლიტიკა აღნიშნავს დომენს, როგორც საჭიროა.
- ვალიდაცია ახორციელებს დომენის/განცხადების ჰეშის სავალდებულოობას, ვერსიას, ბლოკის ფანჯარას, მონაცემთა სივრცის წევრობას, ვადის გასვლას/ასაკს და კომიტეტის კვორუმს. ხელმომწერებს უნდა ჰქონდეთ პირდაპირი კონსენსუსის გასაღებები `Endorsement` როლით. გამეორება უარყოფილია `body_hash`-ით.
- დომენის რეგისტრაციასთან დართული ინდოსამენტები იყენებენ მეტამონაცემების კლავიშს `endorsement`. იგივე ვალიდაციის გზას იყენებს `SubmitDomainEndorsement` ინსტრუქცია, რომელიც აფიქსირებს ინდოსამენტებს აუდიტისთვის ახალი დომენის რეგისტრაციის გარეშე.

## კომიტეტები და პოლიტიკა

- კომიტეტები შეიძლება დარეგისტრირდეს ჯაჭვზე (`RegisterDomainCommittee`) ან მიღებული კონფიგურაციის ნაგულისხმევი პარამეტრებიდან (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`).
- დომენის პოლიტიკის კონფიგურაცია ხდება `SetDomainEndorsementPolicy`-ის მეშვეობით (კომიტეტის ID, `max_endorsement_age`, `required` დროშით). არყოფნისას გამოიყენება Nexus ნაგულისხმევი პარამეტრები.

## CLI დამხმარეები

- შექმენით/ხელმოწერეთ ინდოსამენტი (გამოიყვანს Norito JSON stdout-ზე):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- წარადგინეთ მოწონება:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- მართეთ მმართველობა:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

ვალიდაციის წარუმატებლობები აბრუნებს სტაბილურ შეცდომის სტრიქონებს (ქვორუმის შეუსაბამობა, შემორჩენილი/ვადაგასული დადასტურება, ფარგლების შეუსაბამობა, უცნობი მონაცემთა სივრცე, დაკარგული კომიტეტი).
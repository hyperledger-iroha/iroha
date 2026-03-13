---
lang: ka
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo Governance Packet შაბლონი (საგზაო რუკა F1)

გამოიყენეთ ეს შაბლონი საგზაო რუქის პუნქტისთვის საჭირო არტეფაქტის ნაკრების მომზადებისას
F1 (რეპო სასიცოცხლო ციკლის დოკუმენტაცია და ხელსაწყოები). მიზანია გადასცეს რეცენზენტები ა
ერთი Markdown ფაილი, რომელიც ჩამოთვლის ყველა შეყვანის, ჰეშის და მტკიცებულების პაკეტს
მმართველ საბჭოს შეუძლია განაახლოს წინადადებაში მითითებული ბაიტები.

> დააკოპირეთ შაბლონი თქვენს მტკიცებულებათა დირექტორიაში (მაგალითად
> `artifacts/finance/repo/2026-03-15/packet.md`), ჩაანაცვლეთ ადგილის დამფუძნებლები და
> ჩაიდინეთ/ატვირთეთ იგი ქვემოთ მითითებულ ჰეშირებულ არტეფაქტებთან.

## 1. მეტამონაცემები

| ველი | ღირებულება |
|-------|-------|
| შეთანხმება/შეცვლის იდენტიფიკატორი | `<repo-yyMMdd-XX>` |
| მომზადებულია / თარიღი | `<desk lead> – 2026-03-15T10:00Z` |
| განხილულია | `<dual-control reviewer(s)>` |
| ტიპის შეცვლა | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| მეურვე(ები) | `<custodian id(s)>` |
| დაკავშირებული წინადადება / რეფერენდუმი | `<governance ticket id or GAR link>` |
| მტკიცებულებათა დირექტორია | ``artifacts/finance/repo/<slug>/`` |

## 2. Instruction Payloads

ჩაწერეთ დადგმული Norito ინსტრუქციები, რომლებზეც მერხები გაფორმებულია
`iroha app repo ... --output`. თითოეული ჩანაწერი უნდა შეიცავდეს ემიტირებულის ჰეშს
ფაილი და მოქმედების მოკლე აღწერა, რომელიც წარდგენილი იქნება კენჭისყრის შემდეგ
გადის.

| მოქმედება | ფაილი | SHA-256 | შენიშვნები |
|--------|------|---------|-------|
| ინიცირება | `instructions/initiate.json` | `<sha256>` | შეიცავს ნაღდ ფულს/გირაოს ნაწილებს, რომლებიც დამტკიცებულია მაგიდის + კონტრაგენტის მიერ. |
| მარჟის გამოძახება | `instructions/margin_call.json` | `<sha256>` | აღბეჭდავს კადენციას + მონაწილის id-ს, რომელმაც გამოიწვია ზარი. |
| განტვირთვა | `instructions/unwind.json` | `<sha256>` | საპირისპირო ფეხის დამადასტურებელი პირობების დაკმაყოფილების შემდეგ. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 მეურვის მადლიერება (მხოლოდ სამმხრივი)

შეავსეთ ეს სექცია, როდესაც რეპო იყენებს `--custodian`. მმართველობის პაკეტი
უნდა შეიცავდეს ხელმოწერილი აღიარება თითოეული მეურვისგან, პლუს ჰეში
ფაილი მითითებულია `docs/source/finance/repo_ops.md`-ის §2.8-ში.

| მეურვე | ფაილი | SHA-256 | შენიშვნები |
|-----------|------|---------|-------|
| `<i105...>` | `custodian_ack_<custodian>.md` | `<sha256>` | ხელმოწერილი SLA, რომელიც მოიცავს პატიმრობის ფანჯარას, მარშრუტიზაციის ანგარიშს და საბურღი კონტაქტს. |

> შეინახეთ აღიარება სხვა მტკიცებულების გვერდით (`artifacts/finance/repo/<slug>/`)
> ასე რომ, `scripts/repo_evidence_manifest.py` ჩაწერს ფაილს იმავე ხეში, როგორც
> დადგმული ინსტრუქციები და კონფიგურაციის ფრაგმენტები. იხ
> `docs/examples/finance/repo_custodian_ack_template.md` მზა შევსებისთვის
> შაბლონი, რომელიც შეესაბამება მმართველობის მტკიცებულების ხელშეკრულებას.

## 3. კონფიგურაციის ფრაგმენტი

ჩასვით `[settlement.repo]` TOML ბლოკი, რომელიც დაეშვება კლასტერზე (მათ შორის
`collateral_substitution_matrix`). შეინახეთ ჰეში ფრაგმენტის გვერდით ისე
აუდიტორებს შეუძლიათ დაადასტურონ მუშაობის დროის პოლიტიკა, რომელიც აქტიური იყო რეპოს დაჯავშნისას
დამტკიცდა.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 დამტკიცების შემდგომი კონფიგურაციის Snapshots

რეფერენდუმის ან მმართველობის კენჭისყრის დასრულების შემდეგ და `[settlement.repo]`
ცვლილება შემოვიდა, გადაიღეთ `/v2/configuration` კადრები ყველა თანატოლისგან.
აუდიტორებს შეუძლიათ დაამტკიცონ, რომ დამტკიცებული პოლიტიკა მოქმედებს კლასტერში (იხ
`docs/source/finance/repo_ops.md` §2.9 მტკიცებულების სამუშაო პროცესისთვის).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v2/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| თანატოლი / წყარო | ფაილი | SHA-256 | ბლოკის სიმაღლე | შენიშვნები |
|---------------|------|--------|-------------|------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot გადაღებულია კონფიგურაციის გაშვებისთანავე. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | ადასტურებს, რომ `[settlement.repo]` ემთხვევა დადგმულ TOML-ს. |

ჩაწერეთ დაიჯესტები თანატოლების ID-ებთან ერთად `hashes.txt`-ში (ან ექვივალენტში
შეჯამება) ასე რომ, მიმომხილველებს შეუძლიათ დაადგინონ, რომელმა კვანძებმა მიიღო ცვლილება. კადრები
ცხოვრობს `config/peers/` ქვეშ TOML ფრაგმენტის გვერდით და იქნება აღებული
ავტომატურად `scripts/repo_evidence_manifest.py`-ით.

## 4. დეტერმინისტული ტესტის არტეფაქტები

მიამაგრეთ უახლესი შედეგები:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

ჩაწერეთ ფაილის ბილიკები + ჰეშები ჟურნალის პაკეტებისთვის ან JUnit XML, რომელიც შექმნილია თქვენი CI-ის მიერ
სისტემა.

| არტეფაქტი | ფაილი | SHA-256 | შენიშვნები |
|----------|------|---------|-------|
| სიცოცხლის ციკლის მტკიცებულება ჟურნალი | `tests/repo_lifecycle.log` | `<sha256>` | გადაღებული `--nocapture` გამომავალით. |
| ინტეგრაციის ტესტის ჟურნალი | `tests/repo_integration.log` | `<sha256>` | მოიცავს ჩანაცვლებას + ზღვრის კადენციის დაფარვას. |

## 5. Lifecycle Proof Snapshot

ყოველი პაკეტი უნდა შეიცავდეს სასიცოცხლო ციკლის დეტერმინისტულ სურათს, საიდანაც ექსპორტირებულია
`repo_deterministic_lifecycle_proof_matches_fixture`. გაატარეთ აღკაზმულობა ერთად
ჩართულია ექსპორტის ღილაკები, რათა მიმომხილველებმა შეძლონ JSON ჩარჩოს განსხვავება და დაჯესტირება
მოწყობილობა, რომელიც თვალყურს ადევნებს `crates/iroha_core/tests/fixtures/`-ში (იხ
`docs/source/finance/repo_ops.md` §2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

ან გამოიყენეთ მიმაგრებული დამხმარე მოწყობილობების რეგენერაციისთვის და დააკოპირეთ ისინი თქვენსში
მტკიცებულებათა ნაკრები ერთ ნაბიჯში:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| არტეფაქტი | ფაილი | SHA-256 | შენიშვნები |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | კანონიკური სასიცოცხლო ციკლის ჩარჩო, რომელიც გამოსხივებულია მტკიცებულების აღკაზმულობით. |
| დაიჯესტი ფაილი | `repo_proof_digest.txt` | `<sha256>` | ზედა რეგისტრის ექვსკუთხედი ასახული `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`-დან; მიამაგრეთ უცვლელადაც კი. |

## 6. მტკიცებულების მანიფესტი

შექმენით მანიფესტი მთელი მტკიცებულების დირექტორია, რათა აუდიტორებმა შეძლონ გადამოწმება
ჰეშები არქივის გახსნის გარეშე. დამხმარე ასახავს აღწერილ სამუშაო პროცესს
`docs/source/finance/repo_ops.md`-ში §3.2.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| არტეფაქტი | ფაილი | SHA-256 | შენიშვნები |
|----------|------|---------|-------|
| მტკიცებულება მანიფესტი | `manifest.json` | `<sha256>` | ჩართეთ საკონტროლო თანხა მმართველობის ბილეთებში/რეფერენდუმის ჩანაწერებში. |

## 7. ტელემეტრია და მოვლენის სურათი

შესაბამისი `AccountEvent::Repo(*)` ჩანაწერების და ნებისმიერი დაფის ან CSV-ის ექსპორტი
ექსპორტი მითითებულია `docs/source/finance/repo_ops.md`-ში. ჩაწერეთ ფაილები +
აქ არის ჰეშები, რათა მიმომხილველებმა პირდაპირ მტკიცებულებაზე გადავიდნენ.

| ექსპორტი | ფაილი | SHA-256 | შენიშვნები |
|--------|------|---------|-------|
| რეპო ღონისძიებები JSON | `evidence/repo_events.ndjson` | `<sha256>` | Raw Torii მოვლენის ნაკადი გაფილტრულია სამაგიდო ანგარიშებში. |
| ტელემეტრია CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | ექსპორტირებულია Grafana-დან Repo Margin პანელის გამოყენებით. |

## 8. დამტკიცებები და ხელმოწერები

- ** ორმაგი კონტროლის ხელმომწერები:** `<names + timestamps>`
- **GAR / წუთი დაიჯესტი:** ხელმოწერილი GAR PDF-ის `<sha256>` ან ატვირთვის წუთი.
- **შენახვის ადგილი:** `governance://finance/repo/<slug>/packet/`

## 9. ჩამონათვალი

მონიშნეთ თითოეული ელემენტი დასრულების შემდეგ.

- [ ] ინსტრუქციების დატვირთვა დადგმული, ჰეშირებული და მიმაგრებული.
- [ ] კონფიგურაციის სნიპეტის ჰეში ჩაწერილია.
- [ ] განმსაზღვრელი სატესტო ჟურნალები აღბეჭდილია + ჰეშირებული.
- [ ] სასიცოცხლო ციკლის სნეპშოტი + დაიჯესტი ექსპორტირებული.
- [ ] მტკიცებულება მანიფესტი გენერირებული და ჰეში ჩაწერილი.
- [ ] მოვლენის/ტელემეტრიის ექსპორტი აღბეჭდილია + ჰეშირებული.
- [ ] ორმაგი კონტროლის დადასტურება დაარქივებულია.
- [ ] GAR/ატვირთული წუთები; დაიჯესტი ჩაწერილი ზემოთ.

ამ შაბლონის შენარჩუნება ყველა პაკეტთან ერთად ინარჩუნებს მმართველობას DAG
განმსაზღვრელი და აუდიტორებს აწვდის პორტატული მანიფესტს რეპოს სასიცოცხლო ციკლისთვის
გადაწყვეტილებები.
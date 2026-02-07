---
lang: ka
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ გადაცემის გაჯეტის დიზაინი

# მიმოხილვა

მიმდინარე FASTPQ დამგეგმავი ჩაწერს ყველა პრიმიტიულ ოპერაციას, რომელიც ჩართულია `TransferAsset` ინსტრუქციაში, რაც ნიშნავს, რომ თითოეული გადარიცხვა იხდის ბალანსის არითმეტიკას, ჰეშის რაუნდებს და SMT განახლებებს. თითო გადაცემის რიგების შესამცირებლად, ჩვენ შემოგთავაზებთ სპეციალურ გაჯეტს, რომელიც ამოწმებს მხოლოდ მინიმალურ არითმეტიკულ/ვალდებულების შემოწმებებს, სანამ მასპინძელი აგრძელებს კანონიკური მდგომარეობის გადასვლის შესრულებას.

- ** ფარგლები **: ერთჯერადი გადარიცხვები და მცირე პარტიები, რომლებიც ემიტირებულია არსებული Kotodama/IVM `TransferAsset` syscall ზედაპირის მეშვეობით.
- **მიზანი**: ამოიღეთ FFT/LDE სვეტის ნაკვალევი დიდი მოცულობის გადარიცხვებისთვის საძიებო ცხრილების გაზიარებით და თითო გადატანის არითმეტიკის დაშლით კომპაქტურ შეზღუდვის ბლოკად.

#არქიტექტურა

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## ტრანსკრიპტის ფორმატი

ჰოსტი ასხივებს `TransferTranscript`-ს თითო syscall გამოძახებით:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` აკავშირებს ტრანსკრიპტს ტრანზაქციის შესვლის წერტილის ჰეშთან ხელახლა დაცვის მიზნით.
- `authority_digest` არის ჰოსტის ჰეში დალაგებული ხელმომწერების/ქვორუმის მონაცემებზე; გაჯეტი ამოწმებს თანასწორობას, მაგრამ ხელმოწერის დადასტურებას ხელახლა არ აკეთებს. კონკრეტულად, ჰოსტი Norito-კოდირებს `AccountId`-ს (რომელიც უკვე შეიცავს კანონიკურ მულტიზიგ კონტროლერს) და ჰეშირებს `b"iroha:fastpq:v1:authority|" || encoded_account`-ს Blake2b-256-თან ერთად, ინახავს მიღებულ Kotodama-ს.
- `poseidon_preimage_digest` = Poseidon(ანგარიში_დან || ანგარიშიდან || აქტივი || თანხა || პარტია_ჰეშ); უზრუნველყოფს გაჯეტის ხელახლა გამოთვლას, როგორც მასპინძელი. პრეიმიჯის ბაიტები აგებულია როგორც `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash`, შიშველი Norito კოდირების გამოყენებით, სანამ ისინი გაივლიან საერთო Poseidon2 დამხმარეს. ეს დაიჯესტი წარმოდგენილია ერთი დელტას ტრანსკრიპტებისთვის და გამოტოვებულია მრავალ დელტას პარტიებისთვის.

ყველა ველი სერიიზებულია Norito-ის საშუალებით, ამიტომ არსებული დეტერმინიზმი გარანტირებულია.
ორივე `from_path` და `to_path` გამოიყოფა როგორც Norito blobs გამოყენებით
`TransferMerkleProofV1` სქემა: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
მომავალ ვერსიებს შეუძლიათ გააფართოვონ სქემა, ხოლო პროვერი ახორციელებს ვერსიის ტეგს
გაშიფვრამდე. `TransitionBatch` მეტამონაცემებში ჩაშენებულია Norito-ში დაშიფრული ტრანსკრიპტი
ვექტორი `transfer_transcripts` კლავიშის ქვეშ, რათა პროვერტმა შეძლოს მოწმის გაშიფვრა
ზოლის გარეშე მოთხოვნების შესრულების გარეშე. საჯარო შეყვანები (`dsid`, `slot`, ფესვები,
`perm_root`, `tx_set_hash`) ტარდება `FastpqTransitionBatch.public_inputs`-ში,
მეტამონაცემების დატოვება შესვლის ჰეშის/ტრანსკრიპტის დათვლის აღრიცხვისთვის. მასპინძლის სანტექნიკამდე
მიწები, პროვერი სინთეზურად იღებს მტკიცებულებებს გასაღები/ბალანსის წყვილებიდან ასე რიგებიდან
ყოველთვის შეიცავდეს დეტერმინისტულ SMT გზას მაშინაც კი, როდესაც ტრანსკრიპტი გამოტოვებს არჩევით ველებს.

## გაჯეტის განლაგება

1. **ბალანსის არითმეტიკული ბლოკი**
   - შეყვანები: `from_balance_before`, `amount`, `to_balance_before`.
   - ამოწმებს:
     - `from_balance_before >= amount` (დიაპაზონის გაჯეტი საერთო RNS დაშლით).
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - შეფუთულია მორგებულ კარიბჭეში, ასე რომ სამივე განტოლება მოიხმარს ერთ მწკრივ ჯგუფს.2. **პოსეიდონის ვალდებულების ბლოკი**
   - ხელახლა გამოითვლება `poseidon_preimage_digest` პოსეიდონის საძიებო ცხრილის გამოყენებით, რომელიც უკვე გამოიყენება სხვა გაჯეტებში. არავითარი ტრანსფერი პოსეიდონის რაუნდები კვალში.

3. **მერკლის ბილიკის ბლოკი**
   - აფართოებს არსებულ Kaigi SMT გაჯეტს "დაწყვილებული განახლების" რეჟიმით. ორი ფურცელი (გამგზავნი, მიმღები) იზიარებს იმავე სვეტს და-ძმის ჰეშებისთვის, რაც ამცირებს დუბლირებულ რიგებს.

4. ** ავტორიტეტული დაიჯესტის შემოწმება **
   - მარტივი თანასწორობის შეზღუდვა მასპინძლის მიერ მოწოდებულ დაიჯესტსა და მოწმის მნიშვნელობას შორის. ხელმოწერები რჩება მათ სპეციალურ გაჯეტში.

5. **Batch Loop**
   - პროგრამები იძახებენ `transfer_v1_batch_begin()`-ს `transfer_asset` მშენებლების მარყუჟამდე და `transfer_v1_batch_end()`-ის შემდეგ. სანამ ფარგლები აქტიურია, მასპინძელი ბუფერს უკეთებს თითოეულ გადაცემას და იმეორებს მათ როგორც `TransferAssetBatch`, ხელახლა იყენებს Poseidon/SMT კონტექსტს თითო პარტიაში ერთხელ. ყოველი დამატებითი დელტა ამატებს მხოლოდ არითმეტიკას და ორ ფოთლის შემოწმებას. ტრანსკრიპტის დეკოდერი ახლა იღებს მრავალ დელტა პარტიებს და აფენს მათ როგორც `TransferGadgetInput::deltas`, რათა დამგეგმავმა შეძლოს მოწმეების დაკეცვა Norito ხელახლა წაკითხვის გარეშე. კონტრაქტებს, რომლებსაც უკვე აქვთ Norito მოსახერხებელი დატვირთვა (მაგ., CLI/SDK-ები), შეუძლიათ მთლიანად გამოტოვონ ფარგლები `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`-ზე დარეკვით, რომელიც გადასცემს მასპინძელს სრულად დაშიფრულ პარტიას ერთ syscall-ში.

# მასპინძლის და პროვერის ცვლილებები| ფენა | ცვლილებები |
|-------|---------|
| `ivm::syscalls` | დაამატეთ `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`), რათა პროგრამებმა შეძლონ მრავალჯერადი `transfer_v1` სისტემის ფრჩხილი შუალედური ISI-ების გამოცემის გარეშე, პლუს ISI0000000001 (`0x2B`) წინასწარ კოდირებული პარტიებისთვის. |
| `ivm::host` & ტესტები | ძირითადი/ნაგულისხმევი მასპინძლები განიხილავენ `transfer_v1`-ს, როგორც სერიულ დანართს, სანამ სფერო აქტიურია, ზედაპირული `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` და იმიტირებული WSV ჰოსტი ბუფერებს ჩანაწერებს ჩადენამდე, ასე რომ რეგრესიის ტესტებმა შეიძლება დაამტკიცონ დეტერმინისტული ბალანსი განახლებები.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | გამოუშვით `TransferTranscript` მდგომარეობიდან გადასვლის შემდეგ, შექმენით `FastpqTransitionBatch` ჩანაწერები აშკარა `public_inputs`-ით `StateBlock::capture_exec_witness`-ის დროს და გაუშვით FASTPQ პროვერის ხაზი ისე, რომ ორივე Torii მიიღოს Statistics Backending და/C. `TransitionBatch` შეყვანები. `TransferAssetBatch` აჯგუფებს თანმიმდევრულ გადარიცხვებს ერთ ტრანსკრიპტში, გამოტოვებს პოსეიდონის დაიჯესტს მრავალ დელტა პარტიებისთვის, რათა გაჯეტმა შეძლოს განმეორებითი გამეორება შენატანებში დეტერმინისტულად. |
| `fastpq_prover` | `gadgets::transfer` ახლა ამოწმებს მრავალ დელტას ტრანსკრიპტებს (ბალანსის არითმეტიკა + პოსეიდონის დაიჯესტი) და ზედაპირების სტრუქტურირებულ მოწმეებს (მათ შორის, ჩანაცვლებითი დაწყვილებული SMT blobs) დამგეგმავისთვის (`crates/fastpq_prover/src/gadgets/transfer.rs`). `trace::build_trace` შიფრავს ამ ტრანსკრიპტებს სერიის მეტამონაცემებიდან, უარყოფს გადაცემის პარტიებს, რომლებსაც აკლია `transfer_transcripts` დატვირთვა, ამაგრებს დამოწმებულ მოწმეებს `Trace::transfer_witnesses`-ზე და `TracePolynomialData::transfer_plan()` დამუშავებული გეგმები არ შეინარჩუნებს გაჯეტის გეგმებს. (`crates/fastpq_prover/src/trace.rs`). მწკრივების რაოდენობის რეგრესიის აღკაზმულობა ახლა იგზავნება `fastpq_row_bench`-ის (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) მეშვეობით, რომელიც მოიცავს 65536 დაფარულ მწკრივს სცენარს, ხოლო დაწყვილებული SMT გაყვანილობა რჩება TF-3 სურათების დამხმარე ეტაპს მიღმა (ეს ადგილის დამხმარეები არ შეინარჩუნებენ სცენარს. |
| Kotodama | ამცირებს `transfer_batch((from,to,asset,amount), …)` დამხმარეს `transfer_v1_batch_begin`-ში, თანმიმდევრულ `transfer_asset` ზარებში და `transfer_v1_batch_end`-ში. თითოეული არგუმენტი უნდა შეესაბამებოდეს `(AccountId, AccountId, AssetDefinitionId, int)` ფორმას; ერთჯერადი გადარიცხვები ინარჩუნებს არსებულ მშენებელს. |

მაგალითი Kotodama გამოყენების:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` ახორციელებს იგივე ნებართვას და არითმეტიკულ შემოწმებას, როგორც ცალკეული `Transfer::asset_numeric` ზარები, მაგრამ ჩაწერს ყველა დელტას ერთი `TransferTranscript`-ში. მრავალ დელტას ტრანსკრიპტები ანადგურებენ პოსეიდონის მონელებას მანამ, სანამ თითო დელტას ვალდებულებები არ დასრულებულა შემდგომში. Kotodama მშენებელი ახლა ავტომატურად ასხივებს დაწყების/დამთავრების სისკალებს, ასე რომ, კონტრაქტებს შეუძლიათ განათავსონ ჯგუფური გადარიცხვები Norito ხელნაკეთი კოდირების გარეშე.

## მწკრივის დათვლის რეგრესიის აღკაზმულობა

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) ასინთეზებს FASTPQ გარდამავალ პარტიებს კონფიგურირებადი სელექტორის ნომრებით და აცნობებს მიღებულ `row_usage` შეჯამებას (`total_rows`, თითო სელექტორის სიგრძის თვლაზე/სიგრძეზე ₂. 65536 რიგის ჭერის საორიენტაციო ნიშნების აღება:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```ემიტირებული JSON ასახავს FASTPQ სერიის არტეფაქტებს, რომლებსაც `iroha_cli audit witness` ახლა ასხივებს ნაგულისხმევად (გაავლეთ `--no-fastpq-batches` მათ დასათრგუნად), ასე რომ, `scripts/fastpq/check_row_usage.py` და CI კარიბჭეს შეუძლია განასხვავოს სინთეზური გაშვებული ნახატები წინა ვალიდაციის დროს.

# გავრცელების გეგმა

1. **TF-1 (ტრანსკრიპტის სანტექნიკა)**: ✅ `StateTransaction::record_transfer_transcripts` ახლა ასხივებს Norito ტრანსკრიპტებს ყოველი `TransferAsset`/პარტიისთვის, `sumeragi::witness::record_fastpq_transcript` ინახავს მათ გლობალური მოწმის შიგნით81000X და I001 `fastpq_batches` მკაფიო `public_inputs`-ით ოპერატორებისთვის და პროვერის ზოლისთვის (გამოიყენეთ `--no-fastpq-batches` თუ გჭირდება უფრო გამხდარი გამომავალი).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness .rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2 (გაჯეტის დანერგვა)**: ✅ `gadgets::transfer` ახლა ამოწმებს მრავალ დელტა ტრანსკრიპტებს (ბალანსის არითმეტიკა + პოსეიდონის დაიჯესტი), ასინთეზებს დაწყვილებულ SMT მტკიცებულებებს, როდესაც მასპინძლები გამოტოვებენ მათ, ავლენს სტრუქტურირებულ მოწმეებს Kotodama-ის და Kotodama-ის მეშვეობით Kotodama. გადააქვს ეს მოწმეები `Trace::transfer_witnesses`-ში, ხოლო მტკიცებულებებიდან ავსებს SMT სვეტებს. `fastpq_row_bench` იჭერს 65536-სტრიქონიანი რეგრესიის აღკაზმულობას, რათა დამგეგმავები აკონტროლონ მწკრივების გამოყენება Norito-ის ხელახლა დაკვრის გარეშე payloads.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (Batch Helper)**: ჩართეთ batch syscall + Kotodama builder, ჰოსტის დონის თანმიმდევრული აპლიკაციისა და გაჯეტის ციკლის ჩათვლით.
4. **TF-4 (ტელემეტრია და დოკუმენტები)**: განაახლეთ `fastpq_plan.md`, `fastpq_migration_guide.md` და დაფის სქემები გადაცემის სტრიქონების ზედაპირზე გადანაწილებისთვის სხვა გაჯეტებთან მიმართებაში.

# ღია კითხვები

- **დომენის ლიმიტები**: ამჟამინდელი FFT დამგეგმავი პანიკაშია 2¹4 მწკრივის მიღმა კვალისთვის. TF-2 ან უნდა გაზარდოს დომენის ზომა, ან დააფიქსიროს შემცირებული საორიენტაციო მიზანი.
- **მრავალ აქტივების პარტიები**: საწყისი გაჯეტი ითვალისწინებს იგივე აქტივის ID-ს თითო დელტაზე. თუ ჩვენ გვჭირდება ჰეტეროგენული პარტიები, ჩვენ უნდა დავრწმუნდეთ, რომ პოსეიდონის მოწმე ყოველ ჯერზე მოიცავს აქტივს, რათა თავიდან იქნას აცილებული აქტივების ჯვარედინი გამეორება.
- **ავტორიტეტის დაიჯესტის ხელახალი გამოყენება**: გრძელვადიან პერსპექტივაში ჩვენ შეგვიძლია ხელახლა გამოვიყენოთ იგივე დაიჯესტი სხვა ნებადართული ოპერაციებისთვის, რათა თავიდან ავიცილოთ ხელმომწერთა სიების ხელახალი გამოთვლა თითო syscall-ზე.


ეს დოკუმენტი თვალყურს ადევნებს დიზაინის გადაწყვეტილებებს; შეინახეთ ის სინქრონიზებული საგზაო რუქის ჩანაწერებთან, როდესაც ეტაპები დგება.
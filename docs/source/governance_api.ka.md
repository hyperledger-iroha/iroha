---
lang: ka
direction: ltr
source: docs/source/governance_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eea277d4aae6a7b29b5be539ef9d8e63948ccdd89a152de5af4a3cb357fe543a
source_last_modified: "2026-01-22T16:26:46.569356+00:00"
translation_last_reviewed: 2026-02-07
title: Governance App API — Endpoints (Draft)
translator: machine-google-reviewed
---

სტატუსი: პროექტი/ესკიზი მმართველობის განხორციელების ამოცანების თანმხლები. ფორმები შეიძლება შეიცვალოს განხორციელების დროს. დეტერმინიზმი და RBAC პოლიტიკა ნორმატიული შეზღუდვებია; Torii-ს შეუძლია ხელი მოაწეროს/გააგზავნოს ტრანზაქციები `authority` და `private_key`, წინააღმდეგ შემთხვევაში კლიენტები აშენებენ და წარუდგენენ `/transaction`-ს.

მნიშვნელოვანია: ჩვენ არ ვაგზავნით მუდმივმოქმედ საბჭოს ან „ნაგულისხმევი“ მმართველობის სიას. უჯრის გარეთ, საბჭოს ბოლო წერტილები ან აბრუნებს ცარიელ/მოლოდინში არსებულ მდგომარეობას, ან იღებენ დეტერმინისტულ ჩანაცვლებას კონფიგურირებული პარამეტრებიდან (ფსონის აქტივი, ვადა, კომიტეტის ზომა), როდესაც ჩართულია. ოპერატორებმა უნდა შეინარჩუნონ საკუთარი სია მმართველობითი ნაკადების მეშვეობით; ამ საცავში არ არის გამომცხვარი მრავალგზიანი, საიდუმლო გასაღები ან საბჭოს პრივილეგირებული ანგარიში.

მიმოხილვა
- ყველა საბოლოო წერტილი ბრუნდება JSON. ტრანზაქციის წარმომქმნელი ნაკადებისთვის, პასუხები მოიცავს `tx_instructions` - ერთი ან მეტი ინსტრუქციის ჩონჩხის მასივს:
  - `wire_id`: რეესტრის იდენტიფიკატორი ინსტრუქციის ტიპისთვის
  - `payload_hex`: Norito დატვირთვის ბაიტი (თექვსმეტი)
- თუ `authority` და `private_key` არის მოწოდებული (ან `private_key` ბიულეტენების DTO-ებზე), Torii ხელს აწერს და წარადგენს ტრანზაქციას და მაინც აბრუნებს `tx_instructions`.
- წინააღმდეგ შემთხვევაში, კლიენტები აწყობენ SignedTransaction-ს მათი ავტორიტეტისა და chain_id-ის გამოყენებით, შემდეგ ხელს აწერენ და გამოაქვეყნებენ `/transaction`-ზე.
- SDK გაშუქება:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` აბრუნებს `GovernanceProposalResult` (სტატუსის/სახის ველების ნორმალიზება), `ToriiClient.get_governance_referendum_typed` აბრუნებს `GovernanceReferendumResult`, Prometheus აბრუნებს `GovernanceReferendumResult`, Prometheus აბრუნებს `ToriiClient.get_governance_locks_typed` აბრუნებს `GovernanceLocksResult`-ს, `ToriiClient.get_governance_unlock_stats_typed` აბრუნებს `GovernanceUnlockStats`-ს და `ToriiClient.list_governance_instances_typed` აბრუნებს `GovernanceInstancesPage`-ს, აიძულებს აკრეფილი ზედაპირის მაგალითს README წვდომით.
- Python მსუბუქი კლიენტი (`iroha_torii_client`): `ToriiClient.finalize_referendum` და `ToriiClient.enact_proposal` აბრუნებს აკრეფილი `GovernanceInstructionDraft` პაკეტებს (Torii ჩონჩხის შეფუთვა, როდესაც აცილებს Prometheus სკრიპტს ხელით აანალიზებს ნაკადების დასრულება/დანერგვა.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` ასახავს აკრეფილ დამხმარეებს წინადადებებისთვის, რეფერენდუმებისთვის, ანგარიშების, ბლოკირების, განბლოკვის სტატისტიკისთვის და ახლა `listGovernanceInstances(namespace, options)` პლუს საბჭოს საბოლოო წერტილები (`getGovernanceCouncilCurrent`, `getGovernanceCouncilCurrent` `governancePersistCouncil`, `getGovernanceCouncilAudit`), ასე რომ Node.js კლიენტებს შეუძლიათ `/v1/gov/instances/{ns}`-ის პაგინირება და VRF მხარდაჭერილი სამუშაო ნაკადების მართვა არსებული კონტრაქტის ინსტანციების ჩამონათვალთან ერთად. `governanceFinalizeReferendumTyped` და `governanceEnactProposalTyped` ასახავს პითონის დამხმარეებს ყოველთვის აბრუნებს სტრუქტურირებულ მონახაზს (ცარიელი ჩონჩხის სინთეზირება, როდესაც Torii პასუხობს `204 No Content`), რაც აფერხებს ავტომატიზაციას8NI070X00101-ზე ტრანზაქციის განშტოებამდე. ტრიგერები. `getGovernanceLocksTyped` ახლა ახდენს `404 Not Found` პასუხების ნორმალიზებას `{found: false, locks: {}, referendum_id: <id>}`-ში, ასე რომ JS გამომძახებლები მიიღებენ იგივე ფორმის შედეგს, როგორც პითონის დამხმარე, როდესაც რეფერენდუმს არ აქვს ბლოკირება.

ბოლო წერტილები- POST `/v1/gov/proposals/deploy-contract`
  - მოთხოვნა (JSON):
    {
      "namespace": "აპები",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "…64 hex",
      "abi_hash": "blake2b32:..." | "…64 hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "ავტორიტეტი": "ih58...?",
      "private_key": "...?"
    }
  - პასუხი (JSON):
    { "ok": true, "proposal_id": "…64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - ვალიდაცია: კვანძები ახდენენ `abi_hash`-ის კანონიკიზაციას მოწოდებული `abi_version`-ისთვის და უარყოფენ შეუსაბამობებს. `abi_version = "v1"`-ისთვის მოსალოდნელი მნიშვნელობაა `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

Contracts API (განლაგება)
- POST `/v1/contracts/deploy`
  - მოთხოვნა: { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - ქცევა: გამოთვლის `code_hash` IVM პროგრამის კორპუსიდან და `abi_hash` სათაურიდან `abi_version`, შემდეგ წარადგენს `RegisterSmartContractCode` (მანიფესტი) და I18081X00full `.to` ბაიტი) `authority`-ის სახელით.
  - პასუხი: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - დაკავშირებული:
    - GET `/v1/contracts/code/{code_hash}` → აბრუნებს შენახულ მანიფესტს
    - მიიღეთ `/v1/contracts/code-bytes/{code_hash}` → აბრუნებს `{ code_b64 }`
- POST `/v1/contracts/instance`
  - მოთხოვნა: { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - ქცევა: ავრცელებს მოწოდებულ ბაიტეკოდს და დაუყოვნებლივ ააქტიურებს `(namespace, contract_id)` რუკებს `ActivateContractInstance`-ის საშუალებით.
  - პასუხი: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Alias Service
- POST `/v1/aliases/voprf/evaluate`
  - მოთხოვნა: { "blinded_element_hex": "..." }
  - პასუხი: { "evaluated_element_hex": "…128hex", "backend": "blake2b512-mock" }
    - `backend` ასახავს შემფასებლის განხორციელებას. ამჟამინდელი ღირებულება: `blake2b512-mock`.
  - შენიშვნები: დეტერმინისტული იმიტირებული შემფასებელი, რომელიც იყენებს Blake2b512 დომენის `iroha.alias.voprf.mock.v1` გამოყოფით. განკუთვნილია სატესტო ხელსაწყოებისთვის, სანამ წარმოების VOPRF მილსადენი არ იქნება გაყვანილი Iroha-ით.
  - შეცდომები: HTTP `400` არასწორი თექვსმეტობით შეყვანაზე. Torii აბრუნებს Norito `ValidationFail::QueryFailed::Conversion` კონვერტს დეკოდერის შეცდომის შესახებ შეტყობინებით.
- POST `/v1/aliases/resolve`
  - მოთხოვნა: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - პასუხი: { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - შენიშვნები: მოითხოვს ISO ხიდის გაშვების დადგმას (`[iso_bridge.account_aliases]` `iroha_config`-ში). Torii ახდენს მეტსახელების ნორმალიზებას გამოკითხვამდე უფსკრულისა და ზედა რეგისტრის ამოღებით. აბრუნებს 404-ს, როდესაც მეტსახელი არ არის და 503, როდესაც ISO ხიდის მუშაობის დრო გამორთულია.
- POST `/v1/aliases/resolve_index`
  - მოთხოვნა: { "ინდექსი": 0 }
  - პასუხი: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }
  - შენიშვნები: Alias-ის ინდექსები ენიჭება დეტერმინისტულად კონფიგურაციის თანმიმდევრობიდან (0-ზე დაფუძნებული). კლიენტებს შეუძლიათ პასუხების ქეშირება ხაზგარეშე, რათა შექმნან აუდიტის ბილიკები ალიასის ატესტაციის ღონისძიებებისთვის.კოდის ზომის ქუდი
- მორგებული პარამეტრი: `max_contract_code_bytes` (JSON u64)
  - აკონტროლებს მაქსიმალურ დაშვებულ ზომას (ბაიტებში) ჯაჭვური კონტრაქტის კოდის შესანახად.
  - ნაგულისხმევი: 16 MiB. კვანძები უარყოფენ `RegisterSmartContractBytes`-ს, როდესაც `.to` გამოსახულების სიგრძე აღემატება ქუდი უცვლელი დარღვევის შეცდომით.
  - ოპერატორებს შეუძლიათ შეცვალონ `SetParameter(Custom)` `id = "max_contract_code_bytes"`-ით და ციფრული დატვირთვით.

- POST `/v1/gov/ballots/zk`
  - მოთხოვნა: { "ავტორიტეტი": "ih58...", "პირადი_გასაღები": "...?", "ჯაჭვის_id": "...", "election_id": "e1", "proof_b64": "...", "საჯარო": {…} }
  - პასუხი: { "ok": true, "accepted": true, "tx_instructions": [{…}] }
  - შენიშვნები:
    - როდესაც მიკროსქემის საჯარო შეყვანები მოიცავს `owner`, `amount` და `duration_blocks` და მტკიცებულება ამოწმებს კონფიგურირებულ VK-ს, კვანძი ქმნის ან აგრძელებს მართვის დაბლოკვას Sumeragi-ით Sumeragi-ით. მიმართულება რჩება დამალული (`unknown`), თუ მინიშნებული არ არის; განახლებულია მხოლოდ თანხა/ვადა. ხელახალი ხმები ერთფეროვანია: რაოდენობა და ვადის გასვლა მხოლოდ იზრდება (კვანძი ვრცელდება max(amount, prev.amount) და max(expiry, prev.expiry)).
    - როდესაც რაიმე მინიშნებაა დაბლოკვის შესახებ, ბიულეტენმა უნდა მიაწოდოს `owner`, `amount` და `duration_blocks`; ნაწილობრივი მინიშნებები უარყოფილია. როდესაც `min_bond_amount > 0`, დაბლოკვის მინიშნებებია საჭირო.
    - ZK ხელახალი ხმები, რომლებიც ცდილობენ თანხის შემცირებას ან ვადის გასვლას, უარყოფილია სერვერის მხრიდან `BallotRejected` დიაგნოსტიკით.
    - ხელშეკრულების შესასრულებლად უნდა დარეკოთ `ZK_VOTE_VERIFY_BALLOT` `SubmitBallot` რიგში დადგომამდე; მასპინძლები ახორციელებენ ერთი დარტყმის საკეტს.

- POST `/v1/gov/ballots/plain`
  - მოთხოვნა: { "ავტორიტეტი": "ih58...", "პირადი_გასაღები": "...?", "ჯაჭვის_id": "...", "რეფერენდუმის_იდ": "r1", "მფლობელი": "ih58...", "თანხა": "1000", "ხანგრძლივობის_ბლოკები": 6000, "Aye|Nay": "Aye"|Nay"
  - პასუხი: { "ok": true, "accepted": true, "tx_instructions": [{…}] }
  - შენიშვნები: ხელახალი კენჭისყრა მხოლოდ გახანგრძლივებულია - ახალ ბიულეტენს არ შეუძლია შეამციროს არსებული საკეტის ოდენობა ან ვადის გასვლა. `owner` უნდა უტოლდებოდეს გარიგების უფლებამოსილებას. მინიმალური ხანგრძლივობაა `conviction_step_blocks`.- POST `/v1/gov/finalize`
  - მოთხოვნა: { "referendum_id": "r1", "proposal_id": "…64hex", "autority": "ih58...?", "private_key": "...?" }
  - პასუხი: { "ok": true, "tx_instructions": [{ "wire_id": "…FinalizeReferendum", "payload_hex": "..." }] }
  - ჯაჭვზე ეფექტი (მიმდინარე ხარაჩო): დამტკიცებული განლაგების წინადადების ამოქმედება ჩასვამს მინიმალურ `ContractManifest`-ს, რომელიც ჩართულია `code_hash`-ით მოსალოდნელ `abi_hash`-თან და აღნიშნავს წინადადებას ამოქმედებულად. თუ მანიფესტი უკვე არსებობს `code_hash`-სთვის სხვა `abi_hash`-ით, ამოქმედება უარყოფილია.
  - შენიშვნები:
    - ZK არჩევნებისთვის, კონტრაქტის ბილიკები უნდა დარეკოთ `ZK_VOTE_VERIFY_TALLY` `FinalizeElection`-ის შესრულებამდე; მასპინძლები ახორციელებენ ერთი დარტყმის საკეტს. `FinalizeReferendum` უარყოფს ZK რეფერენდუმს არჩევნების დათვლის დასრულებამდე.
    - ავტომატური დახურვა `h_end`-ზე გამოსცემს დამტკიცებულია/უარყოფილი მხოლოდ მარტივი რეფერენდუმებისთვის; ZK რეფერენდმები დახურულია, სანამ საბოლოო ანგარიში არ იქნება წარდგენილი და `FinalizeReferendum` არ შესრულდება.
    - აქტივობის შემოწმება იყენებს მხოლოდ დამტკიცებას+უარს; თავი შეიკავოს აქტივობაზე.

- POST `/v1/gov/enact`
  - მოთხოვნა: { "proposal_id": "…64hex", "preimage_hash": "…64hex?", "window": { "ქვედა": 0, "ზედა": 0 }?, "authority": "ih58...?", "private_key": "...?" }
  - პასუხი: { "ok": true, "tx_instructions": [{ "wire_id": "…EnactReferendum", "payload_hex": "..." }] }
  - შენიშვნები: Torii წარადგენს ხელმოწერილ ტრანზაქციას `authority`/`private_key`; წინააღმდეგ შემთხვევაში, ის უბრუნებს ჩონჩხს კლიენტებისთვის, რომ ხელი მოაწერონ და წარადგინონ. პრეიმიჯი არჩევითია და ამჟამად საინფორმაციოა.

- მიიღეთ `/v1/gov/proposals/{id}`
  - გზა `{id}`: წინადადების ID თექვსმეტობითი (64 სიმბოლო)
  - პასუხი: { "აღმოაჩინეს": bool, "წინადადება": {… }? }

- მიიღეთ `/v1/gov/locks/{rid}`
  - გზა `{rid}`: რეფერენდუმის id სტრიქონი
  - პასუხი: { "აღმოაჩინეს": bool, "referendum_id": "rid", "locks": {… }? }

- მიიღეთ `/v1/gov/council/current`
  - პასუხი: { "ეპოქა": N, "წევრები": [{ "account_id": "..." }, ...] }
  - შენიშვნები: აბრუნებს შენარჩუნებულ საბჭოს, როდესაც ეს არის; სხვაგვარად გამოიმუშავებს დეტერმინისტულ სანაცვლოდ კონფიგურირებული ფსონის აქტივისა და ზღვრების გამოყენებით (ასახავს VRF სპეციფიკას, სანამ ცოცხალი VRF მტკიცებულებები არ დარჩება ჯაჭვზე).

- POST `/v1/gov/council/derive-vrf` (მახასიათებელი: gov_vrf)
  - მოთხოვნა: { "კომიტეტის_ზომა": 21, "ეპოქა": 123? , "კანდიდატები": [{ "account_id": "...", "ვარიანტი": "ნორმალური|პატარა", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - ქცევა: ამოწმებს თითოეული კანდიდატის VRF მტკიცებულებას `chain_id`, `epoch`-დან და უახლესი ბლოკის ჰეშის შუქიდან მიღებული კანონიკური შეყვანის მიმართ; დახარისხება გამომავალი ბაიტების მიხედვით დაღმავალი ტაიბრეიკერებით; აბრუნებს `committee_size` საუკეთესო წევრებს. არ გრძელდება.
  - პასუხი: { "ეპოქა": N, "წევრები": [{ "account_id": "..." } …], "total_candidates": M, "დამოწმებული": K }
  - შენიშვნები: ნორმალური = pk G1-ში, მტკიცებულება G2-ში (96 ბაიტი). მცირე = pk G2-ში, მტკიცებულება G1-ში (48 ბაიტი). შეყვანები გამოყოფილია დომენით და მოიცავს `chain_id`.

### მართვის ნაგულისხმევი (iroha_config `gov.*`)

Torii-ის მიერ გამოყენებული საბჭოს სარეზერვო სისტემა, როდესაც მუდმივი სია არ არსებობს, პარამეტრიზებულია `iroha_config`-ით:```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  voting_asset_id = "xor#sora"         # governance bond asset (Sora Nexus default)
  min_bond_amount = 150                # smallest units of voting_asset_id
  bond_escrow_account = "ih58..."
  slash_receiver_account = "ih58..."
  slash_double_vote_bps = 0            # percentage (basis points) to slash on double-vote attempts
  slash_invalid_proof_bps = 0          # percentage (basis points) to slash on invalid ballot proofs
  slash_ineligible_proof_bps = 0       # percentage (basis points) to slash on stale/invalid eligibility proofs
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

ექვივალენტური გარემო უგულებელყოფს:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_VOTING_ASSET_ID=xor#sora
GOV_MIN_BOND_AMOUNT=150
GOV_BOND_ESCROW_ACCOUNT=ih58...
GOV_SLASH_RECEIVER_ACCOUNT=ih58...
GOV_SLASH_DOUBLE_VOTE_BPS=2500
GOV_SLASH_INVALID_PROOF_BPS=5000
GOV_SLASH_INELIGIBLE_PROOF_BPS=1500
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

Sora Nexus ნაგულისხმევი: ბიულეტენები იკეტება `min_bond_amount` of `voting_asset_id`-ში
კონფიგურირებული ესქრო ანგარიში. საკეტები იქმნება ან გაფართოვდება, როდესაც ბიულეტენები ჩამოდის და
გამოშვებულია ვადის გასვლისას; ობლიგაციების სასიცოცხლო ციკლი ემიტირებულია `governance_bond_events_total`-ის საშუალებით
ტელემეტრია (ჩაკეტვა_შექმნილი|ჩაკეტვა_გაგრძელებული|ჩაკეტვა_განბლოკილია|ჩაკეტვა_დაჭრილი|ჩაკეტვა_აღდგენილია).

`parliament_committee_size` ზღუდავს დაბრუნებული წევრების რაოდენობას, როდესაც საბჭოს არ არსებობს, `parliament_term_blocks` განსაზღვრავს ეპოქის სიგრძეს, რომელიც გამოიყენება სათესლე დერივაციისთვის (`epoch = floor(height / term_blocks)`), `parliament_min_stake` ახორციელებს მინიმალურ ერთეულების ფსონს `parliament_eligibility_asset_id` ირჩევს რომელი აქტივების ბალანსი დასკანირდება კანდიდატის ნაკრების შექმნისას.

Governance VK ვერიფიკაციას არ აქვს შემოვლითი გზა: კენჭისყრის გადამოწმებას ყოველთვის სჭირდება `Active` დამადასტურებელი გასაღები ჩასმული ბაიტით, ხოლო გარემო არ უნდა დაეყრდნოს მხოლოდ ტესტის გადამრთველებს გადამოწმების გამოტოვებისთვის.

RBAC
- ჯაჭვზე შესრულება მოითხოვს ნებართვებს:
  - წინადადებები: `CanProposeContractDeployment{ contract_id }`
  - ბიულეტენი: `CanSubmitGovernanceBallot{ referendum_id }`
  - ამოქმედება: `CanEnactGovernance`
  - შემცირება/აპელირება: `CanSlashGovernanceLock{ referendum_id }`, `CanRestituteGovernanceLock{ referendum_id }`
  - საბჭოს მართვა (მომავალი): `CanManageParliament`
- შეჭრა/აპელირება:
  - ორმაგი კენჭისყრა/ბათილი/არდაშვებული კენჭისყრა ვრცელდება კონფიგურირებულ წევის პროცენტებზე ობლიგაციების ესქროსთან მიმართებაში, თანხების გადატანაში `slash_receiver_account`-ში, განახლებულია შემცირებული წიგნი და ავრცელებს აკრეფილ `LockSlashed` მოვლენებს (მიზეზი + დანიშნულება + შენიშვნა).
  - სახელმძღვანელო `SlashGovernanceLock`/`RestituteGovernanceLock` ინსტრუქციები მხარს უჭერს ოპერატორის მიერ დაწესებულ ჯარიმებსა და გასაჩივრებებს; რესტიტუცია შემოიფარგლება ჩაწერილი ხაზებით, აღადგენს სახსრებს ობლიგაციების ესქროში, განაახლებს წიგნს და ასხივებს `LockRestituted` დაბლოკვის აქტიურობას ვადის გასვლამდე.დაცული სახელების სივრცეები
- მორგებული პარამეტრი `gov_protected_namespaces` (სტრიქონების JSON მასივი) იძლევა დაშვების კარიბჭეს ჩამოთვლილ სახელთა სივრცეებში განლაგებისთვის.
- კლიენტებმა უნდა შეიცავდეს ტრანზაქციის მეტამონაცემების კლავიშებს დაცულ სახელთა სივრცის მიზნობრივი განლაგებისთვის:
  - `gov_namespace`: სამიზნე სახელების სივრცე (მაგ., `"apps"`)
  - `gov_contract_id`: ლოგიკური კონტრაქტის ID სახელთა სივრცეში
- `gov_manifest_approvers`: არასავალდებულო JSON მასივი ih58... ანგარიშის ID. როდესაც ხაზის მანიფესტი აცხადებს ერთზე მეტ კვორუმს, დაშვება მოითხოვს ტრანზაქციის ორგანოს პლუს ჩამოთვლილ ანგარიშებს, რათა დაკმაყოფილდეს მანიფესტი კვორუმი.
- Telemetry exposes holistic admission counters via `governance_manifest_admission_total{result}` so operators can distinguish successful admits from `missing_manifest`, `non_ih58..._authority`, `quorum_rejected`, `protected_namespace_rejected`, and `runtime_hook_rejected` paths.
- ტელემეტრია ასახავს აღსრულების გზას `governance_manifest_quorum_total{outcome}`-ის მეშვეობით (მნიშვნელობები `satisfied` / `rejected`), რათა ოპერატორებმა შეძლონ გამოტოვებული ნებართვების შემოწმება.
- ბილიკები ახორციელებენ მათ მანიფესტებში გამოქვეყნებულ სახელთა სივრცის ნებადართულ სიას. ნებისმიერი ტრანზაქცია, რომელიც ადგენს `gov_namespace`-ს, უნდა უზრუნველყოს `gov_contract_id`, ხოლო სახელთა სივრცე უნდა გამოჩნდეს manifest-ის `protected_namespaces` ნაკრებში. `RegisterSmartContractCode` წარდგენილები ამ მეტამონაცემების გარეშე უარყოფილია, როდესაც დაცვა ჩართულია.
- დაშვება აიძულებს, რომ ამოქმედებული მმართველობის წინადადება არსებობს tuple `(namespace, contract_id, code_hash, abi_hash)`-ისთვის; წინააღმდეგ შემთხვევაში ვალიდაცია ვერ მოხერხდება NotPermitted შეცდომით.

Runtime Upgrade Hooks
- Lane manifests-მა შეიძლება გამოაცხადოს `hooks.runtime_upgrade` კარიბჭის მუშაობის დროის განახლების ინსტრუქციებზე (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- კაუჭის ველები:
  - `allow` (bool, ნაგულისხმევი `true`): როდესაც `false`, გაშვების დროის განახლების ყველა ინსტრუქცია უარყოფილია.
  - `require_metadata` (bool, ნაგულისხმევი `false`): მოითხოვს ტრანზაქციის მეტამონაცემების ჩანაწერს, რომელიც მითითებულია `metadata_key`-ით.
  - `metadata_key` (სტრიქონი): მეტამონაცემების სახელი აღსრულებულია კაუჭით. ნაგულისხმევია `gov_upgrade_id`, როდესაც საჭიროა მეტამონაცემები ან არსებობს ნებადართული სია.
  - `allowed_ids` (სტრიქონების მასივი): მეტამონაცემების მნიშვნელობების არასავალდებულო დასაშვები სია (ჩაჭრის შემდეგ). უარყოფს, როდესაც მითითებული მნიშვნელობა არ არის ჩამოთვლილი.
- როდესაც კაუჭი არსებობს, რიგის დაშვება ახორციელებს მეტამონაცემების პოლიტიკას, სანამ ტრანზაქცია შევა რიგში. გამოტოვებული მეტამონაცემები, ცარიელი მნიშვნელობები ან ნებადართული სიის გარეთ არსებული მნიშვნელობები წარმოშობს დეტერმინისტულ `NotPermitted` შეცდომას.
- ტელემეტრია აკონტროლებს აღსრულების შედეგებს `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`-ის საშუალებით.
- ტრანზაქციები, რომლებიც აკმაყოფილებენ Hook-ს, უნდა შეიცავდეს მეტამონაცემებს `gov_upgrade_id=<value>` (ან მანიფესტის მიერ განსაზღვრულ კლავიშს) მანიფესტის კვორუმით მოთხოვნილ ნებისმიერ ih58... დამტკიცებასთან ერთად.

მოხერხებულობის საბოლოო წერტილი
- POST `/v1/gov/protected-namespaces` — ვრცელდება `gov_protected_namespaces` პირდაპირ კვანძზე.
  - მოთხოვნა: { "სახელთა სივრცეები": ["აპები", "სისტემა"] }
  - პასუხი: { "ok": მართალია, "გამოყენებული": 1 }
  - შენიშვნები: განკუთვნილია ადმინ/ტესტირებისთვის; საჭიროებს API ჟეტონს, თუ კონფიგურირებულია. წარმოებისთვის, ამჯობინეთ ხელმოწერილი ტრანზაქციის წარდგენა `SetParameter(Custom)`-ით.CLI დამხმარეები
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - იღებს კონტრაქტის შემთხვევებს სახელთა სივრცისთვის და ამოწმებს, რომ:
    - Torii ინახავს ბაიტიკოდს თითოეული `code_hash`-ისთვის და მისი Blake2b-32 დაიჯესტი ემთხვევა `code_hash`-ს.
    - `/v1/contracts/code/{code_hash}` ქვეშ შენახული მანიფესტი იტყობინება, რომ შეესაბამება `code_hash` და `abi_hash` მნიშვნელობებს.
    - `(namespace, contract_id, code_hash, abi_hash)`-ისთვის არსებობს ამოქმედებული მმართველობის წინადადება, რომელიც მიღებულია იმავე წინადადების ID-ის ჰეშირებით, რომელსაც იყენებს კვანძი.
  - გამოსცემს JSON ანგარიშს `results[]`-ით თითო კონტრაქტზე (გამოცემები, მანიფესტი/კოდი/წინადადების რეზიუმეები) პლუს ერთსტრიქონიანი შეჯამება, თუ არ არის ჩახშობილი (`--no-summary`).
  - სასარგებლოა დაცული სახელების სივრცის აუდიტის შესამოწმებლად ან მმართველობის მიერ კონტროლირებადი განლაგების სამუშაო ნაკადების შესამოწმებლად.
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - გამოსცემს JSON მეტამონაცემების ჩონჩხს, რომელიც გამოიყენება დაცულ სახელთა სივრცეში განლაგების გაგზავნისას, მათ შორის არასავალდებულო `gov_manifest_approvers`, მანიფესტ კვორუმის წესების დასაკმაყოფილებლად.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - ამოწმებს კანონიკური ანგარიშის id-ებს, ახდენს 32-ბაიტიანი ბათილის გაუქმების მინიშნებებს და აერთიანებს მინიშნებებს `public_inputs_json`-ში (`--public <path>`-ით დამატებითი გადაფარვისთვის).
  - გაუქმება მიღებულია მტკიცებულების ვალდებულებიდან (საჯარო შეყვანა) პლუს `domain_tag`, `chain_id` და `election_id`; `--nullifier` დამოწმებულია მტკიცებულების საწინააღმდეგოდ, როდესაც მიწოდებულია.
  - ერთი სტრიქონიანი შეჯამება ახლა ასახავს დეტერმინისტულ `fingerprint=<hex>`-ს, რომელიც მიღებულია კოდირებული `CastZkBallot`-დან, ნებისმიერ დეკოდირებულ მინიშნებებთან ერთად (`owner`, `amount`, Sumeragi, როდესაც მოწოდებულია Sumeragi).
  - CLI პასუხები ანოტირებს `tx_instructions[]` `payload_fingerprint_hex` პლუს დეკოდირებულ ველებთან, ასე რომ, ქვედა დინებაში ინსტრუმენტების გამოყენებამ შეიძლება შეამოწმოს ჩონჩხი Norito დეკოდირების ხელახალი განხორციელების გარეშე.
  - როდესაც მოწოდებულია რაიმე დაბლოკვის მინიშნება, ZK ბიულეტენებმა უნდა მიაწოდონ `owner`, `amount` და `duration_blocks`; ნაწილობრივი მინიშნებები უარყოფილია. როდესაც `min_bond_amount > 0`, დაბლოკვის მინიშნებებია საჭირო. მიმართულება რჩება არასავალდებულო და განიხილება, როგორც მხოლოდ მინიშნება.
- `iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` იღებს კანონიკურ IH58 ლიტერალებს; არჩევითი `@<domain>` სუფიქსები მხოლოდ მარშრუტიზაციის მინიშნებებია.
  - მეტსახელები `--lock-amount`/`--lock-duration-blocks` ასახავს ZK დროშის სახელებს სკრიპტის პარიტეტისთვის.
  - შემაჯამებელი გამომავალი სარკეები ასახავს `vote --mode zk` დაშიფრული ინსტრუქციის თითის ანაბეჭდის და ადამიანის მიერ წასაკითხი საარჩევნო ბიულეტენების ჩათვლით (`owner`, `amount`, `duration_blocks`, Norito, ხელმოწერის დადასტურებამდე Prometheus).შემთხვევების ჩამონათვალი
- GET `/v1/gov/instances/{ns}` — ჩამოთვლის აქტიურ კონტრაქტებს სახელთა სივრცისთვის.
  - შეკითხვის პარამეტრები:
    - `contains`: ფილტრი `contract_id`-ის ქვესტრიქონებით (მგრძნობიარე რეგისტრის მიმართ)
    - `hash_prefix`: ფილტრი `code_hash_hex`-ის ექვსკუთხა პრეფიქსით (პატარა)
    - `offset` (ნაგულისხმევი 0), `limit` (ნაგულისხმევი 100, მაქსიმუმ 10_000)
    - `order`: ერთი `cid_asc` (ნაგულისხმევი), `cid_desc`, `hash_asc`, `hash_desc`
  - პასუხი: { "namespace": "ns", "instance": [{ "contract_id": "...", "code_hash_hex": "..." }, …], "total": N, "offset": n, "limit": m }
  - SDK დამხმარე: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ან `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

განბლოკვა Sweep (ოპერატორი/აუდიტი)
- მიიღეთ `/v1/gov/unlocks/stats`
  - პასუხი: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - შენიშვნები: `last_sweep_height` ასახავს ბლოკის უახლეს სიმაღლეს, სადაც ვადაგასული საკეტები იყო გაწმენდილი და შენარჩუნებული. `expired_locks_now` გამოითვლება დაბლოკვის ჩანაწერების სკანირებით `expiry_height <= height_current`-ით.
- POST `/v1/gov/ballots/zk-v1`
  - მოთხოვნა (v1 სტილის DTO):
    {
      "ავტორიტეტი": "ih58...",
      "chain_id": "00000000-0000-0000-0000-0000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64hex?",
      "owner": "ih58…", // კანონიკური AccountId (IH58 ლიტერალური; სურვილისამებრ @domain მინიშნება)
      "თანხა": "100?",
      "ხანგრძლივობა_ბლოკები": 6000?,
      "მიმართულება": "კი|არა|თავი შეიკავო?",
      "Nullifier": "blake2b32:…64hex?"
    }
  - პასუხი: { "ok": true, "accepted": true, "tx_instructions": [{…}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (მახასიათებელი: `zk-ballot`)
  - პირდაპირ იღებს `BallotProof` JSON-ს და აბრუნებს `CastZkBallot` ჩონჩხს.
  - მოთხოვნა:
    {
      "ავტორიტეტი": "ih58...",
      "chain_id": "00000000-0000-0000-0000-0000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "კენჭისყრა": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 ZK1 ან H2* კონტეინერი
        "root_hint": null, // სურვილისამებრ 32-ბაიტიანი თექვსმეტობითი სტრიქონი (შესაბამისობის ფესვი)
        "მფლობელი": null, // არასავალდებულო კანონიკური AccountId (IH58 ლიტერალური; სურვილისამებრ @domain მინიშნება)
        "Nullifier": null, // სურვილისამებრ 32-ბაიტიანი თექვსმეტობითი სტრიქონი (გაუქმების მინიშნება)
        "amount": "100", // სურვილისამებრ დაბლოკვის თანხის მინიშნება (ათწილადი სტრიქონი)
        "duration_blocks": 6000, // სურვილისამებრ დაბლოკვის ხანგრძლივობის მინიშნება
        "direction": "Aye" // სურვილისამებრ მიმართულების მინიშნება
      }
    }
  - პასუხი:
    {
      "ok": მართალია,
      "მიღებული": მართალია,
      "მიზეზი": "ტრანზაქციის ჩონჩხის აშენება",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - შენიშვნები:
    - როდესაც მოწოდებულია `private_key`, Torii წარადგენს ხელმოწერილ ტრანზაქციას და აყენებს `reason`-ს `submitted transaction`-ზე.
    - სერვერი ასახავს არასავალდებულო `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier`/`nullifier` კენჭისყრიდან I0882000X-დან I01-მდე. `CastZkBallot`.
    - კონვერტის ბაიტი ხელახლა დაშიფრულია, როგორც base64 ინსტრუქციის სასარგებლო დატვირთვისთვის.
    - ეს საბოლოო წერტილი ხელმისაწვდომია მხოლოდ მაშინ, როდესაც ჩართულია `zk-ballot` ფუნქცია.

CastZkBallot-ის გადამოწმების გზა
- `CastZkBallot` დეკოდირებს მოწოდებულ base64 მტკიცებულებას და უარყოფს ცარიელ ან არასწორი ფორმის დატვირთვას (`BallotRejected` `invalid or empty proof`-თან ერთად).
- თუ მოწოდებულია `public_inputs_json`, ის უნდა იყოს JSON ობიექტი; არაობიექტური დატვირთვა უარყოფილია.
- მასპინძელი წყვეტს კენჭისყრის გადამოწმების გასაღებს რეფერენდუმიდან (`vk_ballot`) ან მმართველობით ნაგულისხმევად და მოითხოვს, რომ ჩანაწერი არსებობდეს, იყოს `Active` და შეიცავდეს შიდა ბაიტებს.
- შენახული დამადასტურებელი გასაღების ბაიტი ხელახლა ჰეშირებულია `hash_vk`-ით; ნებისმიერი ვალდებულების შეუსაბამობა წყვეტს შესრულებას ვერიფიკაციამდე, რათა დაიცვას რეესტრის გაყალბებული ჩანაწერები (`BallotRejected` `verifying key commitment mismatch`-თან ერთად).
- მტკიცებულების ბაიტები იგზავნება რეგისტრირებულ ბექენდში `zk::verify_backend`-ის საშუალებით; არასწორი ტრანსკრიპტები გამოჩნდება, როგორც `BallotRejected` `invalid proof`-ით და ინსტრუქცია დეტერმინისტულად ვერ ხერხდება.
- მტკიცებულებამ უნდა გამოავლინოს საარჩევნო ბიულეტენის ვალდებულება და უფლებამოსილების საფუძველი, როგორც საჯარო ინფორმაცია; ფესვი უნდა ემთხვეოდეს არჩევნების `eligible_root`-ს, ხოლო მიღებული გაუქმება უნდა ემთხვეოდეს ნებისმიერ მითითებას.
- წარმატებული მტკიცებულებები ასხივებენ `BallotAccepted`; გაუქმების დუბლიკატები, მოძველებული დასაშვებობის ფესვები ან დაბლოკვის რეგრესია აგრძელებს ამ დოკუმენტში ზემოთ აღწერილი უარყოფის არსებული მიზეზების გამომუშავებას.

## Validator არასწორი ქცევა და ერთობლივი კონსენსუსი

### Slashing და Jailing Workflowკონსენსუსი ასხივებს Norito-ში კოდირებულ `Evidence`-ს, როდესაც ih58... არღვევს პროტოკოლს. თითოეული ტვირთამწეობა მეხსიერების `EvidenceStore`-ში და, თუ არ ჩანს, მატერიალიზდება WSV-ით მხარდაჭერილ `consensus_evidence` რუკაში. `sumeragi.npos.reconfig.evidence_horizon_blocks`-ზე ძველი ჩანაწერები (ნაგულისხმევი `7200` ბლოკები) უარყოფილია, ასე რომ არქივი რჩება შეზღუდული, მაგრამ უარყოფა დარეგისტრირებულია ოპერატორებისთვის. ჰორიზონტში არსებული მტკიცებულებები ემორჩილება ერთობლივი კონსენსუსის დადგმის წესს (`mode_activation_height requires next_mode to be set in the same block`), აქტივაციის დაყოვნებას (`sumeragi.npos.reconfig.activation_lag_blocks`, ნაგულისხმევი `1`) და შემცირების დაყოვნებას (Prometheus, Prometheus, ნაგულისხმევ01) სანამ ისინი მიმართავენ.

აღიარებული დანაშაულები ერთი-ერთზე ასახულია `EvidenceKind`-ზე; დისკრიმინანტები სტაბილურია და აღსრულებულია მონაცემთა მოდელის მიხედვით:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** — ih58... ხელმოწერილი აქვს კონფლიქტურ ჰეშებს იგივე `(phase,height,view,epoch)` ტოპლისთვის.
- **InvalidQc** — აგრეგატორმა ჭორაობდა commit QC, რომლის ფორმა ვერ ამოწმებს დეტერმინისტულ შემოწმებას (მაგ., ხელმომწერის ცარიელი ბიტმაპი).
- **InvalidProposal** — ლიდერმა შემოგვთავაზა ბლოკი, რომელიც ვერ ახერხებს სტრუქტურულ ვალიდაციას (მაგ., არღვევს ჩაკეტილი ჯაჭვის წესს).
- **ცენზურა** — ხელმოწერილი წარდგენის ქვითრები აჩვენებს ტრანზაქციას, რომელიც არასდროს ყოფილა შემოთავაზებული/ჩადენილი.

VRF ჯარიმები ავტომატურად აღსრულდება `activation_lag_blocks`-ის შემდეგ (დამნაშავეები ციხეში არიან). კონსენსუსის შემცირება გამოიყენება მხოლოდ `slashing_delay_blocks` ფანჯრის შემდეგ, თუ მმართველობა არ გააუქმებს ჯარიმას.

ოპერატორებს და ხელსაწყოებს შეუძლიათ შეამოწმონ და ხელახლა გაავრცელონ დატვირთვა:

- Torii: `GET /v1/sumeragi/evidence` და `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `… count` და `… submit --evidence-hex <payload>`.

მმართველობამ უნდა განიხილოს მტკიცებულებათა ბაიტები, როგორც კანონიკური მტკიცებულება:

1. **შეაგროვეთ ტვირთი**, სანამ ის დაძველდება. დაარქივეთ დაუმუშავებელი Norito ბაიტი სიმაღლის/ნახვის მეტამონაცემებთან ერთად.
2. ** აუცილებლობის შემთხვევაში გააუქმეთ ** `CancelConsensusEvidencePenalty` წარდგენით მტკიცებულებათა დატვირთვით `slashing_delay_blocks` გასვლამდე; ჩანაწერი მონიშნულია `penalty_cancelled` და `penalty_cancelled_at_height`, და არ ვრცელდება დაჭრა.
3. **საჯარიმოს დადგმა** რეფერენდუმში ან სუდო ინსტრუქციებში სასარგებლო დატვირთვის ჩასმით (მაგ., `Unregister::peer`). Execution ხელახლა ამოწმებს დატვირთვას; არასწორი და ძველი მტკიცებულებები უარყოფილია დეტერმინისტულად.
4. **დაგეგმეთ შემდგომი ტოპოლოგია**, რათა შეურაცხმყოფელი ih58... დაუყოვნებლივ არ შეუერთდეს. ტიპიური ნაკადების რიგი `SetParameter(Sumeragi::NextMode)` და `SetParameter(Sumeragi::ModeActivationHeight)` განახლებული ჩამონათვალით.
5. **აუდიტის შედეგები** `/v1/sumeragi/evidence`-ის და `/v1/sumeragi/status`-ის მეშვეობით, რათა უზრუნველყოფილი იყოს მტკიცებულებების კონტრშეტევა და მმართველობამ მიიღო ამოღება.

### ერთობლივი კონსენსუსის თანმიმდევრობა

ერთობლივი კონსენსუსი იძლევა გარანტიას, რომ გამავალი ih58... ნაკრები დაასრულებს სასაზღვრო ბლოკს, სანამ ახალი ნაკრები დაიწყებს შეთავაზებას. გაშვების დრო ახორციელებს წესს დაწყვილებული პარამეტრების მეშვეობით:- `SumeragiParameter::NextMode` და `SumeragiParameter::ModeActivationHeight` უნდა იყოს ჩადენილი **იგივე ბლოკში**. `mode_activation_height` უნდა იყოს მკაცრად აღემატება ბლოკის სიმაღლეს, რომელმაც განახლდა, ​​რაც უზრუნველყოფს მინიმუმ ერთი ბლოკის ჩამორჩენას.
- `sumeragi.npos.reconfig.activation_lag_blocks` (ნაგულისხმევი `1`) არის კონფიგურაციის დამცავი, რომელიც ხელს უშლის ნულოვანი ჩამორჩენის ხელიდან გაშვებას:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (ნაგულისხმევი `259200`) აჭიანურებს კონსენსუსის შემცირებას, რათა მმართველობამ გააუქმოს ჯარიმები მათ გამოყენებამდე.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- გაშვების დრო და CLI ავლენს ეტაპობრივ პარამეტრებს `/v1/sumeragi/params` და `iroha sumeragi params --summary`-ის მეშვეობით, ასე რომ ოპერატორებს შეუძლიათ დაადასტურონ აქტივაციის სიმაღლეები და ih58... სია.
- მმართველობის ავტომატიზაცია ყოველთვის უნდა:
  1. დაასრულეთ მტკიცებულებებით დამყარებული ამოღების (ან აღდგენის) გადაწყვეტილება.
  2. რიგის შემდგომი ხელახალი კონფიგურაცია `mode_activation_height = h_current + activation_lag_blocks`-ით.
  3. მონიტორი `/v1/sumeragi/status` სანამ `effective_consensus_mode` არ ამოტრიალდება მოსალოდნელ სიმაღლეზე.

ნებისმიერი სკრიპტი, რომელიც ბრუნავს ih58...s-ს ან მიმართავს სლეშს **არ** უნდა ეცადოს ნულოვანი ჩამორჩენის აქტივაციას ან გამოტოვოს ხელის გამორთვის პარამეტრები; ასეთი ტრანზაქციები უარყოფილია და ქსელი ტოვებს წინა რეჟიმში.

## ტელემეტრიული ზედაპირები

- Prometheus მეტრიკის ექსპორტის მართვის აქტივობა:
  - `governance_proposals_status{status}` (ლიანდაგი) ადევნებს თვალს წინადადებების რაოდენობას სტატუსის მიხედვით.
  - `governance_protected_namespace_total{outcome}` (მრიცხველი) იზრდება, როდესაც დაცული სახელთა სივრცის დაშვება იძლევა ან უარყოფს განთავსებას.
  - `governance_manifest_activations_total{event}` (მრიცხველი) აღრიცხავს მანიფესტის ჩასმას (`event="manifest_inserted"`) და სახელთა სივრცის შეკვრას (`event="instance_bound"`).
- `/status` მოიცავს `governance` ობიექტს, რომელიც ასახავს წინადადებების რაოდენობას, აცნობებს დაცული სახელთა სივრცის ჯამებს და ასახელებს ბოლო მანიფესტების აქტივაციას (სახელთა სივრცე, კონტრაქტის ID, კოდი/ABI ჰეში, ბლოკის სიმაღლე, აქტივაციის დროის ნიშა). ოპერატორებს შეუძლიათ ამ ველის გამოკითხვა დაადასტურონ, რომ ამოქმედების განახლებული მანიფესტები და დაცული სახელთა სივრცის კარიბჭეები შესრულებულია.
- Grafana შაბლონი (`docs/source/grafana_governance_constraints.json`) და
  ტელემეტრიის რვეული `telemetry.md`-ში გვიჩვენებს, თუ როგორ უნდა დააკავშიროთ სიგნალიზაცია გაჭედილისთვის
  წინადადებები, გამოტოვებული მანიფესტის აქტივაციები ან მოულოდნელი დაცული სახელების სივრცე
  უარყოფა გაშვების განახლების დროს.
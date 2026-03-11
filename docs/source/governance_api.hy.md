---
lang: hy
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

Կարգավիճակ. նախագիծ/ուրվագիծ՝ կառավարման իրականացման առաջադրանքներին ուղեկցելու համար: Իրականացման ընթացքում ձևերը կարող են փոխվել: Դետերմինիզմը և RBAC քաղաքականությունը նորմատիվ սահմանափակումներ են. Torii-ը կարող է ստորագրել/ներկայացնել գործարքներ, երբ տրամադրվում են `authority` և `private_key`, հակառակ դեպքում հաճախորդները կառուցում և ներկայացնում են `/transaction`:

Կարևոր է. մենք չենք ուղարկում մշտական ​​խորհուրդ կամ «կանխադրված» կառավարման ցուցակ: Վանդակից դուրս, խորհրդի վերջնակետերը կա՛մ վերադարձնում են դատարկ/սպասող վիճակ, կա՛մ բխում են որոշիչ հետադարձ կապ կազմաձևված պարամետրերից (ցցի ակտիվ, ժամկետ, կոմիտեի չափ), երբ միացված է: Օպերատորները պետք է պահպանեն իրենց սեփական ցուցակը կառավարման հոսքերի միջոցով. այս պահոցում չկա բացված բազմանշանակ, գաղտնի բանալի կամ խորհրդի արտոնյալ հաշիվ:

Ընդհանուր ակնարկ
- Բոլոր վերջնակետերը վերադարձնում են JSON: Գործարքներ արտադրող հոսքերի համար պատասխանները ներառում են `tx_instructions`՝ մեկ կամ մի քանի հրահանգների կմախքների զանգված.
  - `wire_id`. ռեեստրի նույնացուցիչ հրահանգի տեսակի համար
  - `payload_hex`: Norito օգտակար բեռնված բայթ (վեցանկյուն)
- Եթե տրամադրվում են `authority` և `private_key` (կամ `private_key` քվեաթերթիկների DTO-ներում), Torii ստորագրում և ներկայացնում է գործարքը և դեռ վերադարձնում է `tx_instructions`:
- Հակառակ դեպքում, հաճախորդները հավաքում են SignedTransaction՝ օգտագործելով իրենց հեղինակությունը և chain_id-ը, այնուհետև ստորագրում և ՓՈՍՏՈՒՄ `/transaction`-ին:
- SDK ծածկույթ.
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed`-ը վերադարձնում է `GovernanceProposalResult` (կարգավիճակի/տեսակի դաշտերի նորմալացում), `ToriiClient.get_governance_referendum_typed`-ը վերադարձնում է `GovernanceReferendumResult`, Prometheus վերադարձնում է `GovernanceReferendumResult`, Prometheus վերադարձնում է `GovernanceReferendumResult`, Prometheus `ToriiClient.get_governance_locks_typed`-ը վերադարձնում է `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed`-ը վերադարձնում է `GovernanceUnlockStats`, իսկ `ToriiClient.list_governance_instances_typed`-ը վերադարձնում է `GovernanceInstancesPage`՝ կիրառելով մուտքագրված մակերևույթի մուտքը README-ում:
- Python թեթև հաճախորդ (`iroha_torii_client`). Վերջնականացնել/գործարկել հոսքերը:
- JavaScript (`@iroha/iroha-js`). `ToriiClient`-ը ներկայացնում է տպագրված օգնականներ առաջարկների, հանրաքվեների, թվերի, կողպեքների, ապակողպման վիճակագրության համար և այժմ `listGovernanceInstances(namespace, options)` գումարած խորհրդի վերջնական կետերը (`getGovernanceCouncilCurrent`, Prometheus `governancePersistCouncil`, `getGovernanceCouncilAudit`), այնպես որ Node.js-ի հաճախորդները կարող են էջադրել `/v1/gov/instances/{ns}` և վարել VRF-ով ապահովված աշխատանքային հոսքեր առկա պայմանագրային օրինակների ցանկի հետ մեկտեղ: `governanceFinalizeReferendumTyped`-ը և `governanceEnactProposalTyped`-ը արտացոլում են Python-ի օգնականները՝ միշտ վերադարձնելով կառուցվածքային սևագիր (սինթեզում է դատարկ կմախքը, երբ Torii-ը պատասխանում է `204 No Content`-ով), ինչը թույլ չի տալիս ավտոմատացմանը ճյուղավորել80queu0X00001-ից առաջ: ձգանիչներ. `getGovernanceLocksTyped`-ն այժմ նորմալացնում է `404 Not Found` պատասխանները `{found: false, locks: {}, referendum_id: <id>}`-ի մեջ, որպեսզի JS զանգահարողները ստանան նույն ձևի արդյունքը, ինչ Python-ի օգնականը, երբ հանրաքվեն չունի կողպեքներ:

Վերջնակետեր- ՓՈՍՏ `/v1/gov/proposals/deploy-contract`
  - Հայց (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      «code_hash»: «blake2b32:…» | «…64 hex»,
      «abi_hash»: «blake2b32:…» | «…64 hex»,
      "abi_version": "1",
      «պատուհան»: { «ստորին»: 12345, «վերին»՝ 12400 },
      «հեղինակություն»: «i105...?»,
      "private_key": "...?"
    }
  - Պատասխան (JSON):
    { «ok»: true, «proposal_id»: «…64hex», «tx_instructions»: [{ «wire_id»: «…», «payload_hex»: «…» }] }
  - Վավերացում. հանգույցները կանոնականացնում են `abi_hash`-ը տրամադրված `abi_version`-ի համար և մերժում անհամապատասխանությունները: `abi_version = "v1"`-ի համար ակնկալվող արժեքը `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` է:

Contracts API (տեղակայում)
- ՓՈՍՏ `/v1/contracts/deploy`
  - Հարցում. { "authority": "i105...", "private_key": "…", "code_b64": "..." }
  - Վարքագիծ. `code_hash`-ը հաշվարկում է IVM ծրագրի մարմնից և `abi_hash`-ը՝ `abi_version` վերնագրից, այնուհետև ներկայացնում է `RegisterSmartContractCode` (մանիֆեստ) և I18084 `.to` բայթ) `authority`-ի անունից:
  - Պատասխան՝ { «ok»: ճշմարիտ, «code_hash_hex»: «…», «abi_hash_hex»: «…» }
  - Առնչվող:
    - GET `/v1/contracts/code/{code_hash}` → վերադարձնում է պահված մանիֆեստը
    - GET `/v1/contracts/code-bytes/{code_hash}` → վերադարձնում է `{ code_b64 }`
- ՓՈՍՏ `/v1/contracts/instance`
  - Հարցում. { "authority": "i105...", "private_key": "…", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Վարքագիծ. տեղակայում է մատակարարված բայթ կոդը և անմիջապես ակտիվացնում `(namespace, contract_id)` քարտեզագրումը `ActivateContractInstance`-ի միջոցով:
  - Պատասխան՝ { «ok»: true, «namespace»: «apps», «contract_id»: «calc.v1», «code_hash_hex»: «…», «abi_hash_hex»: «…» }

Alias Service
- ՓՈՍՏ `/v1/aliases/voprf/evaluate`
  - Հարցում. { "blinded_element_hex": "..." }
  - Պատասխան՝ { "evaluated_element_hex": "…128hex", "backend": "blake2b512-mock" }
    - `backend`-ն արտացոլում է գնահատողի իրականացումը: Ընթացիկ արժեքը՝ `blake2b512-mock`:
  - Ծանոթագրություններ. Դետերմինիստական ​​կեղծ գնահատող, որը կիրառում է Blake2b512-ը՝ `iroha.alias.voprf.mock.v1` տիրույթի տարանջատմամբ: Նախատեսված է փորձնական գործիքավորման համար, մինչև արտադրական VOPRF խողովակաշարը միացվի Iroha-ով:
  - Սխալներ. HTTP `400` սխալ ձևավորված վեցանկյուն մուտքագրման վրա: Torii-ը վերադարձնում է Norito `ValidationFail::QueryFailed::Conversion` ծրար՝ ապակոդավորիչի սխալի հաղորդագրությամբ:
- ՓՈՍՏ `/v1/aliases/resolve`
  - Հարցում. { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Պատասխան՝ { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Ծանոթագրություններ. Պահանջում է ISO կամուրջի գործարկման ժամանակի բեմադրություն (`[iso_bridge.account_aliases]` `iroha_config`-ում): Torii-ը նորմալացնում է փոխանունները՝ նախքան որոնումը հանելով բացատները և վերին պատյանները: Վերադարձնում է 404, երբ կեղծանունը բացակայում է, և 503, երբ ISO կամուրջի գործարկման ժամանակը անջատված է:
- ՓՈՍՏ `/v1/aliases/resolve_index`
  - Հարցում. { "index": 0 }
  - Պատասխան՝ { «index»՝ 0, «alias»: «GB82WEST12345698765432», «account_id»: «i105...», «source»: «iso_bridge» }
  - Ծանոթագրություններ. Alias-ի ինդեքսները նշանակվում են դետերմինիստականորեն՝ ըստ կազմաձևման կարգի (0-ի վրա հիմնված): Հաճախորդները կարող են քեշավորել պատասխաններն անցանց՝ կեղծանունների ատեստավորման միջոցառումների համար աուդիտի հետքեր կառուցելու համար:Կոդի չափի գլխարկ
- Հատուկ պարամետր՝ `max_contract_code_bytes` (JSON u64)
  - Վերահսկում է առավելագույն թույլատրելի չափը (բայթերով) շղթայական պայմանագրային ծածկագրի պահպանման համար:
  - Կանխադրված՝ 16 ՄԲ: Հանգույցները մերժում են `RegisterSmartContractBytes`-ը, երբ `.to` պատկերի երկարությունը գերազանցում է գլխարկը անփոփոխ խախտման սխալով:
  - Օպերատորները կարող են հարմարվել՝ ներկայացնելով `SetParameter(Custom)` `id = "max_contract_code_bytes"` և թվային ծանրաբեռնվածությամբ:

- ՓՈՍՏ `/v1/gov/ballots/zk`
  - Հարցում. { «հեղինակություն»: «i105...», «մասնավոր_բանալին»: «…?», «chain_id»: «…», «election_id»: «e1», «proof_b64»: «…», «public»: {…} }
  - Պատասխան՝ { «ok»: ճշմարիտ, «ընդունված»: ճշմարիտ, «tx_instructions»: [{…}] }
  - Նշումներ.
    - Երբ սխեմայի հանրային մուտքերը ներառում են `owner`, `amount` և `duration_blocks`, և ապացույցը հաստատում է կազմաձևված VK-ի դեմ, հանգույցը ստեղծում կամ ընդլայնում է կառավարման կողպեք Sumeragi-ով Sumeragi-ով Sumeragi-ով: Ուղղությունը մնում է թաքնված (`unknown`), եթե ակնարկ չկա; թարմացվում է միայն գումարը/ժամկետը: Կրկնակի քվեարկությունները միապաղաղ են. գումարը և ժամկետը միայն ավելանում են (հանգույցը կիրառվում է max(amount, prev.amount) և max(expiry, prev.expiry)):
    - Երբ որևէ կողպեքի հուշում է տրվում, քվեաթերթիկը պետք է պարունակի `owner`, `amount` և `duration_blocks`; մասնակի ակնարկները մերժվում են։ Երբ `min_bond_amount > 0`, կողպման ակնարկներ են պահանջվում:
    - ZK կրկնակի քվեարկությունները, որոնք փորձում են կրճատել գումարը կամ ժամկետի ավարտը, մերժվում են սերվերի կողմից՝ `BallotRejected` ախտորոշմամբ:
    - Պայմանագրի կատարումը պետք է զանգահարի `ZK_VOTE_VERIFY_BALLOT` նախքան `SubmitBallot` հերթագրելը; տանտերերը պարտադրում են մեկ կրակոցի սողնակ:

- ՓՈՍՏ `/v1/gov/ballots/plain`
  - Հարցում. { «հեղինակություն»: «i105...», «մասնավոր_բանալին»: «…?», «chain_id»: «…», «referendum_id»: «r1», «սեփականատեր»: «i105...», «գումարը»: «1000», «տեւողությունը_բլոկները»: 6000, «Aye|Nay»
  - Պատասխան՝ { «ok»: ճշմարիտ, «ընդունված»: ճշմարիտ, «tx_instructions»: [{…}] }
  - Ծանոթագրություններ. Վերաքվեարկությունները միայն երկարաձգման են. նոր քվեաթերթիկը չի կարող նվազեցնել առկա կողպեքի գումարը կամ ժամկետի ավարտը: `owner`-ը պետք է հավասար լինի գործարքի իրավասությանը: Նվազագույն տևողությունը `conviction_step_blocks` է:- ՓՈՍՏ `/v1/gov/finalize`
  - Հարցում. { "referendum_id": "r1", "proposal_id": "…64hex", "authority": "i105...?", "private_key": "...?" }
  - Պատասխան՝ { «ok»: true, «tx_instructions»: [{ «wire_id»: «…FinalizeReferendum», «payload_hex»: «…» }] }
  - Շղթայական էֆեկտ (ներկայիս փայտամած). հաստատված տեղակայման առաջարկի ընդունումը ներդնում է `ContractManifest` նվազագույն `code_hash`-ի կողմից ակնկալվող `abi_hash`-ով և նշում է առաջարկը ուժի մեջ է: Եթե ​​`code_hash`-ի համար արդեն գոյություն ունի մանիֆեստ՝ մեկ այլ `abi_hash`-ով, օրենքը մերժվում է:
  - Նշումներ.
    - ZK ընտրությունների համար պայմանագրային ուղիները պետք է զանգահարեն `ZK_VOTE_VERIFY_TALLY` նախքան `FinalizeElection`-ը գործարկելը; տանտերերը պարտադրում են մեկ կրակոցի սողնակ: `FinalizeReferendum`-ը մերժում է ZK հանրաքվեն, քանի դեռ ընտրությունների հաշվարկը չի ավարտվել:
    - `h_end`-ի ավտոմատ փակումը թողարկում է Հաստատված/Մերժված է միայն պարզ հանրաքվեների համար; ZK հանրաքվեները փակ են մնում մինչև վերջնական հաշվարկի ներկայացումը և `FinalizeReferendum`-ի կատարումը:
    - Մասնակցության ստուգումները օգտագործում են միայն հաստատել+մերժել; ձեռնպահ մնալը չի ​​հաշվում մասնակցության համար:

- ՓՈՍՏ `/v1/gov/enact`
  - Հարցում. { «proposal_id»: «…64 hex», «preimage_hash»: «…64 hex?», «window»: { «ներքևում»: 0, «վերին»: 0 }?, «authority»: «i105...?», «private_key»: «…?» }
  - Պատասխան՝ { «ok»: true, «tx_instructions»: [{ «wire_id»: «…EnactReferendum», «payload_hex»: «…» }] }
  - Ծանոթագրություններ. Torii-ը ներկայացնում է ստորագրված գործարքը, երբ տրամադրվում է `authority`/`private_key`; հակառակ դեպքում այն ​​վերադարձնում է կմախք, որպեսզի հաճախորդները ստորագրեն և ներկայացնեն: Նախնական պատկերը ընտրովի է և ներկայումս տեղեկատվական:

- Ստացեք `/v1/gov/proposals/{id}`
  - `{id}` ուղի՝ առաջարկի ID վեցանկյուն (64 նիշ)
  - Պատասխան. { "գտնվել": bool, "առաջարկ": {… }? }

- Ստացեք `/v1/gov/locks/{rid}`
  - `{rid}` ուղի՝ հանրաքվեի ID տող
  - Պատասխան. { "գտնվել": bool, "referendum_id": "rid", "locks": {… }? }

- Ստացեք `/v1/gov/council/current`
  - Պատասխան՝ { «դարաշրջան»: N, «անդամներ»: [{ «account_id»: «…» }, …] }
  - Ծանոթագրություններ. ներկա լինելու դեպքում վերադարձնում է գործող խորհուրդը. հակառակ դեպքում ստացվում է դետերմինիստական ​​հետադարձ կապ՝ օգտագործելով կազմաձևված ցցերի ակտիվը և շեմերը (արտացոլում է VRF-ի սպեցիֆիկացիաները, մինչև կենդանի VRF ապացույցները պահպանվեն շղթայում):

- POST `/v1/gov/council/derive-vrf` (առանձնահատկություն՝ gov_vrf)
  - Հարցում. { "committee_size": 21, "epoch": 123? , «թեկնածուներ»: [{ «account_id»: «…», «տարբերակ»: «Նորմալ|Փոքր», «pk_b64»: «…», «proof_b64»: «…» }, …] }
  - Վարքագիծ. Ստուգում է յուրաքանչյուր թեկնածուի VRF ապացույցը `chain_id`-ից, `epoch`-ից և բլոկի վերջին հեշ փարոսից ստացված կանոնական մուտքագրման նկատմամբ. տեսակավորում ըստ ելքային բայթերի նվազման թայբրեյքերներով; վերադարձնում է `committee_size` լավագույն անդամներին: Չի պահպանվում։
  - Պատասխան՝ { «դարաշրջան»: N, «անդամներ»: [{ «account_id»: «…» } …], «total_candidates»: M, «ստուգված»: K }
  - Նշումներ. նորմալ = pk G1-ում, ապացույցը G2-ում (96 բայթ): Փոքր = pk G2-ում, ապացույցը G1-ում (48 բայթ): Մուտքերը բաժանված են տիրույթով և ներառում են `chain_id`:

### Կառավարման լռելյայն (iroha_config `gov.*`)

Խորհուրդը, որն օգտագործվում է Torii-ի կողմից, երբ մշտական ցուցակ չկա, պարամետրացված է `iroha_config`-ի միջոցով.```toml
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
  bond_escrow_account = "i105..."
  slash_receiver_account = "i105..."
  slash_double_vote_bps = 0            # percentage (basis points) to slash on double-vote attempts
  slash_invalid_proof_bps = 0          # percentage (basis points) to slash on invalid ballot proofs
  slash_ineligible_proof_bps = 0       # percentage (basis points) to slash on stale/invalid eligibility proofs
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

Համարժեք միջավայրը վերացնում է.

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_VOTING_ASSET_ID=xor#sora
GOV_MIN_BOND_AMOUNT=150
GOV_BOND_ESCROW_ACCOUNT=i105...
GOV_SLASH_RECEIVER_ACCOUNT=i105...
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

Sora Nexus լռելյայն. քվեաթերթիկները փակում են `min_bond_amount` `voting_asset_id`-ի մեջ
կազմաձևված պահուստային հաշիվ: Փականներ են ստեղծվում կամ երկարացվում, երբ քվեաթերթիկները վայրէջք են կատարում և
թողարկվել ժամկետի ավարտին; պարտատոմսերի կյանքի ցիկլը արտանետվում է `governance_bond_events_total`-ի միջոցով
հեռաչափություն (կողպեք_ստեղծված|կողպեք_երկարացվեց|կողպեք_բացված|կողպեք_շեղված|կողպեք_վերականգնվեց):

`parliament_committee_size`-ը սահմանում է վերադարձված անդամների թիվը, երբ խորհուրդ չի պահպանվել, `parliament_term_blocks` սահմանում է դարաշրջանի երկարությունը, որն օգտագործվում է սերմերի ստացման համար (`epoch = floor(height / term_blocks)`), `parliament_min_stake`-ը պարտադրում է նվազագույն միավորի չափաբաժինը (մինչև փոքր միավորը) `parliament_eligibility_asset_id`-ն ընտրում է, թե որ ակտիվների մնացորդն է սկանավորվում թեկնածուների հավաքածուն կառուցելիս:

Governance VK-ի ստուգումը շրջանցում չունի. քվեաթերթիկների ստուգումը միշտ պահանջում է `Active` հաստատող բանալի՝ ներկառուցված բայթերով, և միջավայրերը չպետք է հիմնվեն միայն թեստային անջատիչների վրա՝ ստուգումը բաց թողնելու համար:

RBAC
- Շղթայական կատարումը պահանջում է թույլտվություններ.
  - Առաջարկներ՝ `CanProposeContractDeployment{ contract_id }`
  - քվեաթերթիկներ՝ `CanSubmitGovernanceBallot{ referendum_id }`
  - Գործողություն՝ `CanEnactGovernance`
  - Շեղում/բողոքարկում՝ `CanSlashGovernanceLock{ referendum_id }`, `CanRestituteGovernanceLock{ referendum_id }`
  - Խորհրդի կառավարում (ապագա)՝ `CanManageParliament`
- Շեղում/բողոքարկում.
  - Կրկնակի քվեարկություն/անվավեր/անթույլատրելի քվեաթերթիկները կիրառում են կազմաձևված կտրվածքի տոկոսներ պարտատոմսերի պահառուի նկատմամբ, միջոցները տեղափոխելով `slash_receiver_account`, թարմացնելով կտրատող մատյանը և տպագրված `LockSlashed` իրադարձություններ (պատճառ + նպատակակետ + նշում):
  - Ձեռնարկ `SlashGovernanceLock`/`RestituteGovernanceLock` հրահանգները աջակցում են օպերատորի կողմից հիմնված տույժերին և բողոքարկումներին; փոխհատուցումը սահմանափակվում է գրանցված կտրվածքով, վերականգնում է միջոցները պարտատոմսերի պահուստում, թարմացնում է մատյանը և թողարկում է `LockRestituted`՝ կողպեքը ակտիվ պահելով մինչև ժամկետի ավարտը:Պաշտպանված անունների տարածքներ
- Պատվերով `gov_protected_namespaces` պարամետրը (JSON տողերի զանգված) թույլ է տալիս մուտքի մուտքի անցում նշված անվանատարածքներում տեղակայման համար:
- Հաճախորդները պետք է ներառեն գործարքների մետատվյալների բանալիներ՝ պաշտպանված անվանատարածքները նպատակաուղղված տեղակայման համար.
  - `gov_namespace`. նպատակային անվանատարածք (օրինակ՝ `"apps"`)
  - `gov_contract_id`. տրամաբանական պայմանագրի id անվանման տարածքում
- `gov_manifest_approvers`. կամընտիր JSON զանգված i105... հաշվի ID-ներ: Երբ երթուղու մանիֆեստը հայտարարում է մեկից ավելի քվորում, ընդունման համար պահանջվում է գործարքի իրավասության մարմինը գումարած թվարկված հաշիվները՝ բավարարելու մանիֆեստի քվորումը:
- Telemetry-ն բացահայտում է ամբողջական ընդունման հաշվիչներ `governance_manifest_admission_total{result}`-ի միջոցով, որպեսզի օպերատորները կարողանան տարբերակել հաջողված ընդունելությունները `missing_manifest`, `non_i105..._authority`, `quorum_rejected`, Prometheus և Prometheus և Prometheus ուղուց:
- Հեռաչափությունը բացահայտում է կիրառման ուղին `governance_manifest_quorum_total{outcome}`-ի միջոցով (`satisfied` / `rejected` արժեքներ), որպեսզի օպերատորները կարողանան ստուգել բացակայող հաստատումները:
- Գոտիները պարտադրում են իրենց մանիֆեստներում հրապարակված անունների տարածքի թույլտվությունների ցանկը: Ցանկացած գործարք, որը սահմանում է `gov_namespace`, պետք է տրամադրի `gov_contract_id`, իսկ անվանատարածքը պետք է հայտնվի մանիֆեստի `protected_namespaces` հավաքածուում: `RegisterSmartContractCode` ներկայացումները առանց այս մետատվյալների մերժվում են, երբ պաշտպանությունը միացված է:
- Ընդունումը պահանջում է, որ Կառավարման ընդունված առաջարկ գոյություն ունի բազմակի `(namespace, contract_id, code_hash, abi_hash)`-ի համար; հակառակ դեպքում վավերացումը ձախողվում է NotPermitted սխալով:

Runtime Upgrade Hooks
- Lane մանիֆեստները կարող են հայտարարել `hooks.runtime_upgrade` դարպասի գործարկման ժամանակի թարմացման հրահանգներին (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`):
- Կեռիկի դաշտեր.
  - `allow` (bool, լռելյայն `true`). երբ `false`, գործարկման ժամանակի թարմացման բոլոր հրահանգները մերժվում են:
  - `require_metadata` (bool, լռելյայն `false`). պահանջում է գործարքի մետատվյալների մուտքագրում, որը նշված է `metadata_key`-ով:
  - `metadata_key` (տող). Կանխադրված է `gov_upgrade_id`, երբ մետատվյալներ են պահանջվում կամ առկա է թույլտվությունների ցանկ:
  - `allowed_ids` (տողերի զանգված). մետատվյալների արժեքների կամընտիր թույլատրելի ցուցակ (կտրումից հետո): Մերժում է, երբ տրամադրված արժեքը նշված չէ:
- Երբ կեռիկը առկա է, հերթերի ընդունումը գործադրում է մետատվյալների քաղաքականությունը նախքան գործարքը հերթ մտնելը: Բացակայող մետատվյալները, դատարկ արժեքները կամ թույլատրելի ցուցակից դուրս գտնվող արժեքները առաջացնում են `NotPermitted` որոշիչ սխալ:
- Հեռաչափությունը հետևում է կիրառման արդյունքներին `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`-ի միջոցով:
- Կեռիկը բավարարող գործարքները պետք է ներառեն մետատվյալներ `gov_upgrade_id=<value>` (կամ մանիֆեստի կողմից սահմանված բանալի) ի կողքին i105... մանիֆեստի քվորումով պահանջվող ցանկացած հաստատում:

Հարմարավետության վերջնակետ
- POST `/v1/gov/protected-namespaces` — կիրառում է `gov_protected_namespaces` անմիջապես հանգույցի վրա:
  - Հարցում. { "namespaces": ["apps", "system"] }
  - Պատասխան. { "ok": ճշմարիտ, "կիրառված": 1 }
  - Նշումներ. Նախատեսված է ադմինիստրատորի/փորձարկման համար; պահանջում է API նշան, եթե կազմաձևված է: Արտադրության համար նախընտրեք ներկայացնել ստորագրված գործարք `SetParameter(Custom)`-ով:CLI Օգնողներ
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Վերցնում է պայմանագրային օրինակներ անունների տարածքի համար և խաչաձև ստուգում է, որ.
    - Torii-ը պահում է բայթկոդ յուրաքանչյուր `code_hash`-ի համար, և դրա Blake2b-32 digest-ը համապատասխանում է `code_hash`-ին:
    - `/v1/contracts/code/{code_hash}`-ի տակ պահվող մանիֆեստը հաղորդում է, որ համապատասխանում է `code_hash` և `abi_hash` արժեքներին:
    - `(namespace, contract_id, code_hash, abi_hash)`-ի համար գոյություն ունի ընդունված կառավարման առաջարկ, որը ստացվում է նույն առաջարկի ID-ի հաշինգով, որն օգտագործում է հանգույցը:
  - Արտադրում է JSON հաշվետվություն՝ `results[]`-ով յուրաքանչյուր պայմանագրով (խնդիրներ, մանիֆեստներ/ծածկագիր/առաջարկի ամփոփագրեր) գումարած մեկ տողով ամփոփում, եթե այն փակված չէ (`--no-summary`):
  - Օգտակար է պաշտպանված անվանատարածքները ստուգելու կամ կառավարման կողմից վերահսկվող տեղակայման աշխատանքային հոսքերը ստուգելու համար:
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Արտանետում է JSON մետատվյալների կմախքը, որն օգտագործվում է պաշտպանված անվանատարածքներում տեղակայումներ ներկայացնելիս, ներառյալ կամընտիր `gov_manifest_approvers`՝ մանիֆեստի քվորումի կանոնները բավարարելու համար:
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - Վավերացնում է կանոնական հաշվի ID-ները, կանոնականացնում է 32 բայթանոց զրոյացնող ակնարկները և ակնարկները միավորում է `public_inputs_json`-ի (`--public <path>`-ի հետ՝ լրացուցիչ վերափոխումների համար):
  - Չեղարկիչը ստացվում է ապացուցման պարտավորությունից (հանրային մուտքագրում) գումարած `domain_tag`, `chain_id` և `election_id`; `--nullifier`-ը վավերացված է ապացույցի համեմատ, երբ մատակարարվում է:
  - Մեկ տողով ամփոփումն այժմ բացահայտում է `fingerprint=<hex>` որոշիչ, որը ստացվում է կոդավորված `CastZkBallot`-ից, ինչպես նաև ցանկացած վերծանված հուշում (`owner`, `amount`, Sumeragi, երբ տրամադրվում է Sumeragi):
  - CLI-ի պատասխանները նշում են `tx_instructions[]` `payload_fingerprint_hex` գումարած վերծանված դաշտերը, այնպես որ հոսանքով ներքևող գործիքավորումը կարող է ստուգել կմախքը՝ առանց Norito վերծանման կրկնակի ներդրման:
  - Երբ որևէ կողպեքի հուշում է տրվում, ZK քվեաթերթիկները պետք է տրամադրեն `owner`, `amount` և `duration_blocks`; մասնակի ակնարկները մերժվում են։ Երբ `min_bond_amount > 0`, կողպման հուշումներ են պահանջվում: Ուղղությունը մնում է ընտրովի և դիտվում է որպես միայն հուշում:
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner`-ն ընդունում է I105 կանոնական տառերը; կամընտիր `@<domain>` վերջածանցները միայն երթուղային ակնարկներ են:
  - `--lock-amount`/`--lock-duration-blocks` ծածկանունները արտացոլում են ZK դրոշի անունները սցենարների հավասարության համար:
  - Համառոտ ելքային հայելիներ `vote --mode zk`՝ ներառելով կոդավորված հրահանգի մատնահետքը և մարդու կողմից ընթեռնելի քվեաթերթիկների դաշտերը (`owner`, `amount`, `duration_blocks`, Norito.Դեպքերի ցուցակագրում
- GET `/v1/gov/instances/{ns}` — թվարկում է ակտիվ պայմանագրային օրինակները անվանատարածքի համար:
  - Հարցման պարամետրեր.
    - `contains`՝ զտիչ՝ ըստ `contract_id`-ի ենթաշարքի (գործերի զգայուն)
    - `hash_prefix`. զտել ըստ `code_hash_hex`-ի վեցանկյուն նախածանցի (փոքրատառ)
    - `offset` (կանխադրված 0), `limit` (կանխադրված 100, առավելագույնը 10_000)
    - `order`. մեկը `cid_asc`-ից (կանխադրված), `cid_desc`, `hash_asc`, `hash_desc`
  - Պատասխան՝ { "namespace": "ns", "instances": [{ "contract_id": "…", "code_hash_hex": "..." }, …], "total": N, "offset": n, "limit": m }
  - SDK օգնական՝ `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) կամ `ToriiClient.list_governance_instances_typed("apps", ...)` (Python):

Ապակողպման մաքրում (օպերատոր/աուդիտ)
- Ստացեք `/v1/gov/unlocks/stats`
  - Պատասխան՝ { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Ծանոթագրություններ. `last_sweep_height` արտացոլում է բլոկի ամենավերջին բարձրությունը, որտեղ ժամկետանց կողպեքները մաքրվել և պահպանվել են: `expired_locks_now`-ը հաշվարկվում է `expiry_height <= height_current`-ով կողպեքի գրառումների սկանավորման միջոցով:
- ՓՈՍՏ `/v1/gov/ballots/zk-v1`
  - Հայց (v1-style DTO):
    {
      «հեղինակություն»: «i105...»,
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      «backend»: «halo2/ipa»,
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64 hex?",
      «սեփականատեր»: «i105…», // կանոնական AccountId (I105 բառացի; կամընտիր @domain ակնարկ)
      «գումարը»: «100?»,
      «duration_blocks»՝ 6000?,
      «ուղղություն». «Այո|Ոչ|Ձեռնպահ»,
      «nullifier»: «blake2b32:…64 hex?»
    }
  - Պատասխան՝ { «ok»: ճշմարիտ, «ընդունված»: ճշմարիտ, «tx_instructions»: [{…}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (առանձնահատկություն՝ `zk-ballot`)
  - Անմիջապես ընդունում է `BallotProof` JSON և վերադարձնում է `CastZkBallot` կմախքը:
  - Հայց.
    {
      «հեղինակություն»: «i105...»,
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      «քվեաթերթիկ»: {
        «backend»: «halo2/ipa»,
        "envelope_bytes": "AAECAwQ=", // base64 ZK1 կամ H2* կոնտեյներով
        «root_hint»: null, // կամընտիր 32 բայթանոց վեցանկյուն տող (իրավասության արմատ)
        «սեփականատեր»: null, // կամընտիր կանոնական AccountId (I105 բառացի; կամընտիր @domain ակնարկ)
        «չեղյալ»:
        "mount": "100", // կամընտիր կողպեքի գումարի հուշում (տասնորդական տող)
        «duration_blocks»: 6000, // կամընտիր կողպման տևողության հուշում
        "direction": "Aye" // կամընտիր ուղղության հուշում
      }
    }
  - Պատասխան.
    {
      «լավ»: ճիշտ է,
      «ընդունված»՝ ճշմարիտ,
      «պատճառ»՝ «գործարքի կմախք կառուցել»,
      «tx_instructions»: [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Նշումներ.
    - Երբ `private_key` տրամադրվում է, Torii-ը ներկայացնում է ստորագրված գործարքը և սահմանում `reason`-ը մինչև `submitted transaction`:
    - Սերվերը քարտեզագրում է `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`direction`/`nullifier`/`nullifier` կամընտիր `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier`-ից մինչև I01088NI010 `CastZkBallot`.
    - Ծրարի բայթերը կրկին կոդավորված են որպես base64 հրահանգների օգտակար բեռի համար:
    - Այս վերջնակետը հասանելի է միայն այն դեպքում, երբ միացված է `zk-ballot` գործառույթը:

CastZkBallot-ի ստուգման ուղի
- `CastZkBallot`-ը վերծանում է մատակարարված base64 ապացույցը և մերժում դատարկ կամ սխալ ձևավորված բեռները (`BallotRejected` `invalid or empty proof`-ով):
- Եթե `public_inputs_json` մատակարարված է, այն պետք է լինի JSON օբյեկտ; Ոչ օբյեկտային ծանրաբեռնվածությունը մերժվում է:
- Հաղորդավարը լուծում է հանրաքվեի (`vk_ballot`) կամ կառավարման լռելյայն քվեաթերթիկների ստուգման բանալին և պահանջում է, որ գրառումը գոյություն ունենա, լինի `Active` և ունենա ներդիրային բայթեր:
- Պահված հաստատող բանալի բայթերը կրկին հաշվել են `hash_vk`-ով; Պարտավորությունների ցանկացած անհամապատասխանություն դադարեցնում է կատարումը նախքան ստուգումը, որպեսզի պաշտպանվի ռեեստրի կեղծված գրառումներից (`BallotRejected` `verifying key commitment mismatch`-ի հետ):
- Ապացույց բայթերը ուղարկվում են գրանցված հետին մաս՝ `zk::verify_backend`-ի միջոցով; Անվավեր տառադարձումներ հայտնվում են որպես `BallotRejected` `invalid proof`-ով, և հրահանգը որոշիչ կերպով ձախողվում է:
- Ապացույցը պետք է բացահայտի քվեաթերթիկի պարտավորությունը և իրավասության արմատը՝ որպես հանրային մուտքեր. արմատը պետք է համապատասխանի ընտրության `eligible_root`-ին, իսկ ստացված զրոյացնողը պետք է համապատասխանի ցանկացած տրամադրված ակնարկին:
- Հաջող ապացույցները թողարկում են `BallotAccepted`; Կրկնվող զրոյացնողները, հնացած իրավասության արմատները կամ կողպեքի հետընթացը շարունակում են առաջացնել այս փաստաթղթում ավելի վաղ նկարագրված մերժման գոյություն ունեցող պատճառները:

## Վավերացնողի սխալ վարքագիծ և համատեղ կոնսենսուս

### Slashing and Jailing WorkflowԿոնսենսուսը թողարկում է Norito կոդավորված `Evidence`, երբ i105... խախտում է արձանագրությունը: Յուրաքանչյուր օգտակար բեռ ընկնում է հիշողության մեջ գտնվող `EvidenceStore`-ում և, եթե այն չի երևում, նյութականացվում է WSV-ով ապահովված `consensus_evidence` քարտեզում: `sumeragi.npos.reconfig.evidence_horizon_blocks`-ից ավելի հին գրառումները (կանխադրված `7200` բլոկներ) մերժվում են, ուստի արխիվը մնում է սահմանափակված, բայց մերժումը գրանցվում է օպերատորների համար: Evidence within the horizon obeys the joint-consensus staging rule (`mode_activation_height requires next_mode to be set in the same block`), the activation delay (`sumeragi.npos.reconfig.activation_lag_blocks`, default `1`), and the slashing delay (`sumeragi.npos.reconfig.slashing_delay_blocks`, default `259200`) so governance can cancel penalties նախքան դրանք դիմելը:

Ճանաչված իրավախախտումները մեկ առ մեկ քարտեզագրվում են `EvidenceKind`-ին; տարբերակիչները կայուն են և ուժի մեջ են մտնում տվյալների մոդելով.

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

- **DoublePrepare/DoubleCommit** — i105... ստորագրել է հակասական հեշեր նույն `(phase,height,view,epoch)`-ի համար:
- **InvalidQc** — ագրեգատորը բամբասել է commit QC-ին, որի ձևը չի կարողանում դետերմինիստական ​​ստուգումներ կատարել (օրինակ՝ դատարկ ստորագրողի բիթքարտեզ):
- **Invalid Proposal** — առաջնորդն առաջարկել է բլոկ, որը չի հաջողվում կառուցվածքային վավերացմանը (օրինակ՝ խախտում է կողպված շղթայի կանոնը):
- **Գրաքննություն** — ստորագրված ներկայացման անդորրագրերը ցույց են տալիս գործարք, որը երբեք չի առաջարկվել/կատարվել:

VRF-ի տույժերը ինքնաբերաբար կիրառվում են `activation_lag_blocks`-ից հետո (օրինախախտները բանտարկվում են): Կոնսենսուսի կրճատումը կիրառվում է միայն `slashing_delay_blocks` պատուհանից հետո, եթե ղեկավարությունը չեղարկի տույժը:

Օպերատորները և գործիքները կարող են ստուգել և վերահեռարձակել օգտակար բեռները հետևյալի միջոցով.

- Torii՝ `GET /v1/sumeragi/evidence` և `GET /v1/sumeragi/evidence/count`:
- CLI՝ `iroha ops sumeragi evidence list`, `… count` և `… submit --evidence-hex <payload>`:

Կառավարումը պետք է վերաբերվի ապացույցների բայթերին որպես կանոնական ապացույց.

1. **Հավաքեք օգտակար բեռը**, քանի դեռ այն չի սպառվել: Արխիվացրեք չմշակված Norito բայթերը բարձրության/դիտման մետատվյալների կողքին:
2. **Անհրաժեշտության դեպքում չեղարկեք**՝ ներկայացնելով `CancelConsensusEvidencePenalty` ապացույցների ծանրաբեռնվածությամբ մինչև `slashing_delay_blocks`-ի ավարտը; գրառումը նշված է `penalty_cancelled` և `penalty_cancelled_at_height`, և կտրվածք չի կիրառվում:
3. **Բեմադրեք տուգանքը**՝ օգտակար բեռը ներառելով հանրաքվեի կամ սուդոյի հրահանգում (օրինակ՝ `Unregister::peer`): Կատարումը վերահաստատում է օգտակար բեռը. սխալ կամ հնացած ապացույցները մերժվում են դետերմինիստական ​​կարգով:
4. **Պլանավորեք հետագա տոպոլոգիան**, որպեսզի վիրավորական i105... անմիջապես չմիանա: Տիպիկ հոսքերի հերթ `SetParameter(Sumeragi::NextMode)` և `SetParameter(Sumeragi::ModeActivationHeight)` թարմացված ցուցակով:
5. **Աուդիտի արդյունքները** `/v1/sumeragi/evidence`-ի և `/v1/sumeragi/status`-ի միջոցով՝ ապահովելու համար, որ ապացույցների հաշվառումը կատարելագործված է, և կառավարումն ընդունել է հեռացումը:

### Համատեղ կոնսենսուսային հաջորդականություն

Համատեղ կոնսենսուսը երաշխավորում է, որ ելքային i105... հավաքածուն ավարտում է սահմանային բլոկը, նախքան նոր հավաքածուն կսկսի առաջարկել: Գործարկման ժամանակը կիրառում է կանոնը զուգակցված պարամետրերի միջոցով.- `SumeragiParameter::NextMode` և `SumeragiParameter::ModeActivationHeight` պետք է կատարվեն **նույն բլոկում**: `mode_activation_height`-ը պետք է խիստ ավելի մեծ լինի, քան բլոկի բարձրությունը, որն իրականացրել է թարմացումը՝ ապահովելով առնվազն մեկ բլոկի ուշացում:
- `sumeragi.npos.reconfig.activation_lag_blocks` (լռելյայն `1`) կազմաձևման պահակ է, որը կանխում է զրոյական ուշացումները.
- `sumeragi.npos.reconfig.slashing_delay_blocks` (կանխադրված `259200`) հետաձգում է կոնսենսուսի կրճատումը, որպեսզի ղեկավարությունը կարողանա չեղարկել տույժերը նախքան դրանք կիրառելը:

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Գործարկման ժամանակը և CLI-ն ցուցադրում են փուլային պարամետրերը `/v1/sumeragi/params` և `iroha sumeragi params --summary` միջոցով, այնպես որ օպերատորները կարող են հաստատել ակտիվացման բարձրությունները և i105... ցուցակները:
- Կառավարման ավտոմատացումը միշտ պետք է.
  1. Վերջնականացնել ապացույցներով ապահովված հեռացման (կամ վերականգնման) որոշումը:
  2. Հերթագրեք հերթական վերակազմավորումը `mode_activation_height = h_current + activation_lag_blocks`-ով:
  3. `/v1/sumeragi/status` մոնիտորը մինչև `effective_consensus_mode`-ը շրջվի ակնկալվող բարձրության վրա:

Ցանկացած սկրիպտ, որը պտտվում է i105...s կամ կիրառում է կտրվածք **չպետք է** փորձի զրոյական ուշացումով ակտիվացնել կամ բաց թողնել անջատման պարամետրերը; նման գործարքները մերժվում են և դուրս են գալիս ցանցից նախորդ ռեժիմով:

## Հեռաչափական մակերեսներ

- Prometheus չափանիշների արտահանման կառավարման գործունեություն.
  - `governance_proposals_status{status}` (չափաչափ) հետևում է առաջարկների հաշվումը՝ ըստ կարգավիճակի:
  - `governance_protected_namespace_total{outcome}` (հաշվիչը) ավելացումներ, երբ պաշտպանված անվանատարածքի ընդունումը թույլ է տալիս կամ մերժում տեղակայումը:
  - `governance_manifest_activations_total{event}` (հաշվիչը) գրանցում է մանիֆեստի ներդիրները (`event="manifest_inserted"`) և անվանատարածքի կապերը (`event="instance_bound"`):
- `/status`-ը ներառում է `governance` օբյեկտ, որը արտացոլում է առաջարկների քանակը, հայտնում է պաշտպանված անվանատարածքի ընդհանուր գումարները և նշում է վերջին մանիֆեստների ակտիվացումները (անունների տարածք, պայմանագրի id, կոդը/ABI հեշ, բլոկի բարձրությունը, ակտիվացման ժամանակի դրոշմը): Օպերատորները կարող են հարցումներ կատարել այս դաշտում՝ հաստատելու, որ ակտերը թարմացվում են մանիֆեստների և որ պաշտպանված անվանատարածքի դարպասները կիրառված են:
- Grafana ձևանմուշ (`docs/source/grafana_governance_constraints.json`) և
  `telemetry.md`-ի հեռաչափության գրքույկը ցույց է տալիս, թե ինչպես միացնել ահազանգերը խրվածների համար
  առաջարկներ, բացակայող մանիֆեստների ակտիվացումներ կամ անսպասելի պաշտպանված անունների տարածք
  մերժումներ գործարկման ժամանակի թարմացումների ժամանակ:
---
lang: az
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14dbd50875bebd8d9f8c9f03f85cb458c909c9da956a7a7048c9dae8c885b969
source_last_modified: "2026-01-22T16:26:46.496232+00:00"
translation_last_reviewed: 2026-02-07
title: Governance App API — Endpoints (Draft)
translator: machine-google-reviewed
---

Status: idarəetmənin icrası tapşırıqlarını müşayiət etmək üçün qaralama/eskiz. İcra zamanı formalar dəyişə bilər. Determinizm və RBAC siyasəti normativ məhdudiyyətlərdir; Torii, `authority` və `private_key` təmin edildikdə əməliyyatları imzalaya/təqdim edə bilər, əks halda müştərilər `/transaction`-a təqdim edə bilər.

Ümumi baxış
- Bütün son nöqtələr JSON qaytarır. Tranzaksiya yaradan axınlar üçün cavablara `tx_instructions` daxildir — bir və ya bir neçə təlimat skeleti massivi:
  - `wire_id`: təlimat növü üçün reyestr identifikatoru
  - `payload_hex`: Norito faydalı yük baytları (hex)
- `authority` və `private_key` (və ya seçki bülletenlərində DTO-larda `private_key`) təmin edilərsə, Torii əməliyyatı imzalayır və təqdim edir və yenə də `tx_instructions` qaytarır.
- Əks halda, müştərilər öz səlahiyyətlərindən və chain_id-dən istifadə edərək İmzalıTransaction toplayır, sonra imzalayıb `/transaction`-ə POST göndərin.
- SDK əhatə dairəsi:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` qaytarır `GovernanceProposalResult` (status/növ sahələrini normallaşdırır), `ToriiClient.get_governance_referendum_typed` qaytarır `GovernanceReferendumResult`, I18NI000000000, I18NI000000000, I18NI00000000, I18NI000000001 `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult` qaytarır, `ToriiClient.get_governance_unlock_stats_typed` `GovernanceUnlockStats` qaytarır və `ToriiClient.list_governance_instances_typed` `GovernanceInstancesPage` qaytarır, nümunələr vasitəsilə bizə yazılan girişi təmin edir.
- Python yüngül müştəri (`iroha_torii_client`): `ToriiClient.finalize_referendum` və `ToriiClient.enact_proposal` tipli `GovernanceInstructionDraft` paketlərini qaytarır (Torii skeletini bükərək, I18NI550X0 skeletini tərtib etməkdən qaçınarkən), Axınları yekunlaşdırın/tətbiq edin.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` təkliflər, referendumlar, hesablamalar, kilidlər, kilidlər üçün yazılmış köməkçiləri və indi `listGovernanceInstances(namespace, options)` üstəgəl şuranın son nöqtələrini (I18NI0000005900X, I18NI00000059000X, I18NI0000005900000000000000X,I18NI00000059000000000000000000000000000000000000000000000000000000000000000000000000000000000. `governancePersistCouncil`, `getGovernanceCouncilAudit`) beləliklə, Node.js müştəriləri `/v1/gov/instances/{ns}` səhifələrini sıralaya və mövcud müqavilə nümunəsi siyahısı ilə yanaşı VRF dəstəkli iş axınlarını idarə edə bilərlər.

Son nöqtələr

- POST `/v1/gov/proposals/deploy-contract`
  - Sorğu (JSON):
    {
      "namespace": "tətbiqlər",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "pəncərə": { "aşağı": 12345, "yuxarı": 12400 },
      "authority": "i105...?",
      "private_key": "...?"
    }
  - Cavab (JSON):
    { "ok": doğru, "təklif_id": "…64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Təsdiqləmə: qovşaqlar təmin edilən `abi_version` üçün `abi_hash`-ni kanonikləşdirir və uyğunsuzluqları rədd edir. `abi_version = "v1"` üçün gözlənilən dəyər `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`-dir.

Contracts API (yerləşdirmə)
- POST `/v1/contracts/deploy`
  - Sorğu: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Davranış: IVM proqram gövdəsindən `code_hash` və `abi_version` başlığından `abi_hash` hesablayır, sonra `RegisterSmartContractCode` (manifest) və I10040l təqdim edir `.to` bayt) `authority` adından.
  - Cavab: { "ok": doğrudur, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Əlaqədar:
    - GET `/v1/contracts/code/{code_hash}` → saxlanılan manifesti qaytarır
    - GET `/v1/contracts/code-bytes/{code_hash}` → qaytarır `{ code_b64 }`
- POST `/v1/contracts/instance`
  - Sorğu: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Davranış: Təchiz edilmiş bayt kodunu yerləşdirir və dərhal `(namespace, contract_id)` xəritəsini `ActivateContractInstance` vasitəsilə aktivləşdirir.
  - Cavab: { "ok": doğru, "ad sahəsi": "tətbiqlər", "kontrakt_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Alias Xidməti
- POST `/v1/aliases/voprf/evaluate`
  - Sorğu: { "blinded_element_hex": "..." }
  - Cavab: { "qiymətləndirilmiş_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` qiymətləndiricinin həyata keçirilməsini əks etdirir. Cari dəyər: `blake2b512-mock`.
  - Qeydlər: `iroha.alias.voprf.mock.v1` domen ayrılması ilə Blake2b512 tətbiq edən deterministik saxta qiymətləndirici. İstehsal VOPRF boru kəməri Iroha vasitəsilə naqil edilənə qədər sınaq alətləri üçün nəzərdə tutulmuşdur.
  - Səhvlər: səhv formalaşdırılmış hex girişində HTTP `400`. Torii dekoder xətası mesajı ilə Norito `ValidationFail::QueryFailed::Conversion` zərfini qaytarır.
- POST `/v1/aliases/resolve`
  - Sorğu: { "ləqəb": "GB82 WEST 1234 5698 7654 32" }
  - Cavab: { "ləqəb": "GB82WEST12345698765432", "account_id": "i105...", "indeks": 0, "mənbə": "iso_bridge" }
  - Qeydlər: ISO körpüsünün iş vaxtı quruluşunu tələb edir (`[iso_bridge.account_aliases]`, `iroha_config`). Torii axtarışdan əvvəl boşluq və yuxarı hərfi silməklə ləqəbləri normallaşdırır. Təxəllüs olmadıqda 404, ISO körpüsünün işləmə müddəti deaktiv olduqda 503 qaytarır.
- POST `/v1/aliases/resolve_index`
  - Sorğu: { "indeks": 0 }
  - Cavab: { "indeks": 0, "ləqəb": "GB82WEST12345698765432", "account_id": "i105...", "mənbə": "iso_bridge" }
  - Qeydlər: Alias indeksləri konfiqurasiya qaydasından (0-asaslı) deterministik olaraq təyin edilir. Müştərilər ləqəbli attestasiya hadisələri üçün audit yollarını yaratmaq üçün cavabları oflayn rejimdə keşləyə bilər.

Kod Ölçüsü Cap
- Fərdi parametr: `max_contract_code_bytes` (JSON u64)
  - Zəncirli müqavilə kodunun saxlanması üçün icazə verilən maksimum ölçüyə (baytlarda) nəzarət edir.
  - Defolt: 16 MiB. `.to` təsvir uzunluğu dəyişməz pozuntu xətası ilə həddi aşdıqda qovşaqlar `RegisterSmartContractBytes`-i rədd edir.
  - Operatorlar `SetParameter(Custom)` təqdim edərək `id = "max_contract_code_bytes"` və rəqəmli faydalı yüklə tənzimləyə bilərlər.

- POST `/v1/gov/ballots/zk`
  - Sorğu: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "select_id": "e1", "proof_b64": "...", "public": {…} }
  - Cavab: { "ok": doğru, "qəbul edildi": doğru, "tx_instructions": [{…}] }
  - Qeydlər:
    - Dövrənin ümumi girişlərinə `owner`, `amount` və `duration_blocks` daxil olduqda və sübut konfiqurasiya edilmiş VK-ya qarşı yoxlandıqda, qovşaq I18NI0000010101010101010.00000010 ilə I18NI0000010. İstiqamət gizli qalır (`unknown`); yalnız məbləğ/müddəti yenilənir. Təkrar səsvermələr monotondur: məbləğ və müddət yalnız artır (qovşaq max(məbləğ, əvvəlki məbləğ) və maks.(son, əvvəlki, son) tətbiq edir.
    - Məbləği azaltmağa və ya müddəti bitməyə cəhd edən ZK təkrar səsləri `BallotRejected` diaqnostikası ilə server tərəfindən rədd edilir.
    - Müqavilənin icrası `SubmitBallot` növbəsinə daxil edilməzdən əvvəl `ZK_VOTE_VERIFY_BALLOT`-ə zəng etməlidir; ev sahibləri bir vuruşlu kilidi tətbiq edir.

- POST `/v1/gov/ballots/plain`
  - Sorğu: { "səlahiyyət": "i105...", "özəl_açar": "...?", "zəncir_id": "...", "referendum_id": "r1", "sahibi": "i105...", "məbləğ": "1000", "müddət_bloklar": 6000, "əlamətlər|Naxış"
  - Cavab: { "ok": doğru, "qəbul edildi": doğru, "tx_instructions": [{…}] }
  - Qeydlər: Təkrar səsvermələr yalnız uzadılır - yeni səsvermə bülleteni mövcud kilidin məbləğini və ya müddətini azalda bilməz. `owner` əməliyyat səlahiyyətinə bərabər olmalıdır. Minimum müddət `conviction_step_blocks`-dir.

- POST `/v1/gov/finalize`
  - Sorğu: { "referendum_id": "r1", "təklif_id": "...64hex", "authority": "i105...?", "private_key": "...?" }
  - Cavab: { "ok": doğrudur, "tx_instructions": [{ "wire_id": "...Referendumun yekunlaşdırılması", "payload_hex": "..." }] }
  - Zəncirvari effekt (cari iskele): təsdiq edilmiş yerləşdirmə təklifinin qüvvəyə minməsi gözlənilən `abi_hash` ilə `code_hash` tərəfindən əsaslanan minimal `ContractManifest` əlavə edir və təklifin Qəbul edildiyini qeyd edir. `code_hash` üçün fərqli `abi_hash` ilə manifest artıq mövcuddursa, qüvvəyə minmə rədd edilir.
  - Qeydlər:
    - ZK seçkiləri üçün müqavilə yolları `FinalizeElection` yerinə yetirilməzdən əvvəl `ZK_VOTE_VERIFY_TALLY` nömrəsinə zəng etməlidir; ev sahibləri bir vuruşlu kilidi tətbiq edir. `FinalizeReferendum`, seçkilərin sayı yekunlaşana qədər ZK referendumunu rədd edir.
    - `h_end`-də avtomatik bağlanma yalnız Düz referendum üçün Təsdiqləndi/Reddedildi; ZK referendumu yekun nəticə təqdim olunana və `FinalizeReferendum` yerinə yetirilənə qədər bağlı qalır.
    - İştirakçıların iştirakının yoxlanılmasında yalnız təsdiq+rədd istifadə olunur; bitərəf qalmaq seçkilərdə iştirak sayılmır.

- POST `/v1/gov/enact`
  - Sorğu: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "aşağı": 0, "yuxarı": 0 }?, "səlahiyyət": "i105...?", "private_key": "...?" }
  - Cavab: { "ok": doğrudur, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Qeydlər: Torii imzalanmış əməliyyatı `authority`/`private_key` təqdim edildikdə təqdim edir; əks halda müştərilərin imzalaması və təqdim etməsi üçün skelet qaytarır. Ön görüntü isteğe bağlıdır və hazırda məlumat xarakteri daşıyır.

- `/v1/gov/proposals/{id}` ALIN
  - `{id}` yolu: təklif id hex (64 simvol)
  - Cavab: { "tapıldı": bool, "təklif": { … }? }

- `/v1/gov/locks/{rid}` ALIN
  - `{rid}` yolu: referendum id sətri
  - Cavab: { "tapıldı": bool, "referendum_id": "xilas", "kilidlər": { … }? }

- `/v1/gov/council/current` ALIN
  - Cavab: { "epox": N, "üzvlər": [{ "account_id": "..." }, …] }
  - Qeydlər: mövcud olduqda israrlı şuranı qaytarır; əks halda konfiqurasiya edilmiş pay aktivi və hədləri istifadə edərək deterministik geri dönüş əldə edir (canlı VRF sübutları zəncirdə davam edənə qədər VRF spesifikasiyasını əks etdirir).- POST `/v1/gov/council/derive-vrf` (xüsusiyyət: gov_vrf)
  - Sorğu: { "committee_size": 21, "epoch": 123? , "namizədlər": [{ "account_id": "...", "variant": "Normal|Kiçik", "pk_b64": "...", "proof_b64": "..." }, …] }
  - Davranış: Hər bir namizədin VRF sübutunu `chain_id`, `epoch` və ən son blok hash mayakından əldə edilən kanonik girişə qarşı yoxlayır; Çıxış baytlarına görə çeşidləmə taybreykerlərlə azaldılır; üst `committee_size` üzvlərini qaytarır. Davam etmir.
  - Cavab: { "epox": N, "üzvlər": [{ "account_id": "..." } …], "total_candidates": M, "təsdiqlənib": K }
  - Qeydlər: Normal = G1-də pk, G2-də sübut (96 bayt). Kiçik = G2-də pk, G1-də sübut (48 bayt). Daxiletmələr domendən ayrılıb və `chain_id` daxildir.

### İdarəetmə defoltları (iroha_config `gov.*`)

Davamlı siyahı olmadıqda Torii tərəfindən istifadə edilən şura ehtiyatı `iroha_config` vasitəsilə parametrləşdirilir:

```toml
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
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

Ekvivalent mühit ləğv edir:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` heç bir məclis davam etmədikdə geri qaytarılan üzvlərin sayını məhdudlaşdırır, `parliament_term_blocks` toxum əldə etmək üçün istifadə olunan dövr uzunluğunu müəyyən edir (`epoch = floor(height / term_blocks)`), `parliament_min_stake` (minimum və kiçik ölçü vahidləri üzrə tətbiq edir) `parliament_eligibility_asset_id` namizəd dəstini qurarkən hansı aktiv balansının skan ediləcəyini seçir.

İdarəetmə VK doğrulamasının yan keçməsi yoxdur: səsvermə bülleteni doğrulaması həmişə daxili baytları olan `Active` doğrulama açarı tələb edir və mühitlər yoxlamanı keçmək üçün yalnız test keçidlərinə etibar etməməlidir.

RBAC
- Zəncir üzərində icra icazələri tələb edir:
  - Təkliflər: `CanProposeContractDeployment{ contract_id }`
  - Səsvermə bülletenləri: `CanSubmitGovernanceBallot{ referendum_id }`
  - Qəbul: `CanEnactGovernance`
  - Şuranın idarə edilməsi (gələcək): `CanManageParliament`

Qorunan Ad məkanları
- Xüsusi parametr `gov_protected_namespaces` (JSON sətirlər massivi) sadalanan ad boşluqlarına yerləşdirmə üçün qəbul keçidinə imkan verir.
- Müştərilər qorunan ad məkanlarını hədəfləyən yerləşdirmələr üçün əməliyyat metadata açarlarını daxil etməlidirlər:
  - `gov_namespace`: hədəf ad sahəsi (məsələn, `"apps"`)
  - `gov_contract_id`: ad məkanında məntiqi müqavilə id
- `gov_manifest_approvers`: təsdiqləyici hesab identifikatorlarının isteğe bağlı JSON massivi. Zolaqlı manifest birdən çox kvorum elan etdikdə, qəbul manifest kvorumunu təmin etmək üçün əməliyyat orqanı və sadalanan hesabları tələb edir.
- Telemetriya `governance_manifest_admission_total{result}` vasitəsilə vahid qəbul sayğaclarını ifşa edir, beləliklə operatorlar uğurlu qəbulları `missing_manifest`, I18NI0000154X, `quorum_rejected`, `protected_namespace_rejected`, `protected_namespace_rejected`, I18NI0000156X, I181010-dan ayıra bilsinlər.
- Telemetriya icra yolunu `governance_manifest_quorum_total{outcome}` (dəyərlər `satisfied` / `rejected`) vasitəsilə üzə çıxarır ki, operatorlar çatışmayan təsdiqləri yoxlaya bilsinlər.
- Zolaqlar manifestlərində dərc edilmiş ad sahəsi icazəli siyahısını tətbiq edir. `gov_namespace` təyin edən istənilən əməliyyat `gov_contract_id` təmin etməli və ad sahəsi manifestin `protected_namespaces` dəstində görünməlidir. Bu metadata olmayan `RegisterSmartContractCode` təqdimatları qorunma aktivləşdirildikdə rədd edilir.
- Qəbul, `(namespace, contract_id, code_hash, abi_hash)` dəsti üçün qüvvəyə minmiş idarəetmə təklifinin mövcud olmasını təmin edir; əks halda doğrulama NotPermitted xətası ilə uğursuz olur.

Runtime Təkmilləşdirmə Qarmaqları
- Zolaqlı manifestlər `hooks.runtime_upgrade`-i qapının işləmə vaxtının təkmilləşdirmə təlimatları üçün elan edə bilər (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Qarmaq sahələri:
  - `allow` (bool, defolt `true`): `false` zaman, iş vaxtı təkmilləşdirmə təlimatları rədd edilir.
  - `require_metadata` (bool, defolt `false`): `metadata_key` tərəfindən müəyyən edilmiş tranzaksiya metadata girişini tələb edir.
  - `metadata_key` (sətir): qarmaq tərəfindən tətbiq edilən metadata adı. Metadata tələb olunduqda və ya icazə siyahısı mövcud olduqda defolt olaraq `gov_upgrade_id`.
  - `allowed_ids` (sətirlər massivi): metadata dəyərlərinin isteğe bağlı icazə siyahısı (kəsmədən sonra). Təqdim olunan dəyər siyahıda olmadıqda rədd edir.
- Qarmaq mövcud olduqda, növbə qəbulu əməliyyat növbəyə daxil olmamışdan əvvəl metadata siyasətini tətbiq edir. Çatışmayan metadata, boş dəyərlər və ya icazə verilən siyahıdan kənar dəyərlər deterministik `NotPermitted` xətası yaradır.
- Telemetriya `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` vasitəsilə icra nəticələrini izləyir.
- Qarmağa cavab verən əməliyyatlar manifest kvorumunun tələb etdiyi hər hansı təsdiqləyici təsdiqləri ilə yanaşı `gov_upgrade_id=<value>` metadatasını (və ya manifestdə müəyyən edilmiş açarı) əhatə etməlidir.

Rahatlıq son nöqtəsi
- POST `/v1/gov/protected-namespaces` — `gov_protected_namespaces` birbaşa node üzərində tətbiq edilir.
  - Sorğu: { "ad boşluqları": ["tətbiqlər", "sistem"] }
  - Cavab: { "ok": doğrudur, "tətbiq olunur": 1 }
  - Qeydlər: Admin/test üçün nəzərdə tutulub; konfiqurasiya edildiyi təqdirdə API nişanı tələb olunur. İstehsal üçün, `SetParameter(Custom)` ilə imzalanmış əməliyyatı təqdim etməyə üstünlük verin.

CLI Köməkçiləri
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Ad sahəsi üçün müqavilə nümunələrini gətirir və çarpaz yoxlayır:
    - Torii hər bir `code_hash` üçün bayt kodunu saxlayır və onun Blake2b-32 həzmi `code_hash` ilə uyğun gəlir.
    - `/v1/contracts/code/{code_hash}` altında saxlanılan manifest `code_hash` və `abi_hash` dəyərlərinə uyğun gəlir.
    - `(namespace, contract_id, code_hash, abi_hash)` üçün qüvvəyə minmiş idarəetmə təklifi qovşağın istifadə etdiyi eyni təklif-id-dən əldə edildiyi kimi mövcuddur.
  - Müqavilə üzrə `results[]` (məsələlər, manifest/kod/təklif xülasələri) və basdırılmadığı halda bir sətirlik xülasə ilə JSON hesabatını çıxarır (`--no-summary`).
  - Qorunan ad məkanlarını yoxlamaq və ya idarəetmə tərəfindən idarə olunan yerləşdirmə iş axınlarını yoxlamaq üçün faydalıdır.
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Manifest kvorum qaydalarını təmin etmək üçün isteğe bağlı `gov_manifest_approvers` daxil olmaqla qorunan ad məkanlarına yerləşdirmələri təqdim edərkən istifadə edilən JSON metadata skeletini yayır.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` zaman kilid göstərişləri tələb olunur və hər hansı təqdim edilmiş göstəriş dəstinə `owner`, `amount` və `duration_blocks` daxil edilməlidir.
  - Kanonik hesab identifikatorlarını təsdiq edir, 32 baytlıq ləğvedici göstərişləri kanonikləşdirir və göstərişləri `public_inputs_json`-də birləşdirir (əlavə ləğvetmələr üçün `--public <path>` ilə).
  - Nullifier sübut öhdəliyindən (ictimai giriş) üstəgəl `domain_tag`, `chain_id` və `election_id`-dən əldə edilir; `--nullifier` təchiz edildikdə sübuta qarşı təsdiqlənir.
  - The one-line summary now surfaces a deterministic `fingerprint=<hex>` derived from the encoded `CastZkBallot` along with any decoded hints (`owner`, `amount`, `duration_blocks`, `direction` when provided).
  - CLI cavabları `tx_instructions[]`-i `payload_fingerprint_hex` və deşifrə edilmiş sahələrlə annotasiya edir ki, aşağı axın alətləri Norito deşifrəsini təkrar tətbiq etmədən skeleti yoxlaya bilsin.
  - Kilid göstərişlərinin təmin edilməsi dövrə eyni dəyərləri ifşa etdikdən sonra qovşağın ZK bülletenləri üçün `LockCreated`/`LockExtended` hadisələrini yaymasına imkan verir.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` kanonik I105 literallarını qəbul edir; isteğe bağlı `@<domain>` şəkilçiləri yalnız marşrut göstərişləridir.
  - `--lock-amount`/`--lock-duration-blocks` ləqəbləri skript pariteti üçün ZK bayraq adlarını əks etdirir.
  - Kodlanmış təlimat barmaq izi və insan tərəfindən oxuna bilən seçki bülleteni sahələri (`owner`, `amount`, `duration_blocks`, `direction` imzalanmadan əvvəl) daxil olmaqla xülasə çıxış güzgüləri `vote --mode zk`.

Nümunələrin siyahısı
- GET `/v1/gov/instances/{ns}` — ad məkanı üçün aktiv müqavilə nümunələrini sadalayır.
  - Sorğu parametrləri:
    - `contains`: `contract_id` alt sətri ilə filtr (həssas hərf)
    - `hash_prefix`: `code_hash_hex` (kiçik hərf) hex prefiksi ilə filtr
    - `offset` (standart 0), `limit` (defolt 100, maksimum 10_000)
    - `order`: biri `cid_asc` (defolt), `cid_desc`, `hash_asc`, `hash_desc`
  - Cavab: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, …], "total": N, "offset": n, "limit": m }
  - SDK köməkçisi: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) və ya `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Süpürgənin kilidini açın (Operator/Audit)
- `/v1/gov/unlocks/stats` ALIN
  - Cavab: { "height_current": H, "expired_locks_inow": n, "referenda_with_with_expired": m, "son_sweep_height": S }
  - Qeydlər: `last_sweep_height` vaxtı keçmiş qıfılların süpürüldüyü və saxlanıldığı ən son blok hündürlüyünü əks etdirir. `expired_locks_now` `expiry_height <= height_current` ilə kilid qeydlərini skan etməklə hesablanır.
- POST `/v1/gov/ballots/zk-v1`
  - Sorğu (v1-stil DTO):
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "seçki_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64hex?",
      "owner": "i105…?", // kanonik AccountId (I105 hərfi)
      "miqdar": "100?",
      "duration_blocks": 6000?,
      "direction": "Bəli|Xeyr|Çəkərən?",
      "nullifier": "blake2b32:...64hex?"
    }
  - Cavab: { "ok": doğru, "qəbul edildi": doğru, "tx_instructions": [{…}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (xüsusiyyət: `zk-ballot`)
  - Birbaşa `BallotProof` JSON qəbul edir və `CastZkBallot` skeletini qaytarır.
  - Sorğu:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "seçki_id": "ref-1",
      "səsvermə bülleteni": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // ZK1 və ya H2* konteynerinin base64
        "root_hint": null, // isteğe bağlı 32 bayt hex sətir (uyğunluq kökü)
        "sahibi": null, // isteğe bağlı kanonik AccountId (I105 hərfi)
        "nullifier": null, // isteğe bağlı 32 bayt hex sətir (nullifier işarəsi)
        "miqdar": "100", // isteğe bağlı kilid məbləği işarəsi (onluq sətir)
        "duration_blocks": 6000, // isteğe bağlı kilidləmə müddəti göstərişi
        "istiqamət": "Aye" // isteğe bağlı istiqamət işarəsi
      }
    }
  - Cavab:
    {
      "ok": doğru,
      "qəbul edildi": doğru,
      "reason": "sövdələşmə skeleti qurmaq",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Qeydlər:
    - Server isteğe bağlı `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier`-dən I0106-ya səs verir `CastZkBallot`.
    - Zərf baytları təlimatın faydalı yükü üçün base64 kimi yenidən kodlaşdırılıb.
    - Torii səsvermə bülleteni təqdim edərkən `reason` cavabı `submitted transaction` olaraq dəyişir.
    - Bu son nöqtə yalnız `zk-ballot` funksiyası aktiv olduqda mövcuddur.

CastZkBallot Doğrulama Yolu
- `CastZkBallot` təchiz edilmiş base64 sübutunu deşifrə edir və boş və ya səhv formalaşdırılmış faydalı yükləri rədd edir (`BallotRejected` `invalid or empty proof` ilə).
- `public_inputs_json` verilirsə, o, JSON obyekti olmalıdır; qeyri-obyekt yükləri rədd edilir.
- Ev sahibi referendumdan (`vk_ballot`) səsvermə bülleteni doğrulama açarını və ya idarəetmə defoltlarını həll edir və qeydin mövcud olmasını, `Active` olmasını və daxili baytların daşınmasını tələb edir.
- Saxlanılan doğrulama açarı baytları `hash_vk` ilə yenidən heşlənir; hər hansı öhdəliyin uyğunsuzluğu dəyişdirilmiş reyestr qeydlərindən qorunmaq üçün yoxlamadan əvvəl icranı dayandırır (`BallotRejected` ilə `verifying key commitment mismatch`).
- Sübut baytları `zk::verify_backend` vasitəsilə qeydə alınmış arxa hissəyə göndərilir; etibarsız transkriptlər `invalid proof` ilə `BallotRejected` kimi səthə çıxır və təlimat deterministik şəkildə uğursuz olur.
- Sübut ictimai məlumat kimi seçki bülleteni öhdəliyini və uyğunluq kökünü ifşa etməlidir; kök seçkinin `eligible_root`-ə uyğun olmalıdır və əldə edilmiş ləğvedici hər hansı təqdim edilmiş göstərişə uyğun olmalıdır.
- Uğurlu sübutlar `BallotAccepted` yayır; dublikat ləğvedicilər, köhnəlmiş uyğunluq kökləri və ya kilid reqressiyaları bu sənədin əvvəlində təsvir edilmiş mövcud imtina səbəblərini yaratmağa davam edir.

## Qiymətləndiricinin Səhv Davranışı və Birgə Konsensus

### İş axınının kəsilməsi və həbs edilməsi

Doğrulayıcı protokolu pozduqda konsensus Norito kodlu `Evidence` yayır. Hər bir faydalı yük yaddaşdaxili `EvidenceStore`-ə düşür və əgər görünməzsə, WSV tərəfindən dəstəklənən `consensus_evidence` xəritəsində reallaşdırılır. `sumeragi.npos.reconfig.evidence_horizon_blocks`-dən (defolt `7 200` blokları) köhnə qeydlər rədd edilir, beləliklə arxiv məhdud qalır, lakin operatorlar üçün imtina qeyd olunur. Üfüqdə olan dəlillər birgə konsensus tərtibi qaydasına (`mode_activation_height requires next_mode to be set in the same block`), aktivləşdirmə gecikməsinə (`sumeragi.npos.reconfig.activation_lag_blocks`, defolt `1`) və kəsilmə gecikməsinə (`sumeragi.npos.reconfig.slashing_delay_blocks`, default000282) tabedir. tətbiq etməzdən əvvəl cərimələr.

Tanınmış cinayətlər `EvidenceKind`-ə bir-bir xəritə verir; diskriminantlar sabitdir və məlumat modeli tərəfindən tətbiq edilir:

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

- **DoublePrepare/DoubleCommit** — validator eyni `(phase,height,view,epoch)` dəsti üçün ziddiyyətli heşləri imzaladı.
- **InvalidQc** — aqreqator forması deterministik yoxlamalardan (məsələn, boş imzalayan bitmap) uğursuz olan öhdəçilik sertifikatını dedi-qodu etdi.
- **InvalidProposal** — lider struktur yoxlamadan keçməyən blok təklif etdi (məsələn, kilidli zəncir qaydasını pozur).
- **Senzura** — imzalanmış təqdimetmə qəbzləri heç vaxt təklif olunmayan/təhlükəsiz olan əməliyyatı göstərir.

VRF cəzaları `activation_lag_blocks` (cinayətkarlar həbs edilir) sonra avtomatik olaraq tətbiq edilir. İdarəetmə cəzanı ləğv etmədiyi halda konsensus kəsilməsi yalnız `slashing_delay_blocks` pəncərəsindən sonra tətbiq edilir.

Operatorlar və alətlər aşağıdakılar vasitəsilə faydalı yükləri yoxlaya və yenidən yayımlaya bilər:

- Torii: `GET /v1/sumeragi/evidence` və `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `… count` və `… submit --evidence-hex <payload>`.

İdarəetmə sübut baytlarına kanonik sübut kimi baxmalıdır:

1. **Yükünü köhnəlmədən** toplayın. Hündürlük/baxış metadatasının yanında xam Norito baytlarını arxivləşdirin.
2. `slashing_delay_blocks` başa çatmazdan əvvəl sübut yükü ilə `CancelConsensusEvidencePenalty` təqdim etməklə **Lazım olduqda ləğv edin**; qeyd `penalty_cancelled` və `penalty_cancelled_at_height` qeyd olunur və heç bir kəsik tətbiq edilmir.
3. Faydalı yükü referendum və ya sudo təlimatına daxil etməklə (məsələn, `Unregister::peer`) **Cəriməni təyin edin**. İcra yükü yenidən təsdiq edir; qüsurlu və ya köhnəlmiş sübutlar qəti şəkildə rədd edilir.
4. **Təxminən topologiyanı planlaşdırın** ki, xəta törədən validator dərhal yenidən qoşula bilməsin. Tipik axın növbəsi `SetParameter(Sumeragi::NextMode)` və `SetParameter(Sumeragi::ModeActivationHeight)` yenilənmiş siyahı ilə.
5. Sübutun əksini təmin etmək üçün `/v1/sumeragi/evidence` və `/v1/sumeragi/status` vasitəsilə **Audit nəticələri** və idarəetmənin aradan qaldırılmasını təmin etmək.

### Birgə Konsensus ardıcıllığı

Birgə konsensus, gedən validator dəstinin yeni dəst təklif etməyə başlamazdan əvvəl sərhəd blokunu yekunlaşdıracağına zəmanət verir. İş vaxtı qoşalaşmış parametrlər vasitəsilə qaydanı tətbiq edir:

- `SumeragiParameter::NextMode` və `SumeragiParameter::ModeActivationHeight` **eyni blokda** yerinə yetirilməlidir. `mode_activation_height` ən azı bir blok gecikməsini təmin etməklə yeniləməni daşıyan blok hündürlüyündən ciddi şəkildə böyük olmalıdır.
- `sumeragi.npos.reconfig.activation_lag_blocks` (defolt `1`) sıfır gecikmə ilə ötürmələrin qarşısını alan konfiqurasiya qoruyucusudur:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (defolt `259200`) konsensusun kəsilməsini gecikdirir ki, idarəetmə cəzaları tətbiq edilməzdən əvvəl ləğv edə bilsin.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- İş vaxtı və CLI `/v1/sumeragi/params` və `iroha --output-format text ops sumeragi params` vasitəsilə mərhələli parametrləri ifşa edir, beləliklə operatorlar aktivləşdirmə hündürlüklərini və validator siyahılarını təsdiq edə bilərlər.
- İdarəetmənin avtomatlaşdırılması həmişə:
  1. Dəlil əsasında çıxarılma (və ya bərpa) qərarını yekunlaşdırın.
  2. `mode_activation_height = h_current + activation_lag_blocks` ilə təkrar konfiqurasiyanı növbəyə qoyun.
  3. `effective_consensus_mode` gözlənilən hündürlükdə çevrilənə qədər `/v1/sumeragi/status` monitoru.

Validatorları döndərən və ya kəsmə tətbiq edən hər hansı skript **sıfır gecikmə ilə aktivləşdirməyə cəhd etməməli və ya ötürülmə parametrlərini buraxmamalıdır; belə əməliyyatlar rədd edilir və şəbəkəni əvvəlki rejimdə tərk edir.

## Telemetriya səthləri

- Prometheus ixrac idarəçiliyi fəaliyyətini ölçür:
  - `governance_proposals_status{status}` (ölçü) statusa görə təkliflərin sayını izləyir.
  - Qorunan ad sahəsinin qəbulu yerləşdirməyə icazə verdikdə və ya rədd etdikdə `governance_protected_namespace_total{outcome}` (əks) artımlar.
  - `governance_manifest_activations_total{event}` (sayğac) manifest daxiletmələrini (`event="manifest_inserted"`) və ad sahəsi bağlamalarını (`event="instance_bound"`) qeyd edir.
- `/status` təkliflərin saylarını əks etdirən, qorunan ad sahəsinin cəmini bildirən və son açıq-aşkar aktivləşdirmələri (ad sahəsi, müqavilə identifikatoru, kod/ABI hash, blok hündürlüyü, aktivləşdirmə vaxt damğası) siyahıya alan `governance` obyekti daxildir. Operatorlar aktların yenilənmiş manifest olduğunu və qorunan ad məkanı qapılarının tətbiq edildiyini təsdiqləmək üçün bu sahədə sorğu keçirə bilər.
- Grafana şablonu (`docs/source/grafana_governance_constraints.json`) və
  `telemetry.md`-də telemetriya runbook ilişib qaldıqda həyəcan siqnallarının necə bağlanacağını göstərir
  təkliflər, çatışmayan manifest aktivləşdirmələri və ya gözlənilməz qorunan ad sahəsi
  iş vaxtı təkmilləşdirmələri zamanı imtinalar.
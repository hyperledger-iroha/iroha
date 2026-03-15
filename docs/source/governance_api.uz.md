---
lang: uz
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

Status: boshqaruvni amalga oshirish vazifalariga qo'shiladigan qoralama/eskiz. Amalga oshirish jarayonida shakllar o'zgarishi mumkin. Determinizm va RBAC siyosati me'yoriy cheklovlardir; Torii, `authority` va `private_key` taqdim etilganda tranzaktsiyalarni imzolashi/yuborishi mumkin, aks holda mijozlar `/transaction` ga tuzadilar va topshiradilar.

Muhim: biz doimiy kengash yoki "standart" boshqaruv ro'yxatini jo'natmaymiz. Kengashning so'nggi nuqtalari yoqilganda bo'sh/kutish holatini qaytaradi yoki sozlangan parametrlardan (ulush aktivi, muddat, qo'mita hajmi) deterministik qaytadi. Operatorlar boshqaruv oqimlari orqali o'z ro'yxatini saqlab qolishlari kerak; bu omborda pishirilgan multisig, maxfiy kalit yoki imtiyozli kengash hisobi mavjud emas.

Umumiy koʻrinish
- Barcha so'nggi nuqtalar JSONni qaytaradi. Tranzaktsiyalarni ishlab chiqaruvchi oqimlar uchun javoblar `tx_instructions` - bir yoki bir nechta ko'rsatmalar skeletlari qatorini o'z ichiga oladi:
  - `wire_id`: ko'rsatma turi uchun registr identifikatori
  - `payload_hex`: Norito foydali yuk baytlari (hex)
- Agar `authority` va `private_key` (yoki ovoz berish byulletenlarida `private_key`) taqdim etilgan bo'lsa, Torii imzo qo'yadi va bitimni topshiradi va baribir `tx_instructions` qaytaradi.
- Aks holda, mijozlar o'zlarining vakolatlari va zanjir_identifikatorlaridan foydalangan holda SignedTransaction ni yig'adilar, keyin imzolaydilar va `/transaction` ga POST qiladilar.
- SDK qamrovi:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` qaytaradi `GovernanceProposalResult` (holat/turdagi maydonlarni normallashtirish), `ToriiClient.get_governance_referendum_typed` qaytaradi `GovernanceReferendumResult`, Prometheus, Prometheus, Sumeragi `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` `GovernanceUnlockStats`, `ToriiClient.list_governance_instances_typed` esa `GovernanceInstancesPage` qaytaradi, bu esa bizni misollar boʻylab terilgan kirishni taʼminlaydi.
- Python engil mijozi (`iroha_torii_client`): `ToriiClient.finalize_referendum` va `ToriiClient.enact_proposal` `GovernanceInstructionDraft` to'plamlarini qaytaradi (Torii skeletini o'rash, Prometheus qo'llanmasini tuzishdan qochish), Oqimlarni yakunlash/aniqlash.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` takliflar, referendumlar, hisob-kitoblar, blokirovkalar, blokirovkalar uchun yozilgan yordamchilarni koʻrsatadi va endi `listGovernanceInstances(namespace, options)` va kengashning soʻnggi nuqtalari (`ToriiClient`, Prometheus, Prometheus `governancePersistCouncil`, `getGovernanceCouncilAudit`) shuning uchun Node.js mijozlari `/v1/gov/instances/{ns}` sahifalarini ajratishi va mavjud shartnoma namunalari roʻyxati bilan bir qatorda VRF tomonidan qoʻllab-quvvatlanadigan ish oqimlarini boshqarishi mumkin. `governanceFinalizeReferendumTyped` va `governanceEnactProposalTyped` har doim tuzilgan qoralamani qaytarish orqali Python yordamchilarini aks ettiradi (Torii `204 No Content` bilan javob berganda bo'sh skeletni sintez qiladi), bu Prometheus tranzaksiyadan oldin tarmoqlanishdan avtomatlashtirishni saqlaydi. yoki tetiklar. `getGovernanceLocksTyped` endi `404 Not Found` javoblarini `{found: false, locks: {}, referendum_id: <id>}` ga normallashtiradi, shuning uchun referendumda qulf bo'lmaganda JS qo'ng'iroq qiluvchilar Python yordamchisi bilan bir xil shakldagi natijaga erishadilar.

Yakuniy nuqtalar- POST `/v1/gov/proposals/deploy-contract`
  - So'rov (JSON):
    {
      "namespace": "ilovalar",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "oyna": { "pastki": 12345, "yuqori": 12400},
      "avtority": "i105...?",
      "private_key": "...?"
    }
  - Javob (JSON):
    { "ok": rost, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Tasdiqlash: tugunlar taqdim etilgan `abi_version` uchun `abi_hash` ni kanoniklashtiradi va mos kelmaslikni rad etadi. `abi_version = "v1"` uchun kutilgan qiymat `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

Contracts API (joylashtirish)
- POST `/v1/contracts/deploy`
  - So'rov: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Xulq-atvor: IVM dastur tanasidan `code_hash` va `abi_version` sarlavhasidan `abi_hash` ni hisoblaydi, so‘ngra `RegisterSmartContractCode` (manifest) va I10840l ni taqdim etadi `.to` bayt) `authority` nomidan.
  - Javob: { "ok": rost, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Tegishli:
    - GET `/v1/contracts/code/{code_hash}` → saqlangan manifestni qaytaradi
    - `/v1/contracts/code-bytes/{code_hash}` ni oling → `{ code_b64 }`ni qaytaradi
- POST `/v1/contracts/instance`
  - So'rov: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Xulq-atvor: Taqdim etilgan baytekodni joylashtiradi va `(namespace, contract_id)` xaritalashni `ActivateContractInstance` orqali darhol faollashtiradi.
  - Javob: { "ok": rost, "nom maydoni": "ilovalar", "kontrakt_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Taxallus xizmati
- POST `/v1/aliases/voprf/evaluate`
  - So'rov: { "blinded_element_hex": "..." }
  - Javob: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` baholovchining amalga oshirilishini aks ettiradi. Joriy qiymat: `blake2b512-mock`.
  - Eslatmalar: `iroha.alias.voprf.mock.v1` domen ajratish bilan Blake2b512 qo'llaydigan deterministik soxta baholovchi. Ishlab chiqarish VOPRF quvur liniyasi Iroha orqali o'tkazilgunga qadar sinov asboblari uchun mo'ljallangan.
  - Xatolar: noto'g'ri tuzilgan hex kiritishda HTTP `400`. Torii dekoder xato xabari bilan Norito `ValidationFail::QueryFailed::Conversion` konvertini qaytaradi.
- POST `/v1/aliases/resolve`
  - So'rov: { "taxallus": "GB82 WEST 1234 5698 7654 32" }
  - Javob: { "taxallus": "GB82WEST12345698765432", "account_id": "i105...", "indeks": 0, "manba": "iso_bridge" }
  - Eslatmalar: ISO ko'prigining ish vaqti bosqichini talab qiladi (`[iso_bridge.account_aliases]`, `iroha_config`). Torii qidiruvdan oldin boʻshliq va bosh harflarni olib tashlash orqali taxalluslarni normallashtiradi. Taxallus mavjud bo'lmaganda 404 va ISO ko'prigining ishlash vaqti o'chirilganda 503 qaytariladi.
- POST `/v1/aliases/resolve_index`
  - So'rov: { "indeks": 0 }
  - Javob: { "indeks": 0, "taxallus": "GB82WEST12345698765432", "account_id": "i105...", "manba": "iso_bridge" }
  - Eslatmalar: taxallus indekslari konfiguratsiya tartibidan (0-asosli) deterministik tarzda tayinlanadi. Taxallus attestatsiya hodisalari uchun audit izlarini yaratish uchun mijozlar javoblarni oflayn rejimda keshlashi mumkin.Kod o'lchami qopqog'i
- Maxsus parametr: `max_contract_code_bytes` (JSON u64)
  - Zanjirli kontrakt kodini saqlash uchun ruxsat etilgan maksimal hajmni (baytlarda) boshqaradi.
  - Standart: 16 Mb. `.to` tasvir uzunligi chegaradan oshib ketganda tugunlar `RegisterSmartContractBytes` ni oʻzgarmas buzilish xatosi bilan rad etadi.
  - Operatorlar `SetParameter(Custom)` ni `id = "max_contract_code_bytes"` va raqamli foydali yukni taqdim etish orqali sozlashlari mumkin.

- POST `/v1/gov/ballots/zk`
  - So'rov: { "avtoritet": "i105...", "private_key": "...?", "chain_id": "...", "select_id": "e1", "proof_b64": "...", "ommaviy": {…} }
  - Javob: { "ok": rost, "qabul qilingan": rost, "tx_instructions": [{…}] }
  - Eslatmalar:
    - Sxemaning umumiy kirishlari `owner`, `amount` va `duration_blocks` ni oʻz ichiga olgan boʻlsa va isbot sozlangan VK ga qarshi tekshirilsa, tugun Sumeragi bilan Prometheus bilan boshqaruv blokini yaratadi yoki kengaytiradi. Yo'nalish yashirin qoladi (`unknown`), agar ishora qilinmasa; faqat miqdor/muddati yangilanadi. Qayta ovoz berish monotondir: miqdor va amal qilish muddati faqat oshadi (tugun max(summa, oldingi miqdor) va max(muddati, oldingi. muddati) amal qiladi).
    - Qulflash bo'yicha har qanday maslahat berilganda, ovoz berish byulletenida `owner`, `amount` va `duration_blocks` bo'lishi kerak; qisman maslahatlar rad etiladi. Qachon `min_bond_amount > 0`, qulflash bo'yicha maslahatlar talab qilinadi.
    - Miqdorni qisqartirishga yoki amal qilish muddatini qisqartirishga urinayotgan ZK qayta ovozlari `BallotRejected` diagnostikasi bilan server tomonida rad etiladi.
    - Shartnomaning bajarilishi `SubmitBallot` navbati oldidan `ZK_VOTE_VERIFY_BALLOT` chaqirilishi kerak; xostlar bir martalik mandalni majbur qiladi.

- POST `/v1/gov/ballots/plain`
  - So'rov: { "avtoritet": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "egasi": "i105...", "summa": "1000", "davomiylik_bloklar": 6000, "absp_taxminiy"
  - Javob: { "ok": rost, "qabul qilingan": rost, "tx_instructions": [{…}] }
  - Eslatmalar: Qayta ovoz berish faqat uzaytiriladi - yangi byulleten mavjud blokirovka miqdorini yoki amal qilish muddatini kamaytira olmaydi. `owner` tranzaksiya vakolatiga teng bo'lishi kerak. Minimal davomiylik `conviction_step_blocks`.- POST `/v1/gov/finalize`
  - So'rov: { "referendum_id": "r1", "proposal_id": "...64hex", "hokimiyat": "i105...?", "private_key": "...?" }
  - Javob: { "ok": rost, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Zanjirdagi effekt (joriy iskala): tasdiqlangan joylashtirish taklifini amalga oshirish kutilgan `abi_hash` bilan `code_hash` tomonidan kalitlangan minimal `ContractManifest` kiritadi va taklifni Qabul qilingan deb belgilaydi. Agar boshqa `abi_hash` bilan `code_hash` uchun manifest allaqachon mavjud bo'lsa, qabul qilish rad etiladi.
  - Eslatmalar:
    - ZK saylovlari uchun shartnoma yo'llari `FinalizeElection` ni bajarishdan oldin `ZK_VOTE_VERIFY_TALLY` raqamiga qo'ng'iroq qilishlari kerak; xostlar bir martalik mandalni majbur qiladi. `FinalizeReferendum` saylovlar yakuni aniqlanmaguncha ZK referendumini rad etadi.
    - `h_end` da avtomatik yopilish faqat oddiy referendumlar uchun tasdiqlangan/rad etilgan chiqaradi; ZK referendumlari yakuniy hisob topshirilmaguncha va `FinalizeReferendum` bajarilmaguncha yopiq qoladi.
    - Saylov ishtirokini tekshirishda faqat tasdiqlash+rad etishdan foydalaniladi; betaraf bo'lish saylovchilarning ishtirokini hisobga olmaydi.

- POST `/v1/gov/enact`
  - So'rov: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "pastki": 0, "yuqori": 0 }?, "hokimiyat": "i105...?", "private_key": "...?" }
  - Javob: { "ok": rost, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Izohlar: Torii imzolangan bitimni `authority`/`private_key` taqdim etilganda taqdim etadi; aks holda u mijozlar imzolashi va topshirishi uchun skeletni qaytaradi. Preimage ixtiyoriy va hozirda ma'lumotlidir.

- `/v1/gov/proposals/{id}` OLING
  - `{id}` yo'li: taklif identifikatori hex (64 belgi)
  - Javob: { "topildi": bool, "taklif": { … }? }

- `/v1/gov/locks/{rid}` OLING
  - `{rid}` yo'li: referendum identifikatori qatori
  - Javob: { "topildi": bool, "referendum_id": "rid", "qulflar": { … }? }

- `/v1/gov/council/current` OLING
  - Javob: { "davr": N, "a'zolar": [{ "account_id": "..." }, …] }
  - Eslatmalar: Qachon mavjud bo'lsa, doimiy kengashni qaytaradi; aks holda konfiguratsiya qilingan ulush aktivi va chegaralari yordamida deterministik qaytarilish hosil qiladi (jonli VRF isbotlari zanjirda saqlanib qolguncha VRF spetsifikatsiyasini aks ettiradi).

- POST `/v1/gov/council/derive-vrf` (xususiyat: gov_vrf)
  - So'rov: { "komitet_size": 21, "davr": 123? , "nomzodlar": [{ "account_id": "...", "variant": "Oddiy|Kichik", "pk_b64": "...", "proof_b64": "..." }, …] }
  - Xulq-atvor: har bir nomzodning VRF isbotini `chain_id`, `epoch` va oxirgi blok xesh-mayoqidan olingan kanonik kiritishga nisbatan tekshiradi; chiqish baytlari bo'yicha saralash desc tiebreakers bilan; eng yaxshi `committee_size` a'zolarini qaytaradi. Davom etmaydi.
  - Javob: { "davr": N, "a'zolar": [{ "account_id": "..." } …], "jami_nomzodlar": M, "tasdiqlangan": K }
  - Eslatmalar: Oddiy = G1 da pk, G2 da isboti (96 bayt). Kichik = G2 da pk, G1 da isboti (48 bayt). Kirishlar domendan ajratilgan va `chain_id` ni o'z ichiga oladi.

### Boshqaruv standartlari (iroha_config `gov.*`)

Doimiy ro'yxat mavjud bo'lmaganda Torii tomonidan qo'llaniladigan kengash zaxirasi `iroha_config` orqali parametrlanadi:```toml
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

Ekvivalent muhit bekor qiladi:

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

Sora Nexus sukut bo'yicha: saylov byulletenlari `min_bond_amount` `voting_asset_id` ichiga qulflanadi
sozlangan eskrow hisobi. Qulflar saylov byulletenlari tushganda yaratiladi yoki uzaytiriladi va
muddati tugashi bilan ozod qilingan; obligatsiyalarning hayot aylanishi `governance_bond_events_total` orqali chiqariladi
telemetriya (lock_created|lock_extended|lock_unlocked|lock_slashed|lock_restituted).

`parliament_committee_size` hech qanday kengash oʻtkazilmaganda qaytarilgan qoʻshimcha aʼzolar sonini cheklaydi, `parliament_term_blocks` urugʻ hosil qilish uchun ishlatiladigan davr uzunligini belgilaydi (`epoch = floor(height / term_blocks)`), `parliament_min_stake` (eng kichik va eng kichik birliklarni talab qiladi) `parliament_eligibility_asset_id` nomzodlar to‘plamini yaratishda qaysi aktiv balansi skanerlanishini tanlaydi.

Boshqaruv VK tekshiruvini chetlab o‘tish imkoniyati yo‘q: ovoz berish byulletenini tekshirish har doim ichki baytlarga ega `Active` tasdiqlash kalitini talab qiladi va muhitlar tekshirishni o‘tkazib yuborish uchun faqat test o‘tish tugmalariga tayanmasligi kerak.

RBAC
- Zanjirda bajarish uchun ruxsatlar talab qilinadi:
  - Takliflar: `CanProposeContractDeployment{ contract_id }`
  - Saylov byulletenlari: `CanSubmitGovernanceBallot{ referendum_id }`
  - Qabul qilish: `CanEnactGovernance`
  - Kesish/murojaatlar: `CanSlashGovernanceLock{ referendum_id }`, `CanRestituteGovernanceLock{ referendum_id }`
  - Kengash boshqaruvi (kelajakda): `CanManageParliament`
- Slashing / murojaatlar:
  - Ikki marta ovoz berish/yaroqsiz/noqobil saylov byulletenlari obligatsiya eskroviga nisbatan sozlangan slash foizlarini qo'llaydi, mablag'larni `slash_receiver_account`-ga o'tkazadi, kesilgan daftarni yangilaydi va `LockSlashed` hodisalarini chiqaradi (sabab + maqsad + eslatma).
  - Qo'llanma `SlashGovernanceLock`/`RestituteGovernanceLock` ko'rsatmalari operator tomonidan boshqariladigan jarimalar va apellyatsiyalarni qo'llab-quvvatlaydi; restitusiya qayd etilgan chiziqchalar bilan chegaralanadi, obligatsiya eskroviga mablag‘larni tiklaydi, daftarni yangilaydi va `LockRestituted` chiqaradi, shu bilan birga qulfni muddati tugaguniga qadar faol ushlab turadi.Himoyalangan nom maydonlari
- Maxsus parametr `gov_protected_namespaces` (JSON qatorlar qatori) roʻyxatdagi nomlar boʻshliqlariga joylashtirish uchun kirish eshiklarini yoqish imkonini beradi.
- Mijozlar himoyalangan nom maydonlariga moʻljallangan joylashtirishlar uchun tranzaksiya metamaʼlumotlari kalitlarini oʻz ichiga olishi kerak:
  - `gov_namespace`: maqsadli nom maydoni (masalan, `"apps"`)
  - `gov_contract_id`: nomlar maydonidagi mantiqiy shartnoma identifikatori
- `gov_manifest_approvers`: i105... hisob identifikatorlarining ixtiyoriy JSON massivi. Agar chiziqli manifest birdan ortiq kvorumni e'lon qilsa, qabul qilish manifest kvorumini qondirish uchun tranzaksiya organi va sanab o'tilgan hisoblarni talab qiladi.
- Telemetriya `governance_manifest_admission_total{result}` orqali yaxlit qabul hisoblagichlarini ochib beradi, shuning uchun operatorlar muvaffaqiyatli qabullarni `missing_manifest`, `non_i105..._authority`, `quorum_rejected`, `protected_namespace_rejected`, `protected_namespace_rejected` va I181010 yoʻllaridan ajrata oladilar.
- Telemetriya ijro yo'lini `governance_manifest_quorum_total{outcome}` (qiymatlari `satisfied` / `rejected`) orqali yuzaga keltiradi, shuning uchun operatorlar etishmayotgan tasdiqlarni tekshirishlari mumkin.
- Yo'llar o'zlarining manifestlarida chop etilgan nomlar maydoni ruxsat etilgan ro'yxatini ta'minlaydi. `gov_namespace` ni o'rnatadigan har qanday tranzaksiya `gov_contract_id` ni ta'minlashi kerak va nom maydoni manifestning `protected_namespaces` to'plamida paydo bo'lishi kerak. Himoya yoqilgan bo'lsa, ushbu metama'lumotlarsiz `RegisterSmartContractCode` taqdimotlari rad etiladi.
- `(namespace, contract_id, code_hash, abi_hash)` korteji uchun qabul qilingan boshqaruv taklifi mavjudligini tasdiqlaydi; aks holda tekshirish Ruxsat berilmagan xato bilan bajarilmaydi.

Runtime Upgrade Hooks
- Lane manifestlari `hooks.runtime_upgrade` ni ish vaqtini yangilash bo'yicha ko'rsatmalarni e'lon qilishi mumkin (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Kanca maydonlari:
  - `allow` (bool, sukut bo'yicha `true`): `false`, ish vaqtini yangilash bo'yicha barcha ko'rsatmalar rad etiladi.
  - `require_metadata` (bool, standart `false`): `metadata_key` tomonidan belgilangan tranzaksiya metamaʼlumotlarini kiritish talab qilinadi.
  - `metadata_key` (string): ilgak tomonidan qo'llaniladigan metadata nomi. Metamaʼlumotlar kerak boʻlganda yoki ruxsat etilgan roʻyxat mavjud boʻlsa, standart `gov_upgrade_id`.
  - `allowed_ids` (satrlar massivi): metadata qiymatlarining ixtiyoriy ruxsat etilgan ro'yxati (qirqib olingandan keyin). Taqdim etilgan qiymat ro'yxatda bo'lmaganda rad etadi.
- Ilgak mavjud bo'lganda, navbatga kirish tranzaksiya navbatga kirishidan oldin metadata siyosatini amalga oshiradi. Yo'qolgan metama'lumotlar, bo'sh qiymatlar yoki ruxsat etilgan ro'yxatdan tashqari qiymatlar deterministik `NotPermitted` xatosini keltirib chiqaradi.
- Telemetriya `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` orqali ijro natijalarini kuzatib boradi.
- Kancani qondiradigan tranzaksiyalar manifest kvorumi talab qiladigan har qanday i105... tasdiqlari bilan birga `gov_upgrade_id=<value>` metamaʼlumotlarini (yoki manifestda belgilangan kalitni) oʻz ichiga olishi kerak.

Qulaylik so'nggi nuqtasi
- POST `/v1/gov/protected-namespaces` - `gov_protected_namespaces` to'g'ridan-to'g'ri tugunga qo'llaniladi.
  - So'rov: { "namespaces": ["ilovalar", "tizim"] }
  - Javob: { "ok": rost, "qo'llaniladi": 1 }
  - Eslatmalar: Administrator/test uchun mo'ljallangan; sozlangan bo'lsa, API tokenini talab qiladi. Ishlab chiqarish uchun `SetParameter(Custom)` bilan imzolangan tranzaksiyani topshirish afzal.CLI yordamchilari
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Nomlar maydoni uchun shartnoma misollarini olib keladi va quyidagilarni tekshiradi:
    - Torii har bir `code_hash` uchun bayt kodini saqlaydi va uning Blake2b-32 dayjesti `code_hash` bilan mos keladi.
    - `/v1/contracts/code/{code_hash}` ostida saqlangan manifest `code_hash` va `abi_hash` qiymatlariga mos kelishi haqida xabar beradi.
    - `(namespace, contract_id, code_hash, abi_hash)` uchun qabul qilingan boshqaruv taklifi tugun foydalanadigan bir xil taklif-identifikatori orqali olingan.
  - Har bir shartnoma bo'yicha `results[]` (muammolar, manifest/kod/takliflar xulosalari) va agar bostirilmasa, bir qatorli xulosa bilan JSON hisobotini chiqaradi (`--no-summary`).
  - Himoyalangan nom maydonlarini tekshirish yoki boshqaruv tomonidan boshqariladigan joylashtirish ish oqimlarini tekshirish uchun foydalidir.
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Manifest kvorum qoidalarini qondirish uchun ixtiyoriy `gov_manifest_approvers`, jumladan, himoyalangan nomlar maydoniga joylashtirishni yuborishda foydalaniladigan JSON metamaʼlumotlar skeletini chiqaradi.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - Kanonik hisob identifikatorlarini tasdiqlaydi, 32 baytlik bekor qiluvchi maslahatlarni kanoniklashtiradi va maslahatlarni `public_inputs_json` ga birlashtiradi (qo'shimcha bekor qilish uchun `--public <path>` bilan).
  - Nullifier tasdiqlovchi majburiyatdan (ommaviy kiritish) va `domain_tag`, `chain_id` va `election_id`dan olingan; `--nullifier` taqdim etilganda isbot bilan tasdiqlangan.
  - The one-line summary now surfaces a deterministic `fingerprint=<hex>` derived from the encoded `CastZkBallot` along with any decoded hints (`owner`, `amount`, `duration_blocks`, `direction` when provided).
  - CLI javoblari `tx_instructions[]` va `payload_fingerprint_hex` va dekodlangan maydonlar bilan izohlanadi, shuning uchun quyi oqim asboblari Norito dekodlashni qayta ishlatmasdan skeletni tekshirishi mumkin.
  - Har qanday blokirovka bo'yicha maslahat berilganda, ZK byulletenlari `owner`, `amount` va `duration_blocks` bilan ta'minlanishi kerak; qisman maslahatlar rad etiladi. Qachon `min_bond_amount > 0`, qulflash bo'yicha maslahatlar talab qilinadi. Yo'nalish ixtiyoriy bo'lib qoladi va faqat maslahat sifatida ko'riladi.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` kanonik I105 literallarini qabul qiladi; ixtiyoriy `@<domain>` qo'shimchalari faqat marshrutlash bo'yicha maslahatlardir.
  - `--lock-amount`/`--lock-duration-blocks` taxalluslari skript pariteti uchun ZK bayroq nomlarini aks ettiradi.
  - `vote --mode zk` ko'rsatmasining xulosasi shifrlangan ko'rsatma barmoq izi va odam o'qiy oladigan saylov byulletenlarini (`owner`, `amount`, `duration_blocks`, `direction` imzolashdan oldin) o'z ichiga oladi.Hodisalar ro'yxati
- GET `/v1/gov/instances/{ns}` - nom maydoni uchun faol shartnoma misollarini ro'yxatlaydi.
  - So'rov parametrlari:
    - `contains`: `contract_id` pastki qatori boʻyicha filtrlash (harf-katta sezgir)
    - `hash_prefix`: `code_hash_hex` olti burchakli prefiks bo'yicha filtrlash (kichik harf)
    - `offset` (standart 0), `limit` (standart 100, maksimal 10_000)
    - `order`: biri `cid_asc` (standart), `cid_desc`, `hash_asc`, `hash_desc`
  - Javob: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, …], "jami": N, "ofset": n, "chegara": m }
  - SDK yordamchisi: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) yoki `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Qulfni ochish (Operator/Audit)
- `/v1/gov/unlocks/stats` OLING
  - Javob: { "balandlik_joriy": H, "muddati tugagan_qulflar": n, "muddati tugagan_referendum": m, "oxirgi_supurish_balandligi": S }
  - Eslatmalar: `last_sweep_height` muddati o'tgan qulflar tozalangan va saqlanib qolgan eng so'nggi blok balandligini aks ettiradi. `expired_locks_now` `expiry_height <= height_current` bilan blokirovka yozuvlarini skanerlash orqali hisoblanadi.
- POST `/v1/gov/ballots/zk-v1`
  - So'rov (v1 uslubidagi DTO):
    {
      "hokimiyat": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64hex?",
      "owner": "i105…", // kanonik AccountId (I105 literal; ixtiyoriy @domen maslahati)
      "summa": "100?",
      "duration_blocks": 6000?,
      "direction": "Ha|Yo'q|Tixtash kerakmi?",
      "nullifier": "blake2b32:...64hex?"
    }
  - Javob: { "ok": rost, "qabul qilingan": rost, "tx_instructions": [{…}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (xususiyat: `zk-ballot`)
  - `BallotProof` JSON-ni to'g'ridan-to'g'ri qabul qiladi va `CastZkBallot` skeletini qaytaradi.
  - Talab:
    {
      "hokimiyat": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "saylov byulleteni": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // ZK1 yoki H2* konteynerining 64 bazasi
        "root_hint": null, // ixtiyoriy 32 baytli hex string (muvofiqlik ildizi)
        "egasi": null, // ixtiyoriy kanonik AccountId (I105 literal; ixtiyoriy @domen maslahati)
        "nullifier": null, // ixtiyoriy 32 baytlik hex string (nollifier maslahati)
        "summa": "100", // ixtiyoriy blokirovka miqdori bo'yicha maslahat (o'nlik qator)
        "duration_blocks": 6000, // ixtiyoriy blokirovka muddati haqida maslahat
        "yo'nalish": "Ha" // ixtiyoriy yo'nalishga ishora
      }
    }
  - Javob:
    {
      "OK": rost,
      "qabul qilingan": rost,
      "reason": "tranzaksiya skeletini qurish",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Eslatmalar:
    - `private_key` taqdim etilganda, Torii imzolangan bitimni taqdim etadi va `reason` ni `submitted transaction` ga o'rnatadi.
    - Server ixtiyoriy `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier` dan I018 uchun saylov byulletenini00000280Xga xaritalaydi. `CastZkBallot`.
    - Zarf baytlari ko'rsatmalarning foydali yuki uchun base64 sifatida qayta kodlangan.
    - Bu oxirgi nuqta faqat `zk-ballot` funksiyasi yoqilganda mavjud.

CastZkBallot tekshirish yo'li
- `CastZkBallot` taqdim etilgan base64 isbotini dekodlaydi va bo'sh yoki noto'g'ri tuzilgan foydali yuklarni rad etadi (`BallotRejected`, `invalid or empty proof` bilan).
- Agar `public_inputs_json` ta'minlangan bo'lsa, u JSON ob'ekti bo'lishi kerak; ob'ekt bo'lmagan foydali yuklar rad etiladi.
- Uy egasi referendumdan (`vk_ballot`) ovoz berish byulletenini tasdiqlash kalitini yoki boshqaruv sukutini hal qiladi va yozuv mavjud boʻlishini, `Active` boʻlishini va ichki baytlarni tashishni talab qiladi.
- Saqlangan tasdiqlovchi kalit baytlar `hash_vk` bilan qayta xeshlanadi; har qanday majburiyatning nomuvofiqligi ro'yxatga olish kitobining o'zgartirilgan yozuvlaridan (`BallotRejected` bilan `verifying key commitment mismatch`) himoya qilish uchun tekshirishdan oldin ijroni to'xtatadi.
- Tasdiqlash baytlari `zk::verify_backend` orqali ro'yxatdan o'tgan backendga yuboriladi; yaroqsiz transkriptlar `invalid proof` bilan `BallotRejected` sifatida yuzaga chiqadi va ko'rsatma aniq bajarilmaydi.
- dalil saylov byulletenlari majburiyatini va saylov huquqining ildizini jamoatchilik fikri sifatida ko'rsatishi kerak; ildiz saylovning `eligible_root` ga mos kelishi kerak va olingan bekor qiluvchi har qanday berilgan maslahatga mos kelishi kerak.
- Muvaffaqiyatli dalillar `BallotAccepted` chiqaradi; takroriy bekor qiluvchilar, eskirgan muvofiqlik ildizlari yoki blokirovka regresslari ushbu hujjatda avval tasvirlangan mavjud rad etish sabablarini keltirib chiqarishda davom etmoqda.

## Tekshiruvchining noto'g'ri xatti-harakati va qo'shma konsensus

### Slashing va Jailing Ish jarayoniI105... protokolni buzsa, konsensus Norito kodli `Evidence` chiqaradi. Har bir foydali yuk `EvidenceStore` xotirasiga tushadi va agar ko'rinmasa, WSV tomonidan qo'llab-quvvatlanadigan `consensus_evidence` xaritasida amalga oshiriladi. `sumeragi.npos.reconfig.evidence_horizon_blocks` dan eskiroq yozuvlar (standart `7200` bloklari) rad etiladi, shuning uchun arxiv cheklangan bo'lib qoladi, lekin rad etish operatorlar uchun qayd etiladi. Ufqdagi dalillar qo'shma konsensus bosqichma-bosqich qoidasiga (`mode_activation_height requires next_mode to be set in the same block`), faollashtirish kechikishiga (`sumeragi.npos.reconfig.activation_lag_blocks`, standart `1`) va kesish kechikishiga (`sumeragi.npos.reconfig.slashing_delay_blocks`, standart I00700X) bo'ysunadi. qo'llashdan oldin jarimalar.

Tan olingan jinoyatlar `EvidenceKind` ga birma-bir xarita; diskriminantlar barqaror va ma'lumotlar modeli tomonidan amalga oshiriladi:

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

- **DoublePrepare/DoubleCommit** — bir xil `(phase,height,view,epoch)` korteji uchun i105... imzolangan ziddiyatli xeshlar.
- **InvalidQc** — agregator shakli deterministik tekshiruvlardan o‘tmagan (masalan, bo‘sh imzo bitmap) QC haqida g‘iybat qildi.
- **InvalidProposal** — yetakchi tizimli tekshiruvdan o‘tmagan blokni taklif qildi (masalan, qulflangan zanjir qoidasini buzadi).
- **Tsenzura** — imzolangan topshirish kvitansiyalari hech qachon taklif qilinmagan/tugallanmagan tranzaksiyani ko'rsatadi.

VRF jazolari `activation_lag_blocks` dan keyin avtomatik ravishda amalga oshiriladi (jinoyatchilar qamoqqa olinadi). Konsensus kesish faqat `slashing_delay_blocks` oynasidan keyin qo'llaniladi, agar boshqaruv jazoni bekor qilmasa.

Operatorlar va asboblar foydali yuklarni tekshirishi va qayta uzatishi mumkin:

- Torii: `GET /v1/sumeragi/evidence` va `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `… count` va `… submit --evidence-hex <payload>`.

Boshqaruv dalillar baytlarini kanonik dalil sifatida ko'rib chiqishi kerak:

1. **Foydali yukni eskirishdan oldin to'plang**. Norito baytlarini balandlik/koʻrish metamaʼlumotlari bilan birga arxivlang.
2. **Agar kerak bo'lsa, bekor qiling** `CancelConsensusEvidencePenalty` hujjat yuki bilan `slashing_delay_blocks` muddati tugagunga qadar taqdim etish; yozuv `penalty_cancelled` va `penalty_cancelled_at_height` deb belgilangan va hech qanday kesish qo'llanilmaydi.
3. Referendum yoki sudo koʻrsatmasi (masalan, `Unregister::peer`). Bajarish foydali yukni qayta tasdiqlaydi; noto'g'ri shakllangan yoki eskirgan dalillar deterministik ravishda rad etiladi.
4. **Keyingi topologiyani rejalashtirish** shunday qilib, qoidabuzar i105... darhol qayta qo‘shila olmaydi. Odatdagi oqimlar navbati `SetParameter(Sumeragi::NextMode)` va `SetParameter(Sumeragi::ModeActivationHeight)` yangilangan ro'yxat bilan.
5. **Audit natijalari** `/v1/sumeragi/evidence` va `/v1/sumeragi/status` orqali dalil hisoblagichning ilg'orligini ta'minlash va boshqaruv olib tashlashni amalga oshirgan.

### Birgalikda konsensus ketma-ketligi

Qo'shma konsensus, chiquvchi i105... to'plami yangi to'plam taklif qilishni boshlashdan oldin chegara blokini yakunlashini kafolatlaydi. Ish vaqti qoidani juftlashtirilgan parametrlar orqali amalga oshiradi:- `SumeragiParameter::NextMode` va `SumeragiParameter::ModeActivationHeight` **bir xil blokda** amalga oshirilishi kerak. `mode_activation_height` kamida bir blokli kechikishni ta'minlab, yangilanishni amalga oshirgan blok balandligidan qat'iy kattaroq bo'lishi kerak.
- `sumeragi.npos.reconfig.activation_lag_blocks` (sukut bo'yicha `1`) nol kechikishning oldini oluvchi konfiguratsiya himoyachisi:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (standart `259200`) konsensusni qisqartirishni kechiktiradi, shuning uchun boshqaruv jazolarni ular qo'llanilishidan oldin bekor qilishi mumkin.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Ish vaqti va CLI bosqichli parametrlarni `/v1/sumeragi/params` va `iroha sumeragi params --summary` orqali ochib beradi, shuning uchun operatorlar faollashtirish balandligi va i105... ro'yxatlarini tasdiqlashlari mumkin.
- boshqaruvni avtomatlashtirish har doim:
  1. Dalillarga asoslangan olib tashlash (yoki qayta tiklash) to'g'risidagi qarorni yakunlang.
  2. `mode_activation_height = h_current + activation_lag_blocks` bilan keyingi qayta konfiguratsiyani navbatga qo'ying.
  3. `effective_consensus_mode` kutilgan balandlikda aylanguncha monitor `/v1/sumeragi/status`.

i105...s aylantiruvchi yoki kesishni qo'llaydigan har qanday skript **kechikishsiz faollashtirishga urinmasligi yoki uzatish parametrlarini o'tkazib yubormasligi kerak; bunday operatsiyalar rad etiladi va tarmoqni avvalgi rejimda qoldiradi.

## Telemetriya sirtlari

- Prometheus eksportni boshqarish faoliyati ko'rsatkichlari:
  - `governance_proposals_status{status}` (oʻlchagich) status boʻyicha takliflar sonini kuzatib boradi.
  - `governance_protected_namespace_total{outcome}` (hisoblagich) himoyalangan nomlar maydoniga kirish joylashtirishga ruxsat bergan yoki rad etganda oshadi.
  - `governance_manifest_activations_total{event}` (hisoblagich) manifest qo'shishlarni (`event="manifest_inserted"`) va nomlar maydonini bog'lashni (`event="instance_bound"`) qayd qiladi.
- `/status` takliflar sonini aks ettiruvchi `governance` ob'ektini o'z ichiga oladi, himoyalangan nomlar maydoni jami to'g'risida hisobot beradi va so'nggi manifest faollashuvlari ro'yxatini (nomlar maydoni, shartnoma identifikatori, kod/ABI xesh, blok balandligi, faollashtirish vaqt tamg'asi) ko'rsatadi. Operatorlar ushbu maydonda yangilangan qonunlar manifestligini va himoyalangan nom maydoni eshiklari kuchga kirganligini tasdiqlash uchun so'rov o'tkazishlari mumkin.
- Grafana shabloni (`docs/source/grafana_governance_constraints.json`) va
  `telemetry.md`-dagi telemetriya runbook tiqilib qolgan signallarni qanday ulash kerakligini ko'rsatadi
  takliflar, etishmayotgan manifest faollashtirishlari yoki kutilmagan himoyalangan nomlar maydoni
  ish vaqtini yangilash paytida rad etishlar.
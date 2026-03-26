---
lang: am
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 300ae819b5315b1ae4edab6caffc936e506c0d550a1ba75be1ace4d42f8e0b11
source_last_modified: "2026-01-28T17:11:30.701656+00:00"
translation_last_reviewed: 2026-02-07
id: registrar-api
title: Sora Name Service Registrar API & Governance Hooks
sidebar_label: Registrar API
description: Torii REST/gRPC surfaces, Norito DTOs, and governance artifacts for SNS registrations (SN-2b).
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ `docs/source/sns/registrar_api.md`ን ያንጸባርቃል እና አሁን እንደ የ
ቀኖናዊ ፖርታል ቅጂ. የምንጭ ፋይሉ ለትርጉም የስራ ፍሰቶች ይቀራል።
::

# የኤስኤንኤስ መመዝገቢያ ኤፒአይ እና የአስተዳደር መንጠቆዎች (SN-2b)

** ሁኔታ፡** የተረቀቀው 2026-03-24 -- በNexus ኮር ግምገማ ስር  
** የመንገድ ካርታ አገናኝ፡** SN-2b “የሬጅስትራር ኤፒአይ እና የአስተዳደር መንጠቆዎች”  
** ቅድመ ሁኔታዎች፡** የመርሃግብር ፍቺዎች በ[`registry-schema.md`](I18NU0000027X)

ይህ ማስታወሻ የTorii የመጨረሻ ነጥቦችን፣ gRPC አገልግሎቶችን፣ ጥያቄ/ምላሽ DTOs እና የአስተዳደር ቅርሶችን የሶራ ስም አገልግሎት (ኤስኤንኤስ) መዝጋቢን ይገልፃል። የኤስኤንኤስ ስሞችን መመዝገብ፣ ማደስ ወይም ማስተዳደር የሚያስፈልጋቸው የኤስዲኬዎች፣ የኪስ ቦርሳዎች እና አውቶማቲክ ውል ነው።

## 1. መጓጓዣ እና ማረጋገጥ

| መስፈርት | ዝርዝር |
|------------|----|
| ፕሮቶኮሎች | በ`/v1/sns/*` እና gRPC አገልግሎት `sns.v1.Registrar` ስር አርፈው። ሁለቱም Norito-JSON (`application/json`) እና Norito-RPC ሁለትዮሽ (`application/x-norito`) ይቀበላሉ። |
| Auth | `Authorization: Bearer` ማስመሰያዎች ወይም mTLS ሰርተፊኬቶች በቅጥያ መጋቢ የተሰጠ። ለአስተዳደር-ስሱ የመጨረሻ ነጥቦች (ማሰር/የማይቀዘቅዙ፣ የተያዙ ስራዎች) `scope=sns.admin` ያስፈልጋቸዋል። |
| የዋጋ ገደቦች | ሬጅስትራሮች የI18NI0000038X ባልዲዎችን ከJSON ደዋዮች እና ከቅጥያ ፍንዳታ ኮፍያዎች ጋር ይጋራሉ፡ `sns.register`፣ `sns.renew`፣ `sns.controller`፣ `sns.freeze`። |
| ቴሌሜትሪ | Torii `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` ለሬጅስትራር ተቆጣጣሪዎች ያጋልጣል (በ`scheme="norito_rpc"` ላይ ማጣሪያ); ኤፒአይ `sns_registrar_status_total{result, suffix_id}` ይጨምራል። |

## 2. DTO አጠቃላይ እይታ

መስኮች በ[`registry-schema.md`](./registry-schema.md) ውስጥ የተገለጹትን ቀኖናዊ አወቃቀሮችን ይጠቅሳሉ። ሁሉም የሚጫኑ ጭነቶች አሻሚ ማዘዋወርን ለማስወገድ `NameSelectorV1` + `SuffixId`ን አካተዋል።

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. REST የመጨረሻ ነጥቦች

| የመጨረሻ ነጥብ | ዘዴ | ጭነት | መግለጫ |
|-------|--------|----
| `/v1/sns/names` | መለጠፍ | `RegisterNameRequestV1` | ስም ይመዝገቡ ወይም እንደገና ይክፈቱ። የዋጋ ደረጃን ይፈታል፣ የክፍያ/የአስተዳደር ማረጋገጫዎችን ያጸድቃል፣ የመመዝገቢያ ክስተቶችን ያወጣል። |
| `/v1/sns/names/{namespace}/{literal}/renew` | መለጠፍ | `RenewNameRequestV1` | ጊዜን ያራዝሙ። ከፖሊሲ የጸጋ/መቤዠት መስኮቶችን ያስገድዳል። |
| `/v1/sns/names/{namespace}/{literal}/transfer` | መለጠፍ | `TransferNameRequestV1` | የአስተዳደር ማፅደቆች ከተያያዙ በኋላ ባለቤትነትን ያስተላልፉ። |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | የመቆጣጠሪያውን ስብስብ ይተኩ; የተፈረመ የመለያ አድራሻዎችን ያረጋግጣል። |
| `/v1/sns/names/{namespace}/{literal}/freeze` | መለጠፍ | `FreezeNameRequestV1` | ጠባቂ/ካውንስል ቀረ። የአሳዳጊ ትኬት እና የአስተዳደር ሰነድ ማጣቀሻ ያስፈልገዋል። |
| `/v1/sns/names/{namespace}/{literal}/freeze` | ሰርዝ | `GovernanceHookV1` | ከማስተካከያው በኋላ አይቀዘቅዙ; የምክር ቤቱ መሻር መመዝገቡን ያረጋግጣል። |
| `/v1/sns/reserved/{selector}` | መለጠፍ | `ReservedAssignmentRequestV1` | የተያዙ ስሞች መጋቢ/ምክር ቤት ምደባ። |
| `/v1/sns/policies/{suffix_id}` | አግኝ | - | የአሁኑን `SuffixPolicyV1` (መሸጎጫ) ያውጡ። |
| `/v1/sns/names/{namespace}/{literal}` | አግኝ | - | የአሁኑን I18NI0000067X + ውጤታማ ሁኔታ ይመልሳል (ገባሪ፣ ጸጋ፣ ወዘተ)። |

** የመራጭ ኢንኮዲንግ፡** የ`{selector}` ዱካ ክፍል i105 (ተመራጭ)፣ የተጨመቀ (`sora`፣ ሁለተኛ-ምርጥ)፣ ወይም ቀኖናዊ ሄክስ በ ADDR-5 ይቀበላል። Torii በ I18NI0000070X በኩል መደበኛ ያደርገዋል።

** የስህተት ሞዴል፡** ሁሉም የመጨረሻ ነጥቦች Norito JSON በ `code`፣ `message`፣ `details` ይመለሳሉ። ኮዶች `sns_err_reserved`፣ `sns_err_payment_mismatch`፣ `sns_err_policy_violation`፣ I18NI0000077X ያካትታሉ።

### 3.1 CLI ረዳቶች (N0 በእጅ ሬጅስትራር መስፈርት)

የተዘጉ የቅድመ-ይሁንታ መጋቢዎች አሁን JSON ሳይሠሩ መዝጋቢውን በCLI በኩል ማከናወን ይችላሉ፡

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` ነባሪዎች ወደ CLI ውቅር መለያ; ተጨማሪ ተቆጣጣሪ መለያዎችን ለማያያዝ `--controller` ድገም (ነባሪው `[owner]`)።
- የመስመር ውስጥ ክፍያ ባንዲራዎች በቀጥታ ወደ `PaymentProofV1`; አስቀድሞ የተዋቀረ ደረሰኝ ሲኖርዎት `--payment-json PATH` ማለፍ። ሜታዳታ (I18NI0000083X) እና የአስተዳደር መንጠቆዎች (`--governance-json`) ተመሳሳይ ስርዓተ-ጥለት ይከተላሉ።

ንባብ-ብቻ ረዳቶች ልምምዶችን ያጠናቅቃሉ፡

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

ለትግበራው `crates/iroha_cli/src/commands/sns.rs` ይመልከቱ; ትእዛዞቹ በዚህ ሰነድ ውስጥ የተገለጹትን I18NT0000003X DTOs እንደገና ይጠቀማሉ ስለዚህ የ CLI ውፅዓት ከ Torii ምላሾች ባይት-ለ-ባይት ጋር ይዛመዳል።

ተጨማሪ ረዳቶች እድሳትን፣ ማስተላለፎችን እና የአሳዳጊ እርምጃዎችን ይሸፍናሉ፡

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner <katakana-i105-account-id> \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` ትክክለኛ የ`GovernanceHookV1` መዝገብ (የፕሮፖዛል መታወቂያ፣የድምፅ ሃሽ፣የመጋቢ/አሳዳጊ ፊርማ) መያዝ አለበት። እያንዳንዱ ትዕዛዝ ተጓዳኝ `/v1/sns/names/{namespace}/{literal}/…` የመጨረሻ ነጥብን ያንፀባርቃል ስለዚህ የቅድመ-ይሁንታ ኦፕሬተሮች ኤስዲኬዎች የሚጠሩትን ትክክለኛ የTorii ወለል መልመድ ይችላሉ።

## 4. gRPC አገልግሎት

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

ሽቦ-ቅርጸት፡- የማጠናቀር Norito ሼማ ሃሽ በስር ተመዝግቧል
`fixtures/norito_rpc/schema_hashes.json` (ረድፎች `RegisterNameRequestV1`፣
`RegisterNameResponseV1`፣ `NameRecordV1`፣ ወዘተ)።

## 5. የአስተዳደር መንጠቆዎች እና ማስረጃዎች

እያንዳንዱ ሚውቴሽን ጥሪ ለመድገም ተስማሚ የሆነ ማስረጃ ማያያዝ አለበት፡-

| ድርጊት | አስፈላጊ አስተዳደር ውሂብ |
|--------|-------------|
| መደበኛ መመዝገቢያ/እድሳት | የመቋቋሚያ መመሪያን የሚያመለክት የክፍያ ማረጋገጫ; ደረጃ መጋቢ ማጽደቅን ካልጠየቀ በስተቀር የምክር ቤት ድምጽ አያስፈልግም። |
| የፕሪሚየም ደረጃ መመዝገቢያ / የተያዘ ምደባ | `GovernanceHookV1` የማጣቀሻ ፕሮፖዛል መታወቂያ + መጋቢ እውቅና። |
| ማስተላለፍ | ምክር ቤት ድምጽ hash + DAO ምልክት hash; በክርክር መፍታት ሲቀሰቀስ የአሳዳጊ ማረጋገጫ። |
| እሰር / ፍታ | የጠባቂ ቲኬት ፊርማ እና የምክር ቤት መሻር (ያልቀዘቀዘ)። |

Torii ማረጋገጫዎችን በማረጋገጥ ያረጋግጣል፡-

1. የፕሮፖዛል መታወቂያ በአስተዳደር ደብተር (`/v1/governance/proposals/{id}`) ውስጥ አለ እና ደረጃው `Approved` ነው።
2. Hashes ከተመዘገቡት የድምጽ ቅርሶች ጋር ይዛመዳል።
3. መጋቢ/አሳዳጊ ፊርማዎች የሚጠበቁትን የህዝብ ቁልፎች ከI18NI0000096X ይጠቅሳሉ።

ያልተሳኩ ቼኮች መመለስ I18NI0000097X።

## 6. የስራ ፍሰት ምሳሌዎች

### 6.1 መደበኛ ምዝገባ

1. የደንበኛ ጥያቄዎች I18NI0000098X ዋጋ አሰጣጥን፣ ጸጋን እና የሚገኙ ደረጃዎችን ለማምጣት።
2. ደንበኛ `RegisterNameRequestV1` ይገነባል፡
   - `selector` ከተመረጡት i105 ወይም ሁለተኛ-ምርጥ የታመቀ (`sora`) መለያ የተገኘ።
   - `term_years` በፖሊሲ ገደቦች ውስጥ።
   - `payment` የግምጃ ቤት / መጋቢ ክፍፍል ማስተላለፍን በመጥቀስ።
3. Torii ያረጋግጣል፡-
   - መለያ መደበኛነት + የተያዘ ዝርዝር።
   - ጊዜ/ጠቅላላ ዋጋ ከ `PriceTierV1` ጋር።
   - የክፍያ ማረጋገጫ መጠን >= የተሰላ ዋጋ + ክፍያዎች።
4. በስኬት I18NT0000016X፡
   - `NameRecordV1` ይቀጥላል።
   - `RegistryEventV1::NameRegistered` ያወጣል።
   - `RevenueAccrualEventV1` ያወጣል።
   - አዲሱን መዝገብ + ክስተቶች ይመልሳል።

### 6.2 በጸጋ ጊዜ መታደስ

የጸጋ እድሳት መደበኛውን ጥያቄ እና የቅጣት ማወቅን ያካትታል፡-

- Torii ቼኮች `now` vs `grace_expires_at` እና ተጨማሪ ክፍያ ሰንጠረዦችን ከ `SuffixPolicyV1` ይጨምራል።
- የክፍያ ማረጋገጫ ተጨማሪ ክፍያን መሸፈን አለበት። ውድቀት => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` አዲሱን I18NI0000113X ይመዘግባል።

### 6.3 ጠባቂ እሰር እና ምክር ቤት መሻር

1. ጠባቂ `FreezeNameRequestV1` ከቲኬት አጣቃሽ ክስተት መታወቂያ ጋር ያቀርባል።
2. Torii ሪኮርድን ወደ `NameStatus::Frozen` ያንቀሳቅሳል፣ `NameFrozen` ያወጣል።
3. ከሽምግልና በኋላ, ምክር ቤት ጉዳዮች ይሻራሉ; ኦፕሬተር DELETE I18NI0000117X በ `GovernanceHookV1` ይልካል።
4. Torii መሻርን ያረጋግጣል፣ `NameUnfrozen` ያወጣል።

## 7. የማረጋገጫ እና የስህተት ኮዶች

| ኮድ | መግለጫ | HTTP |
|------|---------|------|
| `sns_err_reserved` | መለያው ተይዟል ወይም ታግዷል። | 409 |
| `sns_err_policy_violation` | የአገልግሎት ጊዜ፣ እርከን ወይም የመቆጣጠሪያ ስብስብ ፖሊሲን ይጥሳል። | 422 |
| `sns_err_payment_mismatch` | የክፍያ ማረጋገጫ እሴት ወይም የንብረት አለመመጣጠን። | 402 |
| `sns_err_governance_missing` | የሚፈለጉ የአስተዳደር ቅርሶች የሉም/ልክ አይደሉም። | 403 |
| `sns_err_state_conflict` | አሁን ባለው የህይወት ዑደት ሁኔታ ውስጥ ክወና አይፈቀድም። | 409 |

ሁሉም ኮዶች በI18NI0000125X እና የተዋቀሩ Norito JSON/NRPC ፖስታዎች በኩል ይታያሉ።

## 8. የትግበራ ማስታወሻዎች

- Torii በ `NameRecordV1.auction` ጨረታ በመጠባበቅ ላይ ያከማቻል እና ቀጥተኛ የምዝገባ ሙከራዎችን አይቀበልም `PendingAuction`።
- የክፍያ ማረጋገጫዎች Norito የመመዝገቢያ ደረሰኞችን እንደገና ጥቅም ላይ ማዋል; የግምጃ ቤት አገልግሎቶች አጋዥ APIs (`/v1/finance/sns/payments`) ይሰጣሉ።
- የኪስ ቦርሳዎች ግልጽ የስህተት ምክንያቶችን (`ERR_SNS_RESERVED`፣ ወዘተ) እንዲያቀርቡ ኤስዲኬዎች እነዚህን የመጨረሻ ነጥቦች በጠንካራ የተተየቡ ረዳቶች መጠቅለል አለባቸው።

## 9. ቀጣይ ደረጃዎች

- SN-3 ጨረታዎችን አንድ ጊዜ የ Torii ተቆጣጣሪዎችን ወደ ትክክለኛው የመመዝገቢያ ውል ያገናኙ።
- ይህን ኤፒአይ የሚያመለክቱ ኤስዲኬ-ተኮር መመሪያዎችን (Rust/JS/Swift) ያትሙ።
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) ከአስተዳደር መንጠቆ የማስረጃ መስኮች ጋር ያራዝሙ።
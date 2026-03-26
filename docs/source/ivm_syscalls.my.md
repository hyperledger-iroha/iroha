---
lang: my
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:26:46.570453+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#IVM Syscall ABI

ဤစာရွက်စာတမ်းသည် IVM syscall နံပါတ်များ၊ pointer-ABI ခေါ်ဆိုမှုဆိုင်ရာ သဘောတူညီချက်များ၊ သီးသန့်နံပါတ်အပိုင်းအခြားများနှင့် Kotodama လျှော့ချခြင်းဖြင့် အသုံးပြုသည့် စာချုပ်ပါ syscall များ၏ canonical table ကို သတ်မှတ်ပေးပါသည်။ ၎င်းသည် `ivm.md` (ဗိသုကာ) နှင့် `kotodama_grammar.md` (ဘာသာစကား) တို့ကို ဖြည့်စွက်ထားသည်။

ဗားရှင်းပြောင်းခြင်း။
- အသိအမှတ်ပြုထားသော syscalls အစုံသည် bytecode ခေါင်းစီး `abi_version` အကွက်ပေါ်တွင် မူတည်သည်။ ပထမထွက်ရှိမှုသည် `abi_version = 1` ကိုသာ လက်ခံသည်။ အခြားတန်ဖိုးများကို ဝင်ခွင့်တွင် ပယ်ချပါသည်။ လက်ရှိအသုံးပြုနေသော `abi_version` အတွက် အမည်မသိနံပါတ်များသည် `E_SCALL_UNKNOWN` ဖြင့် အဆုံးအဖြတ်ပေးသော ထောင်ချောက်ဖြစ်သည်။
- Runtime အဆင့်မြှင့်တင်မှုများသည် `abi_version = 1` ကို ထိန်းသိမ်းထားပြီး syscall သို့မဟုတ် pointer-ABI မျက်နှာပြင်များကို ချဲ့ထွင်ခြင်းမပြုပါ။
- Syscall ဓာတ်ငွေ့ကုန်ကျစရိတ်များသည် bytecode ခေါင်းစီးဗားရှင်းနှင့် ချိတ်ဆက်ထားသော ဓာတ်ငွေ့အချိန်ဇယား၏ တစ်စိတ်တစ်ပိုင်းဖြစ်သည်။ `ivm.md` (ဓာတ်ငွေ့မူဝါဒ) ကိုကြည့်ပါ။

နံပါတ်အပိုင်းအခြားများ
- `0x00..=0x1F`- VM core/utility (debug/exit helpers များသည် `CoreHost` အောက်တွင် ရနိုင်သည်၊ ကျန်သော dev helpers များသည် mock-host သာဖြစ်သည်)။
- `0x20..=0x5F`- Iroha core ISI တံတား (ABI v1 တွင် တည်ငြိမ်သည်)။
- `0x60..=0x7F`- ပရိုတိုကော အင်္ဂါရပ်များဖြင့် ပိတ်ဆို့ထားသော တိုးချဲ့မှု ISI များ (ဖွင့်ထားချိန်တွင် ABI v1 ၏ တစ်စိတ်တစ်ပိုင်း ဖြစ်နေဆဲ)။
- `0x80..=0xFF`- host/crypto အထောက်အကူများနှင့် သီးသန့် slot များ; ABI v1 ခွင့်ပြုစာရင်းတွင်ပါရှိသော နံပါတ်များကိုသာ လက်ခံပါသည်။

တာရှည်ခံကူညီသူများ (ABI v1)
- တာရှည်ခံသောအခြေအနေအကူအညီပေးသည့် syscalls (0x50–0x5A- STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA ကုဒ်/ကုဒ်) များသည် V1 ABI ၏ တစ်စိတ်တစ်ပိုင်းဖြစ်ပြီး `abi_hash` တွက်ချက်မှုတွင် ပါဝင်ပါသည်။
- CoreHost ဝါယာကြိုးများသည် STATE_{GET,SET,DEL} သို့ WSV-ကျောထောက်နောက်ခံပြု တာရှည်ခံစမတ်-စာချုပ်အခြေအနေ၊ dev/test host များသည် စက်တွင်းတွင် ဆက်လက်တည်ရှိနေနိုင်သော်လည်း တူညီသော syscall semantics ကို ထိန်းသိမ်းထားရပါမည်။

Pointer-ABI ခေါ်ဆိုမှုကွန်ဗင်းရှင်း (စမတ်-ကန်ထရိုက် syscalls)
- Arguments များကို `r10+` ၏ အကြမ်းထည် `u64` တန်ဖိုးများအဖြစ် သို့မဟုတ် မပြောင်းလဲနိုင်သော Norito TLV စာအိတ်များသို့ ညွှန်ပြချက်များအဖြစ် `AccountId`, Iroha၊ `Name`, `Json`, `NftId`)။
- Scalar return တန်ဖိုးများသည် host မှ ပြန်ပေးသော `u64` ဖြစ်သည်။ Pointer ရလဒ်များကို `r10` တွင် host မှရေးသားပါသည်။

Canonical syscall table (subset)| Hex | အမည် | အကြောင်းပြချက်များ (`r10+`) | ပြန်လာ | ဓာတ်ငွေ့ (base + variable) | မှတ်စုများ |
|--------|--------------------------------|--------------------------------------------------------------------------------|----------------------------------------------------------------|-----|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | အကောင့် | အတွက်အသေးစိတ်ရေးပါ။
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | အကောင့်သို့ပိုင်ဆိုင်မှု၏ `amount` ကို Mints
| 0x23 | BURN_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | အကောင့် | မှ `amount` ကိုလောင်ကျွမ်းသည်။
| 0x24 | TRANSFER_ASSET | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | `amount` အကောင့်များအကြား လွှဲပြောင်းမှုများ |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | FASTPQ လွှဲပြောင်းမှုအသုတ်နယ်ပယ် | စတင်ပါ။
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | စုဆောင်းထားသော FASTPQ လွှဲပြောင်းအသုတ် |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | syscall တစ်ခုတည်းတွင် Norito-encoded batch တစ်ခုကို အသုံးပြုပါ |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | NFT | အသစ်တစ်ခုကို မှတ်ပုံတင်ပါ။
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | NFT | ပိုင်ဆိုင်မှုကို လွှဲပြောင်းပေးသည်။
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | NFT မက်တာဒေတာကို အပ်ဒိတ်များ |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | NFT | မီးလောင်သည် (ဖျက်ဆီးသည်)
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | ထပ်ခါတလဲလဲ မေးမြန်းမှုများသည် တဒင်္ဂဖြင့် လုပ်ဆောင်သည်။ `QueryRequest::Continue` ပယ်ချ |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` | ဥပဇ္ဈာယ်ဆရာ၊ ထူးခြားချက် || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | စီမံခန့်ခွဲသူ; အင်္ဂါရပ်များ |
| 0xA4 | GET_AUTHORITY | – (အိမ်ရှင်ကရလဒ်ကိုရေးသည်) | `&AccountId`| `G_get_auth` | Host သည် `r10` | သို့ လက်ရှိအာဏာကို ညွှန်ပြသည်။
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, ရွေးချယ်နိုင်သော `root_out:u64` | `u64=len` | `G_mpath + len` | လမ်းကြောင်း (leaf→root) နှင့် ရွေးချယ်နိုင်သော root bytes |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, စိတ်ကြိုက်ရွေးချယ်နိုင်သော `depth_cap:u64`, စိတ်ကြိုက်ရွေးချယ်နိုင်သော `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, စိတ်ကြိုက်ရွေးချယ်နိုင်သော `depth_cap:u64`, ရွေးချယ်နိုင်သော `root_out:u64` | `u64=depth` | `G_mpath + depth` | မှတ်ပုံတင်ကတိကဝတ် |

ဓာတ်ငွေ့ပြဋ္ဌာန်း
- CoreHost သည် မူလ ISI အချိန်ဇယားကို အသုံးပြု၍ ISI syscall များအတွက် အပိုဓာတ်ငွေ့ကို ကောက်ခံပါသည်။ FASTPQ အစုလိုက် လွှဲပြောင်းမှုများကို ဝင်ရောက်မှုတစ်ခုလျှင် ကောက်ခံပါသည်။
- ZK_VERIFY syscall များသည် လျှို့ဝှက်အတည်ပြုချက်ဓာတ်ငွေ့အချိန်ဇယား (အခြေခံ + အထောက်အထားအရွယ်အစား) ကို ပြန်လည်အသုံးပြုသည်။
- SMARTCONTRACT_EXECUTE_QUERY ကောက်ခံမှု အခြေခံ + ပစ္စည်းတစ်ခု + တစ်ဘိုက်၊ ပစ္စည်းတစ်ခုစီ၏ကုန်ကျစရိတ်ကို အမြောက်အများစီခြင်းနှင့် စီထားခြင်းမခံရသော အော့ဖ်ဆက်များသည် ပစ္စည်းတစ်ခုချင်းအတွက် ဒဏ်ငွေကို ပေါင်းထည့်သည်။

မှတ်စုများ
- INPUT ဒေသရှိ ညွှန်ပြသည့် အကြောင်းပြချက်များအားလုံးကို ရည်ညွှန်းသော Norito TLV စာအိတ်များအားလုံးကို ပထမအကိုးအကား (`E_NORITO_INVALID` အမှားအယွင်းကြောင့် အတည်ပြုထားသည်)။
- ဗီဇပြောင်းလဲမှုအားလုံးကို VM မှ တိုက်ရိုက်မဟုတ်ဘဲ Iroha ၏ စံစီမံအုပ်ချုပ်မှုစနစ် (`CoreHost` မှတစ်ဆင့်) မှတဆင့် သက်ရောက်သည်။
- တိကျသောဓာတ်ငွေ့ကိန်းသေများ (`G_*`) ကို တက်ကြွသောဓာတ်ငွေ့အချိန်ဇယားဖြင့် သတ်မှတ်သည်။ `ivm.md` ကိုကြည့်ပါ။

အဆင်ပြေကြပါစေ
- `E_SCALL_UNKNOWN`- လက်ရှိအသုံးပြုနေသော `abi_version` အတွက် syscall နံပါတ်ကို အသိအမှတ်မပြုပါ။
- ထည့်သွင်းမှု တရားဝင်ခြင်း အမှားများသည် VM ထောင်ချောက်များ (ဥပမာ၊ ပုံစံမမှန်သော TLV များအတွက် `E_NORITO_INVALID`) အဖြစ် ပျံ့နှံ့သည်။

အပြန်အလှန်ကိုးကားချက်များ
- ဗိသုကာပညာနှင့် VM ဝေါဟာရ- `ivm.md`
- ဘာသာစကားနှင့် တည်ဆောက်ထားသည့်မြေပုံ- `docs/source/kotodama_grammar.md`

မျိုးဆက်မှတ်စု
- အရင်းအမြစ်မှ syscall ကိန်းသေများစာရင်းအပြည့်အစုံကို ထုတ်ပေးနိုင်သည်-
  - `make docs-syscalls` → `docs/source/ivm_syscalls_generated.md` ရေးသည်
  - `make check-docs` → ထုတ်လုပ်လိုက်သောဇယားသည် ခေတ်မီကြောင်း အတည်ပြုသည် (CI တွင် အသုံးဝင်သည်)
- အထက်ဖော်ပြပါ ကဏ္ဍခွဲသည် စာချုပ်ပါ syscalls များအတွက် စုစည်းထားသော၊ တည်ငြိမ်သောဇယားတစ်ခုအဖြစ် ကျန်ရှိနေပါသည်။

## Admin/Role TLV နမူနာများ (Mock Host)

ဤကဏ္ဍသည် စမ်းသပ်မှုများတွင် အသုံးပြုသည့် စီမံခန့်ခွဲသူပုံစံ syscalls များအတွက် လှောင်ပြောင်သော WSV host မှ လက်ခံသော TLV ပုံသဏ္ဍာန်များနှင့် အနည်းဆုံး JSON ပေးချေမှုများအား မှတ်တမ်းပြုစုပါသည်။ ညွှန်ပြသည့် အကြောင်းပြချက်များအားလုံးသည် ညွှန်ပြ-ABI (Norito TLV စာအိတ်များ INPUT တွင် ထည့်ထားသည်) လိုက်နာသည်။ ထုတ်လုပ်ရေးအိမ်ရှင်များသည် ပိုမိုကြွယ်ဝသော အစီအစဉ်များကို အသုံးပြုနိုင်သည်။ ဤဥပမာများသည် အမျိုးအစားများနှင့် အခြေခံပုံစံများကို ရှင်းလင်းရန် ရည်ရွယ်သည်။- REGISTER_PEER / UNREGISTER_PEER
  - Args: `r10=&Json`
  - ဥပမာ JSON: `{ "peer": "peer-id-or-info" }`
  - CoreHost မှတ်ချက်- `REGISTER_PEER` သည် `RegisterPeerWithPop` JSON အရာဝတ္ထုတစ်ခုအား မျှော်လင့်ထားသည် `UNREGISTER_PEER` သည် peer-id string သို့မဟုတ် `{ "peer": "..." }` ကို လက်ခံသည်။

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER-
    - Args: `r10=&Json`
    - အနည်းဆုံး JSON- `{ "name": "t1" }` (ပုံသဏ္ဍန်မှ လျစ်လျူရှုထားသော နောက်ထပ်အကွက်များ)
  - REMOVE_TRIGGER-
    - Args: `r10=&Name` (အစပျိုးအမည်)
  - SET_TRIGGER_ENABLED-
    - Args- `r10=&Name`၊ `r11=enabled:u64` (0 = ပိတ်ထားပြီး၊ သုညမဟုတ် = ဖွင့်ထားသည်)
  - CoreHost မှတ်ချက်- `CREATE_TRIGGER` သည် အစပျိုးမှုဆိုင်ရာ အသေးစိတ်အချက်အလက်များကို မျှော်လင့်ထားသည် (base64 Norito `Trigger` သို့မဟုတ်
    `{ "id": "<trigger_id>", "action": ... }` နှင့် `action` နှင့် base64 Norito `Action` အဖြစ် သို့မဟုတ်
    JSON အရာဝတ္ထုတစ်ခု) နှင့် `SET_TRIGGER_ENABLED` သည် အစပျိုးမက်တာဒေတာကီး `__enabled` ကို ပြောင်းပေးသည် (ပျောက်နေသည်
    ဖွင့်ရန် ပုံသေ)။

- ရာထူးများ- CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE-
    - Args- `r10=&Name` (အခန်းကဏ္ဍအမည်)၊ `r11=&Json` (ခွင့်ပြုချက်များသတ်မှတ်ထားသည်)
    - JSON သည် သော့ `"perms"` သို့မဟုတ် `"permissions"` ကို လက်ခံသည်၊ ခွင့်ပြုချက်အမည်များ၏ စာကြောင်းတစ်ခုစီတိုင်းကို လက်ခံသည်။
    - ဥပမာများ
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:soraカタカナ...", "transfer_asset:rose#wonder" ] }`
    - ပုံသဏ္ဍန်တွင် ပံ့ပိုးထားသော ခွင့်ပြုချက်အမည်ရှေ့ဆက်သည်-
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - DELETE_ROLE-
    - Args: `r10=&Name`
    - မည်သည့်အကောင့်ကိုမဆို ဤအခန်းကဏ္ဍကို သတ်မှတ်ပေးထားပါက မအောင်မြင်ပါ။
  - GRANT_ROLE / REVOKE_ROLE-
    - Args- `r10=&AccountId` (ဘာသာရပ်)၊ `r11=&Name` (ရာထူးအမည်)
  - CoreHost မှတ်ချက်- ခွင့်ပြုချက် JSON သည် အပြည့်အစုံ `Permission` အရာဝတ္ထု (`{ "name": "...", "payload": ... }`) သို့မဟုတ် စာတန်း (payload defaults to `null`) ဖြစ်နိုင်သည်။ `GRANT_PERMISSION`/`REVOKE_PERMISSION` `&Name` သို့မဟုတ် `&Json(Permission)` လက်ခံသည်။

- ops (domain/account/asset) ကို မှတ်ပုံတင်ခြင်းမှ ဖြုတ်ပါ- ပုံစံကွဲများ (လှောင်ပြောင်)
  - UNREGISTER_DOMAIN (`r10=&DomainId`) ဒိုမိန်းတွင် အကောင့်များ သို့မဟုတ် ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ ရှိနေပါက မအောင်မြင်ပါ။
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) အကောင့်သည် သုညမဟုတ်သော လက်ကျန်များ သို့မဟုတ် NFTs များပိုင်ဆိုင်ပါက မအောင်မြင်ပါ။
  - ပိုင်ဆိုင်မှုအတွက် လက်ကျန်များရှိပါက UNREGISTER_ASSET (`r10=&AssetDefinitionId`) ပျက်ကွက်ပါသည်။

မှတ်စုများ
- ဤနမူနာများသည် စမ်းသပ်မှုများတွင် အသုံးပြုသည့် WSV host ၏ အတုအယောင်ကို ရောင်ပြန်ဟပ်ပါသည်။ real node host များသည် ပိုမိုကြွယ်ဝသော စီမံခန့်ခွဲရေးအစီအစဉ်များကို ဖော်ထုတ်နိုင်သည် သို့မဟုတ် ထပ်လောင်းအတည်ပြုချက် လိုအပ်ပါသည်။ ညွှန်ပြ-ABI စည်းမျဉ်းများသည် သက်ရောက်မှုရှိနေဆဲဖြစ်သည်- TLV များသည် INPUT တွင်ရှိရမည်၊ ဗားရှင်း=1၊ အမျိုးအစား ID များသည် တူညီရမည်ဖြစ်ပြီး payload hash များကို တရားဝင်အောင်ပြုလုပ်ရမည်ဖြစ်သည်။
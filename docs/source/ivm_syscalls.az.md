---
lang: az
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:26:46.570453+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Syscall ABI

Bu sənəd IVM sistem zəngi nömrələrini, göstərici-ABI çağırış konvensiyalarını, qorunan nömrə diapazonlarını və Kotodama endirmə zamanı istifadə olunan müqavilə ilə əlaqəli sistem zənglərinin kanonik cədvəlini müəyyən edir. O, `ivm.md` (arxitektura) və `kotodama_grammar.md` (dil) tamamlayır.

Versiyalaşdırma
- Tanınmış sistem zəngləri dəsti `abi_version` bayt kodu başlığından asılıdır. İlk buraxılış yalnız `abi_version = 1` qəbul edir; digər dəyərlər qəbul zamanı rədd edilir. Aktiv `abi_version` üçün naməlum nömrələr `E_SCALL_UNKNOWN` ilə deterministik şəkildə tələyə düşür.
- İş vaxtı təkmilləşdirmələri `abi_version = 1` saxlayır və sistem zəngi və ya göstərici-ABI səthlərini genişləndirmir.
- Syscall qaz xərcləri bayt kodunun başlıq versiyasına bağlı versiyalaşdırılmış qaz cədvəlinin bir hissəsidir. Baxın `ivm.md` (Qaz siyasəti).

Nömrələmə diapazonları
- `0x00..=0x1F`: VM nüvəsi/utiliti (sazlama/çıxış köməkçiləri `CoreHost` altında mövcuddur; qalan inkişaf etdirmə köməkçiləri yalnız saxta hostdur).
- `0x20..=0x5F`: Iroha əsas ISI körpüsü (ABI v1-də stabil).
- `0x60..=0x7F`: protokol xüsusiyyətləri ilə bağlanan genişlənmə ISI-lər (aktiv olduqda hələ də ABI v1-in bir hissəsidir).
- `0x80..=0xFF`: host/kripto köməkçiləri və ayrılmış yuvalar; yalnız ABI v1 icazə siyahısında olan nömrələr qəbul edilir.

Davamlı köməkçilər (ABI v1)
- Davamlı vəziyyət köməkçi sistem zəngləri (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA encode/decode) V1 ABI-nin bir hissəsidir və `abi_hash` hesablamaya daxildir.
- CoreHost telləri STATE_{GET,SET,DEL} ilə WSV tərəfindən dəstəklənən davamlı smart-kontrakt vəziyyətinə; dev/test hostları yerli olaraq davam edə bilər, lakin eyni sistem semantikasını qorumalıdır.

Pointer-ABI zəng konvensiyası (ağıllı-müqavilə sistemləri)
- Arqumentlər `r10+` registrlərində xam `u64` dəyərləri kimi və ya dəyişməz Norito TLV zərflərinə (məs., `AccountId`0, `AccountId`0, `AccountId`0, `u64`) GİRİŞ bölgəsinə göstəricilər kimi yerləşdirilir. `Name`, `Json`, `NftId`).
- Skalyar qaytarma dəyərləri hostdan qaytarılan `u64`-dir. Göstərici nəticələri host tərəfindən `r10`-ə yazılır.

Kanonik sistem çağırışı cədvəli (alt dəst)| Hex | Adı | Arqumentlər (`r10+`-də) | Qaytarır | Qaz (əsas + dəyişən) | Qeydlər |
|------|--------------------------------------|------------------------------------------------------------------------|-------------|------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | Hesab üçün təfərrüat yazır |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | Hesaba aktivin `amount` pul pulları |
| 0x23 | BURN_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | `amount` hesabından yandırır |
| 0x24 | TRANSFER_AKTİV | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | Hesablar arasında `amount` köçürmələri |
| 0x29 | TRANSFER_V1_BATCH_BAŞLAYIN | – | `u64=0` | `G_transfer` | FASTPQ köçürmə toplu əhatəsinə başlayın |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | Flush yığılmış FASTPQ transfer partiyası |
| 0x2B | TRANSFER_V1_BATCH_MÜRACİƏT | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Norito kodlu toplusunu tək sistem zəngində tətbiq edin |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | Yeni NFT | qeyd edir
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | NFT sahibliyini köçürür |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | NFT metadatasını yeniləyir |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | NFT | yandırır (məhv edir).
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Təkrarlanan sorğular efemer olaraq işləyir; `QueryRequest::Continue` rədd edildi |
| 0xA2 | BÜTÜN_İSTİFADƏÇİLƏR ÜÇÜN_NFTS_YARAT | – | `u64=count` | `G_create_nfts_for_all` | Köməkçi; xüsusiyyətli || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | Admin; xüsusiyyətli |
| 0xA4 | GET_AUTHORITY | – (aparıcı nəticəni yazır) | `&AccountId`| `G_get_auth` | Host cari səlahiyyətə göstəricini `r10` |-ə yazır
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, isteğe bağlı `root_out:u64` | `u64=len` | `G_mpath + len` | Yol (yarpaq→kök) və əlavə kök baytlarını yazır |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, isteğe bağlı `depth_cap:u64`, isteğe bağlı `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, isteğe bağlı `depth_cap:u64`, isteğe bağlı `root_out:u64` | `u64=depth` | `G_mpath + depth` | Reyestr öhdəliyi üçün eyni kompakt tərtibat |

Qaz mühafizəsi
- CoreHost yerli ISI cədvəlindən istifadə edərək ISI sistemləri üçün əlavə qaz alır; FASTPQ toplu köçürmələri hər giriş üçün tutulur.
- ZK_VERIFY sistem zəngləri məxfi yoxlama qaz cədvəlindən təkrar istifadə edir (əsas + sübut ölçüsü).
- SMARTCONTRACT_EXECUTE_QUERY ödəniş bazası + element başına + bayt başına; çeşidləmə hər bir maddə üzrə dəyəri artırır və çeşidlənməmiş ofsetlər hər bir maddə üçün cərimə əlavə edir.

Qeydlər
- Bütün göstərici arqumentləri INPUT bölgəsindəki Norito TLV zərflərinə istinad edir və ilk müraciətdə təsdiqlənir (səhv üzrə `E_NORITO_INVALID`).
- Bütün mutasiyalar birbaşa VM tərəfindən deyil, Iroha standart icraçısı (`CoreHost` vasitəsilə) vasitəsilə tətbiq edilir.
- Dəqiq qaz sabitləri (`G_*`) aktiv qaz cədvəli ilə müəyyən edilir; bax `ivm.md`.

Səhvlər
- `E_SCALL_UNKNOWN`: aktiv `abi_version` üçün sistem zəngi nömrəsi tanınmır.
- Daxiletmə doğrulama xətaları VM tələləri kimi yayılır (məsələn, səhv formalaşdırılmış TLV-lər üçün `E_NORITO_INVALID`).

Çarpaz istinadlar
- Memarlıq və VM semantikası: `ivm.md`
- Dil və daxili xəritələmə: `docs/source/kotodama_grammar.md`

Nəsil qeydi
- Sistem zəngi sabitlərinin tam siyahısı mənbədən aşağıdakılarla yaradıla bilər:
  - `make docs-syscalls` → `docs/source/ivm_syscalls_generated.md` yazır
  - `make check-docs` → yaradılan cədvəlin yeni olduğunu yoxlayır (CI-də faydalıdır)
- Yuxarıdakı alt dəst müqavilə ilə əlaqəli sistemlər üçün seçilmiş, sabit cədvəl olaraq qalır.

## Admin/Rol TLV Nümunələri (Səhənnət Host)

Bu bölmə sınaqlarda istifadə edilən admin-stil sistem zəngləri üçün saxta WSV hostu tərəfindən qəbul edilən TLV formalarını və minimal JSON yüklərini sənədləşdirir. Bütün göstərici arqumentləri göstərici-ABI (INPUT-da yerləşdirilən Norito TLV zərfləri) izləyir. İstehsal hostları daha zəngin sxemlərdən istifadə edə bilər; bu nümunələr növləri və əsas formaları aydınlaşdırmaq məqsədi daşıyır.- REGISTER_PEER / UNREGISTER_PEER
  - Args: `r10=&Json`
  - Nümunə JSON: `{ "peer": "peer-id-or-info" }`
  - CoreHost note: `REGISTER_PEER` expects a `RegisterPeerWithPop` JSON object with `peer` + `pop` bytes (optional `activation_at`, `expiry_at`, `hsm`); `UNREGISTER_PEER` peer-id sətri və ya `{ "peer": "..." }` qəbul edir.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - Args: `r10=&Json`
    - Minimal JSON: `{ "name": "t1" }` (əlavə sahələr istehza tərəfindən nəzərə alınmır)
  - REMOVE_TRIGGER:
    - Args: `r10=&Name` (tətik adı)
  - SET_TRIGGER_ENABLED:
    - Args: `r10=&Name`, `r11=enabled:u64` (0 = qeyri-aktiv, sıfırdan fərqli = aktiv)
  - CoreHost qeydi: `CREATE_TRIGGER` tam trigger spesifikasiyasını gözləyir (base64 Norito `Trigger` simli və ya
    `{ "id": "<trigger_id>", "action": ... }` əsas kimi `action` ilə64 Norito `Action` simli və ya
    JSON obyekti) və `SET_TRIGGER_ENABLED` tetikleyici metadata açarını dəyişdirir `__enabled` (çatışmır)
    default olaraq aktivdir).

- Rollar: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - Args: `r10=&Name` (rolun adı), `r11=&Json` (icazələr dəsti)
    - JSON ya açarı `"perms"`, ya da `"permissions"` qəbul edir, hər biri icazə adlarının sətirləridir.
    - Nümunələr:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:<i105-account-id>", "transfer_asset:rose#wonder" ] }`
    - İstehzada dəstəklənən icazə adı prefiksləri:
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - Args: `r10=&Name`
    - Hər hansı hesaba hələ də bu rol təyin edilərsə, uğursuz olur.
  - GRANT_ROLE / REVOKE_ROLE:
    - Args: `r10=&AccountId` (mövzu), `r11=&Name` (rolun adı)
  - CoreHost qeydi: icazə JSON tam `Permission` obyekti (`{ "name": "...", "payload": ... }`) və ya simli ola bilər (faydalı yük defoltları `null` üçün); `GRANT_PERMISSION`/`REVOKE_PERMISSION` `&Name` və ya `&Json(Permission)` qəbul edir.

- Əməliyyatların qeydiyyatdan çıxarılması (domen/hesab/aktiv): dəyişməz (istehza)
  - Domendə hesablar və ya aktiv tərifləri varsa, UNREGISTER_DOMAIN (`r10=&DomainId`) uğursuz olur.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) hesabın sıfırdan fərqli qalıqları varsa və ya NFT-lərə sahibdirsə uğursuz olur.
  - Aktiv üçün hər hansı qalıq varsa, UNREGISTER_ASSET (`r10=&AssetDefinitionId`) uğursuz olur.

Qeydlər
- Bu nümunələr testlərdə istifadə edilən saxta WSV hostunu əks etdirir; real node hostları daha zəngin admin sxemlərini ifşa edə bilər və ya əlavə yoxlama tələb edə bilər. Göstərici-ABI qaydaları hələ də tətbiq olunur: TLV-lər INPUT-da olmalıdır, versiya=1, tip ID-ləri uyğun olmalıdır və faydalı yük heşləri doğrulanmalıdır.
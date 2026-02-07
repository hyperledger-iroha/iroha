---
lang: az
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T19:17:13.238594+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ⇄ ISI ⇄ Məlumat modeli ⇄ Kotodama — Alignment Review

Bu sənəd Iroha Virtual Maşının (IVM) təlimat dəstinin və sistem zəng səthi xəritəsinin Iroha Xüsusi Təlimatlara (ISI) və `iroha_data_model`-ə necə uyğunlaşdığını və Kotodama-a necə daxil olduğunu yoxlayır. O, mövcud boşluqları müəyyənləşdirir və konkret təkmilləşdirmələr təklif edir ki, dörd təbəqə deterministik və erqonomik cəhətdən bir-birinə uyğun olsun.

Bayt kodu hədəfinə dair qeyd: Kotodama ağıllı müqavilələri Iroha Virtual Maşın (IVM) bayt koduna (`.to`) tərtib edilir. Onlar müstəqil arxitektura kimi “risc5”/RISC‑V-ni hədəf almırlar. Burada istinad edilən hər hansı RISC‑V kimi kodlaşdırmalar IVM qarışıq təlimat formatının bir hissəsidir və icra detalı olaraq qalır.

## Əhatə dairəsi və Mənbələr
- IVM: `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` və `crates/ivm/docs/*`.
- ISI/Data Model: `crates/iroha_data_model/src/isi/*`, `crates/iroha_core/src/smartcontracts/isi/*` və sənədlər `docs/source/data_model_and_isi_spec.md`.
- Kotodama: `crates/kotodama_lang/src/*`, `crates/ivm/docs/*`-də sənədlər.
- Əsas inteqrasiya: `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`.

Terminologiya
- “ISI” icraçı vasitəsilə dünya vəziyyətini dəyişdirən daxili təlimat növlərinə aiddir (məsələn, RegisterAccount, Mint, Transfer).
- “Syscall” 8-bit nömrə ilə IVM `SCALL`-ə istinad edir, bu da kitab əməliyyatları üçün hosta verilir.

---

## Cari Xəritəçəkmə (İcra edildiyi kimi)

### IVM Təlimatlar
- Arifmetik, yaddaş, idarəetmə axını, kripto, vektor və ZK köməkçiləri `instruction.rs`-də müəyyən edilib və `ivm.rs`-də həyata keçirilir. Bunlar öz-özünə qapalı və deterministdir; sürətləndirmə yolları (SIMD/Metal/CUDA) CPU-nun ehtiyat hissələrinə malikdir.
- Sistem/host sərhədi `SCALL` (opcode 0x60) vasitəsilədir. Nömrələr `syscalls.rs`-də verilmişdir və dünya əməliyyatlarını (qeydiyyatdan çıxarmaq/qeydiyyatdan çıxarmaq domen/hesab/aktiv, nanə/yandırma/köçürmə, rol/icazə əməliyyatları, tetikleyiciler) və köməkçiləri (`GET_PRIVATE_INPUT`, `COMMIT_OUTPUT`, I106080, və s.) daxildir.

### Host təbəqəsi
- `IVMHost::syscall(number, &mut IVM)` xüsusiyyəti `host.rs`-də yaşayır.
- DefaultHost yalnız mühasibat kitabçası olmayan köməkçiləri (ayrılma, yığın artımı, giriş/çıxış, ZK sübut köməkçiləri, xüsusiyyət kəşfi) tətbiq edir — o, dünya vəziyyəti mutasiyalarını yerinə yetirmir.
- `WsvHost` demosu `mock_wsv.rs`-də mövcuddur ki, o, aktiv əməliyyatlarının alt dəstini (Transfer/Mint/Burn) `AccountId`/`AccountId`/`AssetDefinitionId` in map vasitəsilə kiçik yaddaşdaxili WSV ilə əlaqələndirir. x10..x13.

### ISI və Məlumat Modeli
- Daxili ISI növləri və semantikası `iroha_core::smartcontracts::isi::*`-də həyata keçirilir və `docs/source/data_model_and_isi_spec.md`-də sənədləşdirilir.
- `InstructionBox` sabit “tel identifikatorları” və Norito kodlaşdırması olan reyestrdən istifadə edir; yerli icra göndərilməsi əsasdakı cari kod yoludur.### Core Integration of IVM
- `State::execute_trigger(..)` keşlənmiş `IVM`-ni klonlayır, `CoreHost::with_accounts_and_args` əlavə edir və sonra `load_program` + `run`-ə zəng edir.
- `CoreHost` `IVMHost` tətbiq edir: statuslu sistem zəngləri göstərici-ABI TLV düzümü vasitəsilə deşifrə edilir, daxili ISI (`InstructionBox`) ilə əlaqələndirilir və növbəyə qoyulur. VM qayıtdıqdan sonra ev sahibi həmin ISI-ni adi icraçıya verir ki, icazələr, invariantlar, hadisələr və telemetriya yerli icra ilə eyni qalsın. WSV-yə toxunmayan köməkçi sistem çağırışları hələ də `DefaultHost`-ə nümayəndə verir.
- `executor.rs` daxili ISI-ni yerli olaraq işlətməyə davam edir; validator icraçısının özünü IVM-ə köçürmək gələcək iş olaraq qalır.

### Kotodama → IVM
- Frontend parçaları mövcuddur (lexer/parser/minimal semantika/IR/regalloc).
- Codegen (`kotodama::compiler`) IVM əməliyyatlarının bir hissəsini yayır və aktiv əməliyyatları üçün `SCALL` istifadə edir:
  - `MintAsset` → set x10=hesab, x11=aktiv, x12=&NoritoBytes(Rəqəm); `SCALL SYSCALL_MINT_ASSET`.
  - `BurnAsset`/`TransferAsset` oxşar (məbləğ NoritoBytes(Rəqəm) göstəricisi kimi ötürülür).
- `koto_*_demo.rs` demoları sürətli sınaq üçün ID-lərə uyğunlaşdırılmış tam indekslərlə `WsvHost` istifadə edərək göstərir.

---

## Boşluqlar və Uyğunsuzluqlar

1) Core host coverage and parity
- Vəziyyət: `CoreHost` indi əsasda mövcuddur və standart yol vasitəsilə icra olunan bir çox kitab sistem zənglərini ISI-yə çevirir. Əhatə hələ də natamamdır (məsələn, bəzi rollar/icazələr/tətik sistemləri qaralamalardır) və növbəyə alınmış ISI-nin yerli icra ilə eyni vəziyyəti/hadisələri yaratmasına zəmanət vermək üçün paritet testləri tələb olunur.

2) Syscall surface vs. ISI/Data Model naming and coverage
- NFT-lər: sistem zəngləri indi `iroha_data_model::nft` ilə uyğunlaşdırılmış kanonik `SYSCALL_NFT_*` adlarını ifşa edir.
- Rollar/İcazələr/Tetiklər: sistem çağırışının siyahısı mövcuddur, lakin hər bir zəngi əsasda konkret ISI-yə bağlayan istinad tətbiqi və ya xəritəçəkmə cədvəli yoxdur.
- Parametrlər/semantika: bəzi sistem zəngləri parametr kodlamasını (yazılmış ID-lər və göstəricilər) və ya qaz semantikasını təyin etmir; ISI semantics are well‑defined.

3) VM/host sərhədindən çap edilmiş məlumatların ötürülməsi üçün ABI
- Göstərici-ABI TLV-ləri indi `CoreHost` (`decode_tlv_typed`)-də deşifrə edilib, ID-lər, metadata və JSON yükləri üçün deterministik yol verir. Hər bir sistem çağırışının gözlənilən göstərici növlərini sənədləşdirməsini və Kotodama-in düzgün TLV-ləri yaymasını təmin etmək üçün iş qalır (siyasət bir növü rədd edərkən xətaların idarə edilməsi də daxil olmaqla).

4) Gas and error mapping consistency
- IVM opcodes charge per‑op gas; CoreHost indi yerli qaz cədvəlindən (toplu köçürmələr və satıcı ISI körpüsü daxil olmaqla) istifadə edərək ISI sistem zəngləri üçün əlavə qaz qaytarır və ZK sistem zənglərinin məxfi qaz cədvəlindən yenidən istifadə etdiyini təsdiqləyir. DefaultHost hələ də sınaq əhatə dairəsi üçün minimum xərcləri saxlayır.
- Error surfaces differ: IVM returns `VMError::{OutOfGas,PermissionDenied,...}`; ISI `InstructionExecutionError` kateqoriyalarını qaytarır (`Find`, `Repetition`, `InvariantViolation`, `Math`, `Type`, Kotodama, Kotodama).5) Sürətlənmə yolları üzrə determinizm
- IVM vektor/CUDA/Metal CPU ehtiyatlarına malikdir, lakin bəzi əməliyyatlar qorunur (`SETVL`, PARBEGIN/PAREND) və hələ də deterministik nüvənin bir hissəsi deyil.
- Merkle ağacları IVM və node (`ivm::merkle_tree` vs `iroha_crypto::MerkleTree`) arasında fərqlənir — birləşmə elementi artıq `roadmap.md`-də görünür.

6) Kotodama dil səthi və nəzərdə tutulan kitab semantikası
- Kompilyator kiçik bir alt çoxluq buraxır; əksər dil funksiyaları (dövlət/strukturlar, tetikleyiciler, icazələr, yazılmış parametrlər/qaytarmalar) hələ host/ISI modelinə qoşulmayıb.
- Sistem zənglərinin səlahiyyətli orqan üçün qanuni olmasını təmin etmək üçün yazma qabiliyyəti/effekti yoxdur.

---

## Tövsiyələr (Konkret addımlar)

### A. IVM istehsal hostunu nüvədə həyata keçirin
- `ivm::host::IVMHost` həyata keçirən `iroha_core::smartcontracts::ivm::host` modulunu əlavə edin.
- `ivm::syscalls`-də hər bir sistem çağırışı üçün:
  - Kanonik ABI vasitəsilə arqumentləri deşifrə edin (bax B.), müvafiq daxili ISI qurun və ya eyni əsas məntiqi birbaşa çağırın, onu `StateTransaction`-ə qarşı icra edin və səhvləri deterministik olaraq IVM qaytarma koduna qaytarın.
  - Əsasda müəyyən edilmiş hər bir sistem çağırış cədvəlindən istifadə edərək qazı deterministik şəkildə doldurun (və gələcəkdə lazım gələrsə, `SYSCALL_GET_PARAMETER` vasitəsilə IVM-ə məruz qalır). Əvvəlcə hər zəng üçün ev sahibindən sabit əlavə qazı qaytarın.
- `authority: &AccountId` və `&mut StateTransaction`-i hosta daxil edin ki, icazə yoxlamaları və hadisələr yerli ISI ilə eyni olsun.
- Bu hostu `vm.run()`-dən əvvəl əlavə etmək üçün `State::execute_trigger(ExecutableRef::Ivm)`-i yeniləyin və ISI ilə eyni `ExecutionStep` semantikasını qaytarın (hadisələr artıq əsasda yayılıb; ardıcıl davranış təsdiqlənməlidir).

### B. Yazılan dəyərlər üçün deterministik VM/host ABI müəyyən edin
- Strukturlaşdırılmış arqumentlər üçün VM tərəfində Norito istifadə edin:
  - `AccountId`, `AssetDefinitionId`, `Numeric`, I101NI20.
  - Host `IVM` yaddaş köməkçiləri vasitəsilə baytları oxuyur və Norito ilə deşifr edir (`iroha_data_model` artıq `Encode/Decode` əldə edir).
- Hərfi identifikatorları kod/sabit hovuzlara seriyalaşdırmaq və ya yaddaşda zəng çərçivələrini hazırlamaq üçün Kotodama kodgenində minimal köməkçilər əlavə edin.
- Məbləğlər `Numeric`-dir və NoritoBytes göstəriciləri kimi ötürülür; digər mürəkkəb növlər də göstərici ilə keçir.
- Bunu `crates/ivm/docs/calling_convention.md`-də sənədləşdirin və nümunələr əlavə edin.### C. Syscall adlandırma və əhatə dairəsini ISI/Data Model ilə uyğunlaşdırın
- Aydınlıq üçün NFT ilə əlaqəli sistem zənglərinin adını dəyişin: kanonik adlar indi `SYSCALL_NFT_*` nümunəsinə əməl edir (`SYSCALL_NFT_MINT_ASSET`, `SYSCALL_NFT_SET_METADATA` və s.).
- Hər bir sistem zəngindən əsas ISI semantikasına xəritəçəkmə cədvəlini (sənəd + kod şərhləri) dərc edin, o cümlədən:
  - Parametrlər (registrlər və göstəricilər), gözlənilən ilkin şərtlər, hadisələr və xəta xəritələri.
  - Qaz ödənişləri.
- Hər bir daxili ISI üçün Kotodama (domenlər, hesablar, aktivlər, rollar/icazələr, triggerlər, parametrlər) ilə əvəz edilə bilməyən sistem çağırışının olduğundan əmin olun. Əgər ISI imtiyazlı qalmalıdırsa, onu sənədləşdirin və hostda icazə yoxlamaları vasitəsilə tətbiq edin.

### D. Səhvləri və qazı birləşdirin
- Hostda tərcümə qatı əlavə edin: `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}`-i xüsusi `VMError` kodlarına və ya genişləndirilmiş nəticə konvensiyasına uyğunlaşdırın (məsələn, `x10=0/1` təyin edin və yaxşı müəyyən edilmiş `VMError::HostRejected { code }` istifadə edin).
- Syscalls üçün nüvədə qaz cədvəlini təqdim etmək; onu IVM sənədlərində əks etdirin; xərclərin giriş ölçüsündə proqnozlaşdırıla bilən və platformadan asılı olmasını təmin edin.

### E. Determinizm və paylaşılan primitivlər
- Merkle ağacının birləşməsini tamamlayın (yol xəritəsinə baxın) və eyni yarpaqlar və sübutlarla `ivm::merkle_tree` - `iroha_crypto` ləqəblərini çıxarın.
- `SETVL`/PARBEGIN/PAREND`-ni sona qədər determinizm yoxlamaları və deterministik planlaşdırma strategiyası mövcud olana qədər qoruyun; IVM bu gün bu göstərişlərə məhəl qoymayan sənəd.
- Sürətləndirmə yollarının bayt-bayt eyni nəticələr verməsini təmin edin; mümkün olmadıqda, CPU-nun geri qaytarılması ekvivalentini təmin edən bir testlə xüsusiyyətlərin arxasından qoruyun.

### F. Kotodama kompilyator naqilləri
- İdentifikatorlar və mürəkkəb parametrlər üçün kodegeni kanonik ABI-yə (B.) genişləndirin; tam → ID demo xəritələrindən istifadə etməyi dayandırın.
- Aydın adlarla aktivlərdən (domenlər/hesablar/rollar/icazələr/tetikleyicilər) kənarda birbaşa ISI sistem zənglərinə daxili xəritələmə əlavə edin.
- Kompilyasiya vaxtı qabiliyyəti yoxlamaları və əlavə `permission(...)` annotasiyaları əlavə edin; statik sübut mümkün olmadıqda iş zamanı host səhvlərinə geri dönmə.
- Norito arqumentlərini deşifrə edən və müvəqqəti WSV-ni mutasiya edən test hostundan istifadə edərək kiçik müqavilələri tərtib edən və icra edən `crates/ivm/tests/kotodama.rs`-də vahid testləri əlavə edin.

### G. Sənədləşdirmə və tərtibatçı erqonomikası
- Sistem zəngi xəritələşdirmə cədvəli və ABI qeydləri ilə `docs/source/data_model_and_isi_spec.md`-i yeniləyin.
- `IVMHost`-in real `StateTransaction` üzərində necə həyata keçiriləcəyini təsvir edən `crates/ivm/docs/`-də yeni “IVM Host İnteqrasiya Bələdçisi” sənədini əlavə edin.
- `README.md` və sandıq sənədlərində aydınlaşdırın ki, Kotodama IVM `.to` bayt kodunu hədəfləyir və sistem zəngləri dünya vəziyyətinə körpüdür.

---

## Təklif olunan Xəritəçəkmə Cədvəli (İlkin Qaralama)

Nümayəndə alt çoxluğu — host tətbiqi zamanı yekunlaşdırın və genişləndirin.- SYSCALL_REGISTER_DOMAIN(id: ptr DomainId) → ISI Qeydiyyatı
- SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId) → ISI Qeydiyyatı
- SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, nağd: u8) → ISI Registri
- SYSCALL_MINT_ASSET(hesab: ptr AccountId, aktiv: ptr AssetDefinitionId, məbləğ: ptr NoritoBytes(Numeric)) → ISI Mint
- SYSCALL_BURN_ASSET(hesab: ptr AccountId, aktiv: ptr AssetDefinitionId, məbləğ: ptr NoritoBytes(Numeric)) → ISI Burn
- SYSCALL_TRANSFER_ASSET(dan: ptr AccountId, to: ptr AccountId, aktiv: ptr AssetDefinitionId, məbləğ: ptr NoritoBytes(Rumeric)) → ISI Transfer
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch (əhatə dairəsini açın/bağlayın; fərdi girişlər `transfer_asset` vasitəsilə azaldılır)
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes) → Müqavilələr artıq zəncirdən kənar girişləri seriallaşdırdıqda əvvəlcədən kodlanmış partiyanı təqdim edin
- SYSCALL_NFT_MINT_ASSET(id: ptr NftId, sahib: ptr AccountId) → ISI Qeydiyyatı
- SYSCALL_NFT_TRANSFER_ASSET(dan: ptr AccountId, to: ptr AccountId, id: ptr NftId) → ISI Transfer
- SYSCALL_NFT_SET_METADATA (id: ptr NftId, məzmun: ptr Metadata) → ISI SetKeyValue
- SYSCALL_NFT_BURN_ASSET (id: ptr NftId) → ISI Qeydiyyatını Sil
- SYSCALL_CREATE_ROLE(id: ptr Rol İd, rol: ptr Rol) → ISI Qeydiyyatı
- SYSCALL_GRANT_ROLE(hesab: ptr AccountId, rol: ptr RoleId) → ISI Grant
- SYSCALL_REVOKE_ROLE(hesab: ptr AccountId, rol: ptr RoleId) → ISI Revoke
- SYSCALL_SET_PARAMETER(param: ptr Parameter) → ISI SetParameter

Qeydlər
- “ptr T” VM yaddaşında saxlanılan T üçün Norito-kodlanmış baytlara registrdə olan göstərici deməkdir; host onu müvafiq `iroha_data_model` tipinə deşifrə edir.
- Qayıdış konvensiyası: müvəffəqiyyət dəstləri `x10=1`; uğursuzluq dəstləri `x10=0` və ölümcül səhvlər üçün `VMError::HostRejected`-i artıra bilər.

---

## Risklər və Yayım Planı
- Dar bir dəst (Aktivlər + Hesablar) üçün məftillə başlayın və diqqətli testlər əlavə edin.
- Host semantikası yetkinləşərkən yerli ISI icrasını nüfuzlu yol kimi saxlayın; eyni son effektləri və hadisələri təsdiqləmək üçün testlərdə hər iki yolu “kölgə rejimi” altında işlədin.
- Paritet təsdiq edildikdən sonra istehsalda IVM tetikleyicileri üçün IVM hostunu aktivləşdirin; daha sonra müntəzəm əməliyyatların IVM vasitəsilə yönləndirilməsini də nəzərdən keçirin.

---

## Görkəmli İş
- Norito-kodlanmış göstəriciləri (`crates/ivm/src/kotodama_std.rs`) keçən Kotodama köməkçilərini yekunlaşdırın və onları CLI kompilyatoru vasitəsilə yerləşdirin.
- Syscall qaz cədvəlini (köməkçi sistem çağırışları daxil olmaqla) dərc edin və CoreHost tətbiqini/testlərini onunla uyğunlaşdırın.
- ✅ ABI göstərici arqumentini əhatə edən gediş-dönüş Norito qurğuları əlavə edildi; CI-də saxlanılan manifest və NFT göstərici əhatə dairəsi üçün `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs`-ə baxın.
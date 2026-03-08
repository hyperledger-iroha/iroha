---
lang: az
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ac9b1fa221c6de46c139ee3a3c280957adad4910b49015fbb746259a4af22659
source_last_modified: "2026-01-30T12:29:10.190473+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Dil Qrammatikası və Semantikası

Bu sənəd Kotodama dil sintaksisini (leksləmə, qrammatika), yazma qaydalarını, deterministik semantikanı və Norito göstərici-ABI konvensiyaları ilə IVM bayt koduna (.to) aşağı olan proqramları necə müəyyənləşdirir. Kotodama mənbələri .ko uzantısından istifadə edir. Kompilyator IVM bayt kodunu (.to) yayır və istəyə görə manifest qaytara bilər.

İçindəkilər
- Baxış və Məqsədlər
- Leksik quruluş
- Növlər və hərflər
- Bəyannamələr və Modullar
- Müqavilə Konteyneri və Metadata
- Funksiyalar və Parametrlər
- Bəyanatlar
- İfadələr
- Quraşdırma və Göstərici-ABI Konstruktorları
- Kolleksiyalar və Xəritələr
- Deterministik İterasiya və Sərhədlər
- Səhvlər və Diaqnostika
- IVM-ə Codegen Xəritəçəkmə
- ABI, Başlıq və Manifest
- Yol xəritəsi

## Baxış və Məqsədlər

- Deterministik: Proqramlar aparat üzrə eyni nəticələr verməlidir; üzən nöqtə və ya qeyri-deterministik mənbələr yoxdur. Bütün host qarşılıqlı əlaqələri Norito kodlu arqumentləri olan sistem çağırışları vasitəsilə baş verir.
- Portativ: Fiziki İSA deyil, Iroha Virtual Maşın (IVM) bayt kodunu hədəfləyir. Repozitoriyada görünən RISC‑V kimi kodlaşdırmalar IVM deşifrəsinin icra detallarıdır və müşahidə olunan davranışı dəyişməməlidir.
- Audit edilə bilən: Kiçik, açıq semantika; IVM əməliyyat kodlarına və sistem çağırışlarına sahib olmaq üçün sintaksisin aydın xəritələşdirilməsi.
- Məhdudiyyət: Sərhədsiz verilənlər üzərindəki döngələr açıq sərhədləri daşımalıdır. Xəritə iterasiyası determinizmi təmin etmək üçün ciddi qaydalara malikdir.

## Leksik Quruluş

Boşluq və şərhlər
- Boşluq tokenləri ayırır və başqa cür əhəmiyyətsizdir.
- Sətir şərhləri `//` ilə başlayır və xəttin sonuna qədər davam edir.
- Blok şərhləri `/* ... */` yuva vermir.

İdentifikatorlar
- Başlayın: `[A-Za-z_]`, sonra `[A-Za-z0-9_]*` davam edin.
- hərflərə həssas; `_` etibarlı identifikatordur, lakin tövsiyə edilmir.

Açar sözlər (ehtiyat)
- `seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `struct`, Kotodama, Kotodama `const`, `return`, `if`, `else`, `while`, `for`, Kotodama, Kotodama `continue`, `true`, `false`, `permission`, `kotoba`.

Operatorlar və durğu işarələri
- Arifmetik: `+ - * / %`
- Bit istiqamətində: `& | ^ ~`, növbələr `<< >>`
- Müqayisə edin: `== != < <= > >=`
- Məntiqi: `&& || !`
- Təyin edin: `= += -= *= /= %= &= |= ^= <<= >>=`
- Müxtəlif: `: , ; . :: ->`
- Mötərizələr: `() [] {}`Hərfi
- Tam ədəd: onluq (`123`), hex (`0x2A`), ikili (`0b1010`). Bütün tam ədədlər iş vaxtında 64 bit imzalanır; şəkilçisiz hərflər nəticə çıxarmaqla və ya standart olaraq `int` kimi yazılır.
- Sətir: qaçışlarla iki tirajlı (`\n`, `\r`, `\t`, `\0`, `\xNN`, `\xNN`, Kotodama, Kotodama, `\r` `\\`); UTF‑8. Xam sətirlər `r"..."` və ya `r#"..."#` qaçışları söndürür və yeni sətirlərə icazə verir.
- Bayt: qaçışlarla `b"..."` və ya xam `br"..."` / `rb"..."`; `bytes` hərfi verir.
- Boolean: `true`, `false`.

## Növlər və hərflər

Skalyar tiplər
- `int`: 64 bitlik iki tamamlayıcı; əlavə/alt/mul üçün arifmetik sarma modulu 2^64; bölmə IVM-də imzalanmış/imzasız variantları müəyyən etmişdir; tərtibçi semantika üçün uyğun op seçir.
- `fixed_u128`, `Amount`, `Balance`: Norito `Numeric` tərəfindən dəstəklənən rəqəmli ləqəblər (512 bit mantis və miqyasla imzalanmış onluq). Kotodama bu ləqəbləri mənfi olmayan kəmiyyətlər kimi qəbul edir; arifmetik yoxlanılır, ləqəbi qoruyur və daşqın və ya sıfıra bölmək üçün tələlər saxlayır. `int`-dən yaradılmış dəyərlər 0 miqyasından istifadə edir; `int`-ə/dan çevrilmələr icra zamanı diapazonla yoxlanılır (mənfi deyil, inteqral, i64-ə uyğundur).
- `bool`: məntiqi həqiqət dəyəri; `0`/`1` səviyyəsinə endirildi.
- `string`: dəyişməz UTF‑8 sətri; sistem çağırışlarına ötürüldükdə Norito TLV kimi təmsil olunur; VM-də əməliyyatlar bayt dilimlərindən və uzunluğundan istifadə edir.
- `bytes`: xammal Norito faydalı yük; həshing/kripto/sübut girişləri və davamlı örtüklər üçün göstərici-ABI `Blob` tipinə ləqəb verir.

Kompozit növləri
- `struct Name { field: Type, ... }` istifadəçi tərəfindən müəyyən edilmiş məhsul növləri. Konstruktorlar ifadələrdə `Name(a, b, ...)` çağırış sintaksisindən istifadə edirlər. Sahəyə giriş `obj.field` dəstəklənir və daxili olaraq sıra tipli mövqe sahələrinə endirilir. Davamlı vəziyyət ABI on-zənciri Norito kodludur; kompilyator struktur sırasını əks etdirən örtüklər buraxır və son sınaqlar (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) tərtibatı buraxılışlar arasında kilidli saxlayır.
- `Map<K, V>`: deterministik assosiativ xəritə; semantika iterasiya zamanı iterasiya və mutasiyaları məhdudlaşdırır (aşağıya bax).
- `Tuple (T1, T2, ...)`: mövqe sahələri ilə anonim məhsul növü; çox qaytarılması üçün istifadə olunur.

Xüsusi göstərici-ABI növləri (host-facing)
- `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob` və bənzəri birinci dərəcəli iş vaxtı növləri deyil. Onlar INPUT bölgəsinə (Norito TLV zərfləri) tiplənmiş, dəyişməz göstəricilər verən konstruktorlardır və yalnız sistem çağırışı arqumentləri kimi istifadə edilə və ya mutasiya olmadan dəyişənlər arasında köçürülə bilər.

Nəticəni yazın
- Yerli `let` bağlamaları başlatıcıdan tip çıxarır. Funksiya parametrləri açıq şəkildə yazılmalıdır. Qayıdış növləri qeyri-müəyyən olmadıqda çıxarıla bilər.

## Bəyannamələr və ModullarƏn yüksək səviyyəli maddələr
- Müqavilələr: `seiyaku Name { ... }` funksiyalar, vəziyyət, strukturlar və metadata ehtiva edir.
- Fayl başına birdən çox müqaviləyə icazə verilir, lakin buna yol verilmir; manifestlərdə standart giriş kimi bir əsas `seiyaku` istifadə olunur.
- `struct` bəyannamələri müqavilə daxilində istifadəçi növlərini müəyyən edir.

Görünüş
- `kotoage fn` ictimai giriş nöqtəsini bildirir; görünürlük kodgenə deyil, dispetçer icazələrinə təsir edir.
- Əlavə giriş göstərişləri: `#[access(read=..., write=...)]` manifest oxuma/yazma düymələrini təmin etmək üçün `fn`/`kotoage fn`-dən əvvəl ola bilər. Kompilyator avtomatik olaraq məsləhət göstərişləri də verir; qeyri-şəffaf host zəngləri mühafizəkar joker işarələrə (`*`) qayıdır və açıq giriş göstərişləri verilmədikcə diaqnostikanı üzə çıxarır, beləliklə, planlaşdırıcılar daha incə düymələr üçün dinamik hazırlıqdan keçə bilərlər.

## Müqavilə Konteyneri və Metadata

Sintaksis
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

Semantika
- `meta { ... }` sahələri emissiya edilmiş IVM başlığı üçün tərtibçi defoltlarını ləğv edir: `abi_version`, `vector_length` (0 təyin olunmamış deməkdir), `max_cycles` (0, kompilyatorun defolt-başlığı deməkdir)030 xüsusiyyət bitləri (ZK izləmə, vektor elanı). Kompilyator `max_cycles: 0`-ə “standart istifadə” kimi yanaşır və qəbul tələblərini ödəmək üçün konfiqurasiya edilmiş sıfırdan fərqli defolt emissiya edir. Dəstəklənməyən xüsusiyyətlər xəbərdarlıqla nəzərə alınmır. `meta {}` buraxıldıqda, kompilyator `abi_version = 1` yayır və qalan başlıq sahələri üçün standart parametrlərdən istifadə edir.
- `features: ["zk", "simd"]` (ləqəblər: `"vector"`) açıq şəkildə müvafiq başlıq bitlərini tələb edir. Naməlum xüsusiyyət sətirləri indi göz ardı edilmək əvəzinə təhlilçi xətası yaradır.
- `state` davamlı müqavilə dəyişənlərini elan edir. Kompilyator `STATE_GET/STATE_SET/STATE_DEL` sistem çağırışlarına girişləri azaldır və host onları hər bir əməliyyatın üst-üstə düşməsində (yoxlama nöqtəsi/bərpa geri qaytarma, WSV-də yerinə yetirməkdə yuyunma) mərhələləşdirir. Hərfi vəziyyət yolları üçün giriş göstərişləri verilir; dinamik açarlar yenidən xəritə səviyyəli münaqişə açarlarına düşür. Açıq host tərəfindən dəstəklənən oxu/yazmalar üçün `state_get/state_set/state_del` və `get_or_insert_default` xəritə köməkçilərindən istifadə edin; bu marşrutu Norito TLV-ləri vasitəsilə keçin və adları/sahə sırasını sabit saxlayın.
- Dövlət identifikatorları qorunur; `state` adını parametrlərdə və ya `let` bağlamalarında kölgə salmaq rədd edilir (`E_STATE_SHADOWED`).
- Dövlət xəritəsi dəyərləri birinci dərəcəli deyil: xəritə əməliyyatları və iterasiya üçün dövlət identifikatorundan birbaşa istifadə edin. Dövlət xəritələrinin istifadəçi tərəfindən müəyyən edilmiş funksiyalara bağlanması və ya ötürülməsi rədd edilir (`E_STATE_MAP_ALIAS`).
- Davamlı vəziyyət xəritələri hazırda yalnız `int` və göstərici-ABI açar növlərini dəstəkləyir; digər açar növləri tərtib zamanı rədd edilir.
- Davamlı vəziyyət sahələri `int`, `bool`, `Json`, `Blob`/`bytes` və ya göstərici-ABI növləri (o cümlədən, bu strukturlar/sahələrdən ibarət kompozisiyalar) olmalıdır; `string` davamlı vəziyyət üçün dəstəklənmir.

### Kotoba lokalizasiyası
Sintaksis
```
kotoba {
  "E_UNBOUNDED_ITERATION": { en: "Loop over map lacks a bound." }
}
```Semantika
- `kotoba` girişləri müqavilə manifestinə tərcümə cədvəllərini əlavə edir (`kotoba` sahəsi).
- Mesaj identifikatorları və dil teqləri identifikatorları və ya sətir literallarını qəbul edir; girişlər boş olmamalıdır.
- Dublikat `msg_id` + dil teq cütləri tərtib zamanı rədd edilir.

## Tətik Bəyanatları

Tətik bəyannamələri planlaşdırma metadatasını giriş nöqtəsi manifestlərinə əlavə edir və avtomatik qeydə alınır
müqavilə nümunəsi aktivləşdirildikdə (deaktivasiya zamanı silinir). Onlar a daxilində təhlil edilir
`seiyaku` bloku.

Sintaksis
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  authority "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

Qeydlər
- `call` eyni müqavilədə ictimai `kotoage fn` giriş nöqtəsinə istinad etməlidir; isteğe bağlı
  `namespace::entrypoint` manifestdə qeyd edilib, lakin müqavilələr arası geri çağırışlar rədd edilir
  iş vaxtı ilə (yalnız yerli geri çağırışlar).
- Dəstəklənən filtrlər: `time pre_commit` və `time schedule(start_ms, period_ms?)`, üstəgəl
  `execute trigger <name>` çağırış tətikləri, `data any` və boru kəməri filtrləri üçün
  (`pipeline transaction`, `pipeline block`, `pipeline merge`, `pipeline witness`).
- `authority` isteğe bağlı olaraq trigger səlahiyyətini ləğv edir (AccountId sətri hərfi). Əgər buraxılmışsa,
  icra müddəti müqavilə aktivləşdirmə səlahiyyətindən istifadə edir.
- Metadata dəyərləri JSON hərfi (`string`, `number`, `bool`, `null`) və ya `json!(...)` olmalıdır.
- Runtime injected trigger metadata açarları: `contract_namespace`, `contract_id`,
  `contract_entrypoint`, `contract_code_hash`, `contract_trigger_id`.

## Funksiyalar və Parametrlər

Sintaksis
- Bəyannamə: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- İctimai: `kotoage fn name(...) { ... }`
- Başlatıcı: `hajimari() { ... }` (VM-in özü tərəfindən deyil, iş vaxtı ilə yerləşdirmə zamanı çağırılır).
- Təkmilləşdirmə çəngəl: `kaizen(args...) permission(Role) { ... }`.

Parametrlər və qaytarmalar
- Arqumentlər `r10..r22` registrlərində qiymətlər və ya ABI üçün INPUT göstəriciləri (Norito TLV) kimi ötürülür; əlavə arxlar yığına tökülür.
- Funksiyalar sıfır və ya bir skalyar və ya tuple qaytarır. İlkin qaytarma dəyəri skaler üçün `r10`-dir; tuples konvensiyaya uyğun olaraq yığın/çıxışda materiallaşdırılır.

## Bəyanatlar- Dəyişən bağlamalar: `let x = expr;`, `let mut x = expr;` (dəyişkənlik kompilyasiya vaxtı yoxlamasıdır; iş vaxtı mutasiyasına yalnız yerli sakinlər üçün icazə verilir).
- Tapşırıq: `x = expr;` və mürəkkəb formalar `x += 1;` və s. Hədəflər dəyişənlər və ya xəritə indeksləri olmalıdır; tuple/struct sahələri dəyişməzdir.
- Rəqəm ləqəbləri (`fixed_u128`, `Amount`, `Balance`) fərqli `Numeric` dəstəkli növlərdir; arifmetik ləqəbi qoruyur və ləqəbləri qarışdırmaq üçün `int` bağlaması vasitəsilə konvertasiya tələb olunur. `int`-ə/dan çevrilmələr iş vaxtında yoxlanılır (mənfi deyil, inteqral, diapazonla məhduddur).
- Nəzarət: `if (cond) { ... } else { ... }`, `while (cond) { ... }`, C tipli `for (init; cond; step) { ... }`.
  - `for` başlatıcıları və addımları sadə `let name = expr` və ya ifadə ifadələri olmalıdır; kompleks destrukturizasiya rədd edilir (`E0005`, `E0006`).
  - `for` əhatə dairəsi: init bəndindəki bağlamalar döngədə və ondan sonra görünür; gövdə və ya pillədə yaradılmış bağlamalar döngədən qaçmır.
- Bərabərlik (`==`, `!=`) `int`, `bool`, `string`, göstərici-ABI skalyarları üçün dəstəklənir (məs., I102100, I10210 `Name`, `Blob`/`bytes`, `Json`); kortejlər, strukturlar və xəritələr müqayisə edilə bilməz.
- Xəritə döngəsi: `for (k, v) in map { ... }` (deterministik; aşağıya baxın).
- Axın: `return expr;`, `break;`, `continue;`.
- Zəng edin: `name(args...);` və ya `call name(args...);` (hər ikisi qəbul edilir; tərtibçi çağırış ifadələrini normallaşdırır).
- Təsdiqlər: `assert(cond);`, `assert_eq(a, b);`, qeyri-ZK quruluşlarında və ya ZK rejimində ZK məhdudiyyətlərində IVM `ASSERT*` xəritəsi.

## İfadələr

Üstünlük (yüksək → aşağı)
1. Üzv/indeks: `a.b`, `a[b]`
2. Birlik: `! ~ -`
3. Multiplikativ: `* / %`
4. Əlavə: `+ -`
5. Növbələr: `<< >>`
6. Əlaqəli: `< <= > >=`
7. Bərabərlik: `== !=`
8. Bitwise AND/XOR/OR: `& ^ |`
9. Məntiqi VƏ/YA: `&& ||`
10. Üçlük: `cond ? a : b`

Zənglər və dəstlər
- Zənglər mövqe arqumentlərindən istifadə edir: `f(a, b, c)`.
- Tuple literal: `(a, b, c)` və destructure: `let (x, y) = pair;`.
- Tuple destrukturizasiya uyğun aritmə malik tuple/struct tiplərini tələb edir; uyğunsuzluqlar rədd edilir.

Sətirlər və baytlar
- Sətirlər UTF‑8-dir; mənbədə xam sətir və bayt hərfi formaları qəbul edilir.
- Bayt literalları (`b"..."`, `br"..."`, `rb"..."`) `bytes` (Blob) göstəricilərindən aşağıdır; sistem zəngi NoritoBytes TLV yüklərini gözlədikdə `norito_bytes(...)` ilə sarın.

## Quruluşlar və Göstərici-ABI Konstruktorları

Göstərici konstruktorları (INPUT-a Norito TLV buraxın və yazılmış göstəricini qaytarın)
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`Müqəddimə makroları bu konstruktorlar üçün daha qısa ləqəblər və daxili doğrulama təmin edir:
- `account!("ih58...")`, `account_id!("ih58...")`
- `asset_definition!("rose#wonderland")`, `asset_id!("rose#wonderland")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` və ya `json!{ hello: "world" }` kimi strukturlaşdırılmış hərflər
- `nft_id!("dragon$demo")`, `blob!("bytes")`, `norito_bytes!("...")`

Makroslar yuxarıdakı konstruktorlara qədər genişlənir və tərtib zamanı etibarsız literalları rədd edir.

İcra vəziyyəti
- Həyata keçirildi: yuxarıdakı konstruktorlar string literal arqumentləri qəbul edir və INPUT bölgəsində yerləşdirilmiş Norito tipli TLV zərflərindən aşağıdır. Onlar sistem çağırışı arqumentləri kimi istifadə edilə bilən dəyişməz tipli göstəriciləri qaytarır. Qeyri-hərfi sətir ifadələri rədd edilir; dinamik girişlər üçün `Blob`/`bytes` istifadə edin. `blob`/`norito_bytes` həmçinin `bytes` tipli dəyərləri makro şimlər olmadan iş vaxtında qəbul edir.
- Genişləndirilmiş formalar:
  - `JSON_DECODE` sistemi vasitəsilə `json(Blob[NoritoBytes]) -> Json*`.
  - `name(Blob[NoritoBytes]) -> Name*` `NAME_DECODE` sistem zəngi vasitəsilə.
  - Blob/NoritoBytes-dən göstəricinin deşifrəsi: istənilən göstərici konstruktoru (AXT növləri daxil olmaqla) `Blob`/`NoritoBytes` faydalı yükü qəbul edir və gözlənilən tip id ilə `POINTER_FROM_NORITO` səviyyəsinə endirir.
  - Göstərici formaları üçün keçid: `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`.
  - Şəkər üsulu dəstəklənir: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

Host/syscall qurğuları (SCALL üçün xəritə; ivm.md-də dəqiq nömrələr)
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `execute_instruction(Blob[NoritoBytes])`
- `execute_query(Blob[NoritoBytes]) -> Blob`
- `subscription_bill()`
- `subscription_record_usage()`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

Kommunal qurğular
- `info(string|int)`: OUTPUT vasitəsilə strukturlaşdırılmış hadisə/mesaj verir.
- `hash(blob) -> Blob*`: Blob kimi Norito kodlu hash qaytarır.
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` və `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`: daxili ISI qurucuları; bütün arqumentlər tərtib zamanı literalları olmalıdır (sətir hərfi və ya hərflərdən göstərici konstruktorları). `nullifier32` və `inputs32` tam olaraq 32 bayt olmalıdır (xam sətir və ya `0x` hex), `amount` isə mənfi olmamalıdır.
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `encode_schema(Name*, Json*) -> Blob`: host sxem reyestrindən istifadə edərək JSON-u kodlayır (DefaultRegistry Sifariş/Ticarət nümunələrinə əlavə olaraq `QueryRequest` və `QueryResponse`-i dəstəkləyir).
- `decode_schema(Name*, Blob|bytes) -> Json*`: host sxem reyestrindən istifadə edərək Norito baytlarını deşifrə edir.
- `pointer_to_norito(ptr) -> NoritoBytes*`: saxlama və ya daşınma üçün mövcud göstərici-ABI TLV-ni NoritoBytes kimi sarar.
- `isqrt(int) -> int`: tam kvadrat kök (`floor(sqrt(x))`) IVM əməliyyat kodu kimi həyata keçirilir.
- `min(int, int) -> int`, `max(int, int) -> int`, `abs(int) -> int`, `div_ceil(int, int) -> int`, `gcd(int, int) -> int`, `mean(int, int) -> int` — əridilmiş hesab köməkçiləri tərəfindən dəstəklənir I1NT03000 (sıfıra bölmədə tavan bölmə tələləri).Qeydlər
- Quraşdırmalar nazik şimlərdir; kompilyator hərəkətləri və `SCALL`-i qeyd etmək üçün onları aşağı salır.
- Göstərici konstruktorları təmizdir: VM INPUT-da Norito TLV-nin zəng müddəti üçün dəyişməz olmasını təmin edir.
 - Göstərici-ABI sahələri olan strukturlar (məsələn, `DomainId`, `AccountId`) sistem çağırışı arqumentlərini erqonomik olaraq qruplaşdırmaq üçün istifadə edilə bilər. Kompilyator əlavə ayırmalar olmadan `obj.field`-ni düzgün registr/dəyərlə əlaqələndirir.

## Kolleksiyalar və Xəritələr

Növ: `Map<K, V>`
- Yaddaşdaxili xəritələr (`Map::new()` vasitəsilə yığın ayrılır və ya parametrlər kimi ötürülür) tək açar/dəyər cütünü saxlayır; düymələr və dəyərlər söz ölçülü tiplər olmalıdır: `int`, `bool`, `string`, `Blob`, `bytes`, `Json`, və ya. `AccountId`, `Name`).
- Davamlı vəziyyət xəritələri (`state Map<...>`) Norito kodlu açar/dəyərlərdən istifadə edir. Dəstəklənən düymələr: `int` və ya göstərici növləri. Dəstəklənən dəyərlər: `int`, `bool`, `Json`, `Blob`/`bytes` və ya göstərici növləri.
- `Map::new()` vahid yaddaşdaxili girişi ayırır və sıfırla işə salır (açar/dəyər = 0); `Map<int,int>` olmayan xəritələr üçün açıq tipli annotasiya və ya qaytarma növü təmin edin.
- Dövlət xəritələri birinci dərəcəli qiymətlər deyil: siz onları yenidən təyin edə bilməzsiniz (məsələn, `M = Map::new()`); indeksləşdirmə vasitəsilə daxiletmələri yeniləyin (`M[key] = value`).
- Əməliyyatlar:
  - İndeksləmə: `map[key]` dəyərini əldə etmək/tənzimləmək (host syscall vasitəsilə həyata keçirilir; iş vaxtı API xəritəsinə baxın).
  - Mövcudluq: `contains(map, key) -> bool` (azaldılmış köməkçi; daxili sistem zəngi ola bilər).
  - İterasiya: deterministik qayda və mutasiya qaydaları ilə `for (k, v) in map { ... }`.

Deterministik iterasiya qaydaları
- İterasiya dəsti dövrə girişindəki düymələrin snapshotıdır.
- Sifariş Norito kodlu açarların ciddi şəkildə artan bayt-leksikoqrafik sırasıdır.
- Döngə zamanı təkrarlanan xəritəyə struktur dəyişiklikləri (daxil et/çıxar/təmizlə) deterministik `E_ITER_MUTATION` tələsinə səbəb olur.
- Məhdudiyyət tələb olunur: ya xəritədə elan edilmiş maksimum (`@max_len`), açıq atribut `#[bounded(n)]`, ya da `.take(n)`/`.range(..)` istifadə edərək açıq sərhəd; əks halda kompilyator `E_UNBOUNDED_ITERATION` yayır.

Həddi köməkçilər
- `#[bounded(n)]`: xəritə ifadəsində isteğe bağlı atribut, məs. `for (k, v) in my_map #[bounded(2)] { ... }`.
- `.take(n)`: başlanğıcdan ilk `n` girişlərini təkrarlayın.
- `.range(start, end)`: `[start, end)` yarımaçıq intervalda qeydləri təkrarlayın. Semantika `start` və `n = end - start` ilə bərabərdir.Dinamik sərhədlər haqqında qeydlər
- Hərfi sərhədlər: `n`, `start` və `end` tam ədəd literalları kimi tam dəstəklənir və sabit sayda iterasiyaya tərtib edilir.
- Qeyri-hərfi sərhədlər: `kotodama_dynamic_bounds` xüsusiyyəti `ivm` qutusunda aktiv edildikdə, kompilyator dinamik `n`, `start` və `end` (təhlükəsizlik və daxilolma ifadəsi) kimi qəbul edir. `end >= start`). Azaldılması əlavə bədən icralarından qaçmaq üçün `if (i < n)` yoxlamaları ilə K qorunan iterasiyaya qədər emissiya edir (defolt K=2). `CompilerOptions { dynamic_iter_cap, .. }` vasitəsilə K-ni proqramlı şəkildə kökləyə bilərsiniz.
- Tərtib etməzdən əvvəl Kotodama lint xəbərdarlıqlarını yoxlamaq üçün `koto_lint`-i işə salın; əsas kompilyator təhlildən və tip yoxlamasından sonra həmişə endirmə ilə davam edir.
- Səhv kodları [Kotodama Compiler Error Codes](./kotodama_error_codes.md) bölməsində sənədləşdirilmişdir; sürətli izahatlar üçün `koto_compile --explain <code>` istifadə edin.

## Səhvlər və Diaqnostika

Kompilyasiya vaxtı diaqnostikası (nümunələr)
- `E_UNBOUNDED_ITERATION`: xəritə üzərində dövrə sərhədi yoxdur.
- `E_MUT_DURING_ITER`: döngə gövdəsində təkrarlanan xəritənin struktur mutasiyası.
- `E_STATE_SHADOWED`: yerli bağlamalar `state` bəyannamələrinə kölgə sala bilməz.
- `E_BREAK_OUTSIDE_LOOP`: `break` dövrə xaricində istifadə olunur.
- `E_CONTINUE_OUTSIDE_LOOP`: `continue` dövrə xaricində istifadə olunur.
- `E0005`: for-loop başlatıcı dəstəklənəndən daha mürəkkəbdir.
- `E0006`: for-loop addım bəndi dəstəklənəndən daha mürəkkəbdir.
- `E_BAD_POINTER_USE`: birinci dərəcəli növün tələb olunduğu bir göstərici-ABI konstruktor nəticəsinin istifadəsi.
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`.
- Alətlər: `koto_compile` bayt kodunu buraxmazdan əvvəl lint keçidini işə salır; keçmək üçün `--no-lint` istifadə edin və ya lint çıxışı üzərində qurulma uğursuzluğu üçün `--deny-lint-warnings` istifadə edin.

Runtime VM xətaları (seçilmiş; ivm.md-də tam siyahı)
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, Norito.

Səhv mesajları
- Diaqnostika mövcud olduqda `kotoba {}` tərcümə cədvəllərindəki girişlərə uyğun gələn sabit `msg_id`-ləri daşıyır.

## IVM ilə Codegen Xəritəçəkmə

Boru kəməri
1. Lexer/Parser AST istehsal edir.
2. Semantik təhlil adları həll edir, növləri yoxlayır və simvol cədvəllərini doldurur.
3. İQ-nin sadə SSA-ya bənzər formaya endirilməsi.
4. IVM GPR-lərə (Çağırış konvensiyasına görə args/ret üçün `r10+`) ayırmağı qeyd edin; yığmaq üçün tökülür.
5. Baytkod emissiyası: icazə verilən IVM yerli və RV uyğun kodlaşdırmaların qarışığı; `abi_version`, xüsusiyyətlər, vektor uzunluğu və `max_cycles` ilə yayılan metadata başlığı.Diqqət çəkən məqamların xəritələşdirilməsi
- IVM ALU əməliyyatlarına arifmetik və məntiq xəritəsi.
- Şərti budaqlara və atlamalara budaqlanma və nəzarət xəritəsi; kompilyator sərfəli olduqda sıxılmış formalardan istifadə edir.
- Yerlilər üçün yaddaş VM yığınına tökülür; uyğunlaşdırılması həyata keçirilir.
- Hərəkətləri qeyd etmək üçün aşağı quraşdırılmış qurğular və 8 bitlik nömrə ilə `SCALL`.
- Göstərici konstruktorları Norito TLV-ləri INPUT bölgəsinə yerləşdirir və onların ünvanlarını yaradır.
- `ASSERT`/`ASSERT_EQ` üçün təsdiqləmə xəritəsi, ZK olmayan icrada tələyə salır və ZK quruluşlarında məhdudiyyətlər yaradır.

Determinizm məhdudiyyətləri
- FP yoxdur; qeyri-deterministik sistem zəngləri yoxdur.
- SIMD/GPU sürətləndirilməsi baytkoda görünmür və bit-eyni olmalıdır; kompilyator hardware üçün xüsusi əməliyyatlar yaymır.

## ABI, Başlıq və Manifest

Kompilyator tərəfindən təyin edilmiş IVM başlıq sahələri
- `version`: IVM bayt kodu format versiyası (major.minor).
- `abi_version`: sistem çağırışı cədvəli və göstərici-ABI sxem versiyası.
- `feature_bits`: xüsusiyyət bayraqları (məsələn, `ZK`, `VECTOR`).
- `vector_len`: məntiqi vektor uzunluğu (0 → qurulmamış).
- `max_cycles`: qəbula bağlıdır və ZK doldurma işarəsi.

Manifest (isteğe bağlı yan araba)
- `code_hash`, `abi_hash`, `meta {}` blokundan metadata, kompilyator versiyası və təkrar istehsal üçün göstərişlər qurun.

## Yol Xəritəsi

- **KD-231 (Aprel 2026):** təkrarlama hədləri üçün tərtib vaxtı diapazonu təhlili əlavə edin ki, döngələr planlaşdırıcıya məhdud giriş dəstlərini ifşa etsin.
- **KD-235 (May 2026):** göstərici konstruktorları və ABI aydınlığı üçün `string`-dən fərqli olaraq birinci dərəcəli `bytes` skalyar təqdim edir.
- **KD-242 (İyun 2026):** deterministik geri dönüşləri olan xüsusiyyət bayraqlarının arxasında daxili əməliyyat kodu dəstini (hesh/imza yoxlaması) genişləndirin.
- **KD-247 (İyun 2026):** `msg_id`s səhvini stabilləşdirin və lokallaşdırılmış diaqnostika üçün `kotoba {}` cədvəllərində xəritələşdirməni qoruyun.
### Manifest Emissiya

- Kotodama kompilyator API `ivm::kotodama::compiler::Compiler::compile_source_with_manifest` vasitəsilə tərtib edilmiş `.to` ilə yanaşı `ContractManifest`-i qaytara bilər.
- Sahələr:
  - `code_hash`: artefaktı bağlamaq üçün kompilyator tərəfindən hesablanan kod baytlarının hashı (IVM başlığı və literallar istisna olmaqla).
  - `abi_hash`: proqramın `abi_version` üçün icazə verilən sistem zəngi səthinin stabil həzmi (bax: `ivm.md` və `ivm::syscalls::compute_abi_hash`).
- İsteğe bağlı `compiler_fingerprint` və `features_bitmap` alət zəncirləri üçün qorunur.
- `entrypoints`: ixrac edilmiş giriş nöqtələrinin sifarişli siyahısı (ictimai, `hajimari`, `kaizen`), o cümlədən onların tələb olunan `permission(...)` sətirləri və tərtibçinin ən yaxşı səy göstərdiyi oxumaq/yazmaq üçün gözlənilən giriş cədvəlləri və giriş cədvəlləri haqqında məlumat.
- Manifest qəbul vaxtı yoxlamaları və reyestrlər üçün nəzərdə tutulub; həyat dövrü üçün `docs/source/new_pipeline.md`-ə baxın.
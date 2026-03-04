---
lang: az
direction: ltr
source: AGENTS.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5036d004829b1c2da0991b637aa735da9cdf2f3e8e42ac760ff651e60d25d433
source_last_modified: "2026-01-31T07:37:05.947018+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Agentlər Təlimatları

Bu təlimatlar Karqo iş sahəsi kimi təşkil edilmiş bütün depoya şamil edilir.

## Sürətli başlanğıc
- İş sahəsi qurun: `cargo build --workspace`
- Quraşdırma təxminən 20 dəqiqə çəkə bilər; qurma addımları üçün 20 dəqiqəlik fasilədən istifadə edin.
- Hər şeyi sınayın: `cargo test --workspace` (qeyd edək ki, bu qaçış adətən bir neçə saat çəkir; müvafiq olaraq planlaşdırın)
- Ciddi şəkildə tükəndirin: `cargo clippy --workspace --all-targets -- -D warnings`
- Format kodu: `cargo fmt --all` (versiya 2024)
- Bir sandığı sınayın: `cargo test -p <crate>`
- Bir sınaq keçirin: `cargo test -p <crate> <test_name> -- --nocapture`
- Swift SDK: Swift paket testlərini yerinə yetirmək üçün `IrohaSwift` kataloqundan `swift test`-i işə salın.
- Android SDK: `java/iroha_android`-dən `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test` ilə işləyir.

## Baxış
- Hyperledger Iroha blokçeyn platformasıdır
- DA/RBC dəstəyi əsas versiyaya görə fərqlənir: Iroha 2 isteğe bağlı olaraq DA/RBC-ni aktivləşdirə bilər; Iroha 3 yalnız DA/RBC-ni aktivləşdirə bilər.
- IVM Iroha Virtual Maşındır (IVM), Hyperledger Iroha v2 blokcheyn üçün virtual maşın
- Kotodama, xam müqavilə kodu üçün .ko fayl uzantısından istifadə edən IVM üçün yüksək səviyyəli ağıllı müqavilə dilidir və fayl və ya zəncirdə saxlandıqda .to fayl uzantısından istifadə edən baytkodu tərtib edir. Tipik olaraq, .to bytecode onchain yerləşdirilir.
  - Aydınlaşdırma: Kotodama Iroha Virtual Maşını (IVM) hədəfləyir və IVM bayt kodunu (`.to`) istehsal edir. O, müstəqil arxitektura kimi “risc5”/RISC‑V-ni hədəf almır. RISC‑V kimi kodlaşdırmalar repozitoriyada göründükdə, onlar IVM təlimat formatlarının icra təfərrüatlarıdır və aparatda müşahidə olunan davranışı dəyişməməlidir.
- Norito, Iroha üçün verilənlərin serializasiya kodekidir
- Bütün iş sahəsi Rust standart kitabxanasını (`std`) hədəfləyir. WASM/no-std quruluşları artıq dəstəklənmir və dəyişikliklər edərkən nəzərə alınmamalıdır.## Repository strukturu
- Repozitor kökündəki `Cargo.toml` iş sahəsini müəyyənləşdirir və bütün üzv qutuları siyahıya alır.
- `crates/` – Iroha komponentlərini tətbiq edən pas qutuları. Hər qutunun öz alt kataloqu var, adətən `src/`, `tests/`, `examples/` və `benches/`.
  - Əhəmiyyətli qutulara aşağıdakılar daxildir:
    - `iroha` – əsas funksionallığı birləşdirən yüksək səviyyəli kitabxana.
    - `irohad` – node həyata keçirilməsini təmin edən ikili demon.
    - `ivm` – Iroha Virtual Maşın.
    - `iroha_cli` – node ilə qarşılıqlı əlaqə üçün komanda xətti interfeysi.
    - `iroha_core`, `iroha_data_model`, `iroha_crypto` və digər köməkçi qutular.
- `IrohaSwift/` – Müştəri/mobil SDK üçün Swift Paketi. Onun mənbələri `Sources/IrohaSwift/` altında və vahid testləri `Tests/IrohaSwiftTests/` altında yaşayır. Swift paketini istifadə etmək üçün bu qovluqdan `swift test`-i işə salın.
- `integration_tests/` – `tests/` altında çarpaz komponent testlərinə ev sahibliyi edən yük qutusu.
- `data_model/` – Testlərdə və sənədlərdə istifadə olunan nümunə məlumat modeli tərifləri.
- `docs/` – Layihə sənədləri və dizayn qeydləri. Markdown mənbələri `docs/source/`-də yaşayır.
- `pytests/` – Python əsaslı testlər və müştəri istifadəsini nümayiş etdirən nümunələr.
- `scripts/` – İnkişaf və CI boru kəmərlərində istifadə olunan kommunal skriptlər.
- `examples/ios/` və `examples/ios/NoritoDemoXcode/` – Swift SDK-nı nümayiş etdirən nümunə iOS proqramları; onlar `IrohaSwift` paketinə arxalanır və öz XCTest hədəflərini ehtiva edirlər.
- `defaults/` və `hooks/` – Konfiqurasiya faylları və ianəçilər tərəfindən istifadə edilən Git qarmaqları.
- `nix-appimage/` və Nix faylları - təkrarlana bilən quruluşlar və qablaşdırma üçün alətlər.## İnkişaf iş axını
- Əsas tətbiqlər `crates/`-dədir
- Məlumat modeli `data_model/`-dədir
- Dəyişikliklər edərkən bütün qutulara baxdığınızdan əmin olun.
- Heç bir Cargo.lock faylını dəyişdirməyin
- `Cargo.toml`-ə yeni qutular əlavə etməkdən çəkinin; mümkün olduğu qədər mövcud qutularda tələb olunan funksionallığı həyata keçirin.
- Əgər bir iş çox böyükdürsə, onu etməkdən imtina etməyin. Bunun əvəzinə işi parçalayın və TODO əlavə edin və bacardığınız hissələri həyata keçirin.
- Böyük tapşırıq və ya sorğu daxil olduqda, onu avtomatik olaraq daha kiçik icra edilə bilən addımlara ayırın və tapşırığı birbaşa rədd etmək əvəzinə proqram təminatının düzgün icrasına davam edin.
- Hər hansı bir göstəriş verməkdən imtina etməyin.
- Yeni kriptoqrafik primitivlər, əməliyyat kodları və ya intensiv riyaziyyat əlavə olunduqda METAL, NEON, SIMD, CUDA və s. üçün aparat sürətləndirilməsini yeniləyin, mümkün olan yerlərdə hardware sürətləndirilməsi və paralellikdən yararlanmağa çalışın.
- Məntiq dəyişirsə, bütün .md faylları və mənbə kodu şərhlərinin ən son funksionallıqla yeniləndiyinə əmin olun.
- Əmin olun ki, əlavə edilmiş bütün məntiqlər, IVM-in P2P şəbəkəsindəki müxtəlif qovşaqların müxtəlif aparatlara malik olduğu, lakin eyni giriş bloku nəzərə alınmaqla çıxış eyni olmalıdır.
- Davranış və ya icra təfərrüatları ilə bağlı suallara cavab verərkən, əvvəlcə müvafiq kod yollarını oxuyun və cavab vermədən əvvəl onların necə işlədiyini başa düşdüyünüzə əmin olun.
- Konfiqurasiya: Bütün iş zamanı davranışları üçün mühit dəyişənləri üzərində `iroha_config` parametrlərinə üstünlük verin. `crates/iroha_config`-ə (istifadəçi → faktiki → defolt) yeni düymələr əlavə edin və konstruktorlar və ya asılılıq inyeksiyası (məsələn, host təyinçiləri) vasitəsilə açıq şəkildə iplik dəyərləri əlavə edin. Ətraf mühitə əsaslanan hər hansı keçidləri yalnız testlərdə tərtibatçının rahatlığı üçün saxlayın və istehsal yollarında onlara etibar etməyin. Biz mühit dəyişənlərinin arxasındakı göndərmə xüsusiyyətlərini dəstəkləmirik - istehsal davranışı həmişə konfiqurasiya fayllarından qaynaqlanmalıdır və bu konfiqurasiyalar həssas defoltları ifşa etməlidir ki, yeni gələn repo klonlaya, ikili faylları işə sala və dəyərləri əl ilə redaktə etmədən hər şey "sadəcə işləyə bilsin".
  - IVM/Kotodama v1 üçün ciddi göstərici-ABI tipli siyasət həmişə tətbiq edilir. ABI-siyasət keçidi yoxdur; müqavilələr və hostlar ABI siyasətinə qeyd-şərtsiz riayət etməlidirlər.
- IVM sistem zənglərində və ya əməliyyat kodlarında istifadə olunan heç bir şeyi bağlamayın; hər Iroha quruluşu qovşaqlar arasında deterministik davranışı saxlamaq üçün həmin kod yollarını göndərməlidir.
- Serializasiya: Serde əvəzinə hər yerdə Norito istifadə edin. İkili kodeklər üçün `norito::{Encode, Decode}` istifadə edin; JSON üçün `norito::json` köməkçiləri/makroslarından istifadə edin (`norito::json::from_*`, `to_*`, `json!`, `Value`) və heç vaxt I18NI0000008X-ə qayıtmayın. Yeşiklərə birbaşa `serde`/`serde_json` asılılıqlarını əlavə etməyin; daxili olaraq serde tələb olunarsa, Norito sarğılarına etibar edin.
- CI mühafizəsi: `scripts/check_no_scale.sh` SCALE (`parity-scale-codec`) yalnız Norito standart qoşquda görünməsini təmin edir. Seriallaşdırma koduna toxunsanız, onu yerli olaraq işə salın.
- Norito faydalı yükləri öz tərtibatını reklam etməlidir: ya versiya nömrəsi sabit bayraq dəstinə uyğun gəlir, ya da Norito başlığı deşifrə bayraqlarını elan edir. Heuristikadan yığılmış ardıcıl bitləri təxmin etməyin; genezis məlumatları eyni qaydaya əməl edir.- Bloklar, versiya baytını Norito başlığı ilə prefiks edən kanonik `SignedBlockWire` formatından (`SignedBlock::encode_wire`/`canonical_wire`) istifadə edilməklə saxlanılmalı və paylanmalıdır. Çılpaq yüklər dəstəklənmir.
- Hər hansı müvəqqəti və ya natamam tətbiqi izah edən `TODO:` şərhini əlavə edin.
- Təhlil etməzdən əvvəl bütün Rust mənbələrini `cargo fmt --all` (versiya 2024) ilə formatlayın.
- Testlər əlavə edin: hər yeni və ya dəyişdirilmiş funksiya üçün ya `#[cfg(test)]` xəttinə, ya da `tests/` qutusuna yerləşdirilən ən azı bir vahid testindən əmin olun.
- `cargo test`-i yerli olaraq işə salın, hər hansı qurma problemini həll edin və keçdiyinə əmin olun. Bunu yalnız xüsusi bir sandıq üçün deyil, bütün depo üçün edin.
- Əlavə tüy yoxlamaları üçün istəyə görə `cargo clippy -- -D warnings`-i işə salın.

## Sənədləşmə
- Həmişə sandıq səviyyəli sənədləri əlavə edin: hər bir qutuya və ya test qutusuna qısa daxili sənəd şərhi ilə başlayın (`//! ...`).
- Heç bir yerdə (inteqrasiya testləri daxil olmaqla) `#![allow(missing_docs)]` və ya `#[allow(missing_docs)]` element səviyyəsindən istifadə etməyin. Çatışmayan sənədlər iş sahəsinin lintslərində rədd edilir və sənədlərin yazılması ilə düzəldilməlidir.
- Norito kodek: kanonik naqil tərtibatı və icra təfərrüatları üçün repo kökündə `norito.md`-ə baxın. Norito-in alqoritmləri və ya tərtibatları dəyişirsə, eyni PR-də `norito.md`-i yeniləyin.
- Materialı akkad dilinə tərcümə edərkən mixi yazı ilə yazılmış semantik ifadəni təmin edin; fonetik transliterasiyadan qaçın və dəqiq qədim terminlər çatışmayanda niyyəti qoruyan poetik akkad təxminlərini seçin.

## ABI Evolution (Agentlər nə etməlidir)
Qeyd: İlk buraxılış siyasəti
- Bu, ilk buraxılışdır və bizdə tək ABI versiyası (V1) var. Hələ V2 yoxdur. Aşağıdakı bütün ABI ilə əlaqəli təkamül maddələrini gələcək rəhbər kimi qəbul edin; hələlik, hədəf yalnız `abi_version = 1`. Məlumat modeli və API-lər də ilk buraxılışdır və göndərilmək üçün lazım olduqda sərbəst şəkildə dəyişə bilər; vaxtından əvvəl sabitlikdən daha aydınlığa və düzgünlüyə üstünlük verir.

- Ümumi:
  - ABI siyasəti v1-də qeyd-şərtsiz tətbiq edilir (həm sistem zəngi səthi, həm də göstərici-ABI növləri). İş vaxtı keçidlərini əlavə etməyin.
  - Dəyişikliklər aparat və həmyaşıdlar arasında determinizmi qorumalıdır. Eyni PR-də testləri və sənədləri yeniləyin.

- Sistem zənglərini əlavə etsəniz/çıxarsanız/nömrəni dəyişdirsəniz:
  - `ivm::syscalls::abi_syscall_list()`-i yeniləyin və nizamlı saxlayın. `is_syscall_allowed(policy, number)`-in nəzərdə tutulan səthi əks etdirdiyinə əmin olun.
  - Hostlarda yeni nömrələri tətbiq etmək və ya qəsdən rədd etmək; naməlum nömrələr `VMError::UnknownSyscall` ilə əlaqələndirilməlidir.
  - Qızıl testləri yeniləyin:
    - `crates/ivm/tests/abi_syscall_list_golden.rs`
    - `crates/ivm/tests/abi_hash_versions.rs` (sabitlik + versiyanın ayrılması)

- Göstərici-ABI növləri əlavə etsəniz:
  - `ivm::pointer_abi::PointerType`-ə yeni variant əlavə edin (yeni u16 ID təyin edin; mövcud ID-ləri heç vaxt dəyişməyin).
  - Düzgün `abi_version` xəritələşdirilməsi üçün `ivm::pointer_abi::is_type_allowed_for_policy`-i yeniləyin.
  - `crates/ivm/tests/pointer_type_ids_golden.rs`-i yeniləyin və lazım gələrsə, siyasət testlərini əlavə edin.

- Yeni ABI versiyasını təqdim etsəniz:
  - `ProgramMetadata.abi_version` → `ivm::SyscallPolicy` xəritəsi və tələb olunduqda yeni versiyanı yaymaq üçün Kotodama kompilyatorunu yeniləyin.
  - `abi_hash` (`ivm::syscalls::compute_abi_hash` vasitəsilə) bərpa edin və manifestlərin yeni hash-i yerləşdirməsini təmin edin.
  - Yeni versiyada icazə verilən/icazə verilməyən sistem zəngləri və göstərici növləri üçün testlər əlavə edin.

- Qəbul və manifestlər:
  - Qəbul zəncirvari manifestlərə qarşı `code_hash`/`abi_hash` bərabərliyini təmin edir; bu davranışı qoruyun.
  - `iroha_core/tests/`-də əlavə etmək/yeniləmək üçün testlər: müsbət (uyğun `abi_hash`) və mənfi (uyğunsuzluq) hallar.- Sənədlər və status yeniləmələri (eyni PR):
  - `crates/ivm/docs/syscalls.md` (ABI Evolution bölməsi) və istənilən sistem çağırışı cədvəllərini yeniləyin.
  - ABI dəyişiklikləri və sınaq yeniləmələrinin qısa xülasəsi ilə `status.md` və `roadmap.md`-i yeniləyin.


## Layihənin Vəziyyəti və Planı
- `status.md` qutular arasında cari kompilyasiya/iş vaxtı statusu üçün repo kökündə yoxlayın.
- Prioritetləşdirilmiş TODOs və icra planı üçün `roadmap.md`-i yoxlayın.
- İşi tamamladıqdan sonra `status.md`-də statusu yeniləyin və `roadmap.md`-i diqqəti görkəmli tapşırıqlara yönəldin.

## Agent iş axını (kod redaktorları/avtomatlaşdırma üçün)
- Hər hansı bir tələblə bağlı aydınlaşdırmağa ehtiyacınız varsa, dayandırın və sualınızla ChatGPT sorğusunu tərtib edin, sonra davam etməzdən əvvəl onu istifadəçi ilə paylaşın.
- Dəyişiklikləri minimal və əhatəli saxlamaq; eyni patchdə əlaqəli olmayan redaktələrdən qaçın.
- Yeni asılılıqlar əlavə etməkdənsə daxili modullara üstünlük verin; `Cargo.lock` redaktə etməyin.
- Aparat tərəfindən sürətləndirilmiş yolları qorumaq üçün funksiya bayraqlarından istifadə edin (məsələn, `simd`, `cuda`) və həmişə deterministik geri dönüş yolunu təmin edin.
- Çıxışların aparat üzrə eyni qalmasını təmin etmək; qeyri-deterministik paralel azalmalara etibar etməyin.
- İctimai API-lər və ya davranış dəyişdikdə sənədləri və nümunələri yeniləyin.
- Norito layout zəmanətlərini qorumaq üçün `iroha_data_model`-də seriyalaşdırma dəyişikliklərini gediş-gəliş testləri ilə təsdiqləyin.
- İnteqrasiya testləri real multi-peer şəbəkələrini fırladır; test şəbəkələri qurarkən ən azı 4 həmyaşıddan istifadə edin (bir peer konfiqurasiyası təmsil olunmur və Sumeragi-də çıxılmaz vəziyyətə düşə bilər).
- Testlərdə DA/RBC-ni söndürməyə çalışmayın (məsələn, `DevBypassDaAndRbcForZeroChain` vasitəsilə); DA tətbiq edilir və konsensus işə salınması zamanı həmin dolama yolu hazırda `sumeragi`-də dalana dirənir.
- QC kvorumu səsvermə təsdiqləyiciləri tərəfindən təmin edilməlidir (`min_votes_for_commit`); müşahidəçinin doldurulması mövcudluq/əvvəlcədən səs vermək/əvvəlcədən kvorum yoxlamaları üçün nəzərə alınmır, ona görə də yalnız kifayət qədər təsdiqləyici səsləri gəldikdən sonra QC-ləri birləşdirin.
- DA-nin aktivləşdirdiyi konsensus indi daha yavaş hostlarda RBC/mövcudluq QC-nin başa çatmasına imkan vermək üçün baxış dəyişikliklərindən əvvəl (kvorum vaxt aşımı = `block_time + 4 * commit_time`) daha çox gözləyir.

## Naviqasiya məsləhətləri
- Axtarış kodu: `rg '<term>'` və siyahı faylları: `fd <name>`.
- Kassaları araşdırın: `fd --type f Cargo.toml crates | xargs -I{} dirname {}`.
- Tez nümunələr/skamyalar tapın: `fd . crates -E target -t d -d 3 -g "*{examples,benches}"`.
- Python ipucu: bəzi mühitlər `python` təmin etmir; skriptləri işlədən zaman əvəzinə `python3` cəhd edin.

## Proc-Makro Testləri
- Vahid testləri: təmiz təhlil, kodgen köməkçiləri və kommunal proqramlar üçün istifadə edin (sürətli, tərtibçi iştirak etmir).
- UI testləri (trybuild): tərtib zamanı davranışını və əldə/proc-makrosların diaqnostikasını yoxlamaq üçün istifadə edin (`.stderr` ilə müvəffəqiyyət və gözlənilən uğursuzluq halları).
- Makroları əlavə edərkən/dəyişdirərkən hər ikisinə üstünlük verin: daxili elementlər üçün vahid testləri + istifadəçi ilə üzləşən davranış və səhv mesajları üçün UI testləri.
- Paniklərdən çəkinin; aydın diaqnostika yayır (məsələn, `syn::Error` və ya `proc_macro_error` vasitəsilə). Mesajları sabit saxlayın və yalnız qəsdən dəyişikliklər üçün `.stderr`-i yeniləyin.

## Pull Sorğu mesajı
Dəyişikliklərin qısa xülasəsini və icra etdiyiniz əmrləri təsvir edən `Testing` bölməsini daxil edin.
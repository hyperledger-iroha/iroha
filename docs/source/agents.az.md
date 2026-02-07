---
lang: az
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2025-12-29T18:16:35.916241+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Avtomatlaşdırma Agentinin İcra Bələdçisi

Bu səhifə istənilən avtomatlaşdırma agenti üçün əməliyyat qoruyucularını ümumiləşdirir
Hyperledger Iroha iş sahəsində işləyir. Kanonikləri əks etdirir
`AGENTS.md` təlimatı və yol xəritəsi istinadları buna görə də qurun, sənədləşdirin və
telemetriya dəyişiklikləri insan tərəfindən istehsal olunmamasından asılı olmayaraq eyni görünür
avtomatlaşdırılmış töhfəçi.

Hər bir tapşırığın deterministik kodu və uyğun sənədləri, testləri,
və əməliyyat sübutları. Aşağıdakı bölmələri əvvəl hazır istinad kimi qəbul edin
`roadmap.md` elementlərinə toxunmaq və ya davranış suallarına cavab vermək.

## Sürətli Başlama Əmrləri

| Fəaliyyət | Komanda |
|--------|---------|
| İş sahəsini yaradın | `cargo build --workspace` |
| Tam sınaq dəstini işə salın | `cargo test --workspace` *(adətən bir neçə saat çəkir)* |
| Clippy-ni defolt xəbərdarlıqları ilə işə salın | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format Rust kodu | `cargo fmt --all` *(2024-cü il nəşri)* |
| Tək sandığı sınayın | `cargo test -p <crate>` |
| Bir sınaq keçirin | `cargo test -p <crate> <test_name> -- --nocapture` |
| Swift SDK testləri | `IrohaSwift/`-dən, `swift test` |

## İş axınının əsasları

- Suallara cavab vermədən və ya məntiqi dəyişməzdən əvvəl müvafiq kod yollarını oxuyun.
- Böyük yol xəritəsi bəndlərini sürüklənən öhdəliklərə bölmək; heç vaxt işi birbaşa rədd etməyin.
- Mövcud iş sahəsi üzvlüyündə qalın, daxili qutuları təkrar istifadə edin və edin
  **açıq göstəriş verilmədiyi halda, `Cargo.lock`-i dəyişdirməyin**.
- Xüsusiyyət bayraqlarından və qabiliyyət keçidlərindən yalnız avadanlıq tərəfindən tələb olunduğu yerlərdə istifadə edin
  sürətləndiricilər; hər platformada deterministik geri dönüşləri mövcud saxlayın.
- İstənilən funksional dəyişikliklə yanaşı sənədləri və Markdown istinadlarını yeniləyin
  buna görə də sənədlər həmişə cari davranışı təsvir edir.
- Hər yeni və ya dəyişdirilmiş funksiya üçün ən azı bir vahid testi əlavə edin. Daxil olana üstünlük verin
  Əhatə dairəsindən asılı olaraq `#[cfg(test)]` modulları və ya qutunun `tests/` qovluğu.
- İşi bitirdikdən sonra `status.md`-i qısa xülasə və arayışla yeniləyin
  müvafiq fayllar; `roadmap.md` diqqətini hələ də işə ehtiyacı olan elementlərə cəmləyin.

## İcra Qoruyucuları

### Serializasiya və Məlumat Modelləri
- Hər yerdə Norito kodekindən istifadə edin (`norito::{Encode, Decode}` vasitəsilə ikili,
  `norito::json::*` vasitəsilə JSON). Birbaşa serde/`serde_json` istifadəsini əlavə etməyin.
- Norito faydalı yükləri öz tərtibatını reklam etməlidir (versiya baytı və ya başlıq bayraqları),
  və yeni formatlar müvafiq sənəd yeniləmələrini tələb edir (məsələn,
  `norito.md`, `docs/source/da/*.md`).
- Yaradılış məlumatları, manifestlər və şəbəkə yükləri deterministik olaraq qalmalıdır
  beləliklə, eyni girişlərə malik iki həmyaşıd eyni hashlər yaradır.

### Konfiqurasiya və İcra Davranışı
- Yeni mühit dəyişənləri əvəzinə `crates/iroha_config`-də yaşayan düymələrə üstünlük verin.
  Dəyərləri konstruktorlar və ya asılılıq inyeksiyası vasitəsilə açıq şəkildə ötürün.
- Heç vaxt IVM sistem zənglərinə və ya əməliyyat kodu davranışına qapılmayın—ABI v1 hər yerdə göndərilir.
- Yeni konfiqurasiya seçimləri əlavə edildikdə, defoltları, sənədləri və hər hansı əlaqəliləri yeniləyin
  şablonlar (`peer.template.toml`, `docs/source/configuration*.md` və s.).### ABI, Syscalls və Pointer Tipləri
- ABI siyasətinə qeyd-şərtsiz yanaşın. Sistem zənglərinin və ya göstərici növlərinin əlavə edilməsi/çıxarılması
  yeniləməni tələb edir:
  - `ivm::syscalls::abi_syscall_list` və `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` plus qızıl testlər
  - ABI hash dəyişdikdə `crates/ivm/tests/abi_hash_versions.rs`
- Naməlum sistem zəngləri `VMError::UnknownSyscall` ilə əlaqələndirilməlidir və manifestlər olmalıdır
  qəbul imtahanlarında imzalanmış `abi_hash` bərabərlik yoxlamalarını qoruyun.

### Hardware Acceleration & Determinism
- Yeni kriptoqrafik primitivlər və ya ağır riyaziyyat hardware sürətləndirilmiş şəkildə göndərilməlidir
  deterministik ehtiyatları qoruyarkən yollar (METAL/NEON/SIMD/CUDA).
- Qeyri-deterministik paralel azalmalardan çəkinin; prioritet eyni çıxışlardır
  hardware fərqli olsa belə, hər bir həmyaşıd.
- Norito və FASTPQ qurğularını təkrar istehsal oluna bilən saxlayın ki, SRE bütün donanmanı yoxlaya bilsin.
  telemetriya.

### Sənədlər və Sübutlar
- Portalda (`docs/portal/...`) hər hansı ictimai sənəd dəyişikliyini əks etdirin
  Sənədlər saytının Markdown mənbələri ilə aktual qalması üçün tətbiq edilir.
- Yeni iş axınları təqdim edildikdə, runbooks, idarəetmə qeydləri və ya əlavə edin
  məşq etməyi, geri qaytarmağı və sübutları ələ keçirməyi izah edən yoxlama siyahıları.
- Məzmunu Akkad dilinə tərcümə edərkən, yazılmış semantik renderləri təmin edin
  fonetik transliterasiyalardan çox mixi yazı ilə.

### Test və Alət Gözləntiləri
- Müvafiq test paketlərini yerli olaraq işə salın (`cargo test`, `swift test`,
  inteqrasiya qoşquları) və PR testi bölməsində əmrləri sənədləşdirin.
- CI qoruyucu skriptlərini (`ci/*.sh`) və idarə panellərini yeni telemetriya ilə sinxronlaşdırın.
- Proc-makroslar üçün diaqnostikanı kilidləmək üçün vahid testlərini `trybuild` UI testləri ilə birləşdirin.

## Göndərməyə Hazır Yoxlama Siyahısı

1. Kod tərtib edir və `cargo fmt` heç bir fərq yaratmadı.
2. Yenilənmiş sənədlər (iş sahəsi Markdown və portal güzgüləri) yeniləri təsvir edir
   davranış, yeni CLI bayraqları və ya konfiqurasiya düymələri.
3. Testlər hər bir yeni kod yolunu əhatə edir və reqressiyalar zamanı deterministik şəkildə uğursuz olur
   görünür.
4. Telemetriya, idarə panelləri və xəbərdarlıq tərifləri hər hansı yeni ölçülərə istinad edir və ya
   səhv kodları.
5. `status.md` müvafiq fayllara istinad edən qısa xülasə daxildir və
   yol xəritəsi bölməsi.

Bu yoxlama siyahısına əməl etmək yol xəritəsinin icrasını yoxlanıla bilir və hər şeyi təmin edir
agent digər komandaların etibar edə biləcəyi sübutları təqdim edir.
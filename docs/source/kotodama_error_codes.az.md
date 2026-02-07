---
lang: az
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2025-12-29T18:16:35.974178+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Kompilyator Səhv Kodları

Kotodama kompilyatoru sabit xəta kodları yayır ki, alətlər və CLI istifadəçiləri
uğursuzluğun səbəbini tez anlayın. `koto_compile --explain <code>` istifadə edin
müvafiq işarəni çap etmək üçün.

| Kod | Təsvir | Tipik Düzəltmə |
|-------|-------------|-------------|
| `E0001` | Filial hədəfi IVM keçid kodlaşdırması üçün əhatə dairəsindən kənardadır. | Əsas blok məsafələri ±1MiB daxilində qalması üçün çox böyük funksiyaları ayırın və ya daxili xətti azaldın. |
| `E0002` | Zəng saytları heç vaxt müəyyən edilməmiş funksiyaya istinad edir. | Zəng edəni silən yazı xətalarını, görünürlük dəyişdiricilərini və ya xüsusiyyət bayraqlarını yoxlayın. |
| `E0003` | Davamlı vəziyyət sistem zəngləri ABI v1 aktiv edilmədən buraxıldı. | `CompilerOptions::abi_version = 1` təyin edin və ya `seiyaku` müqaviləsi daxilində `meta { abi_version: 1 }` əlavə edin. |
| `E0004` | Aktivlə əlaqəli sistemlər hərfi olmayan göstəricilər aldı. | `account_id(...)`, `asset_definition(...)` və s. istifadə edin və ya host defoltları üçün 0 gözətçi keçirin. |
| `E0005` | `for`-loop başlatıcısı bu gün dəstəklənəndən daha mürəkkəbdir. | Döngədən əvvəl kompleks quraşdırmanı köçürün; hazırda yalnız sadə `let`/ifadə başlatıcıları qəbul edilir. |
| `E0006` | `for`-loop addım bəndi bu gün dəstəklənəndən daha mürəkkəbdir. | Döngə sayğacını sadə bir ifadə ilə yeniləyin (məsələn, `i = i + 1`). |
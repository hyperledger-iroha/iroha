---
lang: az
direction: ltr
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-11T04:52:11.136647+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Xəta Xəritəçəkmə Bələdçisi

Son yenilənmə: 21-08-2025

Bu bələdçi Iroha-də ümumi uğursuzluq rejimlərini məlumat modelinin aşkar etdiyi sabit xəta kateqoriyalarına uyğunlaşdırır. Testləri tərtib etmək və müştəri səhvlərinin idarə edilməsini proqnozlaşdırıla bilən etmək üçün ondan istifadə edin.

Prinsiplər
- Təlimat və sorğu yolları strukturlaşdırılmış nömrələr buraxır. Paniklərdən çəkinin; Mümkünsə, müəyyən bir kateqoriya haqqında məlumat verin.
- Kateqoriyalar sabitdir, mesajlar inkişaf edə bilər. Müştərilər sərbəst forma sətirlərində deyil, kateqoriyalar üzrə uyğun olmalıdır.

Kateqoriyalar
- InstructionExecutionError::Find: Müəssisə çatışmır (aktiv, hesab, domen, NFT, rol, tetikleyici, icazə, açıq açar, blok, əməliyyat). Nümunə: mövcud olmayan metadata açarının silinməsi Find(MetadataKey) verir.
- InstructionExecutionError::Repetition: Dublikat qeydiyyat və ya ziddiyyətli ID. Təlimat tipini və təkrarlanan IdBox-u ehtiva edir.
- InstructionExecutionError::Mintability: Mintability invariant pozuldu (`Once` iki dəfə tükəndi, `Limited(n)` həddən artıq çəkildi və ya `Infinitely`-i söndürməyə cəhd etdi). Nümunələr: `Once` kimi müəyyən edilmiş aktivin iki dəfə zərb edilməsi `Mintability(MintUnmintable)` gəliri; `Limited(0)` konfiqurasiyası `Mintability(InvalidMintabilityTokens)` verir.
- InstructionExecutionError::Riyaziyyat: Rəqəmsal domen xətaları (daşmaq, sıfıra bölmək, mənfi dəyər, kifayət qədər kəmiyyət deyil). Nümunə: mövcud məbləğdən daha çox yandırmaq Math(NotEnoughQuantity) verir.
- InstructionExecutionError::InvalidParameter: Yanlış təlimat parametri və ya konfiqurasiyası (məsələn, keçmişdə vaxt tetikleyicisi). Səhv tərtib edilmiş müqavilə yükləri üçün istifadə edin.
- InstructionExecutionError::Qiymətləndirin: təlimat forması və ya növləri üçün DSL/spec uyğunsuzluğu. Nümunə: aktivin dəyəri üçün səhv rəqəmli spesifikasiya Qiymətləndirmə (Type(AssetNumericSpec(...))) verir.
- InstructionExecutionError::InvariantViolation: Digər kateqoriyalarda ifadə edilə bilməyən sistem invariantının pozulması. Misal: sonuncu imzalayanı silməyə cəhd.
- InstructionExecutionError::Query: Təlimatın icrası zamanı sorğu uğursuz olduqda QueryExecutionFail-in paketlənməsi.

QueryExecutionFail
- Tap: Sorğu kontekstində çatışmayan obyekt.
- Dönüşüm: Sorğu tərəfindən gözlənilən səhv növ.
- Tapılmadı: Canlı sorğu kursoru yoxdur.
- CursorMismatch / CursorDone: Kursor protokol xətaları.
- FetchSizeTooBig: Server tərəfindən tətbiq edilən limiti keçdi.
- GasBudgetExceeded: Sorğunun icrası qaz/materiallaşdırma büdcəsini keçib.
- InvalidSingularParameters: Tək sorğular üçün dəstəklənməyən parametrlər.
- CapacityLimit: Canlı sorğu anbarının tutumu çatdı.

Test göstərişləri
- Xətanın mənşəyinə yaxın vahid testlərə üstünlük verin. Məsələn, data-model testlərində aktivin rəqəmli spesifik uyğunsuzluğu yaradıla bilər.
- İnteqrasiya testləri nümayəndəli hallar üçün (məsələn, dublikat reyestr, silinmə zamanı çatışmayan açar, sahiblik olmadan köçürmə) başdan-başa xəritələşdirməni əhatə etməlidir.
- Mesaj alt sətirləri əvəzinə enum variantlarını uyğunlaşdırmaqla təsdiqləri möhkəm saxlayın.
---
lang: az
direction: ltr
source: docs/source/crypto/sm_lock_refresh_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3065571b34a226a5871c4fb68063f9419e48074b20096de215f440bdf54a4e59
source_last_modified: "2025-12-29T18:16:35.943236+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM sıçrayışının tələb etdiyi Cargo.lock yeniləməsini planlaşdırmaq üçün prosedur.

# SM Xüsusiyyəti `Cargo.lock` Yeniləmə Planı

`--locked` tətbiq edilərkən, `iroha_crypto` üçün `sm` xüsusiyyət artımı əvvəlcə `cargo check`-i tamamlaya bilmədi. Bu qeyd təsdiqlənmiş `Cargo.lock` yeniləməsi üçün koordinasiya addımlarını qeyd edir və bu ehtiyacın cari vəziyyətini izləyir.

> **2026-02-12 yeniləmə:** Son yoxlama indi mövcud kilid faylı ilə qurulan isteğe bağlı `sm` funksiyasını göstərir (`cargo check -p iroha_crypto --features sm --locked` 7,9 s soyuqda/0,23 s istidə uğur qazanır). Asılılıq dəstində artıq `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `Cargo.lock`, `Cargo.lock`, `Cargo.lock`, `sm2`, `sm3`, `sm4` və `sm4-gcm`, buna görə də dərhal kilid yeniləməsi tələb olunmur. Gələcək asılılıq zərbələri və ya yeni əlavə qutular üçün aşağıdakı proseduru gözləmə rejimində saxlayın.

## Yeniləmə niyə lazımdır
- Sünbülün əvvəlki iterasiyaları kilid faylında olmayan əlavə qutuların əlavə edilməsini tələb edirdi. Cari kilid snapshotlarına artıq RustCrypto yığını (`sm2`, `sm3`, `sm4`, dəstəkləyən kodeklər və AES köməkçiləri) daxildir.
- Repository siyasəti hələ də fürsətçi kilid faylı redaktələrini bloklayır; gələcək asılılıq təkmilləşdirməsi lazımdırsa, aşağıdakı prosedur qüvvədə qalır.
- Bu planı saxlayın ki, komanda SM ilə əlaqəli yeni asılılıqlar təqdim edildikdə və ya mövcud olanlar versiya zərbələrinə ehtiyac duyduqda idarə olunan yeniləməni həyata keçirə bilsin.

## Təklif olunan koordinasiya addımları
1. **Crypto WG + Release Eng sinxronizasiyasında sorğunu artırın (sahibi: @crypto-wg lead).**
   - `docs/source/crypto/sm_program.md`-ə istinad edin və funksiyanın isteğe bağlı xarakterini qeyd edin.
   - Paralel kilid faylı dəyişdirmə pəncərələrinin olmadığını təsdiqləyin (məsələn, asılılığın dondurulması).
2. **Lock diff ilə yamaq hazırlayın (sahibi: @release-eng).**
   - Yalnız tələb olunan qutuları yeniləmək üçün `scripts/sm_lock_refresh.sh` (təsdiqdən sonra) icra edin.
   - `cargo tree -p iroha_crypto --features sm` çıxışını çəkin (skript `target/sm_dep_tree.txt` yayır).
3. **Təhlükəsizlik baxışı (sahibi: @security-reviews).**
   - Yeni qutuların/versiyaların audit reyestrinə və lisenziyalaşdırma gözləntilərinə uyğunluğunu yoxlayın.
   - Təchizat zənciri izləyicisində hashləri qeyd edin.
4. **Pəncərənin icrasını birləşdirin.**
   - Yalnız kilid faylı deltası, asılılıq ağacı snapshot (artifakt kimi əlavə olunur) və yenilənmiş audit qeydlərindən ibarət PR təqdim edin.
   - Birləşmədən əvvəl CI-nin `cargo check -p iroha_crypto --features sm` ilə işlədiyinə əmin olun.
5. **İzləyici tapşırıqlar.**
   - `docs/source/crypto/sm_program.md` fəaliyyət elementi yoxlama siyahısını yeniləyin.
   - SDK komandasına xüsusiyyətin yerli olaraq `--features sm` ilə tərtib oluna biləcəyi barədə məlumat verin.## Taymlayn və sahiblər
| Addım | Hədəf | Sahibi | Status |
|------|--------|-------|--------|
| Növbəti Crypto WG çağırışında gündəm sahəsini tələb edin | 22-01-2025 | Kripto WG aparıcı | ✅ Tamamlandı (nəzərdən bağlanmış sünbül təzələmədən davam edə bilər) |
| Qaralama seçmə `cargo update` əmri + ağıl fərqi | 24-01-2025 | Release Engineering | ⚪ Gözləmə rejimində (yeni qutular görünsə, yenidən aktivləşdirin) |
| Yeni qutuların təhlükəsizlik baxışı | 27-01-2025 | Təhlükəsizlik Baxışları | ⚪ Gözləmə rejimində (yeniləmə davam etdikdə audit yoxlama siyahısından təkrar istifadə edin) |
| Kilid faylı yeniləməsini birləşdirin PR | 29-01-2025 | Release Engineering | ⚪ Gözləmə rejimində |
| SM proqramı sənəd yoxlama siyahısını yeniləyin | Birləşdikdən sonra | Kripto WG aparıcı | ✅ `docs/source/crypto/sm_program.md` girişi ilə ünvanlandı (2026-02-12) |

## Qeydlər
- İstənilən gələcək yeniləməni yuxarıda sadalanan SM ilə əlaqəli qutularla (və `rfc6979` kimi köməkçilərlə) məhdudlaşdıraraq, geniş iş sahəsində `cargo update`-dən qaçın.
- Əgər hər hansı bir keçid asılılığı MSRV driftini təqdim edərsə, birləşmədən əvvəl onu üzə çıxarın.
- Birləşdikdən sonra `sm` funksiyası üçün qurma vaxtlarına nəzarət etmək üçün müvəqqəti CI işini aktivləşdirin.
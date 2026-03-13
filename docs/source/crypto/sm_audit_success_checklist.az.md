---
lang: az
direction: ltr
source: docs/source/crypto/sm_audit_success_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ef9305dc14d477a616923c80445094c692bc6a38d69465f679b54ccd52e92
source_last_modified: "2025-12-29T18:16:35.940844+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Audit Uğur Meyarları
% Iroha Kripto İşçi Qrupu
% 2026-01-30

# Məqsəd

Bu yoxlama siyahısı müvəffəqiyyət üçün tələb olunan konkret meyarları əks etdirir
SM2/SM3/SM4 xarici auditin tamamlanması. zamanı nəzərdən keçirilməlidir
başlanğıc, hər status yoxlama məntəqəsində yenidən nəzərdən keçirilir və çıxışı təsdiqləmək üçün istifadə olunur
istehsal validatorları üçün SM imzalamasını işə salmadan əvvəl şərtlər.

# Nişanqabağı Hazırlıq

- [ ] Müqavilə imzalanıb, o cümlədən əhatə dairəsi, çatdırılma, məxfilik və
      düzəltməyə dəstək dili.
- [ ] Audit komandası repozitoriya güzgü girişi, CI artefakt paketi və
      sənədlər paketi `docs/source/crypto/sm_audit_brief.md`-də qeyd edilmişdir.
- [ ] Əlaqə nöqtələri hər bir rol üçün ehtiyat nüsxələrlə təsdiqlənir
      (kripto, IVM, platforma əməliyyatları, təhlükəsizlik, sənədlər).
- [ ] Daxili maraqlı tərəflər hədəf buraxılış tarixinə uyğunlaşır və pəncərələri dondurur.
- [ ] SBOM ixracı (`cargo auditable` + CycloneDX) yaradıldı və paylaşıldı.
- [ ] OpenSSL/Tongsuo qurmaq mənşə paketi hazırlanmışdır
      (mənbə tarball hash, script Build, reproducibility qeydləri).
- [ ] Ən son deterministik test nəticələri alındı:
      `scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm` və
      Norito gediş-dönüş qurğuları.
- [ ] Torii `/v2/node/capabilities` reklamı (`iroha runtime capabilities` vasitəsilə) qeydə alınıb, `crypto.sm` manifest sahələrini və sürətləndirmə siyasətinin snapşotunu yoxlayır.

# Nişan icrası

- [ ] Başlanğıc seminarı məqsədlərin ortaq anlaşılması ilə tamamlandı,
      vaxt qrafikləri və ünsiyyət kadansı.
- [ ] Həftəlik status hesabatları alındı ​​və sınaqdan keçirildi; risk reyestri yeniləndi.
- [ ] Nəticələr aşkar edildikdən sonra bir iş günü ərzində ciddilik dərəcəsinə çatdıqda
      Yüksək və ya Kritikdir.
- [ ] Audit qrupu ≥2 CPU arxitekturasında determinizm yollarını təsdiqləyir (x86_64,
      aarch64) uyğun çıxışlarla.
- [ ] Yan kanalın nəzərdən keçirilməsinə daimi sübutlar və ya empirik sınaq daxildir
      həm Rust, həm də FFI yolları üçün sübut.
- [ ] Uyğunluq və sənədlərin nəzərdən keçirilməsi operator rəhbərliyinin uyğunluqlarını təsdiqləyir
      tənzimləyici öhdəliklər.
- [ ] İstinad tətbiqlərinə qarşı diferensial sınaq (RustCrypto,
      OpenSSL/Tongsuo) auditor nəzarəti ilə həyata keçirilir.
- [ ] Fuzz qoşquları qiymətləndirilib; boşluqların mövcud olduğu yerlərdə yeni toxum korpusu.

# Təmir və Çıxış

- [ ] Bütün tapıntılar ciddilik, təsir, istismar və
      tövsiyə olunan düzəliş addımları.
- [ ] Yüksək/Kritik məsələlər auditor tərəfindən təsdiq edilmiş yamaqlar və ya azalmalar alır
      yoxlama; qalıq risklər sənədləşdirilir.
- [ ] Auditor sabit problemləri (fərq, test
      çalışır və ya imzalanmış attestasiya).
- [ ] Təqdim olunan yekun hesabat: icraçı xülasə, ətraflı nəticələr, metodologiya,
      determinizm hökmü, uyğunluq hökmü.
- [ ] Daxili imzalanma iclası növbəti addımları, buraxılış tənzimləmələrini,
      və sənəd yeniləmələri.
- [ ] `status.md` audit nəticəsi və əla təmir işləri ilə yeniləndi
      təqiblər.
- [ ] Ölümdən sonrakı tutuldu `docs/source/crypto/sm_program.md` (dərslər)
      öyrənilmiş, gələcək sərtləşdirmə tapşırıqları).
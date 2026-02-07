---
lang: az
direction: ltr
source: docs/source/crypto/attachments/sm_sales_usage_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14f32b40ff71fa4eef698eac80d8d7dd27104b46b84523d735d054dedea1c47a
source_last_modified: "2025-12-29T18:16:35.938696+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Satış və İstifadə Sənədi (销售/使用备案) Şablonu
% Hyperledger Iroha Uyğunluq İşçi Qrupu
% 2026-05-06

# Təlimat

Quruda SCA ofisinə yerləşdirmə istifadəsini təqdim edərkən bu şablondan istifadə edin
operatorlar. Hər yerləşdirmə klasteri və ya məlumat sahəsi üçün bir təqdimat təmin edin. Yeniləyin
operatora xas təfərrüatlar olan yer tutucuları və sadalanan sübutları əlavə edin
yoxlama siyahısında.

# 1. Operator və Yerləşdirmə Xülasəsi

| Sahə | Dəyər |
|-------|-------|
| Operator adı | {{ OPERATOR_NAME }} |
| Biznes qeydiyyatı ID | {{ REG_ID }} |
| Qeydiyyat ünvanı | {{ ÜNVAN }} |
| Əsas əlaqə (ad / başlıq / e-poçt / telefon) | {{ ƏLAQƏ }} |
| Yerləşdirmə identifikatoru | {{ DEPLOYMENT_ID }} |
| Yerləşdirmə yeri(lər)i | {{ YERLƏR }} |
| Sənədləşdirmə növü | Satış / İstifadə (销售/使用备案) |
| Müraciət tarixi | {{ YYYY-AA-GG }} |

# 2. Yerləşdirmə təfərrüatları

- Proqram qurma ID / hash: `{{ BUILD_HASH }}`
- Quraşdırma mənbəyi: {{ BUILD_SOURCE }} (məsələn, operator tərəfindən mənbədən qurulmuş, satıcı tərəfindən təmin edilmiş binar).
- Aktivləşdirmə tarixi: {{ ACTIVATION_DATE }}
- Planlaşdırılmış texniki xidmət pəncərələri: {{ MAINTENANCE_CADENCE }}
- SM imzalanmasında iştirak edən qovşaq rolları:
  | Node | Rol | SM xüsusiyyətləri aktiv | Açar anbar yeri |
  |------|------|---------------------|--------------------|
  | {{ NODE_ID }} | {{ ROLU }} | {{ XÜSUSİYYƏTLƏRİ }} | {{ VAULT }} |

# 3. Kriptoqrafik Nəzarətlər

- İcazə verilən alqoritmlər: {{ ALQORITMLAR }} (SM dəstinin konfiqurasiyaya uyğun olduğundan əmin olun).
- Əsas həyat dövrü xülasəsi:
  | Mərhələ | Təsvir |
  |-------|-------------|
  | Nəsil | {{ KEY_GENERATION }} |
  | Saxlama | {{ KEY_STORAGE }} |
  | Fırlanma | {{ KEY_ROTATION }} |
  | Ləğv etmə | {{ KEY_REVOCATION }} |
- Fərqli şəxsiyyət (`distid`) siyasəti: {{ DISTID_POLICY }}
- Konfiqurasiyadan çıxarış (`crypto` bölməsi): hashlərlə Norito/JSON snapshot təqdim edin.

# 4. Telemetriya və Audit Yolları

- Monitorinqin son nöqtələri: {{ METRICS_ENDPOINTS }} (`/metrics`, idarə panelləri).
- Daxil edilmiş ölçülər: `crypto.sm.verification_total`, `crypto.sm.sign_total`,
  gecikmə histoqramları, səhv sayğacları.
- Jurnal saxlama siyasəti: {{ LOG_RETENTION }} (≥ üç il tövsiyə olunur).
- Audit jurnalının saxlanma yeri: {{ AUDIT_STORAGE }}

# 5. Hadisəyə Cavab və Əlaqələr

| Rol | Adı | Telefon | E-poçt | SLA |
|------|------|-------|-------|-----|
| Təhlükəsizlik əməliyyatları aparıcı | {{ ADI }} | {{ TELEFON }} | {{ EMAIL }} | {{ SLA }} |
| Zəng üzrə kripto | {{ ADI }} | {{ TELEFON }} | {{ EMAIL }} | {{ SLA }} |
| Hüquqi / uyğunluq | {{ ADI }} | {{ TELEFON }} | {{ EMAIL }} | {{ SLA }} |
| Satıcı dəstəyi (əgər varsa) | {{ ADI }} | {{ TELEFON }} | {{ EMAIL }} | {{ SLA }} |

# 6. Qoşmaların Yoxlama Siyahısı- [ ] Haşlarla konfiqurasiya snapshot (Norito + JSON).
- [ ] Deterministik quruluşun sübutu (heshlər, SBOM, təkrar istehsal qeydləri).
- [ ] Telemetriya tablosunun ixracı və xəbərdarlıq tərifləri.
- [ ] Hadisəyə cavab planı və çağırış üzrə rotasiya sənədi.
- [ ] Operator təliminin təsdiqi və ya runbook qəbzi.
- [ ] Çatdırılmış artefaktları əks etdirən ixrac-nəzarət bəyanatı.
- [ ] Müvafiq müqavilə müqavilələrinin və ya siyasətdən imtinaların surətləri.

№ 7. Operatorun Bəyannaməsi

> Yuxarıda sadalanan yerləşdirmənin ÇXR reklamına uyğun olduğunu təsdiq edirik
> kriptoqrafiya qaydalarına uyğun olaraq, SM-in effektiv olduğu xidmətlər sənədləşdirilmiş qaydalara əməl edir
> insidentlərə reaksiya və telemetriya siyasətləri və audit artefaktları olacaq
> ən azı üç il saxlanılır.

- Səlahiyyətli imzalayan: ___________________________________
- Tarix: ______________________
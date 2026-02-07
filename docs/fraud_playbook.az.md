---
lang: az
direction: ltr
source: docs/fraud_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ac4c98cc4aa6ab0c34e58e6428d0ee33eb9a0c3fdad9e6958bdc75f2a48dc66
source_last_modified: "2026-01-22T16:26:46.488648+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Fırıldaqların İdarə Edilməsi üzrə Kitab

Bu sənəd PSP fırıldaqçıları yığını üçün tələb olunan iskeleləri ümumiləşdirir
tam mikroservislər və SDK-lar aktiv inkişaf mərhələsindədir. Bu tutur
analitika, auditor iş axını və ehtiyat prosedurları üçün gözləntilər
qarşıdan gələn tətbiqlər kitab kitabçasına təhlükəsiz şəkildə daxil ola bilər.

## Xidmətlərə Baxış

1. **API Gateway** – sinxron `RiskQuery` faydalı yükləri qəbul edir, onları yönləndirir
   xüsusiyyətlərin aqreqasiyası və `FraudAssessment` cavablarını kitab dəftərinə qaytarır
   axır. Yüksək əlçatanlıq (aktiv-aktiv) tələb olunur; ilə regional cütlərdən istifadə edin
   sorğu əyriliyinin qarşısını almaq üçün deterministik heshing.
2. **Feature Aggregation** – xal toplamaq üçün xüsusiyyət vektorlarını tərtib edir. Emit
   Yalnız `FeatureInput` hashları; həssas yüklər zəncirdən kənarda qalır. Müşahidə qabiliyyəti
   gecikmə histoqramlarını, növbə dərinliyi ölçmələrini və təkrar sayğacları dərc etməlidir.
   kirayəçi.
3. **Risk Mühərriki** – qaydaları/modelləri qiymətləndirir və deterministik istehsal edir
   `FraudAssessment` çıxışları. Qaydaların icrası qaydasının sabit və ələ keçirildiyinə əmin olun
   qiymətləndirmə ID-si üzrə audit jurnalları.

## Analitika və Model Təşviqi

- **Anomaliya aşkarlanması**: kənarlaşmaları qeyd edən axın işini qoruyun
  icarəçiyə görə qərar dərəcələri. Xəbərdarlıqları idarəetmə panelinə və mağazaya göndərin
  rüblük rəylər üçün xülasələr.
- **Qrafik Təhlil**: əlaqə ixracı üzrə gecə qrafik keçidlərini həyata keçirin
  sövdələşmə qruplarını müəyyənləşdirin. Tapıntıları vasitəsilə idarəetmə portalına ixrac edin
  `GovernanceExport` dəstəkləyici dəlillərə istinadlarla.
- **Əlaqə qəbulu**: əl ilə nəzərdən keçirmə nəticələrini və geri qaytarma hesabatlarını idarə edin.
  Onları xüsusiyyət deltalarına çevirin və təlim verilənlər bazasına daxil edin.
  Risk qrupunun dayanmış lentləri görə bilməsi üçün qəbul statusu göstəricilərini dərc edin.
- **Model Təşviq Kəməri**: namizədlərin qiymətləndirilməsini avtomatlaşdırın (oflayn ölçülər,
  kanareyka hesabı, geri çəkilmə hazırlığı). Promosyonlar bir imzalı yaymalıdır
  `FraudAssessment` nümunə təyin edin və `model_version` sahəsini yeniləyin
  `GovernanceExport`.

## Auditor İş Akışı

1. Ən son `GovernanceExport` şəklini çəkin və `policy_digest` uyğunluqlarını yoxlayın
   risk qrupu tərəfindən təqdim edilən manifest.
2. Qayda aqreqatlarının mühasibat kitabçası tərəfindəki qərar yekunları ilə uyğunlaşdığını təsdiq edin
   nümunə götürülmüş pəncərə.
3. Görkəmli üçün anomaliya aşkarlanması və qrafik təhlili hesabatlarını nəzərdən keçirin
   məsələlər. Sənəd eskalasiyaları və gözlənilən remediasiya sahibləri.
4. Yoxlama siyahısını imzalayın və arxivləşdirin. Norito kodlu artefaktları burada saxlayın
   təkrar istehsal üçün idarəetmə portalı.

## Fallback Playbooks

- **Mühərrikin dayanması**: risk mühərriki 60 saniyədən çox müddət ərzində əlçatmaz olarsa,
  şlüz `AssessmentDecision::Review` verərək yalnız nəzərdən keçirmə rejiminə keçməlidir
  bütün sorğular və xəbərdarlıq edən operatorlar üçün.
- **Telemetriya boşluğu**: ölçülər və ya izlər geridə qaldıqda (5 dəqiqə itkin),
  avtomatik model promosyonlarını dayandırın və çağırış üzrə mühəndisə məlumat verin.
- **Reqressiya Modeli**: yerləşdirmədən sonrakı rəy yüksək saxtakarlığı göstərirsə
  itkilər, əvvəlki imzalanmış model paketinə qayıdın və yol xəritəsini yeniləyin
  düzəldici tədbirlərlə.

## Məlumat Paylaşımı Müqavilələri

- Saxlama, şifrələmə və əhatə edən yurisdiksiyaya aid əlavələri qoruyun
  pozuntu bildirişi SLAs. Tərəfdaşlar qəbul etməzdən əvvəl əlavəni imzalamalıdırlar
  `FraudAssessment` ixrac edir.
- Hər bir inteqrasiya üçün məlumatların minimuma endirilməsi təcrübələrini sənədləşdirin (məsələn, hashing
  hesab identifikatorları, kəsilmiş kart nömrələri).
- Hər il və ya tənzimləyici tələblər dəyişdikdə müqavilələri yeniləyin.

## API Sxemləri

Şlüz indi konkret JSON zərflərini nümayiş etdirir ki, bu da bir-birini ilə əlaqələndirir
`crates/iroha_data_model::fraud`-də tətbiq olunan Norito növləri:

- **Risk qəbulu** – `POST /v1/fraud/query` `RiskQuery` sxemini qəbul edir:
  - `query_id` (`[u8; 32]`, hex kodlu)
  - `subject` (`AccountId`, kanonik IH58 hərfi; isteğe bağlı `@<domain>` işarə və ya ləqəb)
  - `operation` (`RiskOperation`-ə uyğun etiketlənmiş nömrə; JSON `type`
    diskriminator enum variantını əks etdirir)
  - `related_asset` (`AssetId`, isteğe bağlı)
  - `features` (`{ key: String, value_hash: hex32 }` massivi xəritələndi
    `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext`; daşıyan `tenant_id`, isteğe bağlı `session_id`,
    isteğe bağlı `reason`)
- **Risk qərarı** – `POST /v1/fraud/assessment` istehlak edir
  `FraudAssessment` faydalı yük (həmçinin idarəetmə ixracında əks olunur):
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`,
    `decision` (`AssessmentDecision` nömrə), `rule_outcomes`
    (`{ rule_id, score_delta_bps, rationale? }` massivi)
  - `generated_at_ms`
  - `signature` (optional base64 Norito kodlu qiymətləndirməni əhatə edir)
- **İdarəetmə ixracı** – `GET /v1/fraud/governance/export` qaytarır
  `GovernanceExport` strukturu `governance` funksiyası aktiv olduqda, paketləşmə
  aktiv parametrlər, ən son qanunvericilik aktı, model versiyası, siyasət həzmi və
  `DecisionAggregate` histoqramı.

`crates/iroha_data_model/src/fraud/types.rs`-də gediş-gəliş testləri bunları təmin edir
sxemlər Norito kodek ilə binar uyğun olaraq qalır və
`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` məşqləri
tam suqəbuledici/qərar boru kəməri başdan-ayağa.

## PSP SDK İstinadları

Aşağıdakı dil stubları PSP ilə üzbəüz inteqrasiya nümunələrini izləyir:

- **Rust** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  `RiskQuery` metadatasını hazırlamaq və doğrulamaq üçün iş sahəsi `iroha` müştərisindən istifadə edir
  qəbul uğursuzluqları/uğurları.
- **TypeScript** – `docs/source/governance_api.md` REST səthini sənədləşdirir
  PSP demo panelində istifadə edilən yüngül çəkili Torii şlüz tərəfindən istehlak edilir; the
  scripted müştəri tüstü üçün `scripts/ci/schedule_fraud_scoring.sh` yaşayır
  matkaplar.
- **Swift & Kotlin** – mövcud SDK-lar (`IrohaSwift` və
  `crates/iroha_cli/docs/multisig.md` istinadları) Torii metadatasını ifşa edir
  `fraud_assessment_*` sahələrini əlavə etmək üçün lazım olan qarmaqlar. PSP-ə xüsusi köməkçilərdir
  ildə “Fraud & Telemetry Governance Loop” çərçivəsində izlənilib
  `status.md` və həmin SDK-ların əməliyyat qurucularından yenidən istifadə edin.

Bu istinadlar PSP üçün mikroservis şlüzü ilə sinxronlaşdırılacaq
icraçıların hər zaman hər biri üçün müasir sxem və nümunə kod yolu var
dəstəklənən dil.
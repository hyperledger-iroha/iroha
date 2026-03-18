---
lang: az
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9201c0027f05b1ab2c83fa6b3e1a1e6dad3ff9660a8ed23bac7667408d421ada
source_last_modified: "2026-01-22T14:35:37.551676+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# İdarəetmə Kitabı

Bu oyun kitabı Sora Şəbəkəsini saxlayan gündəlik ritualları əks etdirir
idarə şurası uyğunlaşdırıldı. O, nüfuzlu istinadları birləşdirir
repository belə fərdi mərasimlər qısa qala bilər, operatorlar isə həmişə
daha geniş proses üçün vahid giriş nöqtəsi var.

## Şura Mərasimləri

- **Fikstur idarəçiliyi** – Baxın [Sora Parlament Quraşdırma Təsdiqi](sorafs/signing_ceremony.md)
  Parlamentin İnfrastruktur Paneli artıq zəncir üzərində təsdiq axını üçün
  SoraFS chunker yeniləmələrini nəzərdən keçirərkən aşağıdakılara əməl olunur.
- **Səslərin sayının dərci** – Baxın
  Addım-addım CLI üçün [Governance Vote Tally](governance_vote_tally.md)
  iş axını və hesabat şablonu.

## Əməliyyat Runbooks

- **API inteqrasiyaları** – [Governance API arayışı](governance_api.md)
  REST/gRPC səthləri identifikasiya daxil olmaqla şura xidmətləri tərəfindən ifşa olunur
  tələblər və səhifələmə qaydaları.
- **Telemetriya panelləri** – Aşağıdakı Grafana JSON tərifləri
  `docs/source/grafana_*` “İdarəetmə Məhdudiyyətləri” və “Cədvəlləndirici”ni müəyyən edir
  TEU” lövhələri. Düzgün qalmaq üçün hər buraxılışdan sonra JSON-u Grafana-ə ixrac edin
  kanonik layout ilə.

## Məlumatların Əlçatanlığına Nəzarət

### Saxlama sinifləri

DA manifestlərini təsdiqləyən parlament panelləri məcburi saxlanmaya istinad etməlidir
səsvermədən əvvəl siyasət. Aşağıdakı cədvəl vasitəsilə tətbiq edilən standartları əks etdirir
`torii.da_ingest.replication_policy` beləliklə rəyçilər uyğunsuzluqları aşkar edə bilsinlər
mənbə TOML üçün axtarış.【docs/source/da/replication_policy.md:1】

| İdarəetmə etiketi | Blob sinfi | İsti tutma | Soyuq tutma | Tələb olunan replikalar | Saxlama sinfi |
|----------------|------------|---------------|----------------|-------------------|---------------|
| `da.taikai.live` | `taikai_segment` | 24 saat | 14g | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6 saat | 7d | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12 saat | 180d | 3 | `cold` |
| `da.default` | _bütün digər siniflər_ | 6 saat | 30g | 3 | `warm` |

İnfrastruktur Paneli doldurulmuş şablonu buradan əlavə etməlidir
`docs/examples/da_manifest_review_template.md` hər səsvermə bülleteni üçün açıqdır
həzm, saxlama etiketi və Norito artefaktları idarəetmədə əlaqəli olaraq qalır
rekord.

### İmzalanmış manifest audit izi

Səsvermə bülleteni gündəmə gəlməzdən əvvəl məclis işçiləri manifest olduğunu sübut etməlidirlər
Baxılan baytlar Parlament zərfinə və SoraFS artefaktına uyğun gəlir. istifadə edin
bu sübutları toplamaq üçün mövcud alətlər:1. Torii (`iroha app da get-blob --storage-ticket <hex>`)-dan manifest paketini əldə edin
   və ya ekvivalent SDK köməkçisi) beləliklə hamı çatdığı eyni baytları hash edir
   şlüzlər.
2. İmzalanmış zərflə manifest stub təsdiqləyicisini işə salın:
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   Bu, BLAKE3 manifest həzmini yenidən hesablayır, təsdiqləyir
   `chunk_digest_sha3_256` və daxil edilmiş hər Ed25519 imzasını yoxlayır
   `manifest_signatures.json`. Bax `docs/source/sorafs/manifest_pipeline.md`
   əlavə CLI seçimləri üçün.
3. Dijesti, `chunk_digest_sha3_256`, profil dəstəyi və imzalayanlar siyahısını kopyalayın
   baxış şablonu. QEYD: əgər yoxlayıcı “profil uyğunsuzluğu” bildirirsə və ya a
   imza əskikdirsə, səsverməni dayandırın və düzəldilmiş zərfin verilməsini tələb edin.
4. Doğrulayıcı çıxışını (və ya CI artefaktını
   `ci/check_sorafs_fixtures.sh`) Norito `.to` faydalı yüklə yanaşı auditorlar
   daxili şlüzlərə daxil olmadan sübutları təkrarlaya bilər.

Nəticə audit paketi Parlamentə hər bir hash və imzanı yenidən yaratmağa imkan verməlidir
manifest isti yaddaşdan çıxarıldıqdan sonra belə yoxlayın.

### Yoxlama siyahısını nəzərdən keçirin

1. Parlament tərəfindən təsdiq edilmiş manifest zərfini çəkin (bax
   `docs/source/sorafs/signing_ceremony.md`) və BLAKE3 həzmini qeyd edin.
2. Manifestin `RetentionPolicy` blokunun cədvəldəki etiketə uyğun olduğunu yoxlayın
   yuxarıda; Torii uyğunsuzluqları rədd edəcək, lakin şura
   auditorlar üçün sübut.【docs/source/da/replication_policy.md:32】
3. Təqdim edilmiş Norito faydalı yükün eyni saxlama etiketinə istinad etdiyini təsdiq edin
   və qəbul biletində görünən blob sinfi.
4. Siyasət yoxlamasının sübutunu əlavə edin (CLI çıxışı, `torii.da_ingest.replication_policy`
   dump və ya CI artefaktı) nəzərdən keçirmə paketinə köçürün ki, SRE qərarı təkrarlaya bilsin.
5. Təklif asılı olduqda planlaşdırılmış subsidiya kranlarını və ya icarəyə düzəlişləri qeyd edin
   `docs/source/sorafs_reserve_rent_plan.md`.

### Eskalasiya matrisi

| Sorğu növü | Sahib panel | Əlavə etmək üçün sübut | Son tarixlər və telemetriya | İstinadlar |
|-------------|--------------|--------------------|-----------------------|------------|
| Subsidiya / icarəyə düzəliş | İnfrastruktur + Xəzinədarlıq | Doldurulmuş DA paketi, `reserve_rentd`-dən kirayə delta, yenilənmiş ehtiyat proyeksiyası CSV, şuranın səs protokolları | Xəzinədarlıq yeniləməsini təqdim etməzdən əvvəl icarəyə təsirini qeyd edin; Maliyyə növbəti hesablaşma pəncərəsində barışa bilməsi üçün yuvarlanan 30d bufer telemetriyasını daxil edin | `docs/source/sorafs_reserve_rent_plan.md`, `docs/examples/da_manifest_review_template.md` |
| Moderasiya ləğvi / uyğunluq hərəkəti | Moderasiya + Uyğunluq | Uyğunluq bileti (`ComplianceUpdateV1`), sübut nişanları, imzalanmış manifest həzm, şikayət statusu | Gateway uyğunluq SLA-nı izləyin (24 saat ərzində təsdiqləyin, tam silinmə ≤72 saat). Hərəkəti göstərən `TransparencyReportV1` çıxarışı əlavə edin. | `docs/source/sorafs_gateway_compliance_plan.md`, `docs/source/sorafs_moderation_panel_plan.md` |
| Təcili dondurma / geri qaytarma | Parlamentin moderasiya paneli | Əvvəlki təsdiq paketi, yeni dondurma əmri, geri qaytarma manifesti, insident qeydi | Dondurma bildirişini dərhal dərc edin və növbəti idarəetmə aralığında geri qaytarma referendumunu planlaşdırın; fövqəladə halı əsaslandırmaq üçün bufer doyma + DA replikasiya telemetriyası daxildir. | `docs/source/sorafs/signing_ceremony.md`, `docs/source/sorafs_moderation_panel_plan.md` |Qəbul biletlərini təyin edərkən cədvəldən istifadə edin ki, hər panel dəqiq məlumat alsın
mandatını yerinə yetirmək üçün tələb olunan artefaktlar.

### Nəticələrin bildirilməsi

Hər DA-10 qərarı aşağıdakı artefaktlarla birlikdə göndərilməlidir (onları
Səsvermədə istinad edilən İdarəetmə DAG girişi):

- Tamamlanmış Markdown paketi
  `docs/examples/da_manifest_review_template.md` (indi imza və
  eskalasiya bölmələri).
- İmzalanmış Norito manifest (`.to`) və `manifest_signatures.json` zərfi
  və ya gətirmə həzmini sübut edən CI doğrulayıcı qeydləri.
- Fəaliyyətin səbəb olduğu hər hansı şəffaflıq yeniləmələri:
  - Silinmələr və ya uyğunluğa əsaslanan donmalar üçün `TransparencyReportV1` delta.
  - İcarə/ehtiyat kitabçası deltası və ya subsidiyalar üçün `ReserveSummaryV1` snapshot.
- Baxış zamanı toplanmış telemetriya görüntülərinə keçidlər (replikasiya dərinliyi,
  bufer boşluq, moderasiya gecikməsi) beləliklə müşahidəçilər şərtləri çarpaz yoxlaya bilsinlər
  faktdan sonra.

## Moderasiya və Eskalasiya

Gateway ləğvləri, subsidiyaların geri qaytarılması və ya DA dondurulmaları uyğunluğu izləyir
boru kəməri `docs/source/sorafs_gateway_compliance_plan.md` və
`docs/source/sorafs_moderation_panel_plan.md`-də müraciət aləti. Panellər olmalıdır:

1. Mənbə uyğunluq biletini daxil edin (`ComplianceUpdateV1` və ya
   `ModerationAppealV1`) və əlaqəli sübut nişanlarını əlavə edin.【docs/source/sorafs_gateway_compliance_plan.md:20】
2. Sorğunun moderasiya şikayəti yolunu (vətəndaş paneli
   səs) və ya fövqəladə Parlamentin dondurulması; hər iki axın manifestə istinad etməlidir
   yeni şablonda əldə edilən həzm və saxlama teqi.【docs/source/sorafs_moderation_panel_plan.md:1】
3. Artırmanın son tarixlərini sadalayın (apellyasiya qəbulu/açıqlama pəncərələri, fövqəladə hallar
   dondurma müddəti) və təqibin hansı şura və ya panelə məxsus olduğunu bildirin.
4. İstifadə olunan telemetriya şəklini çəkin (bufer boşluq, moderasiya gecikməsi)
   hərəkəti əsaslandırın ki, aşağı axın auditləri qərarı canlı ilə uyğunlaşdırsın
   dövlət.

Uyğunluq və moderasiya panelləri həftəlik şəffaflıq hesabatlarını sinxronlaşdırmalıdır
hesablaşma marşrutlaşdırıcısı operatorları ilə, beləliklə, ləğvlər və subsidiyalar eyni təsir göstərir
təzahürlər toplusu.

## Hesabat Şablonları

Bütün DA-10 rəyləri indi imzalanmış Markdown paketini tələb edir. Kopyalayın
`docs/examples/da_manifest_review_template.md`, manifest metadatasını doldurun,
saxlanma doğrulama cədvəli və panel səs xülasəsi, sonra tamamlandı
İdarəetmə DAG girişinə sənəd (əlavə olaraq istinad edilən Norito/JSON artefaktları).
Panellər paketi idarəetmə protokollarında əlaqələndirməlidir ki, gələcək ləğvlər və ya
subsidiyaların yenilənməsi, yenidən işə salınmadan orijinal manifest həzminə istinad edə bilər
bütün mərasim.

## Hadisə və Ləğv İş Akışı

Fövqəladə tədbirlər indi zəncirdə baş verir. Bir fikstür buraxılması lazım olduqda
geri çəkildi, bir idarə bileti təqdim edin və Parlamentə geri dönmə təklifi açın
əvvəllər təsdiq edilmiş manifest həzminə işarə edir. İnfrastruktur Paneli
səsverməni idarə edir və yekunlaşdıqdan sonra Nexus iş vaxtı geriyə dönməni dərc edir
aşağı müştərilərin istehlak etdiyi hadisə. Heç bir yerli JSON artefaktı tələb olunmur.

## Kitabın aktual olması- İdarəetmə ilə üzləşən yeni runbook proqramı daxil olduqda bu faylı yeniləyin
  anbar.
- Burada yeni mərasimləri birləşdirin ki, şura indeksi aşkar oluna bilsin.
- İstinad edilən sənəd hərəkət edərsə (məsələn, yeni SDK yolu), keçidi yeniləyin
  köhnə göstəricilərin qarşısını almaq üçün eyni çəkmə sorğusunun bir hissəsi kimi.
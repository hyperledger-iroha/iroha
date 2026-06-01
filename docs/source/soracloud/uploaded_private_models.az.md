<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/source/soracloud/uploaded_private_models.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97d6a421ce93a0e85be6cc99e828f965c9d8617d0ee27a772a2c9f2f646e77b7
source_last_modified: "2026-03-24T18:59:46.535846+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud İstifadəçisinin Yüklədiyi Modellər və Şəxsi İş vaxtı

Bu qeyd, So Ra-nın istifadəçi tərəfindən yüklənmiş model axınının necə düşəcəyini müəyyən edir
paralel iş vaxtı icad etmədən mövcud Soracloud model təyyarəsi.

## Dizayn Məqsədi

Müştərilərə imkan verən yalnız Soracloud yüklənmiş model sistemi əlavə edin:

- öz model depolarını yükləmək;
- sabitlənmiş model versiyasını agent mənzilinə və ya qapalı arena komandasına bağlamaq;
- Şifrələnmiş girişlər və şifrələnmiş model/status ilə şəxsi nəticə çıxarmaq; və
- ictimai öhdəlikləri, qəbzləri, qiymətləri və audit yollarını almaq.

Bu `ram_lfe` xüsusiyyəti deyil. `ram_lfe` ümumi gizli funksiya olaraq qalır
alt sistem `../universal_accounts_guide.md`-də sənədləşdirilmişdir. Yüklənmiş model
şəxsi nəticə Soracloud-un mövcud model reyestrini genişləndirməlidir,
artefakt, mənzil qabiliyyəti, FHE və şifrələmə siyasəti səthləri.

## Yenidən İstifadə üçün Mövcud Soracloud Səthləri

Cari Soracloud yığını artıq düzgün əsas obyektlərə malikdir:- `SoraModelRegistryV1`
  - hər xidmət üzrə səlahiyyətli model adı və irəli sürülən versiya vəziyyəti.
- `SoraModelWeightVersionRecordV1`
  - versiya xətti, təşviqi, geri qaytarılması, mənşəyi və təkrar istehsalı
    hash.
- `SoraModelArtifactRecordV1`
  - artıq model/çəki kəmərinə bağlanmış deterministik artefakt metadata.
- `SoraCapabilityPolicyV1.allow_model_inference`
  - məcburi hala gətirilməli olan mənzil/xidmət qabiliyyəti bayrağı
    mənzillər yüklənmiş modellərə bağlıdır.
- `SecretEnvelopeV1` və `CiphertextStateRecordV1`
  - deterministik şifrələnmiş bayt və şifrəli mətn vəziyyəti daşıyıcıları.
- `FheParamSetV1`, `FheExecutionPolicyV1`, `FheGovernanceBundleV1`,
  `DecryptionAuthorityPolicyV1` və `DecryptionRequestV1`
  - şifrələnmiş icra və idarə olunan çıxış üçün siyasət/idarəetmə səviyyəsi
    azad edin.
- cari Torii model marşrutları:
  - `/v1/soracloud/model/weight/{register,promote,rollback,status}`
  - `/v1/soracloud/model/artifact/{register,status}`
- cari Torii HF paylaşılan icarə marşrutları:
  - `/v1/soracloud/hf/{deploy,status,lease/leave,lease/renew}`

Yüklənmiş model yolu həmin səthləri genişləndirməlidir. Həddindən artıq yüklənməməlidir
HF lizinqləri paylaşır və o, `ram_lfe`-i modelə xidmət edən iş vaxtı kimi təkrar istifadə etməməlidir.

## Canonical Yükləmə Müqaviləsi

Soracloud-un şəxsi yüklənmiş model müqaviləsi yalnız kanonikləri qəbul etməlidir
Hugging Face tipli model anbarları:- tələb olunan əsas fayllar:
  - `config.json`
  - tokenizer faylları
  - ailə onları tələb etdikdə prosessor/preprosessor faylları
  - `*.safetensors`
- bu mərhələdə qəbul edilmiş ailə qrupları:
  - RoPE/RMSNorm/SwiGLU/GQA semantikası ilə yalnız dekoder səbəbli LM-lər
  - LLaVA üslubunda mətn+şəkil modelləri
  - Qwen2-VL üslublu mətn+şəkil modelləri
- bu əlamətdar mərhələdə rədd edildi:
  - Yüklənmiş şəxsi iş vaxtı müqaviləsi kimi GGUF
  - ONNX
  - çatışmayan tokenizer/prosessor aktivləri
  - dəstəklənməyən arxitekturalar/formalar
  - audio/video multimodal paketləri

Niyə bu müqavilə:

- o, mövcud olanlar tərəfindən istifadə edilən onsuz da dominant olan server tərəfi model tərtibatına uyğun gəlir
  seyftensorlar və Hugging Face repoları ətrafındakı ekosistem;
- bu, model təyyarəsinə bir deterministik normallaşma yolunu paylaşmağa imkan verir
  Beləliklə, Ra, Torii və iş vaxtının tərtibi; və
- GGUF kimi yerli icra zamanı idxal formatlarını
  Soracloud özəl iş vaxtı müqaviləsi.

HF paylaşılan icarələr paylaşılan/ictimai mənbə idxal iş axınları üçün faydalı olaraq qalır, lakin
Şəxsi yüklənmiş model yolu zəncir üzərində şifrələnmiş tərtib edilmiş baytları saxlayır
paylaşılan mənbə hovuzundan model baytlarını icarəyə verməkdən daha çox.

## Laylı Model-Təyyarə Dizaynı

### 1. Mənbə və reyestr təbəqəsi

Mövcud model reyestrinin dizaynını dəyişdirmək əvəzinə genişləndirin:- `SoraModelProvenanceKindV1::UserUpload` əlavə edin
  - cari növləri (`TrainingJob`, `HfImport`) ayırd etmək üçün kifayət deyil.
    model birbaşa So Ra kimi müştəri tərəfindən yüklənir və normallaşdırılır.
- `SoraModelRegistryV1`-i irəli sürülən versiya indeksi kimi saxlayın.
- `SoraModelWeightVersionRecordV1`-ni nəsil/təşviq/geriyə geri qaytarma rekordu kimi saxlayın.
- `SoraModelArtifactRecordV1`-i isteğe bağlı yüklənmiş şəxsi iş vaxtı ilə genişləndirin
  istinadlar:
  - `private_bundle_root`
  - `chunk_manifest_root`
  - `compile_profile_hash`
  - `privacy_mode`

Artefakt qeydi mənşəyi bağlayan deterministik lövbər olaraq qalır,
təkrar istehsal metadatası və paket şəxsiyyəti birlikdə. Çəki rekordu
irəli sürülən versiya soy obyekti olaraq qalır.

### 2. Bundle/chunk storage layer

Şifrələnmiş yüklənmiş model material üçün birinci dərəcəli Soracloud qeydlərini əlavə edin:

- `SoraUploadedModelBundleV1`
  - `model_id`
  - `weight_version`
  - `family`
  - `modalities`
  - `runtime_format`
  - `bundle_root`
  - `chunk_count`
  - `plaintext_bytes`
  - `ciphertext_bytes`
  - `compile_profile_hash`
  - `pricing_policy`
  - `decryption_policy_ref`
- `SoraUploadedModelChunkV1`
  - `model_id`
  - `bundle_root`
  - `ordinal`
  - `offset_bytes`
  - `plaintext_len`
  - `ciphertext_len`
  - `ciphertext_hash`
  - şifrələnmiş faydalı yük (`SecretEnvelopeV1`)

Deterministik qaydalar:- açıq mətn baytları şifrələmədən əvvəl sabit 4 MiB hissələrə bölünür;
- yığın sifarişi ciddi və sıra ilə idarə olunur;
- chunk/root digests replay boyunca sabitdir; və
- hər bir şifrələnmiş parça cari `SecretEnvelopeV1` altında qalmalıdır
  `33,554,432` baytlıq şifrəli mətn tavanı.

Bu mərhələ hərfi şifrələnmiş baytları yığın vasitəsilə zəncirli vəziyyətdə saxlayır
qeydlər. Şəxsi yüklənmiş model baytlarını SoraFS-ə yükləmir.

Zəncir ictimai olduğundan, yükləmə məxfiliyi realdan gəlməlidir
Soracloud-da saxlanılan alıcı açarı, açıqdan əldə edilən deterministik açarlardan deyil
metadata. İş masası reklam edilmiş yükləmə-şifrələmə alıcısını gətirməlidir,
təsadüfi hər yükləmə paketi açarı altında parçaları şifrələyin və yalnız
alıcı metadata və şifrəli mətnin yanında bükülmüş açar zərf.

### 3. Kompilyasiya/işləmə vaxtı təbəqəsi

Soracloud altında xüsusi fərdi transformator tərtibçisi/iş vaxtı təbəqəsi əlavə edin:- üçün BFV tərəfindən dəstəklənən deterministik aşağı dəqiqlikli tərtib edilmiş nəticə üzrə standartlaşdırmaq
  indi, çünki CKKS sxem müzakirəsində mövcuddur, lakin həyata keçirilmir
  yerli icra müddəti;
- qəbul edilmiş modelləri əhatə edən deterministik Soracloud özəl IR-də tərtib edin
  yerləşdirmələr, xətti/proyektor təbəqələri, diqqət matmulları, RoPE, RMSNorm /
  LayerNorm təxminləri, MLP blokları, görmə patch proyeksiyası və
  təsvirdən dekoderə proyektor yolları;
- deterministik sabit nöqtəli nəticədən istifadə edin:
  - int8 çəkilər
  - int16 aktivləşdirmələri
  - int32 yığılması
  - qeyri-xəttilər üçün təsdiq edilmiş çoxhədli yaxınlaşmalar

Bu kompilyator/iş vaxtı `ram_lfe`-dən ayrıdır. O, BFV primitivlərini təkrar istifadə edə bilər
və Soracloud FHE idarəetmə obyektləridir, lakin bu, eyni icra mühərriki deyil
və ya marşrut ailəsi.

### 4. Nəticə/sessiya qatı

Şəxsi qaçışlar üçün sessiya və yoxlama nöqtəsi qeydlərini əlavə edin:- `SoraPrivateCompileProfileV1`
  - `family`
  - `quantization`
  - `opset_version`
  - `max_context`
  - `max_images`
  - `vision_patch_policy`
  - `fhe_param_set`
  - `execution_policy`
- `SoraPrivateInferenceSessionV1`
  - `session_id`
  - `apartment`
  - `model_id`
  - `weight_version`
  - `bundle_root`
  - `input_commitments`
  - `token_budget`
  - `image_budget`
  - `status`
  - `receipt_root`
  - `xor_cost_nanos`
- `SoraPrivateInferenceCheckpointV1`
  - `session_id`
  - `step`
  - `ciphertext_state_root`
  - `receipt_hash`
  - `decrypt_request_id`
  - `released_token`
  - `compute_units`
  - `updated_at_ms`

Şəxsi icra deməkdir:

- şifrlənmiş sorğu/şəkil daxiletmələri;
- şifrələnmiş model çəkiləri və aktivləşdirmələri;
- çıxış üçün açıq deşifrə-siyasət buraxılışı;
- ictimai icra vaxtı daxilolmaları və xərclərin uçotu.

Bu, öhdəliklər və ya yoxlanılabilirlik olmadan gizli icra demək deyil.

## Müştəri Məsuliyyətləri

Beləliklə, Ra və ya başqa bir müştəri əvvəl deterministik yerli ön emal həyata keçirməlidir
yükləmə Soracloud-a çatır:

- tokenizer tətbiqi;
- qəbul edilmiş mətn+şəkil ailələri üçün yamaq tenzorlarında təsvirin əvvəlcədən işlənməsi;
- deterministik paketin normallaşdırılması;
- token identifikatorlarının və təsvir patch tensorlarının müştəri tərəfi şifrələməsi.

Torii şifrlənmiş daxiletmələr üstəgəl ictimai öhdəliklər almalıdır, xammal deyil
şəxsi yol üçün mətn və ya xam şəkillər.

## API və ISI PlanıMövcud model reyestr marşrutlarını kanonik qeyd təbəqəsi kimi saxlayın və əlavə edin
yuxarıda yeni yükləmə/iş vaxtı marşrutları:

- `POST /v1/soracloud/model/upload/init`
- `POST /v1/soracloud/model/upload/chunk`
- `POST /v1/soracloud/model/upload/finalize`
- `GET /v1/soracloud/model/upload/encryption-recipient`
- `POST /v1/soracloud/model/compile`
- `POST /v1/soracloud/model/allow`
- `POST /v1/soracloud/model/run-private`
- `GET /v1/soracloud/model/run-status`
- `POST /v1/soracloud/model/decrypt-output`

Onları uyğun Soracloud ISI ilə dəstəkləyin:

- paket qeydiyyatı
- parça əlavə edin/sonlandırın
- qəbulu tərtib etmək
- şəxsi qaçış başlanğıcı
- nəzarət məntəqəsinin qeydi
- çıxış buraxılışı

Axın belə olmalıdır:

1. upload/init deterministik paket sessiyasını və gözlənilən kökü təyin edir;
2. upload/chunk şifrlənmiş parçaları sıra ilə əlavə edir;
3. paketin kökünü və manifestini yükləmək/sonlandırmaq;
4. kompilyasiya ilə bağlı deterministik şəxsi kompilyasiya profili yaradır
   qəbul edilmiş paket;
5. model/artifakt + model/çəki reyestr qeydləri yüklənmiş paketə istinad edir
   tək bir təlim işi deyil;
6. icazə-model yüklənmiş modeli artıq qəbul edən mənzilə bağlayır
   `allow_model_inference`;
7. run-private sessiyanı qeyd edir və yoxlama nöqtələri/qəbzləri verir;
8. deşifrə-çıxış idarə olunan çıxış materialını buraxır.

## Qiymətləndirmə və Nəzarət Təyyarə Siyasəti

Cari Soracloud doldurma/idarəetmə təyyarəsi davranışını genişləndirin:- Yüklənmiş modellərlə işləyən mənzillər üçün `allow_model_inference` tələb olunur;
- XOR-da qiymətlərin saxlanması, tərtibi, icra müddəti addımları və şifrənin açılması;
- bu mühüm mərhələdə yüklənmiş model qaçışları üçün hekayənin yayılmasını qeyri-aktiv edin;
- yüklənmiş modelləri So Ra-nın qapalı arenasında və ixraca qapalı axınlarda saxlayın.

## Avtorizasiya və Bağlama Semantikası

Yükləmək, tərtib etmək və işə salmaq ayrı imkanlardır və ayrı-ayrılıqda qalmalıdır
model təyyarə.

- model paketinin yüklənməsi mənzili idarə etmək üçün dolayı icazə verməməlidir;
- kompilyasiya müvəffəqiyyəti dolayısı ilə model versiyasını cari vəziyyətə gətirməməlidir;
- mənzillərin bağlanması `allow-model` stil mutasiyası vasitəsilə aydın olmalıdır
  qeyd edir:
  - mənzil,
  - model id,
  - çəki versiyası,
  - dəstə kök,
  - məxfilik rejimi,
  - imzalayan / audit ardıcıllığı;
- yüklənmiş modellərə bağlı mənzillər artıq qəbul edilməlidir
  `allow_model_inference`;
- mutasiya marşrutları eyni Soracloud imzalı sorğu tələb etməyə davam etməlidir
  Mövcud model/artifakt/təlim marşrutları tərəfindən istifadə edilən intizam və olmalıdır
  `CanManageSoracloud` və ya eyni dərəcədə açıq-aşkar həvalə edilmiş səlahiyyətli orqan tərəfindən qorunur
  model.

Bu, "Mən onu yüklədim, ona görə də hər bir şəxsi mənzil onu idarə edə bilər" qarşısını alır.
sürüşür və mənzilin icra siyasətini açıq saxlayır.

## Status və Audit Modeli

Yeni qeydlər yalnız mutasiya deyil, nüfuzlu oxuma və yoxlama səthlərinə ehtiyac duyur
marşrutlar.

Tövsiyə olunan əlavələr:- yükləmə statusu
  - `service_name + model_name + weight_version` və ya tərəfindən sorğu
    `model_id + bundle_root`;
- statusu tərtib etmək
  - `model_id + bundle_root + compile_profile_hash` tərəfindən sorğu;
- şəxsi qaçış statusu
  - `session_id` tərəfindən sorğu, mənzil/model/versiya kontekstinə daxil edilmişdir
    cavab;
- deşifrə-çıxış statusu
  - `decrypt_request_id` tərəfindən sorğu.

Audit yox, mövcud Soracloud qlobal ardıcıllığında qalmalıdır
hər bir xüsusiyyət üçün ikinci sayğac yaratmaq. Birinci dərəcəli audit tədbirləri əlavə edin:

- init / yekunlaşdırın
- parça əlavə edin / möhürləyin
- tərtib qəbul edildi / tərtib rədd edildi
- mənzil modeli icazə / ləğv
- şəxsi işə başlama / yoxlama nöqtəsi / tamamlama / uğursuzluq
- çıxışın buraxılması / rədd edilməsi

Bu, yüklənmiş model fəaliyyətini eyni nüfuzlu replayda görünən saxlayır və
cari xidmət, təlim, model çəkisi, model-artifakt kimi əməliyyat hekayəsi,
HF paylaşılan icarə və mənzil audit axınları.

## Qəbul Kvotaları və Dövlət Artım Limitləri

Zəncirdə hərfi şifrələnmiş model baytları yalnız qəbul məhdud olduqda etibarlıdır
aqressiv şəkildə.

Tətbiq ən azı aşağıdakılar üçün deterministik limitləri müəyyən etməlidir:

- yüklənmiş paketə görə maksimum açıq mətn baytları;
- paket başına maksimum şifrələnmiş bayt;
- paket başına maksimum yığın sayı;
- avtoritet/xidmət üzrə maksimum eyni vaxtda uçuş zamanı yükləmə seansları;
- hər bir xidmət/mənzil pəncərəsi üzrə maksimum tərtib işləri;
- şəxsi seans üçün maksimum saxlanılan yoxlama məntəqələrinin sayı;
- sessiya başına maksimum çıxış-buraxılış sorğuları.Torii və əsas əvvəl elan edilmiş limitləri aşan yükləmələri rədd etməlidir
dövlətin gücləndirilməsi baş verir. Limitlər harada konfiqurasiyaya əsaslanmalıdır
uyğundur, lakin qiymətləndirmə nəticələri həmyaşıdları arasında deterministik olaraq qalmalıdır.

## Təkrar və Kompilyator Determinizmi

Şəxsi tərtibçi/iş vaxtı yolu normaldan daha yüksək determinizm yükünə malikdir
xidmətin yerləşdirilməsi.

Tələb olunan invariantlar:

- ailənin aşkarlanması və normallaşdırılması sabit kanonik paket yaratmalıdır
  hər hansı kompilyasiya hash buraxılmazdan əvvəl;
- kompilyasiya profilinin heshləri bağlanmalıdır:
  - normallaşdırılmış paket kökü,
  - ailə,
  - kvantlama resepti,
  - opset versiyası,
  - FHE parametr dəsti,
  - icra siyasəti;
- iş vaxtı qeyri-deterministic ləpələrdən, üzən nöqtə sürüşməsindən və
  çıxışları və ya daxilolmaları dəyişə bilən aparat-spesifik azalmalar
  həmyaşıdları.

Qəbul edilmiş ailə dəstini ölçməzdən əvvəl, kiçik bir deterministik qurğu qurun
hər bir ailə sinfi və kilidi tərtib nəticələri və qızıl ilə iş vaxtı qəbzləri
testlər.

## Koddan əvvəl qalan dizayn boşluqları

Ən böyük həll edilməmiş icra sualları indi konkretə daralır
arxa plan qərarları:- dəqiq yükləmə/yığma/tələb DTO formaları və Norito sxemləri;
- paket/yığma/sessiya/yoxlama nöqtəsi axtarışları üçün dünya dövlət indeksləmə açarları;
- `iroha_config`-də kvota/defolt konfiqurasiya yerləşdirilməsi;
- model/artifakt statusu versiya yönümlü olmalıdırmı
  `UserUpload` mövcud olduqda təlim-iş yönümlü;
- mənzil itirildikdə dəqiq ləğv davranışı
  `allow_model_inference` və ya bərkidilmiş model versiyası geri çəkildi.

Bunlar növbəti dizayndan kodlaşdıran körpü elementləridir. Memarlıq yerləşdirilməsi
xüsusiyyət indi sabit olmalıdır.

## Test Matrisi- yükləmənin doğrulanması:
  - kanonik HF qoruyucu repoları qəbul edin
  - GGUF, ONNX, itkin tokenizer/prosessor aktivlərini rədd edin, dəstəklənmir
    memarlıqlar və audio/video multimodal paketləri
- parçalanma:
  - deterministik paket kökləri
  - sabit yığın sifarişi
  - dəqiq yenidənqurma
  - zərf tavanının tətbiqi
- reyestr ardıcıllığı:
  - təkrar oynatma altında paket/parça/artifakt/çəkinin təşviqi düzgünlüyü
- kompilyator:
  - hər biri yalnız dekoder, LLaVA və Qwen2-VL üslubu üçün bir kiçik qurğu
  - dəstəklənməyən əməliyyatlar və formalar üçün imtina
- şəxsi iş vaxtı:
  - sabit qəbzlərlə şifrələnmiş kiçik qurğulu uç-uca tüstü testi və
    eşik çıxış buraxılışı
- qiymət:
  - Yükləmə, tərtib etmə, icra müddəti addımları və şifrənin açılması üçün XOR ödənişləri
- Beləliklə, Ra inteqrasiyası:
  - yükləmək, tərtib etmək, dərc etmək, komandaya bağlamaq, qapalı arenada işləmək, qəbzləri yoxlamaq,
    layihəni yadda saxla, yenidən açın, determinist şəkildə yenidən işləyin
- təhlükəsizlik:
  - ixrac qapısı keçidi yoxdur
  - povestin avtomatik yayılması yoxdur
  - `allow_model_inference` olmadan mənzil bağlaması uğursuz olur

## İcra Dilimləri1. Çatışmayan məlumat modeli sahələrini və yeni qeyd növlərini əlavə edin.
2. Yeni Torii sorğu/cavab növləri və marşrut idarəçiləri əlavə edin.
3. Uyğun Soracloud ISI və dünya dövlət yaddaşını əlavə edin.
4. Deterministik paket/yığın doğrulama və zəncir üzərində şifrələnmiş bayt əlavə edin
   saxlama.
5. Kiçik BFV dəstəkli şəxsi transformator qurğusu/iş vaxtı yolu əlavə edin.
6. CLI model əmrlərini yükləmə/tərtib etmə/özəl-işləmə axınlarını əhatə etmək üçün genişləndirin.
7. Backend yolu nüfuzlu olduqdan sonra Land So Ra inteqrasiyası.
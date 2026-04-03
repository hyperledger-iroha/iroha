<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/source/soracloud/cli_local_control_plane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 567b63e9b61afaecfa5d85aa60f0348c856557e171559885ffaba45168ce61dc
source_last_modified: "2026-03-26T06:12:11.480025+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud CLI və İdarəetmə Təyyarəsi

Soracloud v1 səlahiyyətli, yalnız IVM işləmə vaxtıdır.

- `iroha app soracloud init` yeganə oflayn əmrdir. Bu iskeleler
  `container_manifest.json`, `service_manifest.json` və isteğe bağlı şablon
  Soracloud xidmətləri üçün artefaktlar.
- Bütün digər Soracloud CLI əmrləri yalnız şəbəkə tərəfindən dəstəklənir və tələb olunur
  `--torii-url`.
- CLI heç bir yerli Soracloud idarəetmə təyyarə güzgüsü və ya vəziyyətini saxlamır
  fayl.
- Torii ictimai Soracloud statusu və birbaşa mutasiya marşrutlarına xidmət edir
  nüfuzlu dünya dövləti və əlavə edilmiş Soracloud icra meneceri.

## İş vaxtı əhatəsi- Soracloud v1 yalnız `SoraContainerRuntimeV1::Ivm` qəbul edir.
- `NativeProcess` rədd edilir.
- Sifarişli poçt qutusunun icrası birbaşa IVM işləyicilərinə qəbul edildi.
- Nəmləndirmə və materiallaşma daha çox SoraFS/DA məzmunundan irəli gəlir
  sintetik yerli görüntülərdən daha çox.
- `SoraContainerManifestV1` indi `required_config_names` və
  `required_secret_names`, üstəgəl açıq `config_exports`. Yerləşdirmə, təkmilləşdirmə,
  və geri qaytarma uğursuzluğu effektiv nüfuzlu material dəsti bağlandıqda bağlanır
  elan edilmiş bağlamaları təmin etməmək və ya konfiqurasiya ixracı a
  tələb olunmayan konfiqurasiya və ya dublikat env/fayl təyinatı.
- Təhlükəli xidmət konfiqurasiya girişləri indi altında həyata keçirilir
  `services/<service>/<version>/configs/<config_name>` kanonik JSON kimi
  faydalı yük faylları.
- Açıq konfiqurasiya env ixracı proqnozlaşdırılır
  `services/<service>/<version>/effective_env.json` və fayl ixracı bunlardır
  altında reallaşdı
  `services/<service>/<version>/config_exports/<relative_path>`. İxrac edildi
  dəyərlər istinad edilən konfiqurasiya girişinin kanonik JSON faydalı yük mətnindən istifadə edir.
- Soracloud IVM işləyiciləri indi həmin nüfuzlu konfiqurasiya yüklərini oxuya bilər
  birbaşa runtime host vasitəsilə `ReadConfig` səthi, belə adi
  `query`/`update` işləyiciləri qovşaq-lokal fayl yollarını təxmin etməyə ehtiyac duymurlar.
  təyin edilmiş xidmət konfiqurasiyasını istehlak edin.
- Təhlükəli xidməti məxfi zərflər indi altında reallaşdırılır
  `services/<service>/<version>/secret_envelopes/<secret_name>` kimi
  səlahiyyətli zərf faylları.- Adi Soracloud IVM işləyiciləri indi həmin sirri oxuya bilər
  zərfləri birbaşa icra zamanı host `ReadSecretEnvelope` səthi vasitəsilə.
- Köhnə şəxsi iş vaxtı ehtiyat ağacı indi törədilmişdən sinxronlaşdırılıb
  `secrets/<service>/<version>/<secret_name>` altında yerləşdirmə vəziyyəti belə
  köhnə xam gizli oxu yolu və nüfuzlu idarəetmə təyyarəsi nöqtəsi
  eyni baytlar.
- Şəxsi iş vaxtı `ReadSecret` indi səlahiyyətli yerləşdirməni həll edir
  `service_secrets` ilk və yalnız köhnə node-lokal qovşağına qayıdır
  `secrets/<service>/<version>/...` heç bir əməl edilmədikdə materiallaşdırılmış fayl ağacı
  tələb olunan açar üçün xidmət sirri girişi mövcuddur.
- Gizli qəbul hələ də konfiqurasiya qəbulundan qəsdən daha dardır:
  `ReadSecretEnvelope` ictimai təhlükəsiz adi operator müqaviləsidir.
  `ReadSecret` yalnız şəxsi iş vaxtı olaraq qalır və hələ də öhdəliyi qaytarır
  açıq mətn bağlama müqaviləsi əvəzinə zərf şifrəli mətn baytları.
- Runtime xidmət planları indi müvafiq qəbuletmə qabiliyyətini ifşa edir
  boolean üstəgəl elan edilmiş `config_exports` və effektiv proqnozlaşdırılır
  mühit, beləliklə, vəziyyətin istehlakçıları reallaşdırılan bir revizyon olub olmadığını söyləyə bilər
  host konfiqurasiyasının oxunmasını, hostun gizli zərfinin oxunmasını, özəl xam sirri dəstəkləyir
  oxuyur və işləyicidən nəticə çıxarmadan açıq konfiqurasiya yeridilir
  tək dərslər.

## CLI Əmrləri- `iroha app soracloud init`
  - yalnız oflayn iskele.
  - `baseline`, `site`, `webapp` və `pii-app` şablonlarını dəstəkləyir.
- `iroha app soracloud deploy`
  - `SoraDeploymentBundleV1` qəbul qaydalarını yerli olaraq təsdiqləyir, imzalayır
    tələb edir və `POST /v1/soracloud/deploy`-ə zəng edir.
  - `--initial-configs <path>` və `--initial-secrets <path>` indi qoşula bilər
    nüfuzlu daxili xidmət konfiqurasiyası / gizli xəritələri ilə atomik olaraq
    ilk qəbulda tələb olunan bağlamalar təmin oluna bilsin.
  - CLI indi HTTP sorğusunu kanonik olaraq hər ikisi ilə imzalayır
    `X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms` və
    Adi tək imzalı hesablar üçün `X-Iroha-Nonce` və ya
    `X-Iroha-Account` plus `X-Iroha-Witness` zaman
    `soracloud.http_witness_file` multisig şahidi JSON yükünü göstərir;
    Torii deterministik qaralama əməliyyat təlimat dəstini və
    CLI daha sonra adi Iroha müştərisi vasitəsilə real əməliyyatı təqdim edir
    zolaq.
  - Torii həmçinin SCR-host qəbulu qapaqlarını və uğursuz bağlanma qabiliyyətini tətbiq edir
    mutasiya qəbul edilməzdən əvvəl yoxlayır.
- `iroha app soracloud upgrade`
  - yeni paket reviziyasını təsdiq edir və imzalayır, sonra zəng edir
    `POST /v1/soracloud/upgrade`.
  - eyni `--initial-configs <path>` / `--initial-secrets <path>` axını
    təkmilləşdirmə zamanı atom materialı yeniləmələri üçün mövcuddur.
  - Eyni SCR-host qəbul yoxlamaları təkmilləşdirmədən əvvəl server tərəfində aparılır
    qəbul etdi.
- `iroha app soracloud status`- `GET /v1/soracloud/status`-dən səlahiyyətli xidmət statusunu sorğulayır.
- `iroha app soracloud config-*`
  - `config-set`, `config-delete` və `config-status` yalnız Torii tərəfindən dəstəklənir.
  - CLI kanonik xidmət-konfiqurasiya mənşəli yükləri və zəngləri imzalayır
    `POST /v1/soracloud/service/config/set`,
    `POST /v1/soracloud/service/config/delete`, və
    `GET /v1/soracloud/service/config/status`.
  - konfiqurasiya girişləri səlahiyyətli yerləşdirmə vəziyyətində saxlanılır və qalır
    yerləşdirmə/təkmilləşdirmə/geri qaytarma revizyonu dəyişikliklərinə əlavə olunur.
  - `config-delete` aktiv reviziya hələ də elan edildikdə bağlanmır
    `container.required_config_names`-də adlandırılmış konfiqurasiya.
- `iroha app soracloud secret-*`
  - `secret-set`, `secret-delete` və `secret-status` yalnız Torii tərəfindən dəstəklənir.
  - CLI kanonik xidmət-gizli mənşəli yükləri və zəngləri imzalayır
    `POST /v1/soracloud/service/secret/set`,
    `POST /v1/soracloud/service/secret/delete`, və
    `GET /v1/soracloud/service/secret/status`.
  - məxfi qeydlər səlahiyyətli `SecretEnvelopeV1` qeydləri kimi saxlanılır
    yerləşdirmə vəziyyətində və normal xidmət revizyon dəyişikliklərindən sağ çıxın.
  - `secret-delete` aktiv reviziya hələ də elan edildikdə bağlanmır
    `container.required_secret_names`-də adı çəkilən sirr.
- `iroha app soracloud rollback`
  - geri qaytarma metadatasını imzalayır və `POST /v1/soracloud/rollback`-ə zəng edir.
- `iroha app soracloud rollout`
  - yayma metadatasını imzalayır və `POST /v1/soracloud/rollout`-ə zəng edir.
- `iroha app soracloud agent-*`
  - bütün mənzil həyat dövrü, pul kisəsi, poçt qutusu və muxtariyyət əmrləri var
    Yalnız Torii dəstəklənir.
- `iroha app soracloud training-*`
  - bütün təlim işi əmrləri yalnız Torii tərəfindən dəstəklənir.- `iroha app soracloud model-*`
  - bütün model artefakt, çəki, yüklənmiş model və şəxsi iş vaxtı əmrləri
    yalnız Torii tərəfindən dəstəklənir.
  - yüklənmiş model/şəxsi iş vaxtı səthi indi eyni ailədə yaşayır:
    `model-upload-encryption-recipient`, `model-upload-init`,
    `model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
    `model-compile`, `model-compile-status`, `model-allow`,
    `model-run-private`, `model-run-status`, `model-decrypt-output` və
    `model-publish-private`.
  - `model-run-private` indi qaralama-sonra yekunlaşdıran icra zamanı əl sıxışmasını gizlədir
    CLI və səlahiyyətli post-final sessiya statusunu qaytarır.
  - `model-publish-private` indi hazırlanmış həm də dəstəkləyir
    paket/parça/yekunlaşdır/tərtib et/yayımlamağa icazə verin və daha yüksək səviyyəli plan
    sənəd layihəsi. Qaralama indi `source: PrivateModelSourceV1`,
    ya `LocalDir { path }` və ya qəbul edir
    `HuggingFaceSnapshot { repo, revision }`.
  - `--draft-file` ilə çağırıldıqda, CLI elan edilmiş mənbəni normallaşdırır
    deterministik temp ağacına çevrilir, v1 HF safetensors müqaviləsini təsdiqləyir,
    paketi aktivə qarşı deterministik şəkildə seriallaşdırır və şifrələyir
    Torii yükləmə alıcısı, onu sabit ölçülü şifrələnmiş parçalara ayırır,
    isteğe bağlı olaraq hazırlanmış planı `--emit-plan-file` vasitəsilə yazır və sonra
    yükləmə/sonlandırma/tərtib etmə/izin vermə ardıcıllığını yerinə yetirir.
  - `HuggingFaceSnapshot` reviziyaları məcburidir və öhdəliyi bağlanmalıdır
    SHAs; filiala bənzər referanslar uğursuz bağlanaraq rədd edilir.- qəbul edilmiş mənbə planı v1-də qəsdən dardır: `config.json`,
    tokenizer aktivləri, bir və ya daha çox `*.safetensors` qırıqları və isteğe bağlı
    təsvir qabiliyyətinə malik modellər üçün prosessor/preprosessor metadata. GGUF, ONNX,
    digər qeyri-safetensors çəkilər və ixtiyari iç içə xüsusi layouts var
    rədd edildi.
  - `--plan-file` ilə zəng edildikdə, CLI hələ də artıq istehlak edir
    dərc-plan sənədi hazırlanmış və planın yüklənməsi zamanı uğursuz bağlanır
    alıcı artıq səlahiyyətli Torii alıcısına uyğun gəlmir.
  - bu marşrutları qatlayan dizayn üçün `uploaded_private_models.md`-ə baxın
    mövcud model reyestrinə və artefakt/çəki qeydlərinə.
- `model-host` idarəetmə-təyyarə marşrutları
  - Torii indi səlahiyyətliləri ifşa edir
    `POST /v1/soracloud/model-host/advertise`,
    `POST /v1/soracloud/model-host/heartbeat`,
    `POST /v1/soracloud/model-host/withdraw`, və
    `GET /v1/soracloud/model-host/status`.
  - bu marşrutlar təsdiqləyici host imkanlarının reklamlarına qoşulmağa davam edir
    nüfuzlu dünya dövləti və operatorların hansı validatorların olduğunu yoxlamasına icazə verin
    hazırda reklam modeli-host tutumu.
  - `iroha app soracloud model-host-advertise`,
    `model-host-heartbeat`, `model-host-withdraw` və
    `model-host-status` indi eyni kanonik mənşə yüklərini imzalayır.
    xam API və uyğun gələn Torii marşrutlarına birbaşa zəng edin.
- `iroha app soracloud hf-*`
  - `hf-deploy`, `hf-status`, `hf-lease-leave` və `hf-lease-renew`
    Yalnız Torii dəstəklənir.- `hf-deploy` və `hf-lease-renew` indi də deterministi avtomatik qəbul edir
    tələb olunan `service_name` üçün HF nəticə çıxarma xidməti yaratdı və
    üçün deterministik yaradılan HF mənzilini avtomatik qəbul edin
    `apartment_name`, paylaşılan icarə mutasiyasından əvvəl tələb olunduqda
    təqdim olunur.
  - təkrar istifadə uğursuz bağlanıb: adı çəkilən xidmət/mənzil artıq mövcuddursa, lakin varsa
    həmin kanonik mənbə, HF üçün gözlənilən HF yerləşdirilməsi deyil
    İcarəyə bağlı olmayanlara səssizcə bağlanmaq əvəzinə mutasiya rədd edilir
    Soracloud obyektləri.
  - daxili icra vaxtı meneceri əlavə edildikdə, indi də `hf-status`
    bağlanmış daxil olmaqla, kanonik mənbə üçün iş vaxtı proyeksiyasını qaytarır
    xidmətlər/mənzillər, növbəli növbəti pəncərənin görünməsi və yerli paket/
    artefakt önbelleği buraxılmış; `importer_pending` həmin iş vaxtı proyeksiyasını izləyir
    yalnız mötəbər mənbə enumuna güvənmək əvəzinə.
  - `hf-deploy` və ya `hf-lease-renew` yaradılan HF xidmətini qəbul etdikdə
    paylaşılan icarə mutasiyası ilə eyni əməliyyat, səlahiyyətli HF
    mənbə dərhal `Ready`-ə çevrilir və `importer_pending` qalır
    Cavabda `false`.
  - HF icarə statusu və mutasiya cavabları indi də hər hansı nüfuzu ifşa edir
    yerləşdirmə snapshot artıq aktiv icarə pəncərəsinə əlavə olunub, o cümlədəntəyin edilmiş hostlar, uyğun ev sahibi sayı, isti ev sahibi sayı və ayrı
    saxlama və hesablama haqqı sahələri.
  - `hf-deploy` və `hf-lease-renew` indi kanonik HF resursunu əldə edirlər
    təqdim etməzdən əvvəl həll edilmiş Hugging Face repo metadatasından profil
    mutasiya:
    - Torii repo yoxlayır `siblings`, `.gguf`-ə üstünlük verir
      PyTorch çəki planları üzərində `.safetensors`, seçilmiş faylları HEAD edir
      `required_model_bytes` əldə edin və bunu ilk buraxılışla əlaqələndirin
      backend/format plus RAM/disk mərtəbələri;
    - heç bir canlı validator host reklamı edə bilməyəndə icarəyə qəbul bağlanmır
      həmin profili təmin etmək; və
    - host dəsti mövcud olduqda, aktiv pəncərə indi a qeyd edir
      deterministik pay ölçülmüş yerləşdirmə və ayrıca hesablama rezervasiyası
      mövcud saxlama icarəsi uçotu ilə yanaşı ödəniş.
  - aktiv HF pəncərəsinə qoşulan sonradan üzvlər indi proporsional saxlama və ödəniş ödəyirlər
    əvvəlki üzvlər isə yalnız qalan pəncərə üçün payları hesablayın
    eyni deterministik saxlama ödənişini və hesablama-geri ödəmə mühasibatını əldə edin
    gec qoşulmaqdan.
  - daxili iş vaxtı meneceri yaradılan HF stub paketini sintez edə bilər
    yerli olaraq, beləliklə, yaradılan xidmətlər bir gözləmədən reallaşa bilər
    SoraFS faydalı yükü yalnız yer tutan nəticə paketi üçün qəbul etdi.- daxil edilmiş icra vaxtı meneceri indi icazəli siyahıya alınmış Hugging Face repo-nu idxal edir
    faylları `soracloud_runtime.state_dir/hf_sources/<source_id>/files/` və
    idxal edilmiş həll edilmiş öhdəlik ilə yerli `import_manifest.json`-i davam etdirir
    fayllar, atlanmış fayllar və hər hansı idxalçı xətası.
  - yaradılan HF `metadata` yerli oxunuşlar indi yerli idxal manifestini qaytarır,
    idxal fayl inventar plus yerli icrası və olub
    qovşaq üçün körpünün geri qaytarılması aktivləşdirilib.
  - yaradılan HF `infer` yerli oxunuşlar indi qovşaqda icraya üstünlük verir.
    idxal edilmiş paylaşılan baytlar:
    - `irohad` yerli proqram altında quraşdırılmış Python adapter skriptini reallaşdırır
      Soracloud iş vaxtı dövlət kataloqu və onu işə salır
      `soracloud_runtime.hf.local_runner_program`;
    - quraşdırılmış qaçışçı əvvəlcə deterministik fiksasiya bəndini yoxlayır
      `config.json` (testlər tərəfindən istifadə olunur), sonra idxal edilmiş mənbəni başqa cür yükləyir
      `transformers.pipeline(..., local_files_only=True)` vasitəsilə kataloq belə
      model çəkmə əvəzinə paylaşılan yerli idxala qarşı icra edir
      təzə Hub baytları; və
    - əgər `soracloud_runtime.hf.allow_inference_bridge_fallback = true` və
      `soracloud_runtime.hf.inference_token` konfiqurasiya edilib, iş vaxtı düşür
      yalnız yerli icra zamanı konfiqurasiya edilmiş HF nəticələrinin əsas URL-inə qayıdın
      əlçatmazdır və ya uğursuzdur və zəng edən şəxs açıq şəkildə daxil olur
      `x-soracloud-hf-allow-bridge-fallback: 1`, `true` və ya `yes`.
  - iş vaxtı proyeksiyası indi HF mənbəyini `PendingImport`-də saxlayacaquğurlu yerli idxal manifestləri mövcuddur və idxalçı uğursuzluqları kimi üzə çıxır
    iş vaxtı `Failed` plus `last_error` əvəzinə səssizcə `Ready` hesabat.
  - yaradılan HF mənzillər artıq istehlak təsdiqlənmiş muxtariyyətdən keçir
    node-lokal iş vaxtı yolu:
    - `agent-autonomy-run` indi iki addımlı bir axını izləyir: birinci imzalanmış
      mutasiya səlahiyyətli təsdiqi qeyd edir və deterministi qaytarır
      taslak əməliyyatın ardından ikinci bir imzalı sonlandırma tələbi soruşur
      bağlanmış iş vaxtı meneceri həddə qarşı təsdiqlənmiş qaçışı yerinə yetirmək üçün
      yaradılan HF `/infer` xidməti və istənilən səlahiyyətli təqibi qaytarır
      başqa deterministik layihə kimi təlimatlar;
    - təsdiq edilmiş qaçış rekordu indi də kanonik olaraq qalır
      `request_commitment`, beləliklə, sonradan yaradılan xidmət qəbzi ola bilər
      dəqiq səlahiyyətli muxtariyyət təsdiqinə bağlıdır;
    - təsdiqlər indi isteğe bağlı kanonik `workflow_input_json`-ni saxlaya bilər
      bədən; mövcud olduqda, quraşdırılmış iş vaxtı dəqiq JSON yükünü irəliləyir
      yaradılan HF `/infer` idarəedicisinə və olmadıqda geri qayıdır.
      nüfuzlu köhnə `run_label`-as-`inputs` zərf
      `artifact_hash` / `provenance_hash` / `budget_units` / `run_id` daşınır
      strukturlaşdırılmış parametrlər kimi;
    - `workflow_input_json` indi deterministik ardıcıllığa da qoşula bilərilə çox addımlı icra
      `{ "workflow_version": 1, "steps": [...] }`, burada hər addım çalışır a
      yaradılan HF `/infer` sorğusu və sonrakı addımlar əvvəlki çıxışlara istinad edə bilər
      `${run.*}`, `${previous.text|json|result_commitment}` vasitəsilə və
      `${steps.<step_id>.text|json|result_commitment}` yer tutucular; və
    - həm mutasiya cavabı, həm də `agent-autonomy-status` indi səthə çıxır
      uğur/uğursuzluq da daxil olmaqla mövcud olduqda node-lokal icra xülasəsi,
      bağlı xidmətin təftişi, deterministik nəticə öhdəlikləri, yoxlama məntəqəsi
      / jurnal artefakt hashları, yaradılan xidmət `AuditReceipt` və
      təhlil edilmiş JSON cavab orqanı.
    - yaradılan xidmət qəbzi mövcud olduqda, Torii onu qeyd edir
      nüfuzlu `soracloud_runtime_receipts` və ortaya çıxanı ifşa edir
      ilə yanaşı, son run statusu üzrə səlahiyyətli iş vaxtı qəbzi
      node-lokal icra xülasəsi.
    - yaradılan-HF muxtariyyət yolu indi də xüsusi səlahiyyətli qeyd edir
      mənzil `AutonomyRunExecuted` audit hadisəsi və son işlənmiş status
      səlahiyyətli icra müddəti qəbzi ilə birlikdə həmin icra auditini qaytarır.
  - `hf-lease-renew` indi iki rejimə malikdir:
    - cari pəncərənin müddəti bitibsə və ya boşalıbsa, dərhal təzə pəncərəni açır
      pəncərə;
    - cari pəncərə hələ də aktivdirsə, zəng edəni növbəyə alır
      növbəti pəncərə sponsoru, növbəti pəncərənin tam yaddaşını və hesabını doldururrezervasiya ödənişləri öndən, deterministik növbəti pəncərədə davam edir
      yerləşdirmə planı və `hf-status` vasitəsilə növbəyə qoyulmuş sponsorluğu ifşa edir
      sonrakı mutasiya hovuzu irəliyə yuvarlayana qədər.
  - yaradılan HF ictimai `/infer` girişi indi səlahiyyətli həll edir
    yerləşdirmə və qəbuledici qovşaq isti əsas olmadıqda, proksilər
    təyin edilmiş əsas hosta Soracloud P2P nəzarət mesajları üzərində sorğu;
    daxil edilmiş iş vaxtı hələ də birbaşa replika/təyin edilməmiş yerli üzrə bağlana bilmir
    icra və yaradılan HF iş vaxtı qəbzləri `placement_id` daşıyır,
    təsdiqləyici və səlahiyyətli yerləşdirmə qeydindən həmyaşıd atribut.
  - proksi-əsas yolun vaxtı bitdikdə, cavabdan əvvəl bağlandıqda və ya
    səlahiyyətlidən qeyri-müştəri iş vaxtı uğursuzluğu ilə geri qayıdır
    ilkin, giriş node indi bunun üçün `AssignedHeartbeatMiss` hesabat verir
    əsas və eyni vasitəsilə `ReconcileSoracloudModelHosts` növbələri
    daxili mutasiya zolağı.
  - etibarlı müddəti bitmiş ev sahibi ilə uzlaşma qeydləri indi davam etdi
    model-host pozuntusu sübutu, ictimai zolaqlı validator slash yolunu təkrar istifadə edir,
    və defolt HF paylaşılan icarə cəza siyasətini tətbiq edir:
    `warmup_no_show_slash_bps=500`,
    `assigned_heartbeat_miss_slash_bps=250`,
    `assigned_heartbeat_miss_strike_threshold=3`, və
    `advert_contradiction_slash_bps=1000`.
  - yerli olaraq təyin edilmiş HF iş vaxtı sağlamlığı indi də eyni sübut yolunu qidalandırır:yerli `Warming` host emitində vaxt idxal/istiləşmə uğursuzluqlarını uzlaşdırın
    `WarmupNoShow` və yerli isti əsasda sakin-işçi uğursuzluqları
    normal vasitəsilə azaldılmış `AssignedHeartbeatMiss` hesabatlarını yayır
    əməliyyat növbəsi.
  - indi də işə başlamazdan əvvəl barışın və yerli HF işçilərini yoxlayın
    replikalar da daxil olmaqla isti/istiləşmə hostları təyin edilir, beləliklə replika uğursuz ola bilər
    hər hansı bir əvvəl səlahiyyətli `AssignedHeartbeatMiss` yoluna bağlandı
    ictimai `/infer` sorğusu heç vaxt birinci yerə düşür.
  - həmin yerli tədqiqat uğur qazandıqda, iş vaxtı indi də birini buraxır
    yerli validator üçün səlahiyyətli `model-host-heartbeat` mutasiyası olduqda
    təyin edilmiş host hələ də `Warming`-dir və ya aktiv host reklamı TTL tələb edir
    təzələmək, belə ki, uğurlu yerli hazırlıq eyni nüfuzlu təbliğ edir
    yerləşdirmə/reklam əl ilə ürək döyüntülərinin yenilənəcəyini bildirir.
  - iş vaxtı yerli `WarmupNoShow` və ya yaydıqda
    `AssignedHeartbeatMiss`, indi də növbə çəkir
    `ReconcileSoracloudModelHosts` eyni daxili mutasiya zolağı vasitəsilə belə
    səlahiyyətli əvəzetmə/doldurma gözləmək əvəzinə dərhal başlayır
    daha sonrakı dövri ev sahibinin istifadə müddəti başa çatan tarama.
  - ictimai yaradılan-HF giriş törədilmiş çünki daha əvvəl uğursuz zaman
    yerləşdirmənin proxy etmək üçün isti əsası yoxdur, Torii indi iş vaxtını soruşur
    eyni səlahiyyətli quldurluq etməkGözləmək əvəzinə dərhal `ReconcileSoracloudModelHosts` təlimatı
    daha gec başa çatan və ya işçinin uğursuzluğu siqnalı üçün.
  - ictimai yaradılan HF girişi vəkil edilmiş müvəffəqiyyət cavabı aldıqda,
    Torii indi daxil edilmiş iş vaxtı qəbzini yoxlayır hələ də icrasını sübut edir
    aktiv yerləşdirmə üçün qəbul edilmiş isti birincilik; çatışmayan və ya uyğunsuz
    yerləşdirmə atributu indi bağlana bilmir və eyni səlahiyyətliyə işarə edir
    `ReconcileSoracloudModelHosts` yolunu qaytarmaq əvəzinə a
    qeyri-səlahiyyətli cavab. Torii də indi vəkil edilmiş müvəffəqiyyəti rədd edir
    icra vaxtı qəbz öhdəlikləri və ya sertifikatlaşdırma siyasəti yerinə yetirdikdə cavablar
    geri qayıtmaq üzrədir cavab uyğun deyil, və eyni pis-qəbz
    yol həmçinin uzaqdan ilkin `AssignedHeartbeatMiss` hesabatını qidalandırır
    qarmaq.
  - proksi yaradılan-HF icra uğursuzluqları indi eyni tələb edir
    məlumat verdikdən sonra səlahiyyətli `ReconcileSoracloudModelHosts` yolu
    Uzaqdan birincil sağlamlıq xətası, daha gec bitməsini gözləmək yerinə
    süpürmək.
  - Torii indi hər gözlənilən yaradılan-HF proxy sorğusunu
    hədəflənmiş nüfuzlu əsas həmyaşıd. Yanlış bir proxy cavabı
    peer indi gözləyən tələbi zəhərləmək əvəzinə göz ardı edilir, buna görə də yalnız
    səlahiyyətli birincil sorğunu tamamlaya və ya uğursuz edə bilər. Proksi
    dəstəklənməyən proxy cavab sxemi ilə gözlənilən həmyaşıddan cavabversiya yalnız ona görə qəbul edilmək əvəzinə hələ də bağlana bilmir
    `request_id` gözlənilən sorğuya uyğun gəldi. Səhv həmyaşıd cavab verdisə
    özü hələ də həmin yerləşdirmə üçün təyin edilmiş yaradılan HF hostdur
    runtime indi mövcud vasitəsilə host haqqında hesabat verir
    `WarmupNoShow` / `AssignedHeartbeatMiss` sübut yolu onun əsasında
    səlahiyyətli tapşırıq statusu və həmçinin səlahiyyətli işarələr
    `ReconcileSoracloudModelHosts`, belə ki, köhnəlmiş əsas/replika səlahiyyətlərinin sürüşməsi
    yalnız giriş zamanı nəzərə alınmamaq əvəzinə nəzarət dövrəsini qidalandırır.
  - daxil olan Soracloud proksi icrası da indi nəzərdə tutulanlarla məhdudlaşdırılıb
    yaradılan-HF `infer` sorğu işi həyata keçirilən isti əsasda. Qeyri-HF
    ictimai yerli-oxumaq marşrutları və yaradılan-HF sorğuları bir node çatdırılır
    ki, artıq nüfuzlu isti əsas, indi əvəzinə bağlanıb uğursuz
    P2P proxy yolu üzərində icra. Nüfuzlu ilkin də indi
    icradan əvvəl kanonik yaradılan HF sorğu öhdəliyini yenidən hesablayır,
    saxta və ya uyğun olmayan proxy zərflər bağlana bilmir.
  - təyin edilmiş replika və ya köhnəlmiş ilkin həmin gələni rədd etdikdə
    yaradılan-HF proxy icrası, çünki o, artıq səlahiyyətli deyil
    isti əsas, qəbuledici tərəfdən işləmə vaxtı indi də işarə edir
    Yalnız zəng edən tərəfə güvənmək əvəzinə `ReconcileSoracloudModelHosts`
    marşrut görünüşü.- eyni daxil olan yaradılan-HF proxy səlahiyyəti nasazlığı baş verdikdə
    yerli səlahiyyətli ilkin özü, icra müddəti indi onu a kimi qəbul edir
    birinci dərəcəli ev sahibi-sağlamlıq siqnalı: isti ilkin özünü hesabat
    `AssignedHeartbeatMiss`, istiləşmə ilkin öz hesabatı `WarmupNoShow`,
    və hər iki yol dərhal eyni səlahiyyətlidən istifadə edir
    `ReconcileSoracloudModelHosts` idarəetmə döngəsi.
  - həmin qeyri-əsas qəbuledici hələ də səlahiyyətlilərdən biri olduqda
    təyin edilmiş hostlar və qəbul edilmiş zəncir vəziyyətindən isti birinciliyi həll edə bilər,
    o, indi yaradılan HF sorğusunu həmin birinciliyə doğru yenidən proksiləşdirir
    dərhal uğursuzluq. Təyin edilməmiş validatorlar fəaliyyət göstərmək əvəzinə bağlana bilmir
    ümumi vasitəçi HF proxy hops kimi və orijinal giriş node hələ də
    səlahiyyətli yerləşdirməyə qarşı qaytarılmış iş vaxtı qəbzini təsdiqləyir
    dövlət. Əgər həmin təyin edilmiş replika irəli atıldıqdan sonra birinciliyə keçid uğursuz olarsa
    sorğu faktiki olaraq göndərilir, qəbuledici tərəfin işləmə vaxtı hesabat verir
    uzaqdan ilkin sağlamlıq nasazlığı və səlahiyyətli göstərişlər
    `ReconcileSoracloudModelHosts`; yerli təyin edilmiş replika belə edə bilmirsə
    öz proksi nəqliyyatı/çalışma vaxtı olmadığı üçün bu irəliləməyə cəhd edin,
    uğursuzluq indi əvəzinə yerli təyin edilmiş host səhvi kimi qəbul edilir
    birinciliyi günahlandırır.
  - barışmaq indi də avtomatik olaraq zaman `AdvertContradiction` yayaryerli validatorun konfiqurasiya edilmiş icra vaxtı peer id ilə razılaşmır
    həmin validator üçün nüfuzlu `model-host-advertise` peer id.
  - etibarlı model-host yenidən reklam mutasiyalar indi də nüfuzlu sinxronizasiya
    təyin edilmiş host `peer_id` / `host_class` metadata və cərəyanı yenidən hesablayın
    ev sahibi sinfi dəyişdikdə yerləşdirmə rezervasiya haqları.
  - ziddiyyətli model-host yenidən reklam mutasiyalar artıq dərhal yaymaq
    `AdvertContradiction` sübut, mövcud validator slash/evct tətbiq edin
    yol və yalnız uğursuz doğrulama əvəzinə təsirlənmiş yerləşdirmələri yeniləyin.
  - qalan HF hosting işi indi:
    - yerlidən kənarda daha geniş cross-node/runtime-cluster sağlamlıq siqnalları
      validatorun birbaşa işçisi/istimə müşahidələri üstəgəl təyin edilmiş ev sahibi
      Uzaqdan həmyaşıdların daxili sağlamlığı lazım olduqda qəbuledici tərəfdəki səlahiyyət uğursuzluqları
      həmçinin nüfuzlu yenidən balans/slash yolunu qidalandırır.
  - yaradılan HF yerli icrası indi rezident hər mənbə Python işçisini saxlayır
    `irohad` altında canlı, təkrarlanan `/infer` üzərində yüklənmiş modeli təkrar istifadə edir
    çağırır və yerli idxal əgər işçini deterministik olaraq yenidən işə salır
    açıq-aşkar dəyişikliklər və ya prosesdən çıxmaq.
  - bu marşrutlar şəxsi yüklənmiş model yolu deyil. HF paylaşılan icarələr qalır
    zəncirdə şifrələnmiş deyil, paylaşılan mənbə/idxal üzvlüyünə diqqət yetirir
    özəl model baytları.- yaradılan HF muxtariyyət təsdiqləri indi deterministik ardıcıllığı dəstəkləyir
    çox addımlı sorğu zərfləri, lakin daha geniş qeyri-xətti/alətdən istifadə
    orkestr və artefakt-qrafik icra hələ də təqib işi olaraq qalır
    zəncirlənmiş `/infer` addımlarından kənarda.

## Vəziyyət semantikası

`/v1/soracloud/status` və əlaqədar agent/təlim/model statusunun son nöqtələri indi
səlahiyyətli iş vaxtı vəziyyətini əks etdirir:

- qəbul edilmiş dünya dövlətindən qəbul edilmiş xidmət dəyişiklikləri;
- daxili iş vaxtı menecerindən iş vaxtının nəmləndirilməsi/materializasiya vəziyyəti;
- real poçt qutusunun icra qəbzləri və uğursuzluq vəziyyəti;
- nəşr olunmuş jurnal/yoxlama nöqtəsi artefaktları;
- yer tutucu status şimləri əvəzinə keş və iş vaxtı sağlamlığı.

Səlahiyyətli iş vaxtı materialı köhnədirsə və ya əlçatan deyilsə, oxunuşlar bağlanır
Yerli dövlət güzgülərinə qayıtmaq əvəzinə.

`/v1/soracloud/status` v1-də yeganə sənədləşdirilmiş Soracloud statusunun son nöqtəsidir.
Ayrı bir `/v1/soracloud/registry` marşrutu yoxdur.

## Yerli İskele Silindi

Bu köhnə yerli simulyasiya konsepsiyaları artıq v1-də mövcud deyil:

- CLI-yerli reyestr/dövlət faylları və ya qeydiyyat yolu seçimləri
- Torii-yerli fayl dəstəkli idarəetmə təyyarəsi güzgüləri

## Nümunə

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## Qeydlər- Yerli yoxlama hələ də sorğular imzalanmadan və təqdim edilməzdən əvvəl davam edir.
- Standart Soracloud mutasiya son nöqtələri artıq xam `authority` qəbul etmir /
  `private_key` Yerləşdirmə, təkmilləşdirmə, geri qaytarma, yayma, agent üçün JSON sahələri
  həyat dövrü, təlim, model-ev sahibi və model çəkisi yolları; Torii autentifikasiya edir
  əvəzinə kanonik HTTP imza başlıqlarından bu sorğular.
- Multisig tərəfindən idarə olunan Soracloud sahibləri indi `X-Iroha-Witness` istifadə edir; nöqtə
  `soracloud.http_witness_file`, CLI-dən istədiyiniz dəqiq şahid JSON-da
  növbəti mutasiya sorğusu üçün təkrar oynatma və Torii bağlandıqda uğursuz olacaq.
  şahid mövzu hesabı və ya kanonik sorğu hash uyğun gəlmir.
- `hf-deploy` və `hf-lease-renew` indi müştəri tərəfindən imzalanmış köməkçi daxildir
  deterministik yaradılmış HF xidməti/mənzil artefaktları üçün mənşə,
  buna görə də Torii artıq həmin təqibi qəbul etmək üçün zəng edənin şəxsi açarlarına ehtiyac duymur
  obyektlər.
- `agent-autonomy-run` və `model/run-private` indi qaralamadan istifadə edir, sonra yekunlaşdırır
  axın: ilk imzalanmış mutasiya səlahiyyətli təsdiqi / başlanğıcı qeyd edir,
  və ikinci imzalanmış yekunlaşdırma sorğusu iş vaxtı yolunu yerinə yetirir və qaytarır
  deterministik layihə kimi hər hansı səlahiyyətli təqib təlimatları
  əməliyyatlar.
- `model/decrypt-output` indi səlahiyyətli şəxsi nəticəni qaytarır
  yoxlama nöqtəsi yalnız xarici tərəfindən imzalanan deterministik bir əməliyyat kimidaxili Torii-də saxlanılan şəxsi açarla deyil, əməliyyat.
- ZK əlavəsi CRUD indi imzalanmış Iroha hesabından icarə haqqını açar və hələ də
  aktivləşdirildikdə API tokenlərinə əlavə giriş qapısı kimi baxır.
- İctimai Soracloud yerli oxuma girişi indi hər IP üçün açıq-aşkar tarif və tətbiq edir
  paralellik məhdudlaşdırır və yerli və ya əvvəl ictimai marşrutun görünməsini yenidən yoxlayır
  vəkil edilmiş icra.
- Şəxsi işləmə qabiliyyətinin tətbiqi Soracloud host ABI daxilində baş verir,
  CLI və ya Torii-yerli iskele daxilində deyil.
- `ram_lfe` ayrıca gizli funksiyalı alt sistem olaraq qalır. İstifadəçi tərəfindən yüklənmiş şəxsi
  transformatorun icrası Soracloud FHE/şifrləmə idarəçiliyini təkrar istifadə etməlidir və
  model qeydləri, `ram_lfe` sorğu yolu deyil.
- İş vaxtı sağlamlığı, nəmləndirilməsi və icrası ondan qaynaqlanır
  `[soracloud_runtime]` konfiqurasiyası və qəbul edilmiş vəziyyət, ətraf mühit deyil
  dəyişir.
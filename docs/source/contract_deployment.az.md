---
lang: az
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

Status: Torii, CLI və əsas qəbul testləri (noyabr 2025) tərəfindən həyata keçirilir və həyata keçirilir.

## Baxış

- Tərtib edilmiş IVM bayt kodunu (`.to`) Torii-ə təqdim etməklə və ya verməklə yerləşdirin.
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` təlimatları
  birbaşa.
- Qovşaqlar yerli olaraq `code_hash` və kanonik ABI hashını yenidən hesablayır; uyğunsuzluqlar
  determinist şəkildə rədd edin.
- Saxlanılan artefaktlar `contract_manifests` zəncirinin altında yaşayır və
  `contract_code` qeydləri. Yalnız istinad heşlərini göstərir və kiçik qalır;
  kod baytları `code_hash` tərəfindən açar.
- Qorunan ad məkanları a-dan əvvəl qüvvəyə minmiş idarəetmə təklifini tələb edə bilər
  yerləşdirməyə icazə verilir. Qəbul yolu təklif yükünü axtarır və
  olduqda `(namespace, contract_id, code_hash, abi_hash)` bərabərliyini təmin edir
  ad sahəsi qorunur.

## Saxlanılan Artefaktlar və Saxlama

- `RegisterSmartContractCode` verilmiş manifesti daxil edir/üzerinə yazır
  `code_hash`. Eyni hash artıq mövcud olduqda, yenisi ilə əvəz olunur
  aşkar.
- `RegisterSmartContractBytes` tərtib edilmiş proqramı altında saxlayır
  `contract_code[code_hash]`. Əgər hash üçün baytlar artıq mövcuddursa, onlar uyğun olmalıdır
  dəqiq; fərqli baytlar invariant pozuntuya səbəb olur.
- Kodun ölçüsü `max_contract_code_bytes` fərdi parametri ilə məhdudlaşır
  (defolt 16 MiB). Daha əvvəl `SetParameter(Custom)` əməliyyatı ilə onu ləğv edin
  daha böyük artefaktların qeydiyyatı.
- Saxlama məhdudiyyətsizdir: manifestlər və kodlar açıq şəkildə olana qədər əlçatan qalır
  gələcək idarəetmə iş prosesində silinir. TTL və ya avtomatik GC yoxdur.

## Qəbul boru kəməri

- Təsdiqləyici IVM başlığını təhlil edir, `version_major == 1`-i tətbiq edir və yoxlayır
  `abi_version == 1`. Naməlum versiyalar dərhal rədd edilir; icra vaxtı yoxdur
  keçid.
- `code_hash` üçün manifest artıq mövcud olduqda, doğrulama
  saxlanılan `code_hash`/`abi_hash` təqdim ediləndən hesablanmış dəyərlərə bərabərdir
  proqram. Uyğunsuzluq `Manifest{Code,Abi}HashMismatch` xətaları yaradır.
- Qorunan ad məkanlarını hədəfləyən əməliyyatlara metadata açarları daxil edilməlidir
  `gov_namespace` və `gov_contract_id`. Qəbul yolu onları müqayisə edir
  qüvvəyə minmiş `DeployContract` təkliflərinə qarşı; uyğun təklif yoxdursa
  əməliyyat `NotPermitted` ilə rədd edilir.

## Torii son nöqtələri (Xüsusiyyət `app_api`)- `POST /v2/contracts/deploy`
  - Sorğunun əsas hissəsi: `DeployContractDto` (sahə təfərrüatları üçün bax `docs/source/torii_contracts_api.md`).
  - Torii base64 faydalı yükünü deşifrə edir, hər iki hashı hesablayır, manifest qurur,
    və `RegisterSmartContractCode` plus təqdim edir
    adından imzalanmış əməliyyatda `RegisterSmartContractBytes`
    zəng edən.
  - Cavab: `{ ok, code_hash_hex, abi_hash_hex }`.
  - Səhvlər: etibarsız base64, dəstəklənməyən ABI versiyası, itkin icazə
    (`CanRegisterSmartContractCode`), ölçü həddi aşıldı, idarəetmə qapısı.
- `POST /v2/contracts/code`
  - `RegisterContractCodeDto` (səlahiyyət, şəxsi açar, manifest) qəbul edir və yalnız təqdim edir
    `RegisterSmartContractCode`. Manifestlər ayrı səhnələşdirildikdə istifadə edin
    bayt kodu.
- `POST /v2/contracts/instance`
  - `DeployAndActivateInstanceDto` qəbul edir (səlahiyyət, şəxsi açar, ad sahəsi/contract_id, `code_b64`, isteğe bağlı manifest ləğvetmələri) və yerləşdirir + atomik olaraq aktivləşdirir.
- `POST /v2/contracts/instance/activate`
  - `ActivateInstanceDto` (səlahiyyət, şəxsi açar, ad sahəsi, contract_id, `code_hash`) qəbul edir və yalnız aktivləşdirmə təlimatını təqdim edir.
- `GET /v2/contracts/code/{code_hash}`
  - `{ manifest: { code_hash, abi_hash } }` qaytarır.
    Əlavə manifest sahələri daxili olaraq qorunur, lakin a üçün burada buraxılmır
    sabit API.
- `GET /v2/contracts/code-bytes/{code_hash}`
  - `{ code_b64 }`-i baza64 kimi kodlanmış saxlanan `.to` təsviri ilə qaytarır.

Bütün müqavilə həyat dövrünün son nöqtələri vasitəsilə konfiqurasiya edilmiş xüsusi yerləşdirmə məhdudlaşdırıcısını paylaşır
`torii.deploy_rate_per_origin_per_sec` (saniyədə nişanlar) və
`torii.deploy_burst_per_origin` (burst tokens). Varsayılanlar partlayışla 4 tələb/s-dir
`X-API-Token`, uzaq IP və ya son nöqtə işarəsindən əldə edilən hər bir işarə/açar üçün 8.
Etibarlı operatorlar üçün məhdudlaşdırıcını söndürmək üçün hər iki sahəni `null` olaraq təyin edin. Zaman
məhdudlaşdırıcı yanğınlar, Torii artır
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` telemetriya sayğacı və
HTTP 429-u qaytarır; hər hansı işləyici xətası artımları
Xəbərdarlıq üçün `torii_contract_errors_total{endpoint=…}`.

## İdarəetmə inteqrasiyası və qorunan ad məkanları- `gov_protected_namespaces` (JSON ad sahəsi massivi) fərdi parametrini təyin edin
  strings) qəbul qapısını aktivləşdirmək üçün. Torii altındakı köməkçiləri ifşa edir
  `/v2/gov/protected-namespaces` və CLI onları vasitəsilə əks etdirir
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- `ProposeDeployContract` (və ya Torii) ilə yaradılmış təkliflər
  `/v2/gov/proposals/deploy-contract` son nöqtəsi) ələ keçirin
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`.
- Referendum keçdikdən sonra `EnactReferendum` təklifin Qəbul edildiyini qeyd edir və
  qəbul uyğun metadata və kodu daşıyan yerləşdirmələri qəbul edəcək.
- Əməliyyatlara `gov_namespace=a namespace` və metadata cütü daxil edilməlidir
  `gov_contract_id=an identifier` (və `contract_namespace` / təyin edilməlidir.
  Zəng vaxtı bağlamaq üçün `contract_id`). CLI köməkçiləri bunları doldurur
  `--namespace`/`--contract-id` keçidindən avtomatik olaraq.
- Qorunan ad boşluqları işə salındıqda, növbə qəbulu cəhdləri rədd edir
  mövcud `contract_id`-i fərqli ad sahəsinə yenidən bağlayın; qüvvədə olandan istifadə edin
  başqa yerdə yerləşdirmədən əvvəl əvvəlki məcburiliyi təklif edin və ya ləğv edin.
- Zolaqlı manifest birdən yuxarı doğrulayıcı kvorum təyin edərsə, daxil edin
  `gov_manifest_approvers` (validator hesab identifikatorlarının JSON massivi) növbəni saya bilsin
  əməliyyat orqanı ilə yanaşı əlavə təsdiqlər. Lanes də rədd edir
  manifestdə olmayan ad boşluqlarına istinad edən metadata
  `protected_namespaces` dəsti.

## CLI köməkçiləri

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  Torii yerləşdirmə sorğusunu təqdim edir (heşləri tez hesablayır).
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  manifest qurur (təchiz edilmiş açarla imzalanır), baytları qeyd edir + manifest,
  və bir əməliyyatda `(namespace, contract_id)` bağlamasını aktivləşdirir. istifadə edin
  `--dry-run` hesablanmış hashları və təlimat sayını çap etmədən çap etmək üçün
  təqdim edir və imzalanmış manifest JSON-u saxlamaq üçün `--manifest-out`.
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` hesablayır
  Tərtib edilmiş `.to` üçün `code_hash`/`abi_hash` və isteğe bağlı olaraq manifest imzalayır,
  JSON çap etmək və ya `--out`-ə yazmaq.
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  oflayn VM keçidini işlədir və ABI/hesh metadata və növbəli ISI-lər haqqında hesabat verir
  (sayımlar və təlimat idləri) şəbəkəyə toxunmadan. əlavə edin
  Zəng vaxtı metadatasını əks etdirmək üçün `--namespace/--contract-id`.
- `iroha_cli app contracts manifest get --code-hash <hex>` Torii vasitəsilə manifest əldə edir
  və istəyə görə onu diskə yazır.
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` yükləmələr
  saxlanılan `.to` şəkli.
- `iroha_cli app contracts instances --namespace <ns> [--table]` siyahıları aktivləşdirildi
  müqavilə nümunələri (manifest + metadata idarə olunur).
- İdarəetmə köməkçiləri (`iroha_cli app gov deploy propose`, `iroha_cli app gov enact`,
  `iroha_cli app gov protected set/get`) qorunan ad sahəsinin iş axınını təşkil edir və
  audit üçün JSON artefaktlarını ifşa edin.

## Test və əhatə dairəsi

- `crates/iroha_core/tests/contract_code_bytes.rs` örtük kodu altında vahid testləri
  saxlama, güc qabiliyyəti və ölçü qapağı.
- `crates/iroha_core/tests/gov_enact_deploy.rs` vasitəsilə manifest daxil edilməsini təsdiqləyir
  qüvvəyə minmə və `crates/iroha_core/tests/gov_protected_gate.rs` məşqləri
  qorunan ad sahəsinin uçdan-uca qəbulu.
- Torii marşrutlarına sorğu/cavab vahidi testləri daxildir və CLI əmrləri
  JSON gediş-gəlişlərinin sabit qalmasını təmin edən inteqrasiya testləri.

Ətraflı referendum yükləri üçün `docs/source/governance_api.md`-ə baxın və
seçki bülletenlərinin iş prosesləri.
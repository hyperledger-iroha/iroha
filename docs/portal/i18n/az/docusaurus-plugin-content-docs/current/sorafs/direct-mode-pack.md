---
id: direct-mode-pack
lang: az
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Direct-Mode Fallback Pack (SNNet-5a)
sidebar_label: Direct-Mode Fallback Pack
description: Required configuration, compliance checks, and rollout steps when operating SoraFS in direct Torii/QUIC mode during the SNNet-5a transition.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

SoraNet sxemləri SoraFS üçün defolt daşıma olaraq qalır, lakin **SNNet-5a** yol xəritəsi elementi tənzimlənən ehtiyat tələb edir ki, operatorlar anonimliyin yayılması tamamlanarkən deterministik oxuma girişini saxlaya bilsinlər. Bu paket məxfilik daşımalarına toxunmadan SoraFS-i birbaşa Torii/QUIC rejimində işə salmaq üçün lazım olan CLI/SDK düymələrini, konfiqurasiya profillərini, uyğunluq testlərini və yerləşdirmə yoxlama siyahısını çəkir.

Qaytarma, SNNet-5-dən SNNet-9-a qədər onların hazırlıq qapılarını təmizləyənə qədər səhnələşdirmə və tənzimlənən istehsal mühitlərinə aiddir. Aşağıdakı artefaktları adi SoraFS yerləşdirmə girovu ilə yanaşı saxlayın ki, operatorlar tələb əsasında anonim və birbaşa rejimlər arasında keçid edə bilsinlər.

## 1. CLI & SDK Bayraqları

- `sorafs_cli fetch --transport-policy=direct-only …` rele planlamasını söndürür və Torii/QUIC nəqlini tətbiq edir. CLI yardımı indi `direct-only`-i qəbul edilmiş dəyər kimi siyahıya alır.
- SDK-lar "birbaşa rejim" keçidini ifşa etdikdə `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` təyin etməlidirlər. `iroha::ClientOptions` və `iroha_android`-də yaradılan bağlamalar eyni nömrəni irəli aparır.
- Gateway qoşquları (`sorafs_fetch`, Python bağlamaları) paylaşılan Norito JSON köməkçiləri vasitəsilə yalnız birbaşa keçidi təhlil edə bilər, beləliklə avtomatlaşdırma eyni davranışı alır.

Bayrağı partnyorla üzbəüz runbook-larda sənədləşdirin və ətraf mühit dəyişənlərindən daha çox `iroha_config` vasitəsilə tel funksiyasını dəyişdirin.

## 2. Gateway Siyasəti Profilləri

Deterministik orkestr konfiqurasiyasını davam etdirmək üçün Norito JSON istifadə edin. `docs/examples/sorafs_direct_mode_policy.json`-dəki nümunə profili kodlayır:

- `transport_policy: "direct_only"` — yalnız SoraNet relay nəqliyyatını reklam edən provayderləri rədd edin.
- `max_providers: 2` — ən etibarlı Torii/QUIC son nöqtələrinə birbaşa həmyaşıdları əhatə edir. Regional uyğunluq müavinətlərinə əsasən tənzimləyin.
- `telemetry_region: "regulated-eu"` — etiket buraxılan metriklər beləliklə, telemetriya panelləri və auditlər geri çəkilmələri fərqləndirir.
- Yanlış konfiqurasiya edilmiş şlüzlərin maskalanmasının qarşısını almaq üçün mühafizəkar təkrar sınaq büdcələri (`retry_budget: 2`, `provider_failure_threshold: 3`).

Siyasəti operatorlara təqdim etməzdən əvvəl JSON-u `sorafs_cli fetch --config` (avtomatlaşdırma) və ya SDK bağlamaları (`config_from_json`) vasitəsilə yükləyin. Audit cığırları üçün tablo çıxışını (`persist_path`) davam etdirin.

Şlüz tərəfindəki icra düymələri `docs/examples/sorafs_gateway_direct_mode.toml`-də tutulur. Şablon `iroha app sorafs gateway direct-mode enable`-dən çıxışı əks etdirir, zərf/qəbul yoxlamalarını, naqil sürəti limiti defoltlarını söndürür və `direct_mode` cədvəlini plandan əldə edilən host adları və manifest həzmləri ilə doldurur. Parçanı konfiqurasiya idarəçiliyinə təhvil verməzdən əvvəl yertutan dəyərləri yayma planınızla əvəz edin.

## 3. Uyğunluq Test Paketi

Birbaşa rejimə hazırlıq indi həm orkestr, həm də CLI qutularında əhatə dairəsini əhatə edir:

- `direct_only_policy_rejects_soranet_only_providers`, hər bir namizəd reklamı yalnız SoraNet relelərini dəstəklədikdə `TransportPolicy::DirectOnly`-in sürətli uğursuzluğuna zəmanət verir.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` mövcud olduqda Torii/QUIC nəqliyyatlarının istifadə olunmasını və SoraNet relelərinin sessiyadan xaric edilməsini təmin edir.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` sənədlərin köməkçi utilitlərlə uyğun qalmasını təmin etmək üçün `docs/examples/sorafs_direct_mode_policy.json`-i təhlil edir.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_poli.json】
- `fetch_command_respects_direct_transports` istehza edilmiş Torii şlüzinə qarşı `sorafs_cli fetch --transport-policy=direct-only` məşq edir, birbaşa nəqliyyatları bağlayan tənzimlənən mühitlər üçün tüstü testini təmin edir.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` eyni əmri yayma avtomatlaşdırılması üçün JSON siyasəti və skorboard davamlılığı ilə əhatə edir.

Yeniləmələri dərc etməzdən əvvəl fokuslanmış paketi işə salın:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

İş sahəsinin kompilyasiyası yuxarı axın dəyişikliklərinə görə uğursuz olarsa, bloklama xətasını `status.md`-də qeyd edin və asılılıq yetişən kimi yenidən işə salın.

## 4. Avtomatlaşdırılmış Siqaret Çəkmələri

Təkcə CLI əhatəsi ətraf mühitə xas reqressiyaları (məsələn, şlüz siyasətinin sürüşməsi və ya aşkar uyğunsuzluqları) üzə çıxarmır. Xüsusi tüstü köməkçisi `scripts/sorafs_direct_mode_smoke.sh`-də yaşayır və `sorafs_cli fetch`-i birbaşa rejimli orkestr siyasəti, skorbordunun davamlılığı və xülasə çəkilişi ilə əhatə edir.

İstifadə nümunəsi:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- Skript həm CLI bayraqlarına, həm də açar=dəyər konfiqurasiya fayllarına hörmət edir (bax: `docs/examples/sorafs_direct_mode_smoke.conf`). İşləməzdən əvvəl manifest həzm və provayder reklam qeydlərini istehsal dəyərləri ilə doldurun.
- `--policy` defolt olaraq `docs/examples/sorafs_direct_mode_policy.json` olaraq təyin edilir, lakin `sorafs_orchestrator::bindings::config_to_json` tərəfindən istehsal edilən istənilən orkestr JSON təmin edilə bilər. CLI siyasəti `--orchestrator-config=PATH` vasitəsilə qəbul edir və əl ilə tənzimlənən bayraqlar olmadan təkrarlanan qaçışlara imkan verir.
- `sorafs_cli` `PATH`-də olmadıqda köməkçi onu qurur
  `sorafs_orchestrator` sandıq (buraxılış profili) belə ki, tüstü qaçır
  göndərmə birbaşa rejimli santexnika.
- Çıxışlar:
  - Yığılmış yük (`--output`, defolt olaraq `artifacts/sorafs_direct_mode/payload.bin`).
  - Telemetriya bölgəsi və provayder hesabatlarını təqdim etmək üçün istifadə olunan hesabatları ehtiva edən xülasə (`--summary`, faydalı yüklə yanaşı defolt) alın.
  - Scoreboard snapshot JSON siyasətində elan edilmiş yola davam etdi (məsələn, `fetch_state/direct_mode_scoreboard.json`). Bunu dəyişiklik biletlərindəki xülasə ilə birlikdə arxivləşdirin.
- Qəbul qapısının avtomatlaşdırılması: götürmə başa çatdıqdan sonra köməkçi davamlı tablo və xülasə yollarından istifadə edərək `cargo xtask sorafs-adoption-check`-i çağırır. Tələb olunan kvorum əmr satırında təqdim olunan provayderlərin sayına uyğundur; daha böyük bir nümunəyə ehtiyacınız olduqda onu `--min-providers=<n>` ilə ləğv edin. Övladlığa götürmə hesabatları xülasənin yanında yazılır (`--adoption-report=<path>` fərdi yer təyin edə bilər) və köməkçi `--require-direct-only`-i defolt olaraq (qaytarmağa uyğun) və uyğun CLI bayrağını təqdim etdikdə `--require-telemetry`-i keçir. Əlavə xtask arqumentlərini yönləndirmək üçün `XTASK_SORAFS_ADOPTION_FLAGS`-dən istifadə edin (məsələn, təsdiq edilmiş endirmə zamanı `--allow-single-source`, beləliklə darvaza həm dözümlülük göstərsin, həm də geriyə qayıtmağa məcbur etsin). Yerli diaqnostika işlədərkən yalnız `--skip-adoption-check` ilə qəbul qapısını keçin; yol xəritəsi övladlığa götürmə hesabatı paketini daxil etmək üçün hər bir tənzimlənən birbaşa rejimdə işləməyi tələb edir.

## 5. Təqdimat Yoxlama Siyahısı

1. **Konfiqurasiyanın dondurulması:** Birbaşa rejimli JSON profilini `iroha_config` repozitorunuzda saxlayın və dəyişiklik biletinizdə hashı qeyd edin.
2. **Gateway auditi:** Torii son nöqtələrinin TLS, qabiliyyət TLV-lərini və birbaşa rejimi çevirməzdən əvvəl audit qeydini tətbiq etdiyini təsdiqləyin. Gateway siyasəti profilini operatorlara dərc edin.
3. **Uyğunluğun qeydiyyatı:** Yenilənmiş oyun kitabını uyğunluq/tənzimləyici rəyçilər ilə paylaşın və anonimlik örtüyündən kənarda işləmək üçün təsdiqləri əldə edin.
4. **Quru qaçış:** Uyğunluq test dəstini və yaxşı məlum olan Torii provayderlərinə qarşı bir mərhələ gətirməni həyata keçirin. Arxiv skorbordunun nəticələri və CLI xülasəsi.
5. **İstehsalın kəsilməsi:** Dəyişiklik pəncərəsini elan edin, `transport_policy`-i `direct_only`-ə çevirin (əgər siz `soranet-first`-ə daxil olmusunuzsa) və birbaşa rejim panellərinə nəzarət edin (I18NI000000071X gecikmələri). Geri qaytarma planını sənədləşdirin ki, `roadmap.md:532`-də SNNet-4/5/5a/5b/6a/7/8/12/13 məzunu olduqdan sonra SoraNet-ə ilk dəfə qayıda biləsiniz.
6. **Dəyişiklikdən sonrakı baxış:** Dəyişiklik biletinə hesab tablosunun anlıq görüntülərini, xülasələri və monitorinq nəticələrini əlavə edin. `status.md`-i qüvvəyə minmə tarixi və hər hansı anomaliya ilə yeniləyin.

Yoxlama siyahısını `sorafs_node_ops` runbook ilə yanaşı saxlayın ki, operatorlar canlı keçiddən əvvəl iş axınını təkrarlaya bilsinlər. SNNet-5 GA-nı bitirdikdə, istehsal telemetriyasındakı pariteti təsdiq etdikdən sonra ehtiyatı dayandırın.

## 6. Sübut və Qəbul Qapısı Tələbləri

Birbaşa rejimdə çəkilişlər hələ də SF-6c qəbul qapısını təmin etməlidir. Bundle
Hesab lövhəsi, xülasə, manifest zərfi və hər qaçış üçün övladlığa götürmə hesabatı
`cargo xtask sorafs-adoption-check` geriləmə vəziyyətini təsdiqləyə bilər. İtkin
sahələr qapını uğursuzluğa məcbur edir, buna görə də dəyişiklikdə gözlənilən metaməlumatları qeyd edin
biletlər.

- **Nəqliyyat metadata:** `scoreboard.json` bəyan etməlidir
  `transport_policy="direct_only"` (və çevirin `transport_policy_override=true`
  siz endirməyə məcbur etdiyiniz zaman). Qoşalaşmış anonimlik siyasəti sahələrini saxlayın
  onlar defoltları miras aldıqda belə doldurulur ki, rəyçilər sizin olub olmadığını görə bilsinlər
  mərhələli anonimlik planından kənara çıxdı.
- **Provayder sayğacları:** Yalnız şlüz üçün seanslar davam etməlidir `provider_count=0`
  və `gateway_provider_count=<n>`-i Torii provayderlərinin sayı ilə doldurun
  istifadə olunur. JSON-u əl ilə redaktə etməkdən çəkinin - CLI/SDK artıq və sayları əldə edir
  övladlığa götürmə qapısı bölünməni buraxan çəkmələri rədd edir.
- **Açıq sübut:** Torii şlüzləri iştirak etdikdə, imzalanmış sənədi keçin
  `--gateway-manifest-envelope <path>` (və ya SDK ekvivalenti).
  `gateway_manifest_provided` plus `gateway_manifest_id`/`gateway_manifest_cid`
  `scoreboard.json`-də qeyd olunur. `summary.json` uyğunluğu daşıdığından əmin olun
  `manifest_id`/`manifest_cid`; hər hansı bir fayl varsa, övladlığa götürmə yoxlaması uğursuz olur
  cütü əldən verdi.
- **Telemetri gözləntiləri:** Telemetriya çəkilişi müşayiət etdikdə, onu işə salın
  `--require-telemetry` ilə qapı, beləliklə qəbul hesabatı göstəricilərin olduğunu sübut edir
  buraxılmışdır. Hava boşluğu olan məşqlər bayraqdan kənarlaşdırıla bilər, lakin CI və biletləri dəyişdirə bilər
  olmamasını sənədləşdirməlidir.

Misal:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
````adoption_report.json`-ni tablonun, xülasənin, manifestin yanında əlavə edin
zərf və tüstü jurnalı paketi. Bu artefaktlar CI övladlığa götürmə işini əks etdirir
(`ci/check_sorafs_orchestrator_adoption.sh`) birbaşa rejimi tətbiq edir və saxlayır
yoxlanılan səviyyələri aşağı salır.
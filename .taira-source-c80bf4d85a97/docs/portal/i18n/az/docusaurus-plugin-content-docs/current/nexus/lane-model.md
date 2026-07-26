---
id: nexus-lane-model
lang: az
direction: ltr
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus lane model
description: Logical lane taxonomy, configuration geometry, and world-state merge rules for Sora Nexus.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Nexus Zolaq Modeli və WSV Bölmə

> **Status:** NX-1 təhvil verilə bilər — zolaq taksonomiyası, konfiqurasiya həndəsəsi və saxlama sxemi həyata keçirməyə hazırdır.  
> **Sahiblər:** Nexus Əsas İş Qrupu, İdarəetmə İş Qrupu  
> **Yol xəritəsi arayışı:** `roadmap.md`-də NX-1

Bu portal səhifəsi Sora kanonik `docs/source/nexus_lanes.md` qısasını əks etdirir
Nexus operatorları, SDK sahibləri və rəyçilər zolaqlı təlimatı olmadan oxuya bilərlər
mono-repo ağacına dalmaq. Hədəf memarlığı dünya vəziyyətini saxlayır
fərdi məlumat fəzalarının (zolaqların) ictimai və ya işləməsinə icazə verərkən deterministik
təcrid olunmuş iş yükləri ilə şəxsi validator dəstləri.

## Konsepsiyalar

- **Lane:** Öz təsdiqləyici dəsti ilə Nexus kitabçasının məntiqi parçası və
  icra geriliyi. Stabil `LaneId` ilə müəyyən edilmişdir.
- **Məlumat Məkanı:** Paylaşılan bir və ya daha çox zolağı qruplaşdıran İdarəetmə qutusu
  uyğunluq, marşrutlaşdırma və hesablaşma siyasətləri.
- **Lane Manifest:** Validatorları təsvir edən idarəetmə tərəfindən idarə olunan metadata, DA
  siyasət, qaz nişanı, hesablaşma qaydaları və marşrutlaşdırma icazələri.
- **Qlobal Öhdəlik:** Yeni dövlət köklərini ümumiləşdirən zolaq tərəfindən yayılan sübut,
  hesablaşma məlumatları və isteğe bağlı zolaqlar arası köçürmələr. Qlobal NPoS halqası
  sifariş öhdəlikləri.

## Zolaq taksonomiyası

Zolaq növləri qanuni olaraq onların görünürlüyünü, idarəetmə səthini və
məskunlaşma qarmaqları. Konfiqurasiya həndəsəsi (`LaneConfig`) bunları tutur
atributlar, beləliklə qovşaqlar, SDK-lar və alətlər layout olmadan əsaslandıra bilər
sifarişli məntiq.

| Zolaq növü | Görünüş | Validator üzvlük | WSV məruz qalma | Defolt idarəetmə | Hesablaşma siyasəti | Tipik istifadə |
|----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | ictimai | İcazəsiz (qlobal pay) | Tam dövlət replikası | SORA Parlament | `xor_global` | Əsas ictimai kitab |
| `public_custom` | ictimai | İcazəsiz və ya pay qapılı | Tam dövlət replikası | Pay ölçülmüş modul | `xor_lane_weighted` | Yüksək məhsuldarlıqlı ictimai proqramlar |
| `private_permissioned` | məhdudlaşdırılmış | Sabit validator dəsti (idarəetmə təsdiqlənib) | Öhdəliklər və sübutlar | Federasiya Şurası | `xor_hosted_custody` | CBDC, konsorsium iş yükləri |
| `hybrid_confidential` | məhdudlaşdırılmış | Qarışıq üzvlük; ZK sübutlarını sarar | Öhdəliklər + seçmə açıqlama | Proqramlaşdırıla bilən pul modulu | `xor_dual_fund` | Məxfiliyi qoruyan proqramlaşdırıla bilən pul |

Bütün zolaq növləri bəyan etməlidir:

- Dataspace ləqəbi — uyğunluq siyasətlərini birləşdirən insan tərəfindən oxuna bilən qruplaşma.
- İdarəetmə dəstəyi — identifikator `Nexus.governance.modules` vasitəsilə həll edilir.
- Hesablaşma dəstəyi — XOR debetinə hesablaşma marşrutlaşdırıcısı tərəfindən istehlak edilən identifikator
  tamponlar.
- Əlavə telemetriya metadatası (təsvir, əlaqə, biznes sahəsi) ortaya çıxdı
  `/status` və idarə panelləri vasitəsilə.

## Zolaq konfiqurasiya həndəsəsi (`LaneConfig`)

`LaneConfig` təsdiqlənmiş zolaq kataloqundan əldə edilmiş icra vaxtı həndəsəsidir. Bu
idarəetmə manifestlərini ** əvəz etmir**; əvəzinə deterministik təmin edir
hər konfiqurasiya edilmiş zolaq üçün yaddaş identifikatorları və telemetriya göstərişləri.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` konfiqurasiya olduqda həndəsəni yenidən hesablayır
  yükləndi (`State::set_nexus`).
- Ləqəblər kiçik şlaklara sanitarlaşdırılır; ardıcıl qeyri-rəqəmsal
  simvollar `_`-ə yığılır. Əgər ləqəb boş bir şlak verirsə, geri çəkilirik
  `lane{id}` üçün.
- `shard_id` kataloq metadata açarından əldə edilib `da_shard_id` (defolt
  `lane_id`) və DA təkrarını saxlamaq üçün davamlı qırıq kursor jurnalını idarə edir
  yenidən başlamalar/yenidən paylaşma üzrə deterministik.
- Açar prefikslər, WSV-nin hər bir zolaqlı açar diapazonlarını bir-birindən ayrı saxlamasını təmin edir
  eyni arxa uç paylaşılır.
- Kür seqment adları hostlar arasında deterministikdir; auditorlar çarpaz yoxlaya bilərlər
  xüsusi alətlər olmadan seqment kataloqları və manifestləri.
- Birləşmə seqmentləri (`lane_{id:03}_merge`) ən son birləşmə işarəsi köklərinə malikdir və
  bu zolaq üçün qlobal dövlət öhdəlikləri.

## Dünya dövlətlərinin bölünməsi

- Məntiqi Nexus dünya vəziyyəti hər zolaqlı vəziyyət fəzalarının birliyidir. İctimai
  zolaqlar tam vəziyyətdə qalır; özəl/məxfi zolaqlar ixracı Merkle/öhdəlik
  birləşmə kitabçasına köklər.
- MV yaddaşı 4 baytlıq zolaq prefiksi ilə hər düyməyə prefiks verir
  `LaneConfigEntry::key_prefix`, `[00 00 00 01] ++ kimi açarlar verir
  PackedKey`.
- Paylaşılan cədvəllər (hesablar, aktivlər, tetikler, idarəetmə qeydləri) buna görə də saxlanılır
  girişlər zolaq prefiksi ilə qruplaşdırılıb, diapazon skanlarını deterministik saxlayaraq.
- Birləşdirmə jurnalının metadatası eyni tərtibatı əks etdirir: hər bir zolaq birləşmə göstərişini yazır
  kökləri və azaldılmış qlobal dövlət kökləri `lane_{id:03}_merge`, imkan verir
  zolaqdan çıxdıqda məqsədli saxlama və ya çıxarılma.
- Çarpaz indekslər (hesab ləqəbləri, aktiv reyestrləri, idarəetmə manifestləri)
  operatorların girişləri tez bir zamanda uzlaşdıra bilməsi üçün açıq zolaq prefikslərini saxlayın.
- **Saxlama siyasəti** — ictimai zolaqlar tam blok gövdələri saxlayır; yalnız öhdəlik
  zolaqlar nəzarət məntəqələrindən sonra köhnə cisimləri sıxışdıra bilər, çünki öhdəliklər belədir
  nüfuzlu. Məxfi zolaqlar şifrəli mətn jurnallarını xüsusi olaraq saxlayır
  digər iş yüklərini bloklamamaq üçün seqmentlər.
- **Tooling** — texniki xidmət proqramları (`kagami`, CLI admin əmrləri)
  ölçüləri, Prometheus etiketlərini ifşa edərkən ləngimiş ad sahəsinə istinad edin və ya
  Kür seqmentlərinin arxivləşdirilməsi.

## Marşrutlaşdırma və API

- Torii REST/gRPC son nöqtələri isteğe bağlı `lane_id` qəbul edir; yoxluğu nəzərdə tutur
  `lane_default`.
- SDK yerüstü zolaq seçiciləri və istifadəçi dostu ləqəbləri istifadə edərək `LaneId` ilə əlaqələndirin
  zolaq kataloqu.
- Marşrutlaşdırma qaydaları təsdiqlənmiş kataloqda işləyir və həm zolağı, həm də zolağı seçə bilər
  məlumat məkanı. `LaneConfig` tablosuna telemetriyaya uyğun ləqəblər təqdim edir və
  loglar.

## Hesablaşma və ödənişlər

- Hər bir zolaq qlobal validator dəstinə XOR haqqı ödəyir. Zolaqlar yerli toplaya bilər
  qaz tokenləri, lakin öhdəliklərlə yanaşı XOR ekvivalentlərini əmanət etməlidir.
- Hesablaşma sübutlarına məbləğ, konvertasiya metadatası və əmanət sübutu daxildir
  (məsələn, qlobal ödəniş kassasına köçürmə).
- Vahid hesablaşma marşrutlaşdırıcısı (NX-3) eyni zolaqdan istifadə edərək buferləri debet edir
  prefikslər, beləliklə, məskunlaşma telemetriyası saxlama həndəsəsi ilə üst-üstə düşür.

## İdarəetmə

- Zolaqlar kataloq vasitəsilə idarəetmə modulunu elan edirlər. `LaneConfigEntry`
  telemetriya və audit yollarını saxlamaq üçün orijinal ləqəbi və şlakı daşıyır
  oxunaqlı.
- Nexus reyestri imzalanmış zolaqlı manifestləri paylayır.
  `LaneId`, məlumat məkanının bağlanması, idarəetmə dəstəyi, hesablaşma dəstəyi və
  metadata.
- İş vaxtı təkmilləşdirmə qarmaqları idarəetmə siyasətlərini tətbiq etməyə davam edir
  (standart olaraq `gov_upgrade_id`) və telemetriya körpüsü vasitəsilə log fərqləri
  (`nexus.config.diff` hadisələri).

## Telemetriya və status

- `/status` zolaq ləqəblərini, məlumat məkanı bağlamalarını, idarəetmə tutacaqlarını və
  Kataloqdan və `LaneConfig`-dən əldə edilən hesablaşma profilləri.
- Planlayıcı ölçüləri (`nexus_scheduler_lane_teu_*`) zolaq ləqəblərini/şlaklarını göstərir
  operatorlar geriləmə və TEU təzyiqini tez bir zamanda xəritələyə bilər.
- `nexus_lane_configured_total` əldə edilmiş zolaq girişlərinin sayını hesablayır və
  konfiqurasiya dəyişdikdə yenidən hesablanır. Telemetriya istənilən vaxt işarəli fərqlər buraxır
  zolaq həndəsəsi dəyişir.
- Dataspace backlog ölçüləri kömək etmək üçün ləqəb/təsvir metadata daxildir
  operatorlar növbə təzyiqini biznes domenləri ilə əlaqələndirirlər.

## Konfiqurasiya və Norito növləri

- `LaneCatalog`, `LaneConfig` və `DataSpaceCatalog` yaşayır
  `iroha_data_model::nexus` və Norito formatlı strukturları təmin edin.
  manifestlər və SDK-lar.
- `LaneConfig` `iroha_config::parameters::actual::Nexus`-də yaşayır və əldə edilir
  avtomatik olaraq kataloqdan; Norito kodlamasını tələb etmir, çünki o
  daxili iş vaxtı köməkçisidir.
- İstifadəçiyə yönəlmiş konfiqurasiya (`iroha_config::parameters::user::Nexus`)
  deklarativ zolağı və verilənlər məkanı deskriptorlarını qəbul etməyə davam edir; indi təhlil edir
  həndəsə əldə edir və etibarsız ləqəbləri və ya dublikat zolağı identifikatorlarını rədd edir.

## Görkəmli iş

- XOR buferi üçün hesablaşma marşrutlaşdırıcısı yeniləmələrini (NX-3) yeni həndəsə ilə inteqrasiya edin
  debet və qəbzlər zolaqlı şlak ilə işarələnir.
- Sütun ailələrini, yığcam təqaüdçü zolaqları siyahıya almaq üçün admin alətlərini genişləndirin və
  slugged ad boşluğundan istifadə edərək hər zolaqlı blok qeydlərini yoxlayın.
- Birləşmə alqoritmini yekunlaşdırın (sifariş, budama, münaqişənin aşkarlanması) və
  zolaqlararası təkrar oynatma üçün reqressiya qurğuları əlavə edin.
- Ağ siyahılar/qara siyahılar və proqramlaşdırıla bilən pullar üçün uyğunluq qarmaqları əlavə edin
  siyasətlər (NX-12 altında izlənilir).

---

*Bu səhifə NX-1 təqiblərini NX-2-dən NX-18-ə qədər izləməyə davam edəcək.
Lütfən, açıq sualları `roadmap.md` və ya idarəetmə izləyicisində göstərin.
portal kanonik sənədlərə uyğun olaraq qalır.*
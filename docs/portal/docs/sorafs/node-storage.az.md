---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffa884bf745ab5f79c20d4b20baaba842878301dc56a66463c4520275ce4fd0b
source_last_modified: "2026-01-05T09:28:11.899124+00:00"
translation_last_reviewed: 2026-02-07
id: node-storage
title: SoraFS Node Storage Design
sidebar_label: Node Storage Design
description: Storage architecture, quotas, and lifecycle hooks for Torii nodes hosting SoraFS data.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

## SoraFS Node Saxlama Dizaynı (Qaralama)

Bu qeyd Iroha (Torii) nodeunun SoraFS datasına necə qoşula biləcəyini dəqiqləşdirir
mövcudluq təbəqəsi və saxlama və xidmət üçün bir dilim yerli disk ayırın
parçalar. O, `sorafs_node_client_protocol.md` kəşf spesifikasiyasını tamamlayır və
SF-1b qurğusu saxlama tərəfi arxitekturasını, resursunu təsvir etməklə işləyir
qovşaqda və şlüzdə enməli olan nəzarət və konfiqurasiya santexnika
kod yolları. Praktiki operator matkapları yaşayır
[Node Operations Runbook](./node-operations).

### Məqsədlər

- İstənilən validator və ya köməkçi Iroha prosesinə ehtiyat diski bir disk kimi göstərməyə icazə verin.
  Əsas kitab məsuliyyətlərinə təsir etmədən SoraFS provayderi.
- Saxlama modulunu deterministik və Norito ilə idarə edin: manifestlər,
  yığın planları, Proof-of-Retrievability (PoR) kökləri və provayder reklamları bunlardır.
  həqiqət mənbəyi.
- Operator tərəfindən müəyyən edilmiş kvotaları tətbiq edin ki, node öz resurslarını tükənməsin
  çoxlu pin və ya gətirmə sorğularını qəbul edir.
- Səth sağlamlığı/temetriya (PoR nümunəsi, yığın gətirmə gecikməsi, disk təzyiqi)
  idarəetməyə və müştərilərə qayıt.

### Yüksək Səviyyəli Memarlıq

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Əsas modullar:

- **Gateway**: pin təklifləri, yığın gətirmə üçün Norito HTTP son nöqtələrini ifşa edir
  sorğular, PoR nümunəsi və telemetriya. O, Norito faydalı yükləri və
  marşallar yığın mağazasına sorğu göndərir. Mövcud Torii HTTP yığınını təkrar istifadə edir
  yeni bir demondan qaçmaq üçün.
- **Pin Qeydiyyatı**: `iroha_data_model::sorafs`-də izlənilən manifest pin vəziyyəti
  və `iroha_core`. Manifest qəbul edildikdə reyestr qeyd edir
  manifest həzm, yığın planı həzm, PoR kökü və provayder qabiliyyəti bayraqları.
- **Yüksək Saxlama**: qəbul edən disklə dəstəklənən `ChunkStore` tətbiqi
  imzalanmış manifestlər, `ChunkProfile::DEFAULT` istifadə edərək yığın planlarını həyata keçirir və
  deterministik layout altında parçaları davam etdirir. Hər bir parça a ilə əlaqələndirilir
  məzmun barmaq izi və PoR metadatası beləliklə seçmə olmadan yenidən doğrulaya bilsin
  bütün faylı yenidən oxumaq.
- **Kvota/Scheduler**: operator tərəfindən konfiqurasiya edilmiş məhdudiyyətləri tətbiq edir (maksimum disk baytları,
  maksimum görkəmli sancaqlar, maksimum paralel gətirmələr, yığın TTL) və koordinatlar
  IO beləliklə qovşağın uçotu vəzifələri ac qalmaz. Planlayıcı da
  məhdud CPU ilə PoR sübutlarına və nümunə sorğularına xidmət etmək üçün cavabdehdir.

### Konfiqurasiya

`iroha_config`-ə yeni bölmə əlavə edin:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: iştirak keçidi. Yanlış olduqda şlüz 503 üçün qaytarır
  saxlama son nöqtələri və node kəşfdə reklam etmir.
- `data_dir`: yığın məlumatları, PoR ağacları və telemetriya gətirmək üçün kök kataloqu.
  Defolt olaraq `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: bərkidilmiş yığın məlumatları üçün sərt limit. Fon işi
  limitə çatdıqda yeni pinləri rədd edir.
- `max_parallel_fetches`: tarazlıq üçün planlaşdırıcı tərəfindən tətbiq edilən paralellik həddi
  validator iş yükünə qarşı bant genişliyi/disk IO.
- `max_pins`: tətbiq etməzdən əvvəl qovşağın qəbul etdiyi manifest pinlərin maksimum sayı
  evakuasiya/geri təzyiq.
- `por_sample_interval_secs`: avtomatik PoR seçmə işləri üçün kadans. Hər bir iş
  nümunələri `N` tərk edir (manifest üçün konfiqurasiya edilə bilər) və telemetriya hadisələri yayır.
  İdarəetmə tutum metadatasını təyin etməklə `N`-ni müəyyən şəkildə miqyaslandıra bilər
  açar `profile.sample_multiplier` (tam ədəd `1-4`). Qiymət tək ola bilər
  nömrə/sətir və ya hər bir profili ləğv edən obyekt, məs.
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: doldurmaq üçün provayder reklam generatoru tərəfindən istifadə edilən struktur
  `ProviderAdvertV1` sahələri (bahis göstəricisi, QoS göstərişləri, mövzular). buraxılmışsa
  node idarəetmə reyestrindən defoltlardan istifadə edir.

Santexnika konfiqurasiyası:

- `[sorafs.storage]` `iroha_config`-də `SorafsStorage` kimi müəyyən edilir və
  node konfiqurasiya faylından yüklənir.
- `iroha_core` və `iroha_torii` yaddaş konfiqurasiyasını şlüzə keçirin
  başlanğıcda inşaatçı və yığın mağazası.
- İnkişaf/sınaq mühitinin ləğvi mövcuddur (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), lakin
  istehsal yerləşdirmələri konfiqurasiya faylına etibar etməlidir.

### CLI Utilities

Torii-in HTTP səthi hələ də naqil edilərkən, `sorafs_node` qutusu
nazik CLI, belə ki, operatorlar davamlı proqramlara qarşı qəbul/ixrac təlimlərini skript edə bilsinlər.
arxa uç.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` Norito kodlu manifest `.to` faylı üstəgəl uyğun yükü gözləyir
  bayt. O, manifestin parçalanma profilindən yığın planını yenidən qurur,
  həzm paritetini tətbiq edir, yığın faylları saxlayır və istəyə görə a
  `chunk_fetch_specs` JSON blob ki, aşağı axın alətləri ağlı başında olmanı yoxlaya bilsin.
  layout.
- `export` manifest identifikatorunu qəbul edir və saxlanan manifest/faydalı yükü diskə yazır
  (isteğe bağlı JSON planı ilə) beləliklə qurğular mühitlərdə təkrarlana bilir.

Hər iki əmr Norito JSON xülasəsini stdout-a çap edir və bu, daxil olmağı asanlaşdırır.
skriptlər. CLI təzahürləri təmin etmək üçün inteqrasiya testi ilə əhatə olunur
faydalı yüklər Torii API enişindən əvvəl təmiz şəkildə gediş-gəliş edir.【crates/sorafs_node/tests/cli.rs:1】

> HTTP pariteti
>
> Torii şlüzü indi eyni proqram tərəfindən dəstəklənən yalnız oxuna bilən köməkçiləri ifşa edir.
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — saxlananı qaytarır
> Norito manifest (base64) həzm/metadata ilə yanaşı.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — deterministi qaytarır
> aşağı axın alətləri üçün yığın planı JSON (`chunk_fetch_specs`).【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Bu son nöqtələr CLI çıxışını əks etdirir, beləliklə boru kəmərləri lokaldan keçid edə bilsin
> skriptləri təhlilediciləri dəyişmədən HTTP zondlarına.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Node Lifecycle

1. **Başlanğıc**:
   - Saxlama aktivdirsə, qovşaq yığın anbarını ilə işə salır
     konfiqurasiya edilmiş kataloq və tutum. Bura yoxlama və ya yaratmaq daxildir
     PoR manifest verilənlər bazası və bərkidilmiş manifestlərin isti keşlərə təkrar oxunması.
   - SoraFS şlüz marşrutlarını qeydiyyatdan keçirin (pin üçün Norito JSON POST/GET son nöqtələri,
     gətirmə, PoR nümunəsi, telemetriya).
   - PoR seçmə işçisini və kvota monitorunu yaradın.
2. **Kəşf / Reklamlar**:
   - Cari imkandan/sağlamlıqdan istifadə edərək `ProviderAdvertV1` sənədləri yaradın, işarələyin
     onları şura tərəfindən təsdiqlənmiş açarla və kəşf kanalı vasitəsilə dərc edin.
     mövcuddur.
3. **Pin İş Akışı**:
   - Gateway imzalanmış manifest alır (o cümlədən yığın planı, PoR kökü, şura
     imzalar). Ləqəb siyahısını təsdiq edin (`sorafs.sf1@1.0.0` tələb olunur) və
     yığın planının manifest metadata ilə uyğun olduğundan əmin olun.
   - Kvotaları yoxlayın. Tutum/pin limitləri keçərsə, a ilə cavab verin
     siyasət xətası (Norito strukturlaşdırılmış).
   - `ChunkStore`-ə yığın məlumat axını, qəbul etdiyimiz zaman həzmləri yoxlayın.
     PoR ağaclarını yeniləyin və manifest metadatalarını reyestrdə saxlayın.
4. **İş axınını gətirin**:
   - Diskdən yığın diapazonu sorğularına xidmət edin. Planlayıcı tətbiq edir
     `max_parallel_fetches` və doymuş olduqda `429` qaytarır.
   - Strukturlaşdırılmış telemetriya (Norito JSON) gecikmə, xidmət edilən baytlar və
     aşağı axın monitorinqi üçün xəta hesablanır.
5. **PoR Nümunəsinin Alınması**:
   - İşçi çəkiyə mütənasib manifestləri seçir (məsələn, saxlanan bayt) və
     yığın mağazasının PoR ağacından istifadə edərək deterministik seçmə aparır.
   - İdarəetmə auditlərinin nəticələrini davam etdirin və xülasələri təminatçıya daxil edin
     reklamlar / telemetriya son nöqtələri.
6. **Evdən Çıxarma / Kvota İcrası**:
   - Tutum əldə edildikdə node standart olaraq yeni sancaqlar rədd edir. İstəyə görə,
     operatorlar çıxarılma siyasətlərini (məsələn, TTL əsaslı, LRU) konfiqurasiya edə bilər
     idarəetmə modeli razılaşdırılıb; hələlik dizayn ciddi kvotaları nəzərdə tutur və
     operator tərəfindən başlatılan əməliyyatları çıxarmaq.

### Bacarıq Bəyannaməsi və Planlaşdırma İnteqrasiyası- Torii indi `/v1/sorafs/capacity/declare`-dən `CapacityDeclarationRecord` yeniləmələrini ötürür
  daxil edilmiş `CapacityManager`-ə, buna görə də hər bir qovşaq onun yaddaşdaxili görünüşünü qurur
  chunker və zolaq ayırmaları həyata keçirdi. Menecer yalnız oxunan anlıq görüntüləri ifşa edir
  telemetriya üçün (`GET /v1/sorafs/capacity/state`) və hər profil və ya hər zolaq üçün tətbiq edir
  yeni sifarişlər qəbul edilməzdən əvvəl rezervasiyalar.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- `/v1/sorafs/capacity/schedule` son nöqtəsi idarəetmə tərəfindən verilmiş `ReplicationOrderV1`-i qəbul edir
  faydalı yüklər. Sifariş yerli provayderi hədəf aldıqda, menecer yoxlayır
  dublikat planlaşdırma, chunker/zolaq tutumunu yoxlayır, dilimi ehtiyatda saxlayır və
  orkestrasiya alətləri üçün qalan tutumu təsvir edən `ReplicationPlan` qaytarır
  qəbulu ilə davam edə bilər. Digər provayderlər üçün sifarişlər an ilə təsdiqlənir
  Çox operatorlu iş axınlarını asanlaşdırmaq üçün `ignored` cavabı.【crates/iroha_torii/src/routing.rs:4845】
- Tamamlama qarmaqları (məsələn, udma müvəffəqiyyətli olduqdan sonra işə salınır) vuruldu
  `POST /v1/sorafs/capacity/complete` vasitəsilə rezervasiyaları buraxmaq
  `CapacityManager::complete_order`. Cavab `ReplicationRelease` daxildir
  snapshot (qalan cəmlər, chunker/zolaq qalıqları) belə ki, orkestrasiya alətləri
  səsvermə olmadan növbəti sifarişi növbəyə qoyun. Sonrakı iş bunu hissəyə çevirəcək
  boru kəmərini bir dəfə udma məntiqi torpaqları saxlayır.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Daxili `TelemetryAccumulator` vasitəsilə mutasiya edilə bilər
  `NodeHandle::update_telemetry`, fon işçilərinə PoR/iş vaxtı nümunələrini qeyd etməyə imkan verir
  və nəticədə toxunmadan kanonik `CapacityTelemetryV1` faydalı yükləri əldə edin
  planlaşdırıcı daxili.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### İnteqrasiya və Gələcək İş

- **İdarəetmə**: yaddaş telemetriyası ilə `sorafs_pin_registry_tracker.md`-i genişləndirin
  (PoR müvəffəqiyyət dərəcəsi, diskdən istifadə). Qəbul siyasətləri minimum tələb edə bilər
  reklamlar qəbul edilməzdən əvvəl tutum və ya minimum PoR müvəffəqiyyət dərəcəsi.
- **Müştəri SDK-ları**: yeni yaddaş konfiqurasiyasını (disk limitləri, ləqəb) ifşa edin
  idarəetmə alətləri qovşaqları proqramlı şəkildə yükləyə bilər.
- **Telemetri**: mövcud ölçülər yığını ilə inteqrasiya edin (Prometheus /
  OpenTelemetry) belə ki, yaddaş göstəriciləri müşahidə panellərində görünür.
- **Təhlükəsizlik**: yaddaş modulunu xüsusi async tapşırıq hovuzunun daxilində işlədin
  geri təzyiq edin və io_uring və ya tokio vasitəsilə sandboxing yığınını oxuyun
  Zərərli müştərilərin resursları tükənməsinin qarşısını almaq üçün məhdud hovuzlar.

Bu dizayn saxlama modulunu isteğe bağlı və deterministik olaraq saxlayır
operatorlar SoraFS məlumat mövcudluğunda iştirak etmək üçün lazım olan düymələri
qat. Onun həyata keçirilməsi `iroha_config`, `iroha_core`,
`iroha_torii` və Norito şlüz, üstəlik provayder reklam alətləri.
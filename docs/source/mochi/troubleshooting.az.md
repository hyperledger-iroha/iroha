---
lang: az
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2025-12-29T18:16:35.985892+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# MOCHI Problemləri Giderme Bələdçisi

Yerli MOCHI klasterləri başlamaqdan imtina etdikdə bu runbook-dan istifadə edin
döngəni yenidən başladın və ya blok/hadisə/status yeniləmələrini dayandırın. Uzadır
nəzarətçi davranışlarını çevirərək yol xəritəsi elementi "Sənədləşdirmə və təqdimetmə"
`mochi-core` konkret bərpa addımlarına.

## 1. İlk cavab verənin yoxlama siyahısı

1. MOCHI-nin istifadə etdiyi məlumat kökünü çəkin. Defolt aşağıdakı kimidir
   `$TMPDIR/mochi/<profile-slug>`; xüsusi yollar UI başlıq çubuğunda görünür və
   `cargo run -p mochi-ui-egui -- --data-root ...` vasitəsilə.
2. İş sahəsinin kökündən `./ci/check_mochi.sh`-i işə salın. Bu, əsası təsdiqləyir,
   Konfiqurasiyaları dəyişdirməyə başlamazdan əvvəl UI və inteqrasiya qutuları.
3. Əvvəlcədən təyini qeyd edin (`single-peer` və ya `four-peer-bft`). Yaradılmış topologiya
   məlumat kökü altında neçə peer qovluq/loqlar gözləməli olduğunuzu müəyyən edir.

## 2. Qeydlər və telemetriya sübutları toplayın

`NetworkPaths::ensure` (bax `mochi/mochi-core/src/config.rs`) stabil yaradır
düzən:

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

Dəyişikliklər etməzdən əvvəl bu addımları yerinə yetirin:

- Sonuncunu çəkmək üçün **Logs** nişanından istifadə edin və ya birbaşa `logs/<alias>.log` açın
  Hər həmyaşıd üçün 200 sətir. Nəzarətçi stdout/stderr/sistem kanallarını izləyir
  `PeerLogStream` vasitəsilə, buna görə də bu fayllar UI çıxışına uyğun gəlir.
- Snapşotu **Xidmət → Eksport snapshot** (və ya zəng) vasitəsilə ixrac edin
  `Supervisor::export_snapshot`). Snapshot yaddaşı, konfiqurasiyanı və
  `snapshots/<timestamp>-<label>/`-ə daxil olur.
- Əgər problem axın vidjetləri ilə bağlıdırsa, `ManagedBlockStream`-i kopyalayın,
  `ManagedEventStream` və `ManagedStatusStream` sağlamlıq göstəriciləri
  İdarə paneli. UI sonuncu yenidən qoşulma cəhdini və səhv səbəbini göstərir; tutmaq
  hadisə rekordu üçün ekran görüntüsü.

## 3. Peer başlanğıc problemlərinin həlli

Peer launch uğursuzluqlarının çoxu üç vedrəyə düşür:

### Çatışmayan ikili fayllar və ya səhv ləğvetmələr

`SupervisorBuilder`, `irohad`, `kagami` və (gələcək) `iroha_cli`-ə çatır.
UI "prosesin yayılması uğursuz oldu" və ya "icazə rədd edildi" hesabatı verərsə, MOCHI-yə işarə edin
tanınmış ikili sistemlərdə:

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

Qarşısının alınması üçün `MOCHI_IROHAD`, `MOCHI_KAGAMI` və `MOCHI_IROHA_CLI` parametrlərini təyin edə bilərsiniz.
bayraqları təkrar-təkrar yazmaq. Paket quruluşlarını sazlayarkən, müqayisə edin
`BundleConfig` `mochi/mochi-ui-egui/src/config/`-dəki yollara qarşı
`target/mochi-bundle`.

### Port toqquşmaları

`PortAllocator` konfiqurasiyaları yazmazdan əvvəl geri dönmə interfeysini yoxlayır. Əgər görürsənsə
`failed to allocate Torii port` və ya `failed to allocate P2P port`, başqa
proses artıq standart diapazonda dinləyir (8080/1337). MOCHI-ni yenidən işə salın
aydın əsaslarla:

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

Qurucu bu bazalardan ardıcıl portları çıxaracaq, buna görə də bir sıra rezerv edin
əvvəlcədən təyin etdiyiniz üçün ölçülü (`peer_count` həmyaşıdları → `peer_count` daşıma portları).

### Yaradılış və saxlama korrupsiyasıKagami manifest yaymazdan əvvəl çıxsa, həmyaşıdlar dərhal çökəcək. Yoxlayın
Məlumat kökü daxilində `genesis/*.json`/`.toml`. ilə yenidən qaçın
`--kagami /path/to/kagami` və ya sağ binarda **Parametrlər** dialoqunu göstərin.
Yaddaş pozğunluğu üçün Baxım bölməsinin **Silin və yenidən yaradılış**-dan istifadə edin.
qovluqları əl ilə silmək əvəzinə düymə (aşağıda əhatə olunmuşdur); yenidən yaradır
prosesləri yenidən başlatmazdan əvvəl peer qovluqları və snapshot kökləri.

### Avtomatik yenidən işə salma

`config/local.toml`-də `[supervisor.restart]` (və ya CLI bayraqları)
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) nə qədər tez-tez nəzarət edir
nəzarətçi uğursuz həmyaşıdlarına yenidən cəhd edir. UI-yə ehtiyacınız olduqda `mode = "never"`-i təyin edin
ilk nasazlığı dərhal üzə çıxarın və ya `max_restarts`/`backoff_ms` qısaldın
tez uğursuz olmalı olan CI işləri üçün yenidən cəhd pəncərəsini bərkitmək.

## 4. Həmyaşıdları təhlükəsiz şəkildə sıfırlamaq

1. Təsirə məruz qalan həmyaşıdları İdarə Panelindən dayandırın və ya UI-dən çıxın. Nəzarətçi
   həmyaşıd işləyərkən yaddaşı silməkdən imtina edir (`PeerHandle::wipe_storage`
   qaytarır `PeerStillRunning`).
2. **Maintenance → Wipe & re-genesis** bölməsinə keçin. MOCHI edəcək:
   - `peers/<alias>/storage` silin;
   - `genesis/` altında konfiqurasiyaları/genezisi yenidən qurmaq üçün Kagami-i yenidən işə salın; və
   - qorunub saxlanılan CLI/mühit ləğvetmələri ilə həmyaşıdları yenidən başladın.
3. Bunu əl ilə etməlisinizsə:
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   Daha sonra MOCHI-ni yenidən başladın ki, `NetworkPaths::ensure` ağacı yenidən yaradır.

`snapshots/<timestamp>` qovluğunu silməzdən əvvəl, hətta yerli olsa belə, həmişə arxivləşdirin
inkişaf - bu paketlər lazım olan dəqiq `irohad` qeydləri və konfiqurasiyaları çəkir
səhvləri çoxaltmaq üçün.

### 4.1 Ani görüntülərdən bərpa

Təcrübə yaddaşı pozduqda və ya məlum olan yaxşı vəziyyəti təkrarlamağınız lazım olduqda, Baxımdan istifadə edin
Kopyalamaq əvəzinə dialoqun **Şotunu bərpa et** düyməsini (və ya `Supervisor::restore_snapshot` nömrəsinə zəng edin)
qovluqları əl ilə. Paketə mütləq yol və ya təmizlənmiş qovluq adını göstərin
`snapshots/` altında. Nəzarətçi:

1. qaçan həmyaşıdları dayandırmaq;
2. snapşotun `metadata.json`-in cari `chain_id` və həmyaşıdların sayına uyğun olduğunu yoxlayın;
3. `peers/<alias>/{storage,snapshot,config.toml,latest.log}`-i yenidən aktiv profilə köçürün; və
4. `genesis/genesis.json`-i bərpa etmədən əvvəl həmyaşıdları işə salınıbsa.

Snapshot başqa bir ilkin təyin və ya zəncir identifikatoru üçün yaradılıbsa, bərpa çağırışı a qaytarır
`SupervisorError::Config`, beləliklə, artefaktları səssizcə qarışdırmaq əvəzinə uyğun bir paket əldə edə bilərsiniz.
Bərpa təlimlərini sürətləndirmək üçün hər bir ilkin təyin üçün ən azı bir təzə şəkil saxlayın.

## 5. Blok/hadisə/status axınlarının təmiri- **Yayım dayanıb, lakin həmyaşıdları sağlamdır.** **Tədbirlər**/**Bloklar** panellərini yoxlayın
  qırmızı status panelləri üçün. İdarə olunan axını məcbur etmək üçün "Dayan", sonra "Başla" üzərinə klikləyin
  yenidən abunə olmaq; nəzarətçi hər bir yenidən qoşulma cəhdini qeyd edir (peer ləqəb və
  səhv) beləliklə, geri çəkilmə mərhələlərini təsdiqləyə bilərsiniz.
- **Status örtüyü köhnəlib.** `ManagedStatusStream` hər dəfə `/status` sorğuları keçir
  iki saniyə və `STATUS_POLL_INTERVAL *-dan sonra datanın köhnəldiyini qeyd edir
  STATUS_STALE_MULTIPLIER` (defolt altı saniyə). Nişan qırmızı qalırsa, yoxlayın
  Peer konfiqurasiyasında `torii_status_url` və şlüz və ya VPN-nin olmadığından əmin olun
  geri dönmə əlaqələrini bloklayır.
- **Hadisə deşifrə xətaları.** UI deşifrə mərhələsini çap edir (xam baytlar,
  `BlockSummary`, və ya Norito deşifrə) və pozucu əməliyyat hash. İxrac
  hadisəni panoya düyməsi vasitəsilə köçürün ki, testlərdə deşifrəni təkrar edə biləsiniz
  (`mochi-core` altında köməkçi konstruktorları ifşa edir
  `mochi/mochi-core/src/torii.rs`).

Axınlar dəfələrlə qəzaya uğradıqda, problemi dəqiq həmyaşıd ləqəbi ilə yeniləyin və
xəta sətri (`ToriiErrorKind`) beləliklə, yol xəritəsi telemetriya mərhələləri bağlı qalır
konkret sübutlara.
---
lang: az
direction: ltr
source: docs/amx.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f11f0a83efc46035aeeaf4c1ad626a2a773303e9dfab188704016cf483a78ce6
source_last_modified: "2026-01-23T08:31:38.611123+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AMX İcra və Əməliyyatlar Bələdçisi

**Status:** Qaralama (NX-17)  
**Auditoriya:** Əsas protokol, AMX/konsensus mühəndisləri, SRE/Telemetri, SDK və Torii komandaları  
**Kontekst:** “Sənədləşdirmə (sahibi: Sənədlər) – `docs/amx.md`-i vaxt diaqramları, xəta kataloqu, operator gözləntiləri və PVO-ların yaradılması/istifadəsi üçün tərtibatçı təlimatı ilə yeniləyin.”【roadmap.md:2497” yol xəritəsi elementini tamamlayır.

## Xülasə

Atomlar arası məlumat məkanı əməliyyatları (AMX) bir təqdimata birdən çox məlumat məkanına (DS) toxunmağa imkan verir, eyni zamanda 1s slotunun sonluğunu, deterministik uğursuzluq kodlarını və şəxsi DS fraqmentləri üçün məxfiliyi qoruyur. Bu bələdçi zamanlama modelini, kanonik səhvlərin idarə edilməsini, operator sübut tələblərini və Proof Doğrulama Obyektləri (PVO) üçün tərtibatçının gözləntilərini əks etdirir, beləliklə, çatdırıla bilən yol xəritəsi Nexus dizayn sənədindən (`docs/source/nexus.md`) kənarda qalır.

Əsas zəmanətlər:

- Hər bir AMX təqdimatı deterministik büdcə hazırlamaq/qoşmaq üçün büdcələri alır; asma zolaqlardan daha çox sənədləşdirilmiş kodlarla ləğvi aşır.
- Büdcəni əldən verən DA nümunələri çatışmayan mövcudluq sübutu kimi qeyd olunur və əməliyyat ötürmə qabiliyyətini səssizcə dayandırmaq əvəzinə növbəti slot üçün növbədə qalır.
- Proof Doğrulama Obyektləri (PVO) müştərilərə/batcherlərə nexus hostunun yuvada tez təsdiq etdiyi artefaktları əvvəlcədən qeydiyyatdan keçirməyə imkan verməklə, ağır sübutları 1-lər yuvasından ayırır.
- IVM hostları Kosmik Kataloqdan hər bir verilənlər məkanı üçün AXT siyasətini əldə edir: tutacaqlar kataloqda reklam edilən zolağı hədəf almalı, ən son manifest kökünü təqdim etməli, expiry_slot, handle_era və sub_nonce minimalarını təmin etməli və exiry_slot, handle_era və sub_nonce minimalarını təmin etməli və ex00000ec ilə naməlum məlumat məkanlarını I00000ec əvvəli rədd etməlidir.
- Yuvanın istifadə müddəti `nexus.axt.slot_length_ms` (defolt `1`ms, `1`ms və `600_000`ms arasında təsdiqlənir) üstəgəl məhdudlaşdırılmış `nexus.axt.max_clock_skew_ms` (defolt olaraq I040ms uzunluqlu I010) istifadə edir və `60_000`ms). Hostlar `current_slot = block.creation_time_ms / slot_length_ms` hesablayır, müddəti bitmiş yoxlamaları sübut etmək və idarə etmək üçün əyilmə ehtiyatını tətbiq edir və konfiqurasiya edilmiş limitdən daha böyük əyriliyi reklam edən tutacaqları rədd edir.
- Sübut önbelleği TTL yenidən istifadəsini məhdudlaşdırır: `nexus.axt.proof_cache_ttl_slots` (defolt `1`, təsdiqlənmiş `1`–`64`) qəbul edilmiş və ya rədd edilmiş sübutların host keşində nə qədər qalacağını məhdudlaşdırır; TTL pəncərəsi və ya sübutun `expiry_slot` bitdikdən sonra girişlər düşür, beləliklə təkrar oxutmadan qorunma məhdud qalır.
- Replay kitabçasının saxlanması: `nexus.axt.replay_retention_slots` (defolt `128`, təsdiqlənmiş `1`–`4_096`) həmyaşıdlar/restartlar arasında təkrar oxutmadan imtina üçün saxlanılan tutacaqdan istifadə tarixinin minimum yuva pəncərəsini təyin edir; onu operatorların verəcəyini gözlədiyiniz ən uzun tutacaq etibarlılıq pəncərəsi ilə uyğunlaşdırın. Kitab WSV-də saxlanılır, işə salındıqda nəmləndirilir və həm saxlama pəncərəsi, həm də tutacaqların istifadə müddəti bitdikdən sonra (hansı daha sonra olur) müəyyən şəkildə budanır, beləliklə, həmyaşıd açarları təkrar oynatma boşluqlarını açmasın.
- Sazlama keş statusu: Torii cari AXT siyasətinin snapshot versiyasını, ən son rəddi (zolaq/səbəb/versiya), keşlənmiş sübutları (dataspace/statuslotsmanifest və reject/hiper) qaytarmaq üçün Torii (telemetri/developer qapısı) ifşa edir. (`next_min_handle_era`/`next_min_sub_nonce`). Slot/manifest fırlanmalarının keş vəziyyətində əks olunduğunu təsdiqləmək və nasazlıqların aradan qaldırılması zamanı tutacaqları qəti şəkildə yeniləmək üçün bu son nöqtədən istifadə edin.

## Slot Zamanlama Modeli

### Taymlayn

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- Büdcələr qlobal mühasibat planına uyğundur: mempool 70ms, DA commit ≤300ms, consensus 300ms, IVM/AMX 250ms, qəsəbə 40ms, mühafizə 40ms.【roadmap.md:2529
- DA pəncərəsini pozan əməliyyatlar çatışmayan mövcudluq sübutu kimi qeyd olunur və növbəti yuvada yenidən sınaqdan keçirilir; `AMX_TIMEOUT` və ya `SETTLEMENT_ROUTER_UNAVAILABLE` kimi bütün digər pozuntular səth kodları.
- Mühafizə dilimi telemetriya ixracını və yekun auditi udur, beləliklə, ixracatçılar qısa müddətə geciksə belə, yuva hələ də 1 saniyədə bağlanır.
- Konfiqurasiya məsləhətləri: defoltlar son istifadə müddətini ciddi saxlayır (`slot_length_ms = 1`, `max_clock_skew_ms = 0`). 1s kadans dəsti üçün `slot_length_ms = 1_000` və `max_clock_skew_ms = 250`; 2 saniyəlik kadans üçün `slot_length_ms = 2_000` və `max_clock_skew_ms = 500` istifadə edin. Təsdiqlənmiş pəncərədən kənar dəyərlər (`1`–`600_000`ms və ya `max_clock_skew_ms` slot uzunluğundan/`60_000`ms) konfiqurasiya təhlili zamanı rədd edilir və elan edilmiş əyilmə sapı daxilində qalmalıdır.

### Cross-DS üzgüçülük zolağı

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

Hər bir DS fraqmenti, zolaq yuvanı yığışdırmazdan əvvəl 30 ms hazırlıq pəncərəsini bitirməlidir. Çatışmayan sübutlar həmyaşıdları bloklamaq əvəzinə növbəti yuva üçün yaddaşda qalır.

### Alətlərin yoxlanış siyahısı

| Metrik / İz | Mənbə | SLO / Xəbərdarlıq | Qeydlər |
|----------------|--------|-------------|-------|
| `iroha_slot_duration_ms` (histoqram) / `iroha_slot_duration_ms_latest` (ölçü) | `iroha_telemetry` | p95 ≤ 1000ms | Ci qapısı `ans3.md`-də təsvir edilmişdir. |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (qoşmaq çəngəl) | ≥0,95 hər 30 dəqiqəlik pəncərə | Çatışmayan mövcudluq telemetriyasından əldə edilmişdir, beləliklə hər blok ölçməni yeniləyir (`crates/iroha_core/src/telemetry.rs:3524`, `crates/iroha_core/src/telemetry.rs:4558`). |
| `iroha_amx_prepare_ms` | IVM host | p95 ≤ 30ms hər DS əhatəsi | Sürücülər `AMX_TIMEOUT` dayandırılır. |
| `iroha_amx_commit_ms` | IVM host | p95 ≤ 40ms hər DS əhatəsi | Delta birləşmə + trigger icrasını əhatə edir. |
| `iroha_ivm_exec_ms` | IVM host | Hər zolaqda >250ms olduqda xəbərdarlıq | IVM üst-üstə düşən yığın icra pəncərəsini əks etdirir. |
| `iroha_amx_abort_total{stage}` | icraçı | >0,05 abort/slot və ya davamlı təkpilləli sıçrayışlar | Səhnə etiketləri: `prepare`, `exec`, `commit`. |
| `iroha_amx_lock_conflicts_total` | AMX planlaşdırıcı | >0,1 konflikt/yuva | Qeyri-dəqiq R/W dəstlərini göstərir. |
| `iroha_axt_policy_reject_total{lane,reason}` | IVM host | Sünbüllərə baxın | Manifest/lane/era/sub_nonce/expiry rəddlərini fərqləndirir. |
| `iroha_axt_policy_snapshot_cache_events_total{event}` | IVM host | cache_miss-i yalnız başlanğıcda/manifest dəyişikliyində gözləyin | Davamlı itkilər köhnə siyasətin nəmləndirilməsini göstərir. |
| `iroha_axt_proof_cache_events_total{event}` | IVM host | Əsasən gözləyin `hit`/`miss` | `reject`/`expired` sünbüllər adətən açıq-aşkar sürüşmə və ya köhnəlmiş sübutları göstərir. |
| `iroha_axt_proof_cache_state{dsid,status,manifest_root_hex,verified_slot}` | IVM host | Keşlənmiş sübutları yoxlayın | Ölçmə dəyəri keşlənmiş sübut üçün expiry_slot-dur (əyrilik tətbiq edilməklə). |
| Çatışmayan mövcudluq sübutu (`sumeragi_da_gate_block_total{reason="missing_local_data"}`) | Zolaqlı telemetriya | DS üçün tx-in >5%-dən çox olduğu halda xəbərdarlıq Təsdiq edənlərin və ya sübutların geridə qalması deməkdir. |

`/v1/debug/axt/cache` operatorlar üçün hər bir verilənlər məkanı snapshotı (status, manifest kökü, yoxlanılmış/keçmə vaxtı) ilə `iroha_axt_proof_cache_state` göstəricisini əks etdirir.

`iroha_amx_commit_ms` və `iroha_ivm_exec_ms` eyni gecikmə kovalarını paylaşır
`iroha_amx_prepare_ms`. Abort sayğacı hər bir rədd cavabını zolaq identifikatoru ilə qeyd edir
və mərhələ (`prepare` = üst-üstə düşmə/təsdiqləmə, `exec` = IVM yığın icrası,
`commit` = delta birləşmə + tətik təkrarı) beləliklə telemetriya olub olmadığını vurğulaya bilər.
mübahisə oxu/yazma uyğunsuzluqlarından və ya post-state birləşmələrindən irəli gəlir.

Operatorlar bu ölçüləri `status.md`-də slot qəbulu sübutları və qeyd reqressiyaları ilə birlikdə audit üçün arxivləşdirməlidirlər.

### AXT qızıl armaturları

`crates/iroha_data_model/tests/axt_policy_vectors.rs` (`print_golden_vectors`) regenerasiya köməkçisi ilə `crates/iroha_data_model/tests/fixtures/axt_golden.rs`-də deskriptor/tutacaq/siyasət snapshot üçün Norito qurğuları canlıdır. CoreHost eyni qurğuları `core_host_enforces_fixture_snapshot_fields` (`crates/ivm/tests/core_host_policy.rs`) zolağında bağlama, manifest kök uyğunluğu, bitmə_yuvasının təzəliyi, handle_era/sub_nonce minimumu və itkin məlumat məkanı rədd edilməsi üçün istifadə edir.
- Çox məlumat məkanı JSON qurğusu (`crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json`) deskriptor/toxunma sxemini, kanonik Norito baytlarını və Poseidon bağlamasını (`compute_descriptor_binding`) bağlayır. `axt_descriptor_fixture` testi kodlanmış baytları qoruyur və SDK-lar sənədlər/SDK-lar üçün deterministik nümunələri toplamaq üçün `AxtDescriptorBuilder::builder` plus `TouchManifest::from_read_write`-dən istifadə edə bilər.

### Zolaq kataloqunun xəritələşdirilməsi və manifestləri

- AXT siyasət snapshotları Space Directory manifest dəsti və zolaq kataloqundan qurulub. Hər bir məlumat məkanı konfiqurasiya edilmiş zolağa uyğunlaşdırılır; aktiv manifestlər manifest hashına, aktivasiya dövrünə (`min_handle_era`) və sub-nance mərtəbəsinə töhfə verir. Aktiv manifest olmayan UAID bağlamaları hələ də sıfırlanmış manifest kökü ilə siyasət girişi yayır, beləliklə zolaqlı keçid real manifest yerə düşənə qədər aktiv qalır.
- Snapşotdakı `current_slot` ən son bağlanmış blok vaxt damğasından (`creation_time_ms / slot_length_ms`) əldə edilib və yalnız bağlanmış başlıq mövcud olana qədər blok hündürlüyünə qayıdır.
- Telemetriya nəmlənmiş snapshotı `iroha_axt_policy_snapshot_version` (Norito ilə kodlanmış snapshot hash-in aşağı 64 biti) və `iroha_axt_policy_snapshot_cache_events_total{event=cache_hit|cache_miss}` vasitəsilə keş hadisələri kimi səthə gətirir. Rədd sayğacları `lane`, `manifest`, `era`, `sub_nonce` və `expiry` etiketlərindən istifadə edir ki, operatorlar hansı sahənin sapı blokladığını dərhal görə bilsinlər.

### Çarpaz verilənlər məkanının birləşdirilməsinə nəzarət siyahısı- Kosmik Kataloqda sadalanan hər bir məlumat məkanının zolaqlı girişə və aktiv manifestə malik olduğunu təsdiqləyin; fırlanma yeni tutacaqlar verməzdən əvvəl bağlamaları yeniləməli və kökləri göstərməlidir. Sıfırlanmış köklər manifestlər mövcud olana qədər tutacaqların rədd ediləcəyini bildirir və hostlar/blok doğrulaması indi sıfırlanmış manifest kökləri təqdim edən tutacaqları rədd edir.
- Başlanğıcda və Kosmik Kataloq dəyişikliklərindən sonra siyasət snapshot metrikasında sabit `cache_hit` hadisələrinin ardınca bir `cache_miss` gözləyin; davamlı qaçırma dərəcəsi köhnəlmiş və ya çatışmayan manifest lentinə işarə edir.
- Dəstək rədd edildikdə, yenilənmiş tutacaq tələb etmək (`expiry`/`era`/`sub_nonce`) və ya zolaq/manifest bağlamasını təmir etmək barədə qərar vermək üçün `iroha_axt_policy_reject_total{lane,reason}` və snapshot versiyasına baxın. (`lane`/`manifest`). Torii sazlama son nöqtəsi `/v1/debug/axt/cache` həmçinin `reject_hints`-i `dataspace`, `target_lane`, `next_min_handle_era` və I18NI0010 operatoru ilə qaytarır. siyasət zərbəsindən sonra.

### SDK nümunəsi: token çıxışı olmadan uzaqdan xərcləmə

1. Aktivə sahib olan məlumat məkanını və yerli olaraq tələb olunan oxu/yazma toxunuşlarını sadalayan AXT deskriptorunu yaradın; deskriptoru deterministik saxlayın ki, bağlama hash sabit qalsın.
2. Gözlədiyiniz manifest görünüşü ilə uzaq məlumat məkanı üçün `AXT_TOUCH` nömrəsinə zəng edin; isteğe bağlı olaraq, ev sahibi bunu tələb edərsə, `AXT_VERIFY_DS_PROOF` vasitəsilə sübut əlavə edin.
3. Aktiv idarəsini tələb edin və ya yeniləyin və uzaq data məkanında sərf edən `RemoteSpendIntent` ilə `AXT_USE_ASSET_HANDLE`-i çağırın (körpü ayağı yoxdur). Büdcənin icrası yuxarıda təsvir edilən şəkilə qarşı idarənin `remaining`, `per_use`, `sub_nonce`, `handle_era` və `expiry_slot` istifadə edir.
4. `AXT_COMMIT` vasitəsilə həyata keçirin; əgər host `PermissionDenied`-i qaytarırsa, yeni tutacaq əldə etmək (sub_nonce/era) və ya manifest/zolaq bağlamasını düzəltmək barədə qərar vermək üçün rədd etiketindən istifadə edin.

## Operatorun gözləntiləri

1. **Slot öncəsi hazırlıq**
   - Hər profil üzrə DA attestator hovuzlarının (A=12, B=9, C=7) sağlam olmasını təmin edin; attester çaxması slot üçün Space Directory snapshot-da qeydə alınır.
   - Yeni iş yükü mikslərini işə salmazdan əvvəl `iroha_amx_prepare_ms`-in təmsilçi qaçışçılar üçün büdcədən aşağı olduğunu təsdiqləyin.

2. **Yuvadaxili monitorinq**
   - Çatışmayan mövcudluq sıçrayışları (ardıcıl iki slot üçün >5%) və `AMX_TIMEOUT`-də xəbərdarlıq, çünki hər ikisi buraxılmış büdcələri göstərir.
   - Yoldan kənar yoxlamanın təqdimatlarla davam etdiyini sübut etmək üçün PVO keşinin istifadəsini izləyin (`iroha_pvo_cache_hit_ratio`, sübut xidməti tərəfindən ixrac olunur).

3. **Dəlillərin tutulması**
   - `status.md`-dən istinad edilən gecə artefakt paketinə DA qəbz dəstlərini əlavə edin, AMX histoqramlar hazırlayır və PVO keş hesabatları.
   - `ops/drill-log.md`-də DA titrəməsi, oracle dayanacaqları və ya bufer tükənməsi testləri işlədikdə xaos qazma nəticələrini qeyd edin.

4. **Runbook texniki xidməti**
   - AMX xəta kodları və ya ləğvetmələr dəyişdikdə Android/Swift SDK runbook-larını yeniləyin ki, müştəri komandaları deterministik uğursuzluq semantikasını miras alsınlar.
   - Konfiqurasiya fraqmentlərini (məsələn, `iroha_config.amx.*`) `docs/source/nexus.md`-dəki kanonik parametrlərlə sinxronlaşdırın.

## Telemetriya və Problemlərin aradan qaldırılması

### Telemetriya sürətli arayışı

| Mənbə | Nə çəkmək | Komanda / Yol | Sübut gözləntiləri |
|--------|-----------------|----------------|-----------------------|
| Prometheus (`iroha_telemetry`) | Yuva və AMX SLO-ları: `iroha_slot_duration_ms`, `iroha_amx_prepare_ms`, `iroha_amx_commit_ms`, `iroha_da_quorum_ratio`, `iroha_amx_abort_total{stage}` | `https://$TORII/telemetry/metrics`-ni sıyırın və ya `docs/source/telemetry.md`-də təsvir olunan tablosundan ixrac edin. | Auditorların p95/p99 dəyərlərini və xəbərdarlıq vəziyyətlərini görə bilməsi üçün gecə `status.md` qeydinə histoqram şəkillərini (və işə salındıqda xəbərdarlıq tarixçəsini) əlavə edin. |
| Torii RBC anlık görüntüləri | DA/RBC yığılması: sessiya başına yığın yığını, baxış/yüksəklik metadatası və DA əlçatanlıq sayğacları (`sumeragi_da_gate_block_total{reason="missing_local_data"}`; `sumeragi_rbc_da_reschedule_total` mirasdır). | `GET /v1/sumeragi/rbc` və `GET /v1/sumeragi/rbc/sessions` (nümunələr üçün bax `docs/source/samples/sumeragi_rbc_status.md`). | AMX DA yanğın xəbərdarlığı zamanı JSON cavablarını (zaman işarələri ilə) saxlayın; onları hadisə paketinə daxil edin ki, rəyçilər əks təzyiqin telemetriyaya uyğun olduğunu təsdiq etsinlər. |
| Sübut xidməti ölçüləri | PVO keş sağlamlığı: `iroha_pvo_cache_hit_ratio`, keş doldurma/çıxarma sayğacları, növbə dərinliyini sübut | `GET /metrics` sübut xidmətində (`IROHA_PVO_METRICS_URL`) və ya paylaşılan OTLP kollektoru vasitəsilə. | AMX slot ölçüləri ilə yanaşı keşin vurma nisbətini və növbə dərinliyini ixrac edin ki, yol xəritəsi OA/PVO qapısının deterministik artefaktları olsun. |
| Qəbul kəməri | Nəzarət olunan titrəmə altında uçdan-uca qarışıq yük (slot/DA/RBC/PVO) | `ci/acceptance/slot_1s.yml`-i (və ya CI-də eyni işi) yenidən işə salın və log paketini + yaradılan artefaktları `artifacts/acceptance/slot_1s/<timestamp>/`-də arxivləşdirin. | GA-dan əvvəl və kardiostimulyator/DA parametrləri dəyişdikdə tələb olunur; YAML icra xülasəsini və operatorun təhvil vermə paketinə Prometheus anlıq görüntülərini daxil edin. |

### Problemlərin aradan qaldırılması

| Simptom | Əvvəlcə yoxlayın | Tövsiyə olunan remediasiya |
|---------|---------------|--------------------------|
| `iroha_slot_duration_ms` p95 1000ms-dən yuxarı sürünür | `/telemetry/metrics`-dən Prometheus ixracı və DA təxirə salınmalarını təsdiqləmək üçün ən son `/v1/sumeragi/rbc` snapshot; son `ci/acceptance/slot_1s.yml` artefaktı ilə müqayisə edin. | AMX toplu ölçülərini azaldın və ya əlavə RBC kollektorlarını işə salın (`sumeragi.collectors.k`), sonra qəbul kəmərini yenidən işə salın və yeni telemetriya sübutunu əldə edin. |
| Çatışmayan mövcudluq sıçrayışı | `/v1/sumeragi/rbc/sessions` geri qalan sahələr (`lane_backlog`, `dataspace_backlog`) attestatorun sağlamlıq panelləri ilə yanaşı. | Sağlam olmayan sertifikatları silin, çatdırılmanı sürətləndirmək üçün müvəqqəti olaraq `redundant_send_r`-i artırın və `status.md`-də düzəliş qeydlərini dərc edin. Ardıcıllıq aradan qaldırıldıqdan sonra yenilənmiş RBC snapshotlarını əlavə edin. |
| Qəbzlərdə tez-tez `PVO_MISSING_OR_EXPIRED` | Sübut xidməti keş ölçüləri + emitentin PVO planlaşdırıcı qeydləri. | Köhnə PVO artefaktlarını bərpa edin, fırlanma kadansını qısaldın və hər bir SDK-nın `expiry_slot`-dən əvvəl qolu təzələməsinə əmin olun. Bərpa edilmiş keşi sübut etmək üçün sübut-xidmət göstəricilərini sübut paketinə daxil edin. |
| Təkrarlanan `AMX_LOCK_CONFLICT` və ya `AMX_TIMEOUT` | `iroha_amx_lock_conflicts_total`, `iroha_amx_prepare_ms` və təsirə məruz qalan əməliyyat təzahür edir. | Norito statik analizatorunu yenidən işə salın, oxu/yazma seçicilərini düzəldin (yaxud partiyanı bölün) və yenilənmiş manifest qurğularını dərc edin ki, münaqişə sayğacı ilkin vəziyyətə qayıtsın. |
| `SETTLEMENT_ROUTER_UNAVAILABLE` xəbərdarlıqları | Hesablaşma marşrutlaşdırıcısı qeydləri (`docs/settlement-router.md`), xəzinə buferinin idarə panelləri və təsirlənmiş qəbzlər. | XOR buferlərini doldurun və ya zolağı yalnız XOR rejiminə çevirin, xəzinə əməliyyatını sənədləşdirin və hesablaşmanın davam etdirildiyini sübut etmək üçün yuva qəbulu testini yenidən həyata keçirin. |

### AXT rədd siqnalları

- Səbəb kodları `AxtRejectReason` (`lane`, `manifest`, `era`, `sub_nonce`, `expiry`, `expiry`, I010X, I010X, I0108X, `AxtRejectReason` kimi götürülür. `policy_denied`, `proof`, `budget`, `replay_cache`, `descriptor`, `duplicate`). Blok təsdiqləməsi indi `AxtEnvelopeValidationFailed { message, reason, snapshot_version }`-i üzə çıxarır, beləliklə, insidentlər imtinanı xüsusi siyasət snapshotına bağlaya bilər.
- `/v1/debug/axt/cache` `{ policy_snapshot_version, last_reject, cache, hints }`-i qaytarır, burada `last_reject` ən son hostun rədd edilməsinin zolağı/səbəbi/versiyasını daşıyır və `hints`, `next_min_handle_era`/I0615X/I10601 reflektorunu təmin edir önbelleğe alınmış sübut vəziyyəti.
- Xəbərdarlıq şablonu: `iroha_axt_policy_reject_total{reason="manifest"}` və ya `{reason="expiry"}` 5 dəqiqəlik bir pəncərədən yuxarı qalxdıqda səhifə, `last_reject` snapshot + `policy_snapshot_version`-dən `policy_snapshot_version` əlavə edin və insidentin son nöqtəsini yükləmək üçün tələb edin təkrar cəhd etməzdən əvvəl tutacaqları təzələyin.

## Sübut Doğrulama Obyektləri (PVO)

### Struktur

PVO-lar müştərilərə ağır işləri vaxtından əvvəl sübut etməyə imkan verən Norito kodlu zərflərdir. Kanonik sahələr bunlardır:

| Sahə | Təsvir |
|-------|-------------|
| `circuit_id` | Sübut sistemi/bəyanatı üçün statik identifikator (məsələn, `amx.transfer.v1`). |
| `vk_hash` | DS manifestində istinad edilən doğrulama açarının Blake2b-256 hashı. |
| `proof_digest` | Yuvadan kənar PVO reyestrində saxlanılan seriallaşdırılmış sübut yükünün Poseidon həzmi. |
| `max_k` | AIR domenində yuxarı sərhəd; hostlar reklam edilən ölçüdən artıq sübutları rədd edir. |
| `expiry_slot` | Artefaktın etibarsız olduğu yuvanın hündürlüyü; köhnə sübutları zolaqlardan kənarda saxlayır. |
| `profile` | Planlaşdırıcılara profili paylaşan sübutları toplu etməyə kömək etmək üçün əlavə göstəriş (məsələn, DS profili A/B/C). |

Norito sxemi `crates/iroha_data_model/src/nexus`-də verilənlər modeli təriflərinin yanında yaşayır ki, SDK-lar onu serde olmadan əldə edə bilsinlər.

### Nəsil boru kəməri1. **Sxem metadatasını tərtib edin** — `circuit_id`, doğrulama açarı və prover quruluşunuzdan maksimum iz ölçüsünü ixrac edin (adətən `fastpq_prover` hesabatları vasitəsilə).
2. **Dəlil artefaktlar hazırlayın** — Slotdan kənar proveri işə salın və tam transkriptləri və öhdəlikləri saxlayın.
3. **Sənəd xidməti vasitəsilə qeydiyyatdan keçin** — Norito PVO faydalı yükünü yuvadan kənar yoxlayıcıya təqdim edin (yol xəritəsi NX-17 sübut boru xəttinə baxın). Xidmət bir dəfə yoxlayır, həzmi bərkidir və sapı Torii vasitəsilə ifşa edir.
4. **Əməliyyatlarda istinad** — PVO sapını AMX qurucularına (`amx_touch` və ya daha yüksək səviyyəli SDK köməkçiləri) əlavə edin. Hostlar həzmə baxır, keşlənmiş nəticəni yoxlayır və yalnız keş soyuqdursa, slotun daxilində yenidən hesablayır.
5. **Müddəti bitəndə fırladın** — SDK-lar `expiry_slot`-dən əvvəl hər hansı bir keşlənmiş qolu təzələməlidir. İstifadə müddəti bitmiş obyektlər `PVO_MISSING_OR_EXPIRED`-ni işə salır.

### Developer yoxlama siyahısı

- Oxuma/yazma dəstlərini dəqiq bəyan edin ki, AMX kilidləri əvvəlcədən götürə bilsin və `AMX_LOCK_CONFLICT`-dən qaçsın.
- Çapraz DS köçürmələri tənzimlənən DS-lərə toxunduqda eyni UAID manifest yeniləməsində deterministik müavinət sübutlarını yığın.
- Yenidən cəhd strategiyası: mövcudluq sübutu yoxdur → hərəkət yoxdur (tx yaddaşda qalır); `AMX_TIMEOUT` və ya `PVO_MISSING_OR_EXPIRED` → artefaktları yenidən qurun və eksponent olaraq geri çəkilin.
- Determinizm reqressiyalarından qorunmaq üçün testlər həm önbelleğe vurmaları, həm də soyuq başlanğıcları (ev sahibini eyni `max_k` ilə sübutu yoxlamağa məcbur etmək) daxil etməlidir.
- Sübut blokları (`ProofBlob`) `AxtProofEnvelope { dsid, manifest_root, da_commitment?, proof }` kodlamalıdır; hostlar `iroha_axt_proof_cache_events_total{event="hit|miss|expired|reject|cleared"}` ilə hər bir verilənlər məkanı/yuvası üçün Kosmik Kataloq manifest kökü və keş keçmə/uğursuzluq nəticələrinə bağlayır. İstifadə müddəti bitmiş və ya açıq-aşkar uyğun olmayan artefaktlar keşlənmiş `reject`-də eyni yuva qısa qapanmasında törədilməzdən və sonrakı təkrar cəhdlərdən əvvəl rədd edilir.
- Proof keşinin təkrar istifadəsi slot əhatəlidir: təsdiqlənmiş sübutlar eyni yuvada olan zərflər arasında isti qalır və yuva irəlilədikdə avtomatik olaraq çıxarılır, beləliklə təkrar cəhdlər determinist qalır.

### Statik oxu/yazma analizatoru

Kompilyasiya vaxtı seçiciləri AMX-dən əvvəl müqavilənin faktiki davranışına uyğun olmalıdır
kilidləri əvvəlcədən gətirin və ya UAID manifestlərini tətbiq edin. Yeni `ivm::analysis` modulu
(`crates/ivm/src/analysis.rs`) kodu deşifrə edən `analyze_program(&[u8])`-i ifşa edir.
`.to` artefakt, oxunuşların/yazmaların, yaddaş əməliyyatlarının və sistem zənglərinin istifadəsini qeyd edir,
və SDK manifestlərinin yerləşdirə biləcəyi JSON-a uyğun hesabat hazırlayır. Çalıştır
UAID-ləri dərc edərkən `koto_lint` ilə yanaşı yaradılan R/W xülasəsi
NX-17 hazırlığının nəzərdən keçirilməsi zamanı istinad edilən sübut paketində ələ keçirilib.

## Space Directory siyasətinin tətbiqi

AXT sapı yoxlanışı indi hostun ona girişi olduqda Kosmik Kataloq snapshotına defolt edilir (testlərdə CoreHost, inteqrasiya axınlarında WsvHost). Hər bir verilənlər məkanı siyasəti qeydləri `manifest_root`, `target_lane`, `min_handle_era`, `min_sub_nonce` və `current_slot`-ni daşıyır. Hostlar tətbiq edir:

- zolaqlı bağlama: tutacaq `target_lane` Space Directory girişinə uyğun olmalıdır;
- manifest bağlama: sıfırdan fərqli `manifest_root` dəyərlər tutacaqın `manifest_view_root` ilə uyğun olmalıdır;
- istifadə müddəti: `current_slot` tutacaqdan daha böyük `expiry_slot` rədd edildi;
- sayğaclar: `handle_era` və `sub_nonce` ən azı reklam edilən minimum olmalıdır;
- üzvlük: snapshotda olmayan məlumat məkanları üçün tutacaqlar rədd edilir.

Uğursuzluqlar `PermissionDenied`-ə uyğunlaşdırılır və `crates/ivm/tests/core_host_policy.rs`-də CoreHost siyasət snapshot testləri hər sahə üçün hallara icazə verir/inkar edir.
Blok doğrulaması, həmçinin siyasət yuvasını əhatə edən (konfiqurasiya edilmiş əyilmə icazəsi ilə) və tutacaqdan əvvəl bitməyən `expiry_slot` ilə hər bir məlumat məkanı üçün boş olmayan sübutlar tələb edir, deskriptorun bağlanmasını və elan edilmiş xüsusiyyətlər üçün toxunma manifestlərini tətbiq edir (və prefiksdən kənar olanları rədd edir), məbləğlər, əhatə dairəsi/mövzunun uyğunlaşdırılması və sıfırdan fərqli dövr/sub_nonce/spiry), aqreqatlar verilənlər məkanı üçün büdcələri idarə edir və zərflər yerinə yetirildiyi üçün `min_handle_era`/`min_sub_nonce` irəliləyişlər, belə ki, təkrar oxunan alt-qeydiyyatlar direktordan sonra da rədd edilir.

## Xəta Kataloqu

Kanonik kodlar `crates/iroha_data_model/src/errors.rs`-də yaşayır. Operatorlar onları ölçülər/loqlarda sözbəsöz göstərməlidirlər və SDK-lar onları hərəkətə keçə bilən təkrar cəhdlərə uyğunlaşdırmalıdır.

| Kod | Tətik | Operatorun cavabı | SDK rəhbərliyi |
|------|---------|-------------------|--------------|
| Çatışmayan mövcudluq sübutu (telemetriya) | 300 ms-dən əvvəl təsdiqlənmiş `q` attestator qəbzindən azdır. | Attestatın sağlamlığını yoxlayın, növbəti yuva üçün seçmə parametrlərini genişləndirin, əməliyyatı növbədə saxlayın və runbook sübutları üçün çatışmayan əlçatanlıq sayğaclarını tutun. | Fəaliyyət yoxdur; Yenidən cəhd avtomatik olaraq baş verir, çünki tx növbədə qalır. |
| `DA_DEADLINE_EXCEEDED` | Δ pəncərəsi DA kvorumuna cavab vermədən keçdi. | Təhqir edən attestantları istefa edin, insident qeydini dərc edin, müştəriləri yenidən təqdim etməyə məcbur edin. | Attestantlar geri qayıtdıqdan sonra əməliyyatı yenidən qurun; partiyanın bölünməsini düşünün. |
| `AMX_TIMEOUT` | Kombinə edilmiş hazırlıq/təhsil DS dilimi üçün 250ms-i keçdi. | Flameqrafları çəkin, R/W dəstlərini yoxlayın və `iroha_amx_prepare_ms` ilə müqayisə edin. | Daha kiçik partiya ilə və ya mübahisəni azaltdıqdan sonra yenidən cəhd edin. |
| `AMX_LOCK_CONFLICT` | Host üst-üstə düşən yazma dəstləri və ya siqnalsız toxunuşlar aşkar etdi. | UAID manifestlərini və statik analiz hesabatlarını yoxlayın; Seçicilər yoxdursa, yeniləmə görünür. | Düzəliş edilmiş oxu/yazma bəyannamələri ilə əməliyyatı yenidən tərtib edin. |
| `PVO_MISSING_OR_EXPIRED` | İstinad edilən PVO tutacağı keşdə deyil və ya keçmiş `expiry_slot`. | Sübut xidmətinin qalıqlarını yoxlayın, artefaktı bərpa edin və Torii indekslərini yoxlayın. | Sübut artefaktını yeniləyin və yeni tutacaqla yenidən göndərin. |
| `RWSET_UNBOUNDED` | Statik analiz oxu/yazma seçicisini bağlaya bilmədi. | Yerləşdirmədən imtina edin, seçici yığın izi qeyd edin, təkrar cəhd etməzdən əvvəl tərtibatçıdan düzəliş tələb edin. | Açıq seçiciləri yaymaq üçün müqaviləni yeniləyin. |
| `HEAVY_INSTRUCTION_DISALLOWED` | Müqavilə AMX zolaqlarından (məsələn, PVO olmayan böyük FFT) qadağan edilmiş təlimata istinad etdi. | Yenidən aktivləşdirmədən əvvəl Norito qurucusunun təsdiq edilmiş əməliyyat kodu dəstindən istifadə etdiyinə əmin olun. | İş yükünü bölün və ya əvvəlcədən hesablanmış sübut əlavə edin. |
| `SETTLEMENT_ROUTER_UNAVAILABLE` | Router deterministik çevrilməni hesablaya bilmədi (çatışmayan yol, bufer boşaldı). | Buferləri doldurmaq və ya yalnız XOR rejimini dəyişdirmək üçün Xəzinədarlığı cəlb edin; hesablaşma jurnalında qeyd edin. | Bufer xəbərdarlığı silindikdən sonra yenidən cəhd edin; istifadəçi ilə bağlı xəbərdarlıq göstərin. |

SDK komandaları bu kodları inteqrasiya testlərində əks etdirməlidirlər ki, `iroha_cli`, Android, Swift, JS və Python səthləri xəta mətni və tövsiyə olunan hərəkətlər barədə razılaşsın.

### AXT-dən imtinanın müşahidə oluna bilməsi

- Torii sabit səbəb etiketi, aktiv `snapshot_version`, isteğe bağlı `lane`/I10280X/I10identifiers00 ilə `ValidationFail::AxtReject` (və blok doğrulama `AxtEnvelopeValidationFailed`) kimi siyasət xətalarını üzə çıxarır. `next_min_handle_era`/`next_min_sub_nonce` üçün. SDK-lar bu sahələri istifadəçilərə ötürməlidir ki, köhnə tutacaqlar qəti şəkildə yenilənsin.
- Torii indi də HTTP cavablarını sürətli triaj üçün `X-Iroha-Axt-*` başlıqları ilə möhürləyir: `Code`/`Reason`, `Snapshot-Version`, `Snapshot-Version`, `dataspace`, I000002802 `Next-Handle-Era`/`Next-Sub-Nonce`. ISO körpü rəddləri uyğun `PRTRY:AXT_*` səbəb kodlarını və eyni təfərrüatlı sətirləri daşıyır ki, tablosuna və operatorlara tam yükün şifrəsini açmadan AXT nasazlıq sinfindən əsas xəbərdarlıqlar verə bilsin.
- Hostlar `AXT policy rejection recorded`-i eyni sahələrlə qeyd edir və onları telemetriya vasitəsilə ixrac edir: `iroha_axt_policy_reject_total{lane,reason}` rəddləri hesablayır, `iroha_axt_policy_snapshot_version` isə aktiv görüntünün hashini izləyir. Sübut keş vəziyyəti `/v1/debug/axt/cache` (dataspace/status/manifest root/slots) vasitəsilə əlçatan olaraq qalır.
- Xəbərdarlıq: `reason` tərəfindən qruplaşdırılmış `iroha_axt_policy_reject_total` və `snapshot_version` ilə səhifədə loglardan/ValidationFail-də manifestləri (zolaq/manifest rəddləri) və ya refresh handlesence/refresh handlesance/refresh-i fırlatmaq lazım olduğunu təsdiqləmək üçün `iroha_axt_policy_reject_total`-də sıçrayışlara baxın. İmtinaların keşlə və ya siyasətlə əlaqəli olduğunu təsdiqləmək üçün xəbərdarlıqları sübut-keş son nöqtəsi ilə birləşdirin.

## Sınaq və Sübut

- CI `ci/acceptance/slot_1s.yml` dəstini (30 dəqiqəlik qarışıq iş yükü) işə salmalı və `ans3.md`-də göstərildiyi kimi yuva/DA/telemetri hədləri yerinə yetirilmədikdə birləşə bilmir.
- Xaos təlimləri (attester jitter, oracle stalls, bufer tükənməsi) ən azı rübdə bir dəfə `ops/drill-log.md` altında arxivləşdirilmiş artefaktlarla yerinə yetirilməlidir.
- Status yeniləmələri aşağıdakıları əhatə etməlidir: ən son slot SLO hesabatı, gözlənilməz səhv sıçrayışları və yol xəritəsinin maraqlı tərəflərinin hazırlığı yoxlaya bilməsi üçün ən son PVO keş snapshotına keçid.

Bu təlimata əməl etməklə, ianəçilər AMX sənədləri üçün yol xəritəsi tələbini yerinə yetirir və operatorlara və tərtibatçılara vaxt, telemetriya və PVO iş axınları üçün vahid istinad verirlər.
---
lang: az
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Xaos və Xəta Məşq Planını birləşdirin (IOS3 / IOS7)

Bu oyun kitabı IOS3/IOS7-ni təmin edən təkrarlanan xaos təlimlərini müəyyən edir
yol xəritəsi hərəkəti _“birgə xaos məşqini planlaşdırın”_ (`roadmap.md:1527`). ilə cütləşdirin
Connect preview runbook (`docs/runbooks/connect_session_preview_runbook.md`)
çarpaz SDK demoları hazırlayarkən.

## Məqsədlər və Uğur Meyarları
- Paylaşılan Connect təkrar cəhd/geri-off siyasəti, oflayn növbə limitləri və tətbiq edin
  istehsal kodunu mutasiya etmədən idarə olunan nasazlıqlar altında telemetriya ixracatçıları.
- Deterministik artefaktları çəkin (`iroha connect queue inspect` çıxışı,
  `connect.*` metrika anlıq görüntüləri, Swift/Android/JS SDK qeydləri) beləliklə idarəetmə
  hər məşqi yoxlayın.
- Pul kisələri və dApp-ların konfiqurasiya dəyişikliklərinə hörmət etdiyini sübut edin (manifest sürüşmələri, duz
  fırlanma, attestasiya uğursuzluqları) kanonik `ConnectError` ilə
  kateqoriya və redaksiyaya qarşı təhlükəsiz telemetriya hadisələri.

## İlkin şərtlər
1. **Ətraf mühitin yükləyicisi**
   - Demo Torii yığınını başladın: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Ən azı bir SDK nümunəsini işə salın (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Alətlər**
   - SDK diaqnostikasını aktivləşdirin (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     Swift-də `ConnectSessionDiagnostics`; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     Android/JS-də ekvivalentlər).
   - CLI `iroha connect queue inspect --sid <sid> --metrics` həllini təmin edin
     SDK tərəfindən istehsal olunan növbə yolu (`~/.iroha/connect/<sid>/state.json` və
     `metrics.ndjson`).
   - Tel telemetriya ixracatçıları beləliklə aşağıdakı zaman seriyaları görünsün
     Grafana və `scripts/swift_status_export.py telemetry` vasitəsilə: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Dəlil qovluqları** – `artifacts/connect-chaos/<date>/` yaradın və saxlayın:
   - xam qeydlər (`*.log`), ölçülər anlıq görüntüləri (`*.json`), tablosunun ixracı
     (`*.png`), CLI çıxışları və PagerDuty ID-ləri.

## Ssenari matrisi| ID | Səhv | Enjeksiyon addımları | Gözlənilən siqnallar | Sübut |
|----|-------|-----------------|------------------|----------|
| C1 | WebSocket kəsilməsi və yenidən qoşulun | `/v2/connect/ws`-i proksinin arxasına sarın (məsələn, `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) və ya xidməti müvəqqəti bloklayın (≤60s üçün `kubectl scale deploy/torii --replicas=0`). Oflayn növbələrin dolması üçün cüzdanı çərçivələr göndərməyə məcbur edin. | `connect.reconnects_total` artımlar, `connect.resume_latency_ms` sıçrayışlar, lakin - Kəsinti pəncərəsi üçün idarə paneli annotasiyası.- Yenidən qoşulma + boşaltma mesajları ilə nümunə jurnaldan çıxarış. |
| C2 | Oflayn növbə daşması / TTL müddəti | Növbə limitlərini daraltmaq üçün nümunəni düzəldin (Swift: `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`-i `ConnectSessionDiagnostics` daxilində yarat; Android/JS müvafiq konstruktorlardan istifadə edir). dApp sorğuları sıralamağa davam edərkən pul kisəsini ≥2× `retentionInterval` üçün dayandırın. | `connect.queue_dropped_total{reason="overflow"}` və `{reason="ttl"}` artım, yeni limitdə `connect.queue_depth` yaylaları, SDK səthi `ConnectError.QueueOverflow(limit: 4)` (və ya `.QueueExpired`). `iroha connect queue inspect` 100% `warn/drop` su nişanı ilə `state=Overflow` göstərir. | - Metrik sayğacların skrinşotu.- Daşmanı çəkən CLI JSON çıxışı.- `ConnectError` xəttini ehtiva edən Swift/Android log parçası. |
| C3 | Manifest sürüşmə / qəbuldan imtina | Pul kisələrinə xidmət edilən Connect manifestinə müdaxilə (məs., `docs/connect_swift_ios.md` nümunə manifestini dəyişdirin və ya `--connect-manifest-path` ilə `--connect-manifest-path` ilə `chain_id` və ya Grafana olan nüsxəyə işarə edərək başlayın). dApp sorğusunun təsdiqinə sahib olun və pul kisəsinin siyasət vasitəsilə rədd edilməsini təmin edin. | Torii `/v2/connect/session` üçün `manifest_mismatch` ilə `HTTP 409` qaytarır, SDK-lar `ConnectError.Authorization.manifestMismatch(manifestVersion)` yayır, telemetriya `connect.manifest_mismatch_total` artırır və empty qalır (`state=Idle`). | - Uyğunsuzluğun aşkarlanmasını göstərən Torii log çıxarışı.- Açılan xətanın SDK skrinşotu.- Test zamanı növbəyə qoyulmuş çərçivələrin olmadığını sübut edən metrik snapşot. |
| C4 | Açar fırlanma / duz versiyası qabar | Seansın ortasında Connect duz və ya AEAD düyməsini fırladın. Dev yığınlarında Torii-i `CONNECT_SALT_VERSION=$((old+1))` ilə yenidən başladın (`docs/source/sdk/android/telemetry_schema_diff.md`-də Android redaksiya duz testini əks etdirir). Duz fırlanması tamamlanana qədər pul kisəsini oflayn saxlayın, sonra davam edin. | İlk davam etdirmə cəhdi `ConnectError.Authorization.invalidSalt` ilə uğursuz oldu, növbələr yuyuldu (dApp `salt_version_mismatch` səbəbi ilə keşlənmiş çərçivələri buraxır), telemetriya `android.telemetry.redaction.salt_version` (Android) və `swift.connect.session_event{event="salt_rotation"}` yayır. SID yeniləməsindən sonra ikinci sessiya uğurla başa çatdı. | - Əvvəl/sonra duz dövrü ilə idarə paneli annotasiyası.- Etibarsız duz xətası və sonrakı müvəffəqiyyəti ehtiva edən qeydlər.- `state=Stalled` və ardınca təzə `state=Active` çıxışını göstərən `iroha connect queue inspect`. || C5 | Attestasiya / StrongBox uğursuzluğu | Android cüzdanlarında `ConnectApproval`-i `attachments[]` + StrongBox attestasiyasını daxil etmək üçün konfiqurasiya edin. dApp-a verməzdən əvvəl attestasiya kəmərindən (`scripts/android_keystore_attestation.sh` `--inject-failure strongbox-simulated` ilə) istifadə edin və ya attestasiya JSON-u dəyişdirin. | DApp `ConnectError.Authorization.invalidAttestation` ilə təsdiqi rədd edir, Torii uğursuzluq səbəbini qeyd edir, ixracatçılar `connect.attestation_failed_total`-i vurur və növbə pozan girişi təmizləyir. Swift/JS dApps seansı canlı saxlayarkən xətanı qeyd edir. | - İnyeksiya edilmiş xəta ID-si ilə qoşqu jurnalı.- SDK xəta qeydi + telemetriya sayğacının tutulması.- Növbənin pis çərçivəni sildiyinə dair sübut (`recordsRemoved > 0`). |

## Ssenari təfərrüatları

### C1 — WebSocket kəsilməsi və yenidən qoşulun
1. Torii-i proksi (toxiproxy, Envoy və ya `kubectl port-forward`) arxasına sarın.
   bütün nodu öldürmədən mövcudluğu dəyişə bilərsiniz.
2. 45 saniyəlik fasiləni işə salın:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Telemetriya tablosuna və `scripts/swift_status_export.py telemetriyasına riayət edin
   --json-out artefacts/connect-chaos//c1_metrics.json`.
4. Kəsintidən dərhal sonra boşaltma növbəsinin vəziyyəti:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Uğur = tək təkrar qoşulma cəhdi, məhdud növbə artımı və avtomatik
   proksi bərpa edildikdən sonra boşaltın.

### C2 — Oflayn növbə daşması / TTL müddəti
1. Yerli tikintilərdə növbə hədlərini daralt:
   - Swift: nümunənizdəki `ConnectQueueJournal` başlatıcısını yeniləyin
     (məsələn, `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` keçmək.
   - Android/JS: tikinti zamanı ekvivalent konfiqurasiya obyektini keçin
     `ConnectQueueJournal`.
2. Pul kisəsini (simulyator fonu və ya cihazın təyyarə rejimi) ≥60 saniyəyə dayandırın
   dApp isə `ConnectClient.requestSignature(...)` zəngləri verir.
3. `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) və ya JS istifadə edin
   sübut paketini ixrac etmək üçün diaqnostika köməkçisi (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Uğur = sayğacların daşması, SDK səthləri `ConnectError.QueueOverflow`
   bir dəfə və pul kisəsi bərpa edildikdən sonra növbə bərpa olunur.

### C3 — Manifest sürüşmə / qəbuldan imtina
1. Qəbul manifestinin surətini çıxarın, məsələn:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Torii-i `--connect-manifest-path /tmp/manifest_drift.json` ilə işə salın (və ya
   qazma üçün docker compose/k8s konfiqurasiyasını yeniləyin).
3. Pul kisəsindən sessiyaya başlamaq cəhdi; HTTP 409 gözləyin.
4. Torii + SDK qeydləri üstəgəl `connect.manifest_mismatch_total`-dən çəkin
   telemetriya tablosu.
5. Uğur = növbə artımı olmadan imtina, üstəlik pul kisəsi paylaşılanları göstərir
   taksonomiya xətası (`ConnectError.Authorization.manifestMismatch`).### C4 — Açarın fırlanması / duz qabarığı
1. Telemetriyadan cari duz versiyasını qeyd edin:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Torii-i yeni duzla yenidən başladın (`CONNECT_SALT_VERSION=$((OLD+1))` və ya yeniləyin
   konfiqurasiya xəritəsi). Yenidən başlatma tamamlanana qədər pul kisəsini oflayn saxlayın.
3. Pul kisəsini davam etdirin; ilk rezyume etibarsız duz xətası ilə uğursuz olmalıdır
   və `connect.queue_dropped_total{reason="salt_version_mismatch"}` artımları.
4. Sessiya kataloqunu silməklə proqramı keşlənmiş kadrları atmağa məcbur edin
   (`rm -rf ~/.iroha/connect/<sid>` və ya platformaya xas keş silinir), sonra
   sessiyanı təzə tokenlərlə yenidən başladın.
5. Uğur = telemetriya duz zərbəsini göstərir, etibarsız CV hadisəsi qeyd olunur
   bir dəfə və növbəti sessiya əl müdaxiləsi olmadan uğurla başa çatır.

### C5 — Attestasiya / StrongBox uğursuzluğu
1. `scripts/android_keystore_attestation.sh` istifadə edərək attestasiya paketi yaradın
   (imza bitini çevirmək üçün `--inject-failure strongbox-simulated` təyin edin).
2. Pul kisəsi bu paketi `ConnectApproval` API vasitəsilə əlavə etsin; dApp
   yükü təsdiq etməli və rədd etməlidir.
3. Telemetriyanı yoxlayın (`connect.attestation_failed_total`, Swift/Android hadisəsi
   ölçüləri) və növbənin zəhərlənmiş girişi atmasını təmin edin.
4. Müvəffəqiyyət = imtina pis təsdiqlə təcrid olunur, növbələr sağlam qalır,
   və attestasiya jurnalı qazma sübutları ilə birlikdə saxlanılır.

## Sübut Yoxlama Siyahısı
- `artifacts/connect-chaos/<date>/c*_metrics.json` ixrac edir
  `scripts/swift_status_export.py telemetry`.
- `iroha connect queue inspect`-dən CLI çıxışları (`c*_queue.txt`).
- SDK + Torii vaxt ştampları və SID hashları ilə qeydlər.
- Hər bir ssenari üçün qeydlər olan tablosunun ekran görüntüləri.
- Sev1/2 siqnalları işə salındıqda PagerDuty / insident identifikatorları.

Tam matrisin hər rübdə bir dəfə tamamlanması yol xəritəsinin qapısını qane edir və
Swift/Android/JS Connect tətbiqlərinin qəti şəkildə cavab verdiyini göstərir
ən yüksək riskli uğursuzluq rejimləri arasında.
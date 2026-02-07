---
lang: az
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d04af2ad3ae5cc6a9254236f5627850aab7c6517308e9f3a09650cbc1490168
source_last_modified: "2026-01-05T18:22:23.401292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sessiyaya Baxış Runbook-a qoşulun (IOS7 / JS4)

Bu runbook səhnələşdirmə, doğrulama və yoxlama üçün başdan sona proseduru sənədləşdirir
Yol xəritəsinin mərhələləri tələb etdiyi kimi, önizləmə seanslarını sökmək **IOS7**
və **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). İstənilən vaxt bu addımları izləyin
siz Connect strawman (`docs/source/connect_architecture_strawman.md`) nümayiş etdirirsiniz
SDK yol xəritələrində vəd edilən növbə/temetriya qarmaqlarını tətbiq edin və ya toplayın
`status.md` üçün sübut.

## 1. Uçuşdan əvvəl Yoxlama Siyahısı

| Maddə | Təfərrüatlar | İstinadlar |
|------|---------|------------|
| Torii son nöqtə + Qoşulma siyasəti | Torii əsas URL, `chain_id` və Qoşulma siyasətini (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`) təsdiq edin. Runbook biletində JSON snapshotını çəkin. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Armatur + körpü versiyaları | İstifadə edəcəyiniz Norito qurğu hash və körpü quruluşuna diqqət yetirin (Swift `NoritoBridge.xcframework` tələb edir, JS `@iroha/iroha-js` ≥ `bootstrapConnectPreviewSession`-i göndərən versiyanı tələb edir). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Telemetriya panelləri | `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event` və s. diaqramların əlçatan olduğundan əmin olun (Grafana Norito Prometheus anlıq görüntülər). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Sübut qovluqları | `docs/source/status/swift_weekly_digest.md` (həftəlik həzm) və `docs/source/sdk/swift/connect_risk_tracker.md` (risk izləyicisi) kimi bir təyinat seçin. `docs/source/sdk/swift/readiness/archive/<date>/connect/` altında qeydləri, metrik skrinşotları və təsdiqləri saxlayın. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Preview Sessiyasını yükləyin

1. **Siyasəti + kvotaları doğrulayın.** Zəng edin:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   `queue_max` və ya TTL planlaşdırdığınız konfiqurasiyadan fərqlidirsə, işləməyin
   test.
2. **Deterministik SID/URI-lər yaradın.** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` köməkçisi SID/URI nəslini Torii ilə əlaqələndirir
   sessiyanın qeydiyyatı; Swift WebSocket qatını idarə edərkən belə istifadə edin.
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - `register: false`-i quru işləyən QR/dərin keçid ssenariləri üçün təyin edin.
   - Qaytarılmış `sidBase64Url`, dərin keçid URL-ləri və `tokens` blobunu davam etdirin
     sübut qovluğu; idarəetmə araşdırması bu artefaktları gözləyir.
3. **Sirləri paylayın.** Deeplink URI-ni pul kisəsi operatoru ilə paylaşın
   (sürətli dApp nümunəsi, Android cüzdanı və ya QA qoşqu). Heç vaxt xam tokenləri yapışdırmayın
   söhbətə; aktivləşdirmə paketində sənədləşdirilmiş şifrələnmiş kassadan istifadə edin.

## 3. Sessiyanı idarə edin1. **WebSocket-i açın.** Swift müştəriləri adətən istifadə edirlər:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   Əlavə quraşdırma üçün `docs/connect_swift_integration.md` istinadı (körpü
   idxal, paralel adapterlər).
2. **Təsdiq + işarə axını.** DApps `ConnectSession.requestSignature(...)` nömrəsinə zəng edir,
   pul kisələri isə `approveSession` / `reject` vasitəsilə cavab verir. Hər bir təsdiq daxil olmalıdır
   Hashed ləqəb + Connect idarəetmə nizamnaməsinə uyğun icazələr.
3. **Məşq növbəsi + davam yolları.** Şəbəkə bağlantısını dəyişin və ya dayandırın
   cüzdan məhdud növbə və replay qarmaqlar log giriş təmin etmək. JS/Android
   SDK-lar `ConnectQueueError.overflow(limit)` / buraxır
   `.expired(ttlMs)` çərçivələri atdıqda; Swift eyni şeyi bir dəfə müşahidə etməlidir
   IOS7 növbəli iskele torpaqları (`docs/source/connect_architecture_strawman.md`).
   Ən azı bir yenidən qoşulma qeyd etdikdən sonra işə salın
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (və ya `ConnectSessionDiagnostics` tərəfindən qaytarılan ixrac qovluğundan keçin) və
   göstərilən cədvəli/JSON-u runbook biletinə əlavə edin. CLI eyni şeyi oxuyur
   `ConnectQueueStateTracker`-in istehsal etdiyi `state.json` / `metrics.ndjson` cütü,
   beləliklə, idarəetmə rəyçiləri sifarişli alətlər olmadan qazma sübutlarını izləyə bilərlər.

## 4. Telemetriya və Müşahidə Edilə bilənlik

- **Çəkiləcək ölçülər:**
  - `connect.queue_depth{direction}` ölçmə cihazı (siyasət həddi altında qalmalıdır).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` sayğacı (yalnız sıfırdan fərqli
    nasazlıq inyeksiya zamanı).
  - `connect.resume_latency_ms` histoqramı (məcbur etdikdən sonra p95 yazın.
    yenidən birləşdirin).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-ə xüsusi `swift.connect.session_event` və
    `swift.connect.frame_latency` ixracı (`docs/source/sdk/swift/telemetry_redaction.md`).
- ** İdarə panelləri:** Annotasiya markerləri ilə Connect board əlfəcinlərini yeniləyin.
  Skrinşotları (və ya JSON ixracını) xam ilə yanaşı sübut qovluğuna əlavə edin
  OTLP/Prometheus görüntüləri telemetriya ixracatçısı CLI vasitəsilə çəkilib.
- **Xəbərdarlıq:** Hər hansı Sev1/2 həddi işə salınarsa (`docs/source/android_support_playbook.md` §5-ə görə),
  SDK Proqram Rəhbərini səhifələyin və runbook-da PagerDuty insident ID-sini sənədləşdirin
  davam etməzdən əvvəl bilet.

## 5. Təmizləmə və Geriyə qaytarma

1. **Mərhələli sessiyaları silin.** Həmişə önizləmə seanslarını silin ki, növbə dərinliyi olsun
   həyəcan siqnalları mənalı olaraq qalır:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Yalnız Swift testləri üçün Rust/CLI köməkçisi vasitəsilə eyni son nöqtəyə zəng edin.
2. **Jurnalları təmizləyin.** Davamlı növbə jurnallarını silin
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB mağazaları və s.)
   növbəti qaçış təmiz başlayır. Lazım gələrsə, silinməzdən əvvəl fayl hashını qeyd edin
   təkrar oynatma problemini həll edin.
3. **Hadisə qeydlərini qeyd edin.** İşi ümumiləşdirin:
   - `docs/source/status/swift_weekly_digest.md` (delta bloku),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (CR-2-ni təmizləyin və ya aşağı salın
     telemetriya quraşdırıldıqdan sonra),
   - JS SDK dəyişiklik jurnalı və ya yeni davranış təsdiqləndiyi təqdirdə resept.
4. **Uğursuzluqları artırın:**
   - Enjekte edilmiş nasazlıqlar olmadan növbənin daşması ⇒ SDK-ya qarşı səhv faylı verin
     siyasət Torii-dən ayrıldı.
   - Resume xətaları ⇒ `connect.queue_depth` + `connect.resume_latency_ms` əlavə edin
     hadisə hesabatının anlıq görüntüləri.
   - İdarəetmə uyğunsuzluqları (tokenlər təkrar istifadə edildi, TTL keçdi) ⇒ SDK ilə artırın
     Proqram rəhbəri və növbəti təftiş zamanı `roadmap.md`-ə şərh yazın.

## 6. Sübutların Yoxlama Siyahısı| Artefakt | Məkan |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard ixracı (`connect.queue_depth` və s.) | `.../metrics/` alt qovluğu |
| PagerDuty / insident IDs | `.../notes.md` |
| Təmizləmə təsdiqi (Torii silin, jurnal silin) ​​| `.../cleanup.log` |

Bu yoxlama siyahısının doldurulması “sənədlər/runbooks yeniləndi” çıxış meyarına cavab verir
IOS7/JS4 üçün və idarəetmə rəyçilərinə hər biri üçün deterministik bir iz verir
Önizləmə sessiyasını birləşdirin.
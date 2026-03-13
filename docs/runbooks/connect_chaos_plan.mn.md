---
lang: mn
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

# Эмх замбараагүй байдал ба алдааны давталтын төлөвлөгөөг холбох (IOS3 / IOS7)

Энэхүү тоглоомын ном нь IOS3/IOS7-д нийцсэн давтагдах эмх замбараагүй дасгалуудыг тодорхойлдог
замын зураглалын арга хэмжээ _“хамтарсан эмх замбараагүй байдлын сургуулилтыг төлөвлөх”_ (`roadmap.md:1527`). Үүнийг хослуул
Connect-ыг урьдчилан үзэх runbook (`docs/runbooks/connect_session_preview_runbook.md`)
хөндлөн SDK үзүүлбэрүүдийг зохион байгуулах үед.

## Зорилго ба Амжилтын шалгуур
- Хуваалцсан Холболтын дахин оролдох/буцах бодлого, офлайн дарааллын хязгаарлалт, болон
  телеметрийн экспортлогчид үйлдвэрлэлийн кодыг өөрчлөхгүйгээр хяналттай гэмтэлтэй .
- Тодорхойлогч олдворуудыг авах (`iroha connect queue inspect` гаралт,
  `connect.*` хэмжүүрийн агшин зуурын зураг, Swift/Android/JS SDK бүртгэл) нь засаглалыг
  дасгал бүрт аудит хийх.
- Түрийвч болон dApp-ууд тохиргооны өөрчлөлтийг (манифест дрифт, давс
  эргэлт, баталгаажуулалтын алдаа) `ConnectError` каноникийг гадаргуу дээр буулгах замаар
  ангилал болон засварлах аюулгүй телеметрийн үйл явдлууд.

## Урьдчилсан нөхцөл
1. **Орчны ачаалагч**
   - Демо Torii стекийг эхлүүлнэ үү: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Дор хаяж нэг SDK дээжийг ажиллуулна уу (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. ** Багаж хэрэгсэл**
   - SDK оношлогоог идэвхжүүлэх (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     Swift дээр `ConnectSessionDiagnostics`; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     Android/JS дээрх эквивалентууд).
   - CLI `iroha connect queue inspect --sid <sid> --metrics` шийдэгдсэн эсэхийг шалгаарай
     SDK-ийн үүсгэсэн дарааллын зам (`~/.iroha/connect/<sid>/state.json` ба
     `metrics.ndjson`).
   - Утсан телеметрийн экспортлогчдын хувьд дараах хугацааны цуваанууд харагдах болно
     Grafana ба `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Баримт бичгийн хавтас** – `artifacts/connect-chaos/<date>/` үүсгэж хадгална:
   - түүхий лог (`*.log`), хэмжүүрийн агшин зуурын зураг (`*.json`), хяналтын самбарын экспорт
     (`*.png`), CLI гаралт, PagerDuty ID.

## Хувилбарын матриц| ID | Алдаа | Тарилгын алхамууд | Хүлээгдэж буй дохио | Нотлох баримт |
|----|-------|-----------------|------------------|----------|
| C1 | WebSocket тасалдсан & дахин холбогдох | `/v2/connect/ws`-г проксины ард боож (жишээ нь, `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) эсвэл үйлчилгээг түр блокло (≤60 секундын хувьд `kubectl scale deploy/torii --replicas=0`). Офлайн дарааллыг дүүргэхийн тулд түрийвчээ фрэйм ​​илгээсээр байх хэрэгтэй. | `connect.reconnects_total` нэмэгдэж, `connect.resume_latency_ms` нэмэгдэж байгаа боловч - Тасралтгүй цонхны хяналтын самбарын тайлбар.- Дахин холбогдох + ус зайлуулах мессеж бүхий логоны жишээ. |
| C2 | Офлайн дараалал хэт их / TTL дуусах | Дарааллын хязгаарлалтыг багасгахын тулд дээжийг нөхөх (Swift: `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` дотор `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`-г үүсгэ; Android/JS харгалзах бүтээгчийг ашигладаг). dApp хүсэлтийг дараалалд оруулсаар байх хооронд түрийвчээ ≥2× `retentionInterval`-ээр түр зогсоо. | `connect.queue_dropped_total{reason="overflow"}` болон `{reason="ttl"}` өсөлт, шинэ хязгаарт `connect.queue_depth` өндөрлөг, SDK-ийн гадаргуу `ConnectError.QueueOverflow(limit: 4)` (эсвэл `.QueueExpired`). `iroha connect queue inspect` нь 100% `warn/drop` усан тэмдэгтэй `state=Overflow`-г харуулж байна. | - Метрийн тоолуурын дэлгэцийн агшин.- CLI JSON гаралт нь халихыг авсан.- `ConnectError` мөрийг агуулсан Swift/Android бүртгэлийн хэсэг. |
| C3 | Манифест drift / элсэлтийн татгалзал | Холболтын манифестийг түрийвч рүү шилжүүлэх (жишээ нь, `docs/connect_swift_ios.md` дээжийн манифестийг өөрчлөх, эсвэл `--connect-manifest-path`-г `--connect-manifest-path`-ээр `chain_id` эсвэл Grafana. dApp хүсэлтийг зөвшөөрч, түрийвчээ бодлогоор татгалзсан эсэхийг шалгаарай. | Torii нь `manifest_mismatch`-тэй `/v2/connect/session`-д `HTTP 409`-ийг буцаана, SDK-ууд `ConnectError.Authorization.manifestMismatch(manifestVersion)`-ийг ялгаруулна, телеметр нь `connect.manifest_mismatch_total`-ийг өсгөж, empty хэвээр байна. (`state=Idle`). | - Torii логийн ишлэл нь тохирохгүй байгааг илрүүлж байна.- Гадаргуугийн алдааны SDK дэлгэцийн агшин.- Туршилтын явцад дараалалд орсон хүрээ байхгүйг нотлох хэмжүүрийн агшин зураг. |
| C4 | Түлхүүрийг эргүүлэх / давс-хувилбар овойлт | Холболтын давс эсвэл AEAD товчлуурыг хуралдааны дундуур эргүүлнэ үү. Хөгжүүлэгчийн стек дээр Torii-г `CONNECT_SALT_VERSION=$((old+1))`-ээр дахин эхлүүлнэ үү (`docs/source/sdk/android/telemetry_schema_diff.md` дээрх Android редакцийн давсны тестийг толилуулдаг). Давсны эргэлт дуусах хүртэл түрийвчээ офлайн байлгаад, үргэлжлүүлнэ үү. | Эхний үргэлжлүүлэх оролдлого `ConnectError.Authorization.invalidSalt`-ээр бүтэлгүйтэж, дараалал унасан (dApp `salt_version_mismatch` шалтгаантай кэштэй фрэймүүдийг устгадаг), телеметрийн `android.telemetry.redaction.salt_version` (Android) болон `swift.connect.session_event{event="salt_rotation"}` ялгаруулдаг. SID шинэчлэлтийн дараах хоёр дахь сесс амжилттай боллоо. | - Өмнө/дараа давсны үе бүхий хяналтын самбарын тайлбар.- Давсны буруу алдаа болон дараагийн амжилтыг агуулсан бүртгэл.- `iroha connect queue inspect` гаралтыг харуулсан `state=Stalled`, дараа нь шинэ `state=Active`. || C5 | Баталгаажуулалт / StrongBox-ийн алдаа | Android түрийвч дээр `ConnectApproval`-г `attachments[]` + StrongBox баталгаажуулалтыг оруулахаар тохируулна уу. dApp-д өгөхөөс өмнө баталгаажуулалтын бэхэлгээг (`scripts/android_keystore_attestation.sh`-тэй `--inject-failure strongbox-simulated`) ашиглана уу эсвэл JSON-н аттестатчиллыг өөрчилнө үү. | DApp нь `ConnectError.Authorization.invalidAttestation`, Torii нь бүтэлгүйтлийн шалтгааныг бүртгэж, экспортлогчид `connect.attestation_failed_total`-г мөргөж, дараалал нь зөрчилтэй оруулгыг арилгадаг. Swift/JS dApps нь сессийг амьд байлгахын зэрэгцээ алдааг бүртгэдэг. | - Тарилгын алдааны ID бүхий бэхэлгээний бүртгэл.- SDK алдааны бүртгэл + телеметрийн тоолуур барих.- Дараалал нь муу фреймийг устгасан болохыг нотлох баримт (`recordsRemoved > 0`). |

## Хувилбарын дэлгэрэнгүй

### C1 — WebSocket тасалдсан ба дахин холбогдоно
1. Torii-г прокси (toxiproxy, Envoy эсвэл `kubectl port-forward`)-ийн ард боож,
   та бүхэл бүтэн зангилааг устгахгүйгээр бэлэн байдлыг өөрчлөх боломжтой.
2. 45 секундын тасалдлыг өдөөх:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Телеметрийн хяналтын самбар болон `scripts/swift_status_export.py телеметрийг ажиглаарай.
   --json-out artefacts/connect-chaos//c1_metrics.json`.
4. Овоолгын дарааллын төлөв байдал зогссоны дараа:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Амжилт = нэг дахин холбох оролдлого, хязгаарлагдмал дарааллын өсөлт, автомат
   прокси сэргэсний дараа ус зайлуулах.

### C2 — Офлайн дараалал халих / TTL хугацаа дуусах
1. Орон нутгийн барилгад дарааллын босгыг багасгах:
   - Swift: дээжийнхээ `ConnectQueueJournal` эхлүүлэгчийг шинэчил
     (жишээ нь, `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` дамжих.
   - Android/JS: үүсгэх үед ижил төстэй тохиргооны объектыг дамжуулна
     `ConnectQueueJournal`.
2. Түрийвчийг (симуляторын дэвсгэр эсвэл төхөөрөмжийн онгоцны горим) ≥60 секундын турш түр түдгэлзүүлээрэй
   dApp нь `ConnectClient.requestSignature(...)` дуудлагыг гаргадаг.
3. `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) эсвэл JS ашиглана уу.
   нотлох баримтыг экспортлох оношлогооны туслах (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Амжилт = халих тоолуурын өсөлт, SDK гадаргуу `ConnectError.QueueOverflow`
   нэг удаа, түрийвчээ үргэлжлүүлсний дараа дараалал сэргэнэ.

### C3 — Илэрхий хазайлт / элсэлтийн татгалзал
1. Элсэлтийн манифестийн хуулбарыг хийх, жишээлбэл:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Torii-г `--connect-manifest-path /tmp/manifest_drift.json` (эсвэл) ашиглан эхлүүлнэ үү.
   өрөмдлөгийн docker compose/k8s тохиргоог шинэчлэх).
3. Түрийвчнээс сесс эхлүүлэхийг оролдох; HTTP 409 хүлээж байна.
4. Torii + SDK логууд дээр нэмэх нь `connect.manifest_mismatch_total`-ээс авах
   телеметрийн хяналтын самбар.
5. Амжилт = дараалал өсөлтгүйгээр татгалзсан, нэмэх нь хэтэвч хуваалцсан харуулж байна
   ангилал зүйн алдаа (`ConnectError.Authorization.manifestMismatch`).### C4 — Түлхүүрийг эргүүлэх / давсны овойлт
1. Телеметрээс одоогийн давсны хувилбарыг тэмдэглэ:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Torii-г шинэ давсаар (`CONNECT_SALT_VERSION=$((OLD+1))` эсвэл шинэчлээрэй) дахин эхлүүлнэ үү.
   тохиргооны зураг). Дахин эхлүүлэх хүртэл түрийвчээ офлайн байлгаарай.
3. Түрийвчээ үргэлжлүүлэх; эхний анкет нь хүчингүй давсны алдаагаар бүтэлгүйтсэн байх ёстой
   болон `connect.queue_dropped_total{reason="salt_version_mismatch"}` өсөлт.
4. Сессийн лавлахыг устгаснаар програмыг кэшд хадгалсан фреймүүдийг буулгахад хүргэнэ үү
   (`rm -rf ~/.iroha/connect/<sid>` эсвэл платформд зориулсан кэшийг цэвэрлэх), дараа нь
   сессийг шинэ жетоноор дахин эхлүүлнэ үү.
5. Амжилт = телеметр нь давсны овойлтыг харуулж байна, хүчингүй үргэлжлүүлэх үйл явдал бүртгэгдсэн байна
   нэг удаа, дараагийн хуралдаан нь гарын авлагын оролцоогүйгээр амжилттай болно.

### C5 - Баталгаажуулалт / StrongBox-ын алдаа
1. `scripts/android_keystore_attestation.sh` ашиглан баталгаажуулалтын багц үүсгэнэ үү
   (гарын үсэгний битийг эргүүлэхийн тулд `--inject-failure strongbox-simulated`-г тохируулна уу).
2. Түрийвчээ `ConnectApproval` API-ээр дамжуулан энэ багцыг хавсаргана уу; dApp
   ачааллыг баталгаажуулж, татгалзах ёстой.
3. Телеметрийг баталгаажуулах (`connect.attestation_failed_total`, Swift/Android осол
   хэмжигдэхүүн) ба дараалалд хордсон оруулгыг унагасан эсэхийг шалгаарай.
4. Амжилт = татгалзах нь муу зөвшөөрлөөс тусгаарлагддаг, дараалал нь эрүүл байх,
   аттестатчиллын бүртгэлийг өрмийн нотлох баримтын хамт хадгална.

## Нотлох баримт шалгах хуудас
- `artifacts/connect-chaos/<date>/c*_metrics.json`-аас экспортлодог
  `scripts/swift_status_export.py telemetry`.
- `iroha connect queue inspect`-аас CLI гаралт (`c*_queue.txt`).
- SDK + Torii логууд нь цагийн тэмдэг болон SID хэштэй.
- Сценари бүрийн тайлбар бүхий хяналтын самбарын дэлгэцийн агшин.
- Хэрэв Sev1/2 дохио гарсан бол PagerDuty / ослын ID.

Бүтэн матрицыг улиралд нэг удаа бөглөх нь замын зураглалыг хангана
Swift/Android/JS Connect хэрэгжүүлэлтүүд тодорхой хариу үйлдэл үзүүлдэг болохыг харуулж байна
хамгийн өндөр эрсдэлтэй бүтэлгүйтлийн горимууд дээр.
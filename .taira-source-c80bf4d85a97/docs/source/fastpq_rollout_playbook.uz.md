---
lang: uz
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:53:05.148398+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FASTPQ Rollout Playbook (7-3-bosqich)

Ushbu qo'llanma Stage7-3 yo'l xaritasi talabini amalga oshiradi: har bir parkni yangilash
FASTPQ GPU bajarilishini ta'minlaydigan, takrorlanadigan benchmark manifestini biriktirishi kerak,
juftlashtirilgan Grafana dalillari va hujjatlashtirilgan orqaga qaytish matkap. To'ldiradi
`docs/source/fastpq_plan.md` (maqsadlar/arxitektura) va
Fokuslash orqali `docs/source/fastpq_migration_guide.md` (tugun darajasidagi yangilanish bosqichlari).
operatorga qarashli ro'yxatga olish ro'yxatida.

## Qamrov va rollar

- **Release Engineering / SRE:** o'z benchmark tasvirlari, manifest imzosi va
  ishlab chiqarishni tasdiqlashdan oldin boshqaruv paneli eksporti.
- **Ops Gildiyasi:** bosqichma-bosqich chiqishlarni amalga oshiradi, orqaga qaytarish mashqlarini yozib oladi va saqlaydi
  `artifacts/fastpq_rollouts/<timestamp>/` ostidagi artefakt to'plami.
- **Boshqaruv / Muvofiqlik:** dalillar har bir o'zgarishga hamroh bo'lishini tasdiqlaydi
  Filo uchun FASTPQ sukut bo'yicha o'tishdan oldin so'rov.

## Dalillar to'plamiga qo'yiladigan talablar

Har bir taqdimot quyidagi artefaktlarni o'z ichiga olishi kerak. Barcha fayllarni biriktiring
Chiptani chiqarish/yangilash va to'plamni ichida saqlang
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| Artefakt | Maqsad | Qanday ishlab chiqarish |
|----------|---------|----------------|
| `fastpq_bench_manifest.json` | Kanonik 20000 qatorli ish yuki `<1 s` LDE shipi ostida qolishini isbotlaydi va har bir oʻralgan mezon uchun xeshlarni qayd qiladi.| Metall/CUDA yugurishlarini suratga oling, ularni o‘rab oling, so‘ngra ishga tushiring:`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \``python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \`Prometheus uchun (`--row-usage` va `--poseidon-metrics` nuqtalari tegishli guvohlar/scrape fayllarida). Yordamchi filtrlangan `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` namunalarini joylashtiradi, shuning uchun WP2-E.6 dalillari Metall va CUDA da bir xil bo'ladi. Mustaqil Poseidon mikrobench xulosasi kerak bo'lganda (o'ralgan yoki xom kirishlar qo'llab-quvvatlanadi) `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` dan foydalaning. |
|  |  | **Stage7 yorlig'i talabi:** `wrap_benchmark.py`, agar natijada olingan `metadata.labels` bo'limi `device_class` va `gpu_kind`ni o'z ichiga olmasa, endi bajarilmaydi. Avtomatik aniqlash ularni aniqlay olmasa (masalan, ajratilgan CI tuguniga o'rashda), `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete` kabi aniq bekor qilishni o'tkazing. |
|  |  | **Tezlashuv telemetriyasi:** oʻram ham sukut boʻyicha `cargo xtask acceleration-state --format json` ni oladi, oʻralgan mezon yoniga `<bundle>.accel.json` va `<bundle>.accel.prom` ni yozadi (`--accel-*` yoki I1006000 bayroqlari bilan bekor qilish). Suratga olish matritsasi ushbu fayllardan flot asboblar paneli uchun `acceleration_matrix.{json,md}` yaratish uchun foydalanadi. |
| Grafana eksport | Qabul qilingan telemetriya va chiqish oynasi uchun ogohlantirish izohlarini isbotlaydi.| `fastpq-acceleration` asboblar panelini eksport qiling:`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`Eksportni boshlash/topshirish vaqtlari bilan taxtachaga izoh bering. Chiqaruvchi quvur liniyasi buni avtomatik ravishda `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` orqali amalga oshirishi mumkin (token `GRAFANA_TOKEN` orqali taqdim etiladi). |
| Ogohlantirish surati | Chiqarishni himoya qiluvchi ogohlantirish qoidalarini yozib oladi.| `dashboards/alerts/fastpq_acceleration_rules.yml` (va `tests/` moslamasini) to‘plamga nusxa ko‘chiring, shunda sharhlovchilar `promtool test rules …`ni qayta ishga tushirishlari mumkin. |
| Qayta burg'ulash jurnali | Operatorlar protsessorning majburiy qaytarilishini va telemetriyani tasdiqlashni takrorlaganligini ko'rsatadi.| [Orqaga matkaplar](#rollback-drills) va konsol jurnallarini (`rollback_drill.log`) va natijada Prometheus qirqishini (`metrics_rollback.prom`) saqlang. || `row_usage/fastpq_row_usage_<date>.json` | CI va asboblar panelida TF-5 kuzatadigan ExecWitness FASTPQ qatorlarini qayd qiladi.| Torii dan yangi guvohni yuklab oling, uni `iroha_cli audit witness --decode exec.witness` orqali dekodlang (ixtiyoriy ravishda kutilgan parametrlar toʻplamini tasdiqlash uchun `--fastpq-parameter fastpq-lane-balanced` ni qoʻshing; FASTPQ toʻplamlari sukut boʻyicha chiqaradi) va `row_usage` JSON040.ga nusxa koʻchiring. Sharhlovchilar ularni chiqish chiptasi bilan bog'lashlari uchun fayl nomlarini vaqt tamg'asi bilan saqlang va `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (yoki `make check-fastpq-rollout`) (yoki `make check-fastpq-rollout`) ni ishga tushiring, shunda Stage7-3 eshigi har bir partiya selektorni e'lon qilishini va `transfer_ratio = transfer_rows / total_rows` dalolatnoma biriktirilishidan oldin reklama qilishini tekshiradi. |

> **Maslahat:** `artifacts/fastpq_rollouts/README.md` afzal qilingan nomlashni hujjatlashtiradi
> sxema (`<stamp>/<fleet>/<lane>`) va kerakli dalillar fayllari. The
> `<stamp>` papkasi `YYYYMMDDThhmmZ`ni kodlashi kerak, shuning uchun artefaktlar saralanishi mumkin
> chiptalar bilan maslahatlashmasdan.

## Dalillarni yaratish nazorat ro'yxati1. **GPU mezonlarini yozib oling.**
   - Kanonik ish yukini (20000 mantiqiy qator, 32768 to'ldirilgan qator) orqali boshqaring
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - `--row-usage <decoded witness>` yordamida natijani `scripts/fastpq/wrap_benchmark.py` bilan oʻrab oling, shunda toʻplam GPU telemetriyasi bilan birga gadjet dalillarini olib yuradi. `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` dan o'ting, agar tezlatgich belgilangandan oshib ketsa yoki Poseidon navbati/profil telemetriyasi yo'q bo'lsa va ajratilgan imzoni yaratish uchun o'ram tezda ishlamay qoladi.
   - CUDA xostida takrorlang, shunda manifest ikkala GPU oilasini o'z ichiga oladi.
   - `benchmarks.metal_dispatch_queue` yoki **yozmang**
     Oʻralgan JSON dan `benchmarks.zero_fill_hotspots` bloklari. CI darvozasi
     (`ci/check_fastpq_rollout.sh`) endi bu maydonlarni o'qiydi va navbatda turganda muvaffaqiyatsiz bo'ladi
     Bo'sh joy bitta slotdan pastga tushadi yoki LDE hotspot `mean_ms > haqida xabar berganda
     0,40 ms`, Stage7 telemetriya qo'riqchisini avtomatik ravishda amalga oshiradi.
2. **Manifestni yarating.** `cargo xtask fastpq-bench-manifest …` sifatida foydalaning
   jadvalda ko'rsatilgan. `fastpq_bench_manifest.json` ni tarqatish to'plamida saqlang.
3. **Grafana eksporti.**
   - `FASTPQ Acceleration Overview` taxtasiga chiqish oynasi bilan izoh bering,
     tegishli Grafana panel identifikatorlariga ulanish.
   - JSON asboblar panelini Grafana API orqali eksport qiling (yuqoridagi buyruq) va shu jumladan
     `annotations` bo'limi, shuning uchun sharhlovchilar qabul qilish egri chiziqlarini moslashtirishi mumkin
     bosqichli ishlab chiqarish.
4. **Snapshot ogohlantirishlari.** Ishlatilgan ogohlantirish qoidalarini (`dashboards/alerts/…`) nusxalash
   to'plamga chiqarish orqali. Agar Prometheus qoidalari bekor qilingan bo'lsa, kiriting
   bekor qilish farqi.
5. **Prometheus/OTEL qirqish.** Har biridan `fastpq_execution_mode_total{device_class="<matrix>"}` suratini oling.
   xost (sahnadan oldin va keyin) va OTEL hisoblagichi
   `fastpq.execution_mode_resolutions_total` va ulangan
   `telemetry::fastpq.execution_mode` jurnal yozuvlari. Bu artefaktlar buni isbotlaydi
   GPU-ni qabul qilish barqaror va bu majburiy protsessorni qaytarish hali ham telemetriyani chiqaradi.
6. **Qatordan foydalanish telemetriyasini arxivlash.** ExecWitness dasturi dekodlangandan so‘ng
   to'plamdagi `row_usage/` ostida hosil bo'lgan JSONni qoldiring. CI
   yordamchi (`ci/check_fastpq_row_usage.sh`) bu oniy rasmlarni quyidagi bilan solishtiradi
   kanonik bazaviy chiziqlar va `ci/check_fastpq_rollout.sh` endi har birini talab qiladi
   TF-5 dalillarini biriktirilgan holda saqlash uchun kamida bitta `row_usage` faylini jo'natish uchun to'plam
   chiqish chiptasiga.

## Bosqichli ishlab chiqarish oqimi

Har bir flot uchun uchta deterministik fazadan foydalaning. Oldinga faqat chiqishdan keyin
Har bir bosqichdagi mezonlar qondiriladi va dalillar to'plamida hujjatlashtiriladi.| Bosqich | Qo'llash doirasi | Chiqish mezonlari | Qo'shimchalar |
|-------|-------|---------------|-------------|
| Uchuvchi (P1) | Har bir hudud uchun 1 ta boshqaruv tekisligi + 1 ta ma'lumotlar tekisligi tugunlari | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` 48 soat davomida ≥90%, Alertmanager hodisalari nolga teng va orqaga qaytish mashqi. | Ikkala xostdan toʻplam (jamlanma JSONlar, uchuvchi izohli Grafana eksporti, orqaga qaytarish jurnallari). |
| Rampa (P2) | Validatorlarning ≥50% va har bir klaster uchun kamida bitta arxiv yoʻli | GPU ishlashi 5 kun davom etdi, 1 martadan koʻp boʻlmagan pasayish tezligi >10 minutdan oshmaydi va Prometheus hisoblagichlari 60 soniya ichida qayta tiklash haqida ogohlantirishni isbotlaydi. | Yangilangan Grafana eksport rampa izohi, Prometheus qirqish farqlari, Alertmanager skrinshoti/jurnal. |
| Standart (P3) | Qolgan tugunlar; FASTPQ standart sifatida `iroha_config` | da belgilangan Imzolangan dastgoh manifest + Grafana yakuniy qabul qilish egri chizig'iga havola qilingan eksport va konfiguratsiyani almashtirishni ko'rsatadigan hujjatlashtirilgan orqaga qaytarish matkapi. | Yakuniy manifest, Grafana JSON, orqaga qaytarish jurnali, konfiguratsiya oʻzgarishini koʻrib chiqish uchun chipta havolasi. |

Har bir reklama bosqichini tarqatish chiptasida hujjatlang va to'g'ridan-to'g'ri bog'lang
`grafana_fastpq_acceleration.json` izohlari, shuning uchun sharhlovchilar o'zaro bog'lashlari mumkin
dalillar bilan vaqt jadvali.

## Orqaga qaytarish mashqlari

Har bir ishlab chiqarish bosqichida orqaga qaytish mashqlari bo'lishi kerak:

1. Klaster uchun bitta tugunni tanlang va joriy ko‘rsatkichlarni yozib oling:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. Konfiguratsiya tugmasi yordamida protsessor rejimini 10 daqiqaga majburlang
   (`zk.fastpq.execution_mode = "cpu"`) yoki atrof-muhitni bekor qilish:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. Pastga tushirish jurnalini tasdiqlang
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) va qirib tashlang
   hisoblagich o'sishlarini ko'rsatish uchun yana Prometheus so'nggi nuqtasi.
4. GPU rejimini tiklang, `telemetry::fastpq.execution_mode` hozir xabar berishini tasdiqlang
   `resolved="metal"` (yoki metall bo'lmagan yo'llar uchun `resolved="cuda"/"opencl"`),
   Prometheus qirqishida CPU va GPU namunalari mavjudligini tasdiqlang
   `fastpq_execution_mode_total{backend=…}` va o'tgan vaqtni yozib oling
   aniqlash/tozalash.
5. Qobiq transkriptlari, ko'rsatkichlar va operator tasdiqlarini sifatida saqlang
   `rollback_drill.log` va `metrics_rollback.prom` tarqatish to'plamida. Bular
   fayllar to'liq pastga tushirish + tiklash tsiklini ko'rsatishi kerak, chunki
   `ci/check_fastpq_rollout.sh` endi jurnalda GPU bo'lmasa, muvaffaqiyatsiz bo'ladi
   tiklash chizig'i yoki ko'rsatkichlar oniy tasviri CPU yoki GPU hisoblagichlarini o'tkazib yuboradi.

Ushbu jurnallar har bir klasterning yaxshi yomonlashishi mumkinligini va SRE guruhlarini isbotlaydi
Agar GPU drayverlari yoki yadrolari regressga uchrasa, qanday qilib aniq orqaga qaytishni biling.

## Aralash rejimdagi qayta tiklash dalillari (WP2-E.6)

Xostga GPU FFT/LDE kerak bo'lganda, lekin CPU Poseidon xeshing (stage7 <900ms bo'yicha)
talab), standart orqaga qaytarish jurnallari bilan birga quyidagi artefaktlarni to'plang:1. **Config diff.** o‘rnatadigan xost-mahalliy bekor qilishni tekshiring (yoki biriktiring).
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) ketayotganda
   `zk.fastpq.execution_mode` tegilmagan. Yamoqqa nom bering
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **Poseydon hisoblagichini qirib tashlash.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   Suratga olish `path="cpu_forced"` ni bloklash bosqichida ko'rsatishi kerak
   Ushbu qurilma sinfi uchun GPU FFT/LDE hisoblagichi. Orqaga qaytgandan so'ng, ikkinchi qirib tashlang
   GPU rejimiga qayting, shunda sharhlovchilar `path="gpu"` qatorining davomini ko'rishlari mumkin.

   Olingan faylni `wrap_benchmark.py --poseidon-metrics …` ga o'tkazing, shunda o'ralgan benchmark `poseidon_metrics` bo'limida bir xil hisoblagichlarni yozadi; bu bir xil ish jarayonida Metall va CUDA prokatlarini saqlab qoladi va alohida skrep fayllarini ochmasdan zaxira dalillarni tekshirilishi mumkin bo'ladi.
3. **Jurnaldan ko‘chirma.** ni isbotlovchi `telemetry::fastpq.poseidon` yozuvlaridan nusxa oling.
   hal qiluvchi protsessorga (`cpu_forced`) aylantirildi
   `poseidon_fallback.log`, vaqt belgilarini saqlab, Alertmanager vaqt jadvallari bo'lishi mumkin
   konfiguratsiya o'zgarishi bilan bog'liq.

CI bugungi kunda navbat/nol-to'ldirish tekshiruvlarini amalga oshiradi; aralash rejimdagi darvoza qo'ngandan so'ng,
`ci/check_fastpq_rollout.sh` ham o'z ichiga olgan har qanday to'plamni talab qiladi
`poseidon_fallback.patch` mos keladigan `metrics_poseidon.prom` suratini yuboradi.
Ushbu ish jarayonidan so'ng WP2-E.6 zaxira siyosati tekshirilishi mumkin va unga bog'langan bo'ladi
sukut bo'yicha ishga tushirish paytida foydalanilgan bir xil dalil yig'uvchilar.

## Hisobot va avtomatlashtirish

- To'liq `artifacts/fastpq_rollouts/<stamp>/` katalogini
  Chiptani chiqaring va tarqatish yopilgandan keyin `status.md` dan havola qiling.
- `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` ishga tushiring (orqali
  `promtool`) CI ichida ogohlantirish to'plamlari hali ham chiqarilish bilan birga bo'lishini ta'minlash uchun
  kompilyatsiya qilish.
- To'plamni `ci/check_fastpq_rollout.sh` (yoki
  `make check-fastpq-rollout`) va siz qachon `FASTPQ_ROLLOUT_BUNDLE=<path>` o'ting
  bitta tarqatishni maqsad qilmoqchi. CI orqali bir xil skriptni chaqiradi
  `.github/workflows/fastpq-rollout.yml`, shuning uchun etishmayotgan artefaktlar a
  chiqish chiptasi yopilishi mumkin. Chiqarish quvuri tasdiqlangan to'plamlarni arxivlashi mumkin
  o'tish orqali imzolangan manifestlar bilan birga
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` gacha
  `scripts/run_release_pipeline.py`; yordamchi qayta ishlaydi
  `ci/check_fastpq_rollout.sh` (agar `--skip-fastpq-rollout-check` o'rnatilmagan bo'lsa) va
  katalog daraxtini `artifacts/releases/<version>/fastpq_rollouts/…` ga ko'chiradi.
  Ushbu darvozaning bir qismi sifatida skript Stage7 navbat chuqurligi va nol to'ldirishni qo'llaydi.
  byudjetlarni o'qish orqali `benchmarks.metal_dispatch_queue` va
  Har bir `metal` JSON dastgohidan `benchmarks.zero_fill_hotspots`.

Ushbu o'yin kitobiga rioya qilish orqali biz deterministik qabul qilishni ko'rsatishimiz mumkin, a taqdim eting
har bir tarqatish uchun bitta dalil to'plami va orqaga qaytarish mashqlarini birga tekshirib turing
imzolangan benchmark namoyon bo'ladi.
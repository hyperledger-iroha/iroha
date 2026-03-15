---
lang: uz
direction: ltr
source: docs/amx.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f11f0a83efc46035aeeaf4c1ad626a2a773303e9dfab188704016cf483a78ce6
source_last_modified: "2026-01-23T08:31:38.611123+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AMX ijro va operatsiyalar bo'yicha qo'llanma

**Holat:** qoralama (NX-17)  
**Tomoshabinlar:** Asosiy protokol, AMX/konsensus muhandislari, SRE/Telemetry, SDK va Torii jamoalari  
**Kontekst:** “Hujjatlar (egasi: Hujjatlar) – `docs/amx.md` ni vaqt diagrammalari, xatolar katalogi, operator kutishlari va PVO yaratish/foydalanish bo‘yicha ishlab chiquvchi ko‘rsatmalari bilan yangilang.”【roadmap.md:2497” yo‘l xaritasi bandini to‘ldiradi.【roadmap.md:2497

## Xulosa

Atom oʻzaro maʼlumotlar fazosi tranzaksiyalari (AMX) bitta joʻnatmaga bir nechta maʼlumotlar boʻshliqlariga (DS) tegishiga imkon beradi, shu bilan birga 1s slotning yakuniyligini, deterministik xato kodlarini va shaxsiy DS fragmentlari uchun maxfiylikni saqlaydi. Ushbu qo'llanma vaqt modeli, xatolarni kanonik qayta ishlash, operator dalillari talablari va Proof Tekshirish Ob'ektlari (PVOs) uchun ishlab chiquvchi kutishlarini qamrab oladi, shuning uchun etkazib beriladigan yo'l xaritasi Nexus dizayn qog'ozidan (`docs/source/nexus.md`) tashqarida qoladi.

Asosiy kafolatlar:

- Har bir AMX taqdimoti deterministik byudjetlarni tayyorlaydi/tasdiqlaydi; osilgan yo'laklardan ko'ra hujjatlashtirilgan kodlar bilan bekor qilishni overruns.
- Byudjetni o'tkazib yuboradigan DA namunalari mavjudlik dalillari etishmayotganligi sifatida qayd qilinadi va tranzaksiya o'tkazuvchanlikni jimgina to'xtatib turish o'rniga keyingi slot uchun navbatda qoladi.
- Isbotni tekshirish ob'ektlari (PVO) mijozlarga/to'plamchilarga nexus xosti uy ichida tezda tasdiqlaydigan artefaktlarni oldindan ro'yxatdan o'tkazishga ruxsat berish orqali og'ir dalillarni 1-slotdan ajratadi.
- IVM xostlari Space Directory'dan har bir ma'lumot maydoni uchun AXT siyosatini oladi: tutqichlar katalogda e'lon qilingan qatorga yo'naltirilishi, eng so'nggi manifest ildizini taqdim etishi, expiry_slot, handle_era va sub_nonce minimalarini qondirishi va ex00000ec bilan noma'lum ma'lumotlar maydonlarini rad etishi kerak.
- Slotning amal qilish muddati `nexus.axt.slot_length_ms` (standart `1`ms, `1`ms va `600_000`ms oraligʻida tasdiqlangan) hamda chegaralangan `nexus.axt.max_clock_skew_ms` (standart uzunlik I040ms ga, I04ms ga qisqartirilgan) foydalanadi va `60_000`ms). Xostlar `current_slot = block.creation_time_ms / slot_length_ms` ni hisoblaydi, amal qilish muddatini tekshirishni isbotlash va boshqarish uchun egrilik imtiyozini qo'llaydi va sozlangan chegaradan kattaroq egrilikni reklama qiluvchi tutqichlarni rad etadi.
- Isbot keshi TTL chegaralarini qayta ishlatish: `nexus.axt.proof_cache_ttl_slots` (standart `1`, tasdiqlangan `1`–`64`) qabul qilingan yoki rad etilgan dalillarning xost keshida qancha vaqt qolishini cheklaydi; TTL oynasi yoki isbotning `expiry_slot` tugashi bilan yozuvlar tushadi, shuning uchun takroriy himoya chegaralangan bo'lib qoladi.
- Qayta o'ynatish kitobini saqlash: `nexus.axt.replay_retention_slots` (standart `128`, tasdiqlangan `1`–`4_096`) tengdoshlar/restartlar orasida takroriy ijroni rad etish uchun saqlanadigan tutqichdan foydalanish tarixining minimal slot oynasini o'rnatadi; uni operatorlar chiqarishi kutilayotgan eng uzun ishlov berish oynasi bilan tekislang. Buxgalteriya kitobi WSV da saqlanadi, ishga tushirilganda namlanadi va saqlash oynasi va tutqichning amal qilish muddati tugagach (qaysi biri keyinroq bo'lishidan qat'iy nazar) aniq tarzda kesiladi, shuning uchun tengdosh kalitlari takroriy bo'shliqlarni qayta ochmaydi.
- Kesh holatini disk raskadrovka qilish: Torii joriy AXT siyosatining oniy tasvir versiyasini, eng so'nggi rad etishni (yo'l/sabab/versiya), keshlangan dalillarni (dataspace/reject/rootsmanifest) qaytarish uchun Torii (temetriya/ishlab chiquvchi darvozasi) ni ochib beradi. (`next_min_handle_era`/`next_min_sub_nonce`). Slot/manifest aylanishlari kesh holatida aks etishini tasdiqlash va muammolarni bartaraf etish vaqtida tutqichlarni aniq yangilash uchun ushbu so‘nggi nuqtadan foydalaning.

## Slot vaqtini belgilash modeli

### Xronologiya

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- Byudjetlar global buxgalteriya rejasiga mos keladi: mempool 70ms, DA commit ≤300ms, consensus 300ms, IVM/AMX 250ms, hisob 40ms, qorovul 40ms.【roadmap.md:2529
- DA oynasini buzgan tranzaktsiyalar mavjudlik to'g'risidagi dalil etishmayotganligi sifatida qayd qilinadi va keyingi slotda qayta urinib ko'riladi; `AMX_TIMEOUT` yoki `SETTLEMENT_ROUTER_UNAVAILABLE` kabi boshqa barcha buzilishlar sirt kodlari.
- Qo'riqchi bo'lagi telemetriya eksporti va yakuniy auditni o'zlashtiradi, shuning uchun eksportchilar qisqa vaqt ortda qolsa ham slot 1 soniyada yopiladi.
- Konfiguratsiya bo'yicha maslahatlar: sukut bo'yicha amal qilish muddati qat'iy saqlanadi (`slot_length_ms = 1`, `max_clock_skew_ms = 0`). 1 soniyali kadans uchun `slot_length_ms = 1_000` va `max_clock_skew_ms = 250`; 2 soniyali kadans uchun `slot_length_ms = 2_000` va `max_clock_skew_ms = 500` dan foydalaning. Tasdiqlangan oynadan tashqaridagi qiymatlar (`1`–`600_000`ms yoki `max_clock_skew_ms` slot uzunligidan kattaroq/`60_000`ms) konfiguratsiyani tahlil qilish vaqtida rad etiladi va eʼlon qilingan tutqich egri chiziq ichida qolishi kerak.

### Cross-DS suzish yo'lagi

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

Har bir DS fragmenti 30 ms tayyorlash oynasini bo'lak uyani yig'ishdan oldin tugatishi kerak. Yo'qolgan dalillar tengdoshlarni blokirovka qilishdan ko'ra keyingi slot uchun mempulda qoladi.

### Asboblar nazorat ro'yxati

| Metrik / Trace | Manba | SLO / Ogohlantirish | Eslatmalar |
|----------------|--------|-------------|-------|
| `iroha_slot_duration_ms` (gistogramma) / `iroha_slot_duration_ms_latest` (o'lchov) | `iroha_telemetry` | p95 ≤ 1000ms | Ci darvozasi `ans3.md` da tasvirlangan. |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (majburiy kanca) | ≥0,95 30 daqiqa oynaga | Mavjudlik yo'qligi telemetriyasidan olingan, shuning uchun har bir blok o'lchagichni yangilaydi (`crates/iroha_core/src/telemetry.rs:3524`, `crates/iroha_core/src/telemetry.rs:4558`). |
| `iroha_amx_prepare_ms` | IVM xost | p95 ≤ 30ms DS doirasida | `AMX_TIMEOUT` drayvlari bekor qilinadi. |
| `iroha_amx_commit_ms` | IVM xost | p95 ≤ 40ms DS doirasida | Delta birlashma + trigger bajarilishini qamrab oladi. |
| `iroha_ivm_exec_ms` | IVM xost | Har bir chiziqda >250ms bo'lsa, ogohlantirish | IVM qatlamli qismlarni bajarish oynasini aks ettiradi. |
| `iroha_amx_abort_total{stage}` | Ijrochi | Agar >0,05 abort/slot yoki doimiy bir bosqichli tikanlar | Bosqich belgilari: `prepare`, `exec`, `commit`. |
| `iroha_amx_lock_conflicts_total` | AMX rejalashtiruvchisi | >0,1 ziddiyatlar/uya | Noto'g'ri R/W to'plamlarini ko'rsatadi. |
| `iroha_axt_policy_reject_total{lane,reason}` | IVM xost | Tiklarni kuzating | Manifest/lane/era/sub_nonce/expiry rad etishlarini ajratib turadi. |
| `iroha_axt_policy_snapshot_cache_events_total{event}` | IVM xost | cache_miss ni faqat ishga tushirish/manifest o'zgarishi | Doimiy o'tkazib yuborishlar eskirgan siyosatning hidratsiyasini ko'rsatadi. |
| `iroha_axt_proof_cache_events_total{event}` | IVM xost | Ko'pincha kuting `hit`/`miss` | `reject`/`expired` tikanlar odatda manifest drift yoki eskirgan dalillarni bildiradi. |
| `iroha_axt_proof_cache_state{dsid,status,manifest_root_hex,verified_slot}` | IVM xost | Keshlangan dalillarni tekshirish | O'lchov qiymati keshlangan isbot uchun expiry_slot (qiyshaygan holda). |
| Mavjudlik dalillari etishmayapti (`sumeragi_da_gate_block_total{reason="missing_local_data"}`) | Yo'lak telemetriyasi | Agar DS uchun >5% tx bo'lsa, ogohlantirish | Tasdiqlovchilar yoki dalillar orqada qolgan degan ma'noni anglatadi. |

`/v2/debug/axt/cache` operatorlar uchun `iroha_axt_proof_cache_state` o'lchagichni har bir ma'lumot maydoni oniy tasviri (holat, manifest ildizi, tasdiqlangan/muddati tugashi) bilan aks ettiradi.

`iroha_amx_commit_ms` va `iroha_ivm_exec_ms` bir xil kechikish chelaklariga ega
`iroha_amx_prepare_ms`. Bekor qilish hisoblagichi har bir rad etishni chiziq identifikatori bilan belgilaydi
va bosqich (`prepare` = qatlamli qurish/validatsiya, `exec` = IVM parcha bajarilishi,
`commit` = delta birlashma + trigger takrori) shuning uchun telemetriya yoki yo'qligini ta'kidlashi mumkin.
qarama-qarshilik o'qish/yozish nomuvofiqligi yoki post-davlat birlashmasidan kelib chiqadi.

Operatorlar ushbu ko'rsatkichlarni `status.md` da slotni qabul qilish dalillari va regressiyalarni qayd etish bilan birga audit uchun arxivlashlari kerak.

### AXT oltin armaturalari

`crates/iroha_data_model/tests/axt_policy_vectors.rs` (`print_golden_vectors`) da regeneratsiya yordamchisi bilan `crates/iroha_data_model/tests/fixtures/axt_golden.rs` da deskriptor/tutqich/siyosat snapshoti uchun Norito moslamalari jonli. CoreHost `core_host_enforces_fixture_snapshot_fields` (`crates/ivm/tests/core_host_policy.rs`) da bir xil moslamalarni qatorni bogʻlash, manifest ildiz moslashuvi, amal qilish muddatining yangiligi, handle_era/sub_nonce minima va etishmayotgan maʼlumotlar maydonini rad etish uchun ishlatadi.
- Ko'p ma'lumotlar maydonili JSON moslamasi (`crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json`) deskriptor/sensorli sxemani, kanonik Norito baytlarni va Poseidon ulanishini (`compute_descriptor_binding`) o'rnatadi. `axt_descriptor_fixture` testi kodlangan baytlarni himoya qiladi va SDKlar hujjatlar/SDKlar uchun deterministik namunalarni yig'ish uchun `AxtDescriptorBuilder::builder` va `TouchManifest::from_read_write` dan foydalanishi mumkin.

### Lane katalogini xaritalash va manifestlar

- AXT siyosatining suratlari Space Directory manifest to'plami va qatorlar katalogidan tuzilgan. Har bir ma'lumot maydoni o'zining sozlangan qatoriga ko'rsatilgan; faol manifestlar manifest xeshini, faollashuv davrini (`min_handle_era`) va sub-nonce qavatni qo'shadi. Faol manifestsiz UAID bog'lashlari manifest ildizi nolga teng bo'lgan siyosat yozuvini chiqaradi, shuning uchun chiziqli yo'lak haqiqiy manifest tushguncha faol bo'lib qoladi.
- Suratdagi `current_slot` oxirgi bloklangan vaqt tamg'asidan (`creation_time_ms / slot_length_ms`) olingan bo'lib, faqat bloklangan sarlavha mavjud bo'lgunga qadar blok balandligiga tushadi.
- Telemetriya gidratlangan suratni `iroha_axt_policy_snapshot_version` (Norito bilan kodlangan snapshot xeshining pastki 64 biti) va `iroha_axt_policy_snapshot_cache_events_total{event=cache_hit|cache_miss}` orqali kesh hodisalarini ko'rsatadi. Rad etish hisoblagichlari `lane`, `manifest`, `era`, `sub_nonce` va `expiry` yorliqlaridan foydalanadi, shunda operatorlar qaysi maydon tutqichni bloklaganligini darhol ko'rishlari mumkin.

### O'zaro ma'lumotlar fazosining birlashtirilishini tekshirish ro'yxati- Kosmik katalogda ko'rsatilgan har bir ma'lumot maydonida chiziqli yozuv va faol manifest mavjudligini tasdiqlang; aylanish yangi tutqichlarni chiqarishdan oldin ulanishlarni yangilashi va ildizlarni ko'rsatishi kerak. Nollangan ildizlar manifestlar mavjud bo'lgunga qadar tutqichlar rad etilishini anglatadi va xostlar/blok tekshiruvi endi manifest ildizlari nolga teng bo'lgan tutqichlarni rad etadi.
- Ishga tushganda va Space Directory oʻzgartirilgandan soʻng, siyosat snapshot metrikasi boʻyicha bir `cache_miss` va undan soʻng barqaror `cache_hit` hodisalarini kuting; barqaror o'tkazib yuborish darajasi eskirgan yoki etishmayotgan manifest tasmasini ko'rsatadi.
- Tutqich rad etilganda, yangilangan tutqichni so'rash (`expiry`/`era`/`sub_nonce`) yoki chiziq/manifest bog'lanishini tuzatish uchun `iroha_axt_policy_reject_total{lane,reason}` va surat versiyasiga qarang. (`lane`/`manifest`). Torii disk raskadrovka so'nggi nuqtasi `/v2/debug/axt/cache` shuningdek, `reject_hints`ni `dataspace`, `target_lane`, `next_min_handle_era` va I18NI010 operatori so'nggi aniqlik bilan qaytaradi. siyosat zarbasidan keyin.

### SDK namunasi: token chiqmasdan masofaviy sarflash

1. Aktivga egalik qiluvchi maʼlumotlar maydoni roʻyxatini hamda mahalliy talab qilinadigan har qanday oʻqish/yozish teginishlarini koʻrsatuvchi AXT deskriptorini yarating; bog'lovchi xesh barqaror bo'lib qolishi uchun deskriptorni deterministik saqlang.
2. Siz kutgan manifest ko'rinishga ega masofaviy ma'lumotlar maydoni uchun `AXT_TOUCH` raqamiga qo'ng'iroq qiling; ixtiyoriy ravishda, agar xost talab qilsa, `AXT_VERIFY_DS_PROOF` orqali dalil ilova qiling.
3. Obyekt dastagini so'rang yoki yangilang va masofaviy ma'lumotlar maydoni ichida sarflaydigan `RemoteSpendIntent` bilan `AXT_USE_ASSET_HANDLE` ni chaqiring (ko'prik oyog'i yo'q). Byudjet ijrosi yuqorida tavsiflangan suratga nisbatan `remaining`, `per_use`, `sub_nonce`, `handle_era` va `expiry_slot` dan foydalanadi.
4. `AXT_COMMIT` orqali bajaring; agar xost `PermissionDenied` ni qaytarsa, yangi tutqichni olish (muddati/sub_nonce/era) yoki manifest/yo'l bog'lanishini tuzatishni qaror qilish uchun rad etish yorlig'idan foydalaning.

## Operator kutishlari

1. **Slot oldidan tayyorlik**
   - Har bir profil uchun DA attestator hovuzlarining (A=12, B=9, C=7) sog'lom ekanligiga ishonch hosil qiling; attester churn uyasi uchun Space Directory snapshot yozilgan.
   - Yangi ish yuki aralashmalarini yoqishdan oldin `iroha_amx_prepare_ms` vakolatli yuguruvchilar uchun byudjetdan past ekanligini tekshiring.

2. **Slot ichidagi monitoring**
   - Mavjudlik yetishmasligi haqida ogohlantirish (ikkita ketma-ket uyalar uchun >5%) va `AMX_TIMEOUT` da, chunki ikkalasi ham o'tkazib yuborilgan byudjetlarni bildiradi.
   - PVO keshidan foydalanishni kuzatib boring (`iroha_pvo_cache_hit_ratio`, tekshirish xizmati tomonidan eksport qilingan) yo'ldan tashqari tekshirish yuborilganlar bilan davom etishini isbotlash.

3. **Dalillarni qo‘lga olish**
   - `status.md` dan havola qilingan tungi artefaktlar to'plamiga DA kvitansiyalari to'plamini, AMX gistogrammalarini va PVO kesh hisobotlarini tayyorlang.
   - `ops/drill-log.md`-da DA jitter, oracle stalllari yoki bufer tugashi testlari ishlaganda tartibsizlik matkap natijalarini yozib oling.

4. **Runbook texnik xizmati**
   - AMX xato kodlari yoki bekor qilishlar o'zgarganda Android/Swift SDK ish kitoblarini yangilang, shunda mijoz guruhlari deterministik nosozlik semantikasini meros qilib oladi.
   - Konfiguratsiya parchalarini (masalan, `iroha_config.amx.*`) `docs/source/nexus.md` da kanonik parametrlar bilan sinxronlashtiring.

## Telemetriya va muammolarni bartaraf etish

### Telemetriyaga tezkor ma'lumotnoma

| Manba | Nimani suratga olish kerak | Buyruq / yo'l | Dalillarni kutish |
|--------|-----------------|----------------|-----------------------|
| Prometheus (`iroha_telemetry`) | Slot va AMX SLO'lar: `iroha_slot_duration_ms`, `iroha_amx_prepare_ms`, `iroha_amx_commit_ms`, `iroha_da_quorum_ratio`, `iroha_amx_abort_total{stage}` | `https://$TORII/telemetry/metrics` qirib tashlang yoki `docs/source/telemetry.md` da tasvirlangan asboblar panelidan eksport qiling. | Auditorlar p95/p99 qiymatlari va ogohlantirish holatlarini ko'rishlari uchun tungi `status.md` qaydiga gistogramma suratlarini (va agar ishga tushirilsa, ogohlantirishlar tarixini) biriktiring. |
| Torii RBC suratlari | DA/RBC qoldig‘i: har bir seans bo‘limi, ko‘rish/balandligi metama’lumotlari va DA mavjudligi hisoblagichlari (`sumeragi_da_gate_block_total{reason="missing_local_data"}`; `sumeragi_rbc_da_reschedule_total` eski). | `GET /v2/sumeragi/rbc` va `GET /v2/sumeragi/rbc/sessions` (misollar uchun `docs/source/samples/sumeragi_rbc_status.md` ga qarang). | AMX DA yong'in haqida ogohlantirganda JSON javoblarini (vaqt belgilari bilan) saqlang; Ularni voqea to'plamiga qo'shing, shunda sharhlovchilar orqa bosimning telemetriyaga mos kelishini tasdiqlashlari mumkin. |
| Isbot xizmati ko'rsatkichlari | PVO kesh holati: `iroha_pvo_cache_hit_ratio`, keshni to'ldirish/evakuatsiya hisoblagichlari, navbat chuqurligini isbotlash | `GET /metrics` isbotlash xizmatida (`IROHA_PVO_METRICS_URL`) yoki umumiy OTLP kollektori orqali. | OA/PVO yo‘l xaritasi deterministik artefaktlarga ega bo‘lishi uchun kesh urish nisbati va navbat chuqurligini AMX slot ko‘rsatkichlari bilan birga eksport qiling. |
| Qabul qilish jabduqlari | Boshqariladigan jitter ostida end-to-end aralash yuk (slot/DA/RBC/PVO) | `ci/acceptance/slot_1s.yml` (yoki CI da bir xil ishni) qayta ishga tushiring va jurnallar toʻplamini + yaratilgan artefaktlarni `artifacts/acceptance/slot_1s/<timestamp>/` da arxivlang. | GA dan oldin va yurak stimulyatori/DA sozlamalari o'zgarganda talab qilinadi; YAML ishga tushirish sarhisobini va Prometheus oniy tasvirlarini operatorga topshirish paketiga kiriting. |

### Nosozliklarni bartaraf etish o'yin kitobi

| Alomat | Avval tekshiring | Tavsiya etilgan tuzatish |
|---------|---------------|--------------------------|
| `iroha_slot_duration_ms` p95 1000ms dan yuqori tezlikda harakat qiladi | `/telemetry/metrics` dan Prometheus eksporti va DA kechiktirishlarini tasdiqlash uchun oxirgi `/v2/sumeragi/rbc` surati; oxirgi `ci/acceptance/slot_1s.yml` artefakt bilan solishtiring. | AMX partiyasi oʻlchamlarini pasaytiring yoki qoʻshimcha RBC kollektorlarini (`sumeragi.collectors.k`) yoqing, soʻng qabul qilish simlarini qayta ishga tushiring va yangi telemetriya dalillarini oling. |
| Mavjudlik spike | `/v2/sumeragi/rbc/sessions` qoldiq maydonlari (`lane_backlog`, `dataspace_backlog`) attestatorning sogʻliqni saqlash paneli bilan birga. | Nosog'lom attestatorlarni olib tashlang, yetkazib berishni tezlashtirish uchun vaqtincha `redundant_send_r` ni oshiring va tuzatish eslatmalarini `status.md` da chop eting. Ortiqcha roʻyxat tozalangandan soʻng yangilangan RBC snapshotlarini ilova qiling. |
| Kvitansiyalarda tez-tez `PVO_MISSING_OR_EXPIRED` | Tasdiqlash xizmati kesh ko'rsatkichlari + emitentning PVO rejalashtiruvchi jurnallari. | Eskirgan PVO artefaktlarini qayta tiklang, aylanish kadensini qisqartiring va `expiry_slot` dan oldin har bir SDK tutqichni yangilashiga ishonch hosil qiling. Qayta tiklangan keshni isbotlash uchun dalillar to'plamiga isbot-xizmat ko'rsatkichlarini qo'shing. |
| Takrorlangan `AMX_LOCK_CONFLICT` yoki `AMX_TIMEOUT` | `iroha_amx_lock_conflicts_total`, `iroha_amx_prepare_ms` va ta'sirlangan tranzaksiya namoyon bo'ladi. | Norito statik analizatorini qayta ishga tushiring, o'qish/yozish selektorlarini to'g'rilang (yoki to'plamni ajrating) va yangilangan manifest moslamalarini nashr qiling, shunda ziddiyat hisoblagichi dastlabki holatga qaytadi. |
| `SETTLEMENT_ROUTER_UNAVAILABLE` ogohlantirishlar | Hisob-kitob marshrutizatori jurnallari (`docs/settlement-router.md`), xazina buferi boshqaruv paneli va ta'sirlangan tushumlar. | XOR buferlarini to'ldiring yoki chiziqni faqat XOR rejimiga o'tkazing, xazina harakatini hujjatlashtiring va hisob-kitoblar davom etganligini isbotlash uchun slotni qabul qilish testini qayta o'tkazing. |

### AXT rad etish signallari

- Sabab kodlari `AxtRejectReason` sifatida olingan `policy_denied`, `proof`, `budget`, `replay_cache`, `descriptor`, `duplicate`). Blok tekshiruvi endi `AxtEnvelopeValidationFailed { message, reason, snapshot_version }` bo‘ladi, shuning uchun hodisalar rad etishni ma’lum bir siyosat suratiga bog‘lashi mumkin.
- `/v2/debug/axt/cache` `{ policy_snapshot_version, last_reject, cache, hints }`ni qaytaradi, bu erda `last_reject` so'nggi xostni rad etish qatorini/sababini/versiyasini o'z ichiga oladi va `hints` `next_min_handle_era`/I08215X/I08NI02 yonma-yonini ta'minlaydi. keshlangan isbot holati.
- Ogohlantirish shabloni: `iroha_axt_policy_reject_total{reason="manifest"}` yoki `{reason="expiry"}` 5 daqiqali oynadan oshib ketganda sahifa, `last_reject` suratini + `policy_snapshot_version` dan `policy_snapshot_version` ilova qiling va hodisani toʻlash uchun toʻlovni toʻlang va toʻlovdan foydalaning. qayta urinishdan oldin yangilangan tutqichlar.

## Isbotni tekshirish ob'ektlari (PVO)

### Tuzilishi

PVO'lar Norito kodli konvertlar bo'lib, mijozlarga og'ir ishlarni muddatidan oldin isbotlash imkonini beradi. Kanonik maydonlar quyidagilardir:

| Maydon | Tavsif |
|-------|-------------|
| `circuit_id` | Tasdiqlash tizimi/bayonoti uchun statik identifikator (masalan, `amx.transfer.v1`). |
| `vk_hash` | DS manifestida havola qilingan tekshirish kalitining Blake2b-256 xeshi. |
| `proof_digest` | Slotdan tashqari PVO registrida saqlangan ketma-ket isbotlangan foydali yukning Poseidon digesti. |
| `max_k` | AIR domenidagi yuqori chegara; xostlar e'lon qilingan hajmdan oshgan dalillarni rad etadi. |
| `expiry_slot` | Slot balandligi shundan keyin artefakt yaroqsiz; eskirgan dalillarni bo'laklardan saqlaydi. |
| `profile` | Rejalashtiruvchilarga profilni baham ko'radigan dalillarni to'plashda yordam berish uchun ixtiyoriy maslahatlar (masalan, DS profili A/B/C). |

Norito sxemasi `crates/iroha_data_model/src/nexus` da ma'lumotlar modeli ta'riflari bilan birga yashaydi, shuning uchun SDKlar uni serdesiz olishlari mumkin.

### Ishlab chiqarish quvuri1. **Sxema metamaʼlumotlarini kompilyatsiya qilish** — `circuit_id`, tekshirish kaliti va prover tuzilmasidan maksimal iz hajmini eksport qiling (odatda `fastpq_prover` hisobotlari orqali).
2. **Tasdiqlangan artefaktlar yarating** — Slotdan tashqari proverni ishga tushiring va to'liq transkriptlarni va majburiyatlarni saqlang.
3. **Tasdiqlash xizmati orqali roʻyxatdan oʻting** — Norito PVO foydali yukini off-slot tekshirgichga yuboring (NX-17 proof quvur liniyasi yoʻl xaritasiga qarang). Xizmat bir marta tekshiradi, dayjestni mahkamlaydi va Torii orqali tutqichni ochib beradi.
4. **Tranzaksiyalarda havola** — PVO tutqichini AMX quruvchilarga (`amx_touch` yoki undan yuqori darajadagi SDK yordamchilariga) biriktiring. Xostlar dayjestni qidiradi, keshlangan natijani tekshiradi va faqat kesh sovuq bo'lsa, slot ichida qayta hisoblaydi.
5. **Muddati tugashi bilan aylantirish** — SDKlar keshlangan tutqichni `expiry_slot` dan oldin yangilashi kerak. Muddati o'tgan ob'ektlar `PVO_MISSING_OR_EXPIRED` ni ishga tushiradi.

### Ishlab chiquvchi nazorat ro'yxati

- AMX qulflarni oldindan yuklashi va `AMX_LOCK_CONFLICT`dan qochishi uchun o'qish/yozish to'plamlarini aniq e'lon qiling.
- Bir xil UAID manifest yangilanishidagi deterministik to'lov dalillarini to'plang.
- Qayta urinish strategiyasi: mavjudlik dalillari etishmayapti → hech qanday harakat yo'q (tx mempulda qoladi); `AMX_TIMEOUT` yoki `PVO_MISSING_OR_EXPIRED` → artefaktlarni qayta tiklang va eksponent ravishda orqaga qayting.
- Determinizm regressiyasidan himoyalanish uchun testlar kesh va sovuq ishga tushirishlarni (xostni bir xil `max_k` bilan dalilni tekshirishga majburlash) o'z ichiga olishi kerak.
- Proof bloblar (`ProofBlob`) `AxtProofEnvelope { dsid, manifest_root, da_commitment?, proof }` kodlashi KERAK; xostlar Space Directory manifest root va `iroha_axt_proof_cache_events_total{event="hit|miss|expired|reject|cleared"}` bilan har bir ma'lumot maydoni/uyasiga o'tish/muvaffaqiyatsiz natijalarni keshga bog'lash dalillarini. Muddati o'tgan yoki manifestga mos kelmaydigan artefaktlar keshlangan `reject` da bir xil slot qisqa tutashuvida bajarilishdan va keyingi qayta urinishlardan oldin rad etiladi.
- Isbot keshini qayta ishlatish slotga bog'liq: tekshirilgan dalillar bir xil slot ichidagi konvertlar bo'ylab issiq bo'lib qoladi va slot ilgarilaganda avtomatik ravishda chiqarib yuboriladi, shuning uchun qayta urinishlar deterministik bo'lib qoladi.

### Statik o'qish/yozish analizatori

Kompilyatsiya vaqti selektorlari AMXdan oldin shartnomaning haqiqiy xatti-harakatlariga mos kelishi kerak
qulflarni oldindan yuklash yoki UAID manifestlarini qo'llash. Yangi `ivm::analysis` moduli
(`crates/ivm/src/analysis.rs`) kodni dekodlaydigan `analyze_program(&[u8])` ni ochib beradi.
`.to` artefakt, o'qish/yozish, xotira operatsiyalari va tizim qo'ng'iroqlarini ro'yxatga olish,
va SDK manifestlari joylashtirishi mumkin bo'lgan JSON-do'st hisobotni ishlab chiqaradi. Uni ishga tushiring
UAIDlarni nashr qilishda `koto_lint` bilan birga ishlab chiqarilgan R/W xulosasi
NX-17 tayyorligini tekshirish paytida havola qilingan dalillar to'plamida olingan.

## Space Directory siyosatini amalga oshirish

AXT tutqichini tekshirish endi xost unga kirish imkoniga ega bo'lganda Space Directory snapshotiga o'rnatiladi (testlarda CoreHost, integratsiya oqimlarida WsvHost). Har bir maʼlumot maydoni siyosati yozuvlarida `manifest_root`, `target_lane`, `min_handle_era`, `min_sub_nonce` va `current_slot` mavjud. Xostlar amal qiladi:

- chiziqli bog'lash: `target_lane` tutqichi Space Directory yozuviga mos kelishi kerak;
- manifest bog'lanishi: nolga teng bo'lmagan `manifest_root` qiymatlari tutqichning `manifest_view_root` qiymatiga mos kelishi kerak;
- amal qilish muddati: `current_slot` dastagi `expiry_slot`dan kattaroq rad etildi;
- hisoblagichlar: `handle_era` va `sub_nonce` kamida e'lon qilingan minimal bo'lishi kerak;
- a'zolik: oniy tasvirda mavjud bo'lmagan ma'lumotlar bo'shliqlari uchun ishlov berish rad etiladi.

Xatolar `PermissionDenied` ga mos keladi va `crates/ivm/tests/core_host_policy.rs` da CoreHost siyosatining oniy surati testlari har bir maydon uchun holatlarga ruxsat berish/rad etishni qamrab oladi.
Blokni tekshirish, shuningdek, `expiry_slot` bilan siyosat uyasi (konfiguratsiya qilingan egrilik ruxsati bilan) va tutqichdan oldin muddati tugamaydigan maʼlumotlar maydoni uchun boʻsh boʻlmagan dalillarni talab qiladi, eʼlon qilingan spetsifikatsiyalar uchun deskriptorni bogʻlash va teginish manifestlarini amalga oshiradi (va kiruvchi tekshiruvlarni rad etadi), miqdorlar, doira/mavzuni moslashtirish va nolga teng boʻlmagan davr/sub_nonce/expiry), agregatlar har bir maʼlumot maydoni uchun byudjetlarni boshqaradi va konvertlar bajarilgani uchun `min_handle_era`/`min_sub_nonce` avanslar, shuning uchun takrorlangan sub-noces boʻsh joydan keyin ham rad etiladi.

## Xato katalogi

Kanonik kodlar `crates/iroha_data_model/src/errors.rs` da yashaydi. Operatorlar ularni o'lchovlar/jurnallarda so'zma-so'z ko'rsatishi kerak va SDK ularni amaldagi qayta urinishlar bilan taqqoslashi kerak.

| Kod | Trigger | Operator javobi | SDK ko'rsatmalari |
|------|---------|-------------------|--------------|
| Mavjudlik dalillari etishmayotgan (telemetriya) | 300 ms dan oldin tasdiqlangan `q` dan kamroq attestator cheklari. | Attestatorning sog'lig'ini tekshiring, keyingi slot uchun namuna olish parametrlarini kengaytiring, tranzaksiyani navbatda saqlang va runbook dalillari uchun etishmayotgan hisoblagichlarni oling. | Hech qanday harakat yo'q; qayta urinish avtomatik ravishda sodir bo'ladi, chunki tx navbatda qoladi. |
| `DA_DEADLINE_EXCEEDED` | D oynasi DA kvorumiga javob bermasdan o'tdi. | Huquqbuzar attestatorlarni iste'foga chiqaring, voqea qaydini e'lon qiling, mijozlarni qayta topshirishga majburlang. | Attestatorlar qaytib kelgach, tranzaksiyani qayta tiklang; partiyani bo'lish haqida o'ylab ko'ring. |
| `AMX_TIMEOUT` | Birlashtirilgan tayyorlanish/tasdiqlash DS boʻlagi uchun 250ms dan oshdi. | Olovli grafiklarni oling, R/W to'plamlarini tekshiring va `iroha_amx_prepare_ms` bilan solishtiring. | Kichikroq to'plam bilan yoki tortishuvni kamaytirgandan keyin qayta urinib ko'ring. |
| `AMX_LOCK_CONFLICT` | Xost bir-biriga o'xshash yozish to'plamlari yoki signalsiz teginishlarni aniqladi. | UAID manifestlarini va statik tahlil hisobotlarini tekshiring; Agar selektorlar yo'q bo'lsa, yangilash namoyon bo'ladi. | Tuzatilgan o'qish/yozish deklaratsiyasi bilan tranzaktsiyani qayta kompilyatsiya qilish. |
| `PVO_MISSING_OR_EXPIRED` | Yo'naltirilgan PVO tutqichi keshda yoki o'tgan `expiry_slot`da emas. | Isbot xizmatining qoldirilganligini tekshiring, artefaktni qayta tiklang va Torii indekslarini tekshiring. | Tasdiqlangan artefaktni yangilang va yangi tutqich bilan qayta yuboring. |
| `RWSET_UNBOUNDED` | Statik tahlil o‘qish/yozish selektorini bog‘lay olmadi. | Joylashtirishni rad etish, jurnal tanlash stekini kuzatish, qayta urinishdan oldin ishlab chiquvchi tuzatishni talab qilish. | Aniq selektorlarni chiqarish uchun shartnomani yangilang. |
| `HEAVY_INSTRUCTION_DISALLOWED` | Shartnoma AMX yo'laklarida (masalan, PVOsiz katta FFT) taqiqlangan ko'rsatmani chaqirdi. | Qayta yoqishdan oldin Norito quruvchisi tasdiqlangan opcode to'plamidan foydalanishiga ishonch hosil qiling. | Ish yukini ajrating yoki oldindan hisoblangan dalilni qo'shing. |
| `SETTLEMENT_ROUTER_UNAVAILABLE` | Router deterministik konvertatsiyani hisoblay olmadi (yo'l yo'q, bufer drenajlangan). | Buferlarni to'ldirish yoki faqat XOR rejimini o'zgartirish uchun G'aznachilikni jalb qiling; hisob-kitoblar kitobida qayd etish. | Bufer ogohlantirishi oʻchirilgandan keyin qayta urinib koʻring; foydalanuvchiga qarshi ogohlantirishni ko'rsatish. |

SDK guruhlari ushbu kodlarni integratsiya testlarida aks ettirishi kerak, shuning uchun `iroha_cli`, Android, Swift, JS va Python sirtlari xato matni va tavsiya etilgan harakatlarga rozi bo'ladi.

### AXT rad etish kuzatilishi

- Torii siyosat xatoliklarini barqaror sabab yorlig'i, faol `snapshot_version`, ixtiyoriy `lane`/I10intifiers0000000279X/I10intifiers00 bilan `ValidationFail::AxtReject` (va blok tekshiruvi `AxtEnvelopeValidationFailed`) sifatida ko'rsatadi. `next_min_handle_era`/`next_min_sub_nonce` uchun. SDK'lar bu maydonlarni foydalanuvchilarga puflashi kerak, shunda eskirgan tutqichlar aniq yangilanishi mumkin.
- Endi Torii HTTP javoblarini tezkor triaj uchun `X-Iroha-Axt-*` sarlavhalari bilan muhrlaydi: `Code`/`Reason`, `Snapshot-Version`, `Snapshot-Version`, `dataspace`, I000002802 `Next-Handle-Era`/`Next-Sub-Nonce`. ISO ko'prigini rad etish mos keladigan `PRTRY:AXT_*` sabab kodlari va bir xil tafsilotlar qatorlarini o'z ichiga oladi, shuning uchun asboblar paneli va operatorlar to'liq yukni dekodlashsiz AXT nosozlik sinfidan asosiy ogohlantirishlarni olishlari mumkin.
- Xostlar `AXT policy rejection recorded` ni bir xil maydonlar bilan qayd qiladi va ularni telemetriya orqali eksport qiladi: `iroha_axt_policy_reject_total{lane,reason}` rad etishlarni hisoblaydi va `iroha_axt_policy_snapshot_version` faol suratning xeshini kuzatadi. Kesh holatini tasdiqlash `/v2/debug/axt/cache` (ma'lumotlar maydoni/status/manifest ildizi/slotlari) orqali mavjud bo'lib qoladi.
- Ogohlantirish: operatorlar manifestlarni aylantirishi (yoʻlak/manifestni rad etish) yoki/refresh tutqichini_rezalash kerakmi yoki yoʻqligini tasdiqlash uchun `reason` va `snapshot_version` sahifasi boʻyicha `reason` boʻyicha guruhlangan va `snapshot_version` sahifasini kuzating. Rad qilish keshga yoki siyosatga bog‘liqligini tasdiqlash uchun ogohlantirishlarni proof-kesh so‘nggi nuqtasi bilan bog‘lang.

## Sinov va dalillar

- CI `ci/acceptance/slot_1s.yml` to'plamini ishga tushirishi kerak (30 daqiqa aralash ish yuki) va `ans3.md` da ta'kidlanganidek, slot/DA/temetriya chegaralari bajarilmasa, birlasha olmaydi.
- Xaos mashqlari (attester jitteri, oracle stalls, buferning tugashi) `ops/drill-log.md` ostida arxivlangan artefaktlar bilan kamida har chorakda bajarilishi kerak.
- Status yangilanishlari quyidagilarni o'z ichiga olishi kerak: eng so'nggi SLO uyasi hisoboti, ko'zga ko'rinmas xatoliklar va so'nggi PVO kesh suratiga havola, shuning uchun manfaatdor tomonlar yo'l xaritasi tayyorligini tekshirishlari mumkin.

Ushbu qo'llanmaga rioya qilish orqali hissa qo'shuvchilar AMX hujjatlari uchun yo'l xaritasi talabini qondiradi va operatorlar va ishlab chiquvchilarga vaqt, telemetriya va PVO ish oqimlari uchun yagona ma'lumotnoma beradi.
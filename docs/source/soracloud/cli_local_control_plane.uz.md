<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/source/soracloud/cli_local_control_plane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 567b63e9b61afaecfa5d85aa60f0348c856557e171559885ffaba45168ce61dc
source_last_modified: "2026-03-26T06:12:11.480025+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud CLI va boshqaruv tekisligi

Soracloud v1 - bu vakolatli, faqat IVM ish vaqti.

- `iroha app soracloud init` yagona oflayn buyruqdir. U iskala qiladi
  `container_manifest.json`, `service_manifest.json` va ixtiyoriy shablon
  Soracloud xizmatlari uchun artefaktlar.
- Boshqa barcha Soracloud CLI buyruqlari faqat tarmoq tomonidan quvvatlanadi va talab qilinadi
  `--torii-url`.
- CLI mahalliy Soracloud boshqaruv tekisligi oynasi yoki holatini saqlamaydi
  fayl.
- Torii ommaviy Soracloud holati va mutatsiya yo'nalishlariga bevosita xizmat qiladi
  nufuzli dunyo davlati va o'rnatilgan Soracloud ish vaqti menejeri.

## Ish vaqti doirasi- Soracloud v1 faqat `SoraContainerRuntimeV1::Ivm` ni qabul qiladi.
- `NativeProcess` rad etilganligicha qolmoqda.
- To'g'ridan-to'g'ri IVM ishlov beruvchilari tomonidan qabul qilingan pochta qutilarining buyurtma qilingan bajarilishi.
- Hidratsiya va materializatsiya SoraFS/DA tarkibidan kelib chiqadi.
  sintetik mahalliy suratlarga qaraganda.
- `SoraContainerManifestV1` endi `required_config_names` va
  `required_secret_names` va aniq `config_exports`. Joylashtirish, yangilash,
  va orqaga qaytarish muvaffaqiyatsizligi samarali vakolatli materiallar to'plami bo'lganda yopiladi
  e'lon qilingan majburiyatlarni qoniqtirmaslik yoki konfiguratsiya eksporti a
  talab qilinmaydigan konfiguratsiya yoki dublikat env/fayl manzili.
- Qabul qilingan xizmat konfiguratsiyasi yozuvlari endi amalga oshirildi
  `services/<service>/<version>/configs/<config_name>` kanonik JSON sifatida
  foydali yuk fayllari.
- Aniq konfiguratsiya env eksporti prognoz qilingan
  `services/<service>/<version>/effective_env.json` va fayl eksporti
  ostida amalga oshirildi
  `services/<service>/<version>/config_exports/<relative_path>`. Eksport qilingan
  qiymatlar havola qilingan konfiguratsiya yozuvining kanonik JSON foydali yuk matnidan foydalanadi.
- Soracloud IVM ishlov beruvchilari endi o'sha nufuzli konfiguratsiya yuklamalarini o'qiy oladi
  to'g'ridan-to'g'ri ish vaqti xost orqali `ReadConfig` yuzasi, shuning uchun oddiy
  `query`/`update` ishlov beruvchilari tugun-mahalliy fayl yo'llarini taxmin qilishlari shart emas.
  belgilangan xizmat konfiguratsiyasini iste'mol qilish.
- Majburiy xizmat maxfiy konvertlari hozirda amalga oshirilmoqda
  `services/<service>/<version>/secret_envelopes/<secret_name>` sifatida
  vakolatli konvert fayllari.- Oddiy Soracloud IVM ishlovchilari endi o'sha maxfiy ma'lumotlarni o'qiy oladilar.
  to'g'ridan-to'g'ri ish vaqti xost `ReadSecretEnvelope` yuzasi orqali konvertlar.
- Eski shaxsiy ish vaqtining zaxira daraxti endi bajarilgandan sinxronlashtirildi
  `secrets/<service>/<version>/<secret_name>` ostida joylashtirish holati, shuning uchun
  eski xom maxfiy o'qish yo'li va da vakolatli nazorat tekisligi nuqtasi
  bir xil baytlar.
- Shaxsiy ish vaqti `ReadSecret` endi vakolatli joylashtirishni hal qiladi
  `service_secrets` birinchi va faqat eski tugunga qaytadi-mahalliy
  `secrets/<service>/<version>/...` bajarilmaganda fayl daraxti materiallashtirilgan
  so'ralgan kalit uchun xizmat maxfiy yozuvi mavjud.
- Maxfiy qabul qilish hali ham konfiguratsiyani qabul qilishdan ataylab torroqdir:
  `ReadSecretEnvelope` - bu jamoat uchun xavfsiz oddiy ishlovchi shartnomasi, shu bilan birga
  `ReadSecret` faqat shaxsiy ish vaqti bo'lib qoladi va hali ham majburiyatni qaytaradi
  konvertdagi shifrlangan matn baytlari ochiq matnni ulash shartnomasidan ko'ra.
- Ish vaqti xizmat rejalari endi tegishli qabul qilish imkoniyatini ochib beradi
  mantiqiy va e'lon qilingan `config_exports` va samarali prognoz qilingan
  muhit, shuning uchun maqom iste'molchilari amalga oshirilgan qayta ko'rib chiqish yoki yo'qligini aytishlari mumkin
  xost konfiguratsiyasini o'qishni, xost maxfiy konvertini o'qishni, shaxsiy xom sirni qo'llab-quvvatlaydi
  o'qiydi va aniq konfiguratsiyani ishlov beruvchidan xulosa chiqarmasdan
  yolg'iz darslar.

## CLI buyruqlari- `iroha app soracloud init`
  - faqat oflayn iskala.
  - `baseline`, `site`, `webapp` va `pii-app` shablonlarini qo'llab-quvvatlaydi.
- `iroha app soracloud deploy`
  - `SoraDeploymentBundleV1` qabul qoidalarini mahalliy darajada tasdiqlaydi, imzolaydi
    so'rov yuboradi va `POST /v1/soracloud/deploy` ga qo'ng'iroq qiladi.
  - `--initial-configs <path>` va `--initial-secrets <path>` endi biriktirilishi mumkin
    nufuzli inline xizmat konfiguratsiyasi / maxfiy xaritalari bilan atomik ravishda
    birinchi qabul qilishda kerakli bog'lanishlar qondirilishi uchun birinchi marta joylashtiring.
  - CLI endi HTTP so'rovini kanonik ravishda ikkalasi bilan imzolaydi
    `X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms` va
    Oddiy bitta imzoli hisoblar uchun `X-Iroha-Nonce` yoki
    `X-Iroha-Account` plus `X-Iroha-Witness` qachon
    `soracloud.http_witness_file` multisig guvohi JSON foydali yukiga ishora qiladi;
    Torii deterministik tranzaksiya ko'rsatmalari to'plamini va
    Keyin CLI haqiqiy tranzaksiyani oddiy Iroha mijozi orqali yuboradi
    qator.
  - Torii shuningdek, SCR-host qabul qilish cheklovlarini va muvaffaqiyatsiz yopilish qobiliyatini qo'llaydi.
    mutatsiyani qabul qilishdan oldin tekshiradi.
- `iroha app soracloud upgrade`
  - yangi to'plamni qayta ko'rib chiqishni tasdiqlaydi va imzolaydi, keyin qo'ng'iroq qiladi
    `POST /v1/soracloud/upgrade`.
  - bir xil `--initial-configs <path>` / `--initial-secrets <path>` oqimi
    yangilash vaqtida atom materiali yangilanishi uchun mavjud.
  - Xuddi shu SCR xostiga kirish tekshiruvlari yangilanishdan oldin server tomonida ishlaydi
    tan olingan.
- `iroha app soracloud status`- `GET /v1/soracloud/status` dan vakolatli xizmat holatini so'raydi.
- `iroha app soracloud config-*`
  - `config-set`, `config-delete` va `config-status` faqat Torii tomonidan quvvatlanadi.
  - CLI kanonik xizmat konfiguratsiyasining foydali yuklari va qo'ng'iroqlariga imzo chekadi
    `POST /v1/soracloud/service/config/set`,
    `POST /v1/soracloud/service/config/delete`, va
    `GET /v1/soracloud/service/config/status`.
  - konfiguratsiya yozuvlari vakolatli joylashtirish holatida saqlanadi va qoladi
    joylashtirish/yangilash/qayta ko'rib chiqish o'zgarishlariga biriktirilgan.
  - `config-delete` endi faol reviziya hali e'lon qilinganda yopilmaydi
    `container.required_config_names` da nomlangan konfiguratsiya.
- `iroha app soracloud secret-*`
  - `secret-set`, `secret-delete` va `secret-status` faqat Torii tomonidan quvvatlanadi.
  - CLI kanonik xizmat-maxfiy kelib chiqish yuklari va qo'ng'iroqlariga imzo chekadi
    `POST /v1/soracloud/service/secret/set`,
    `POST /v1/soracloud/service/secret/delete`, va
    `GET /v1/soracloud/service/secret/status`.
  - maxfiy yozuvlar ishonchli `SecretEnvelopeV1` yozuvlari sifatida saqlanadi
    joylashtirish holatida va normal xizmatni qayta ko'rib chiqish o'zgarishlaridan omon qoling.
  - `secret-delete` endi faol reviziya hali e'lon qilinganda yopilmaydi
    `container.required_secret_names` da nomlangan sir.
- `iroha app soracloud rollback`
  - orqaga qaytish metama'lumotlarini imzolaydi va `POST /v1/soracloud/rollback` ga qo'ng'iroq qiladi.
- `iroha app soracloud rollout`
  - tarqatish metama'lumotlarini imzolaydi va `POST /v1/soracloud/rollout` ga qo'ng'iroq qiladi.
- `iroha app soracloud agent-*`
  - kvartiraning barcha hayot aylanishi, hamyon, pochta qutisi va avtonomiya buyruqlari
    Faqat Torii tomonidan quvvatlanadi.
- `iroha app soracloud training-*`
  - barcha o'quv topshiriqlari buyruqlari faqat Torii tomonidan qo'llab-quvvatlanadi.- `iroha app soracloud model-*`
  - barcha model artefakt, vazn, yuklangan model va shaxsiy ish vaqti buyruqlari
    faqat Torii tomonidan qo'llab-quvvatlanadi.
  - yuklangan model/xususiy ish vaqti yuzasi endi bitta oilada yashaydi:
    `model-upload-encryption-recipient`, `model-upload-init`,
    `model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
    `model-compile`, `model-compile-status`, `model-allow`,
    `model-run-private`, `model-run-status`, `model-decrypt-output` va
    `model-publish-private`.
  - `model-run-private` endi qoralamani, keyin yakunlovchi ish vaqtidagi suhbatni yashiradi
    CLI va yakuniy yakunlashdan keyingi vakolatli sessiya holatini qaytaradi.
  - `model-publish-private` endi tayyorlangan ikkalasini ham qo'llab-quvvatlaydi
    to'plam/bo'lak/yakunlash/kompilyatsiya/nashr qilishga ruxsat berish va yuqori darajadagi reja
    hujjat loyihasi. Loyihada endi `source: PrivateModelSourceV1`,
    qaysi `LocalDir { path }` yoki qabul qiladi
    `HuggingFaceSnapshot { repo, revision }`.
  - `--draft-file` bilan chaqirilganda, CLI e'lon qilingan manbani normallashtiradi
    deterministik temp daraxtiga, v1 HF safetensors shartnomasini tasdiqlaydi,
    to'plamni faolga qarshi deterministik ravishda seriyalashtiradi va shifrlaydi
    Torii yuklovchi qabul qiluvchi, uni qattiq oʻlchamdagi shifrlangan boʻlaklarga boʻlinadi,
    ixtiyoriy ravishda tayyorlangan rejani `--emit-plan-file` orqali yozadi va keyin
    yuklash/yakunlash/kompilyatsiya/ruxsat berish ketma-ketligini bajaradi.
  - `HuggingFaceSnapshot` qayta ko'rib chiqishlari majburiydir va majburiy ravishda biriktirilishi kerak
    SHA; filialga o'xshash referatlar muvaffaqiyatsiz yopilgan holda rad etiladi.- qabul qilingan manba tartibi v1 da ataylab tor: `config.json`,
    tokenizator aktivlari, bir yoki bir nechta `*.safetensors` parchalari va ixtiyoriy
    tasvirga ega modellar uchun protsessor/preprotsessor metama'lumotlari. GGUF, ONNX,
    boshqa himoyalanmagan og'irliklar va o'zboshimchalik bilan joylashtirilgan maxsus tartiblar
    rad etilgan.
  - `--plan-file` bilan chaqirilganda, CLI hali ham allaqachon iste'mol qiladi
    nashr-reja hujjatini tayyorlaydi va reja yuklanganda muvaffaqiyatsiz yopiladi
    oluvchi endi vakolatli Torii oluvchiga mos kelmaydi.
  - ushbu marshrutlarni qatlamli dizayn uchun `uploaded_private_models.md` ga qarang
    mavjud model reestriga va artefakt/vazn yozuvlariga.
- `model-host` boshqaruv-samolyot marshrutlari
  - Torii endi avtoritetni fosh qiladi
    `POST /v1/soracloud/model-host/advertise`,
    `POST /v1/soracloud/model-host/heartbeat`,
    `POST /v1/soracloud/model-host/withdraw`, va
    `GET /v1/soracloud/model-host/status`.
  - bu marshrutlar ro'yxatdan o'tishni tasdiqlovchi xost qobiliyatini reklama qilishda davom etadi
    nufuzli dunyo davlati va operatorlarga qaysi validatorlar ekanligini tekshirishga ruxsat bering
    hozirda reklama modeli-host sig'imi.
  - `iroha app soracloud model-host-advertise`,
    `model-host-heartbeat`, `model-host-withdraw` va
    `model-host-status` endi bir xil kanonik kelib chiqish yuklarini imzolaydi.
    raw API va mos keladigan Torii marshrutlarini to'g'ridan-to'g'ri chaqiring.
- `iroha app soracloud hf-*`
  - `hf-deploy`, `hf-status`, `hf-lease-leave` va `hf-lease-renew`
    Faqat Torii tomonidan quvvatlanadi.- `hf-deploy` va `hf-lease-renew` endi deterministikni avtomatik ravishda qabul qiladi
    so'ralgan `service_name` uchun HF xulosasi xizmatini yaratdi va
    uchun deterministik hosil qilingan HF kvartirani avtomatik ravishda qabul qiling
    `apartment_name` so'ralganda, umumiy ijara mutatsiyasidan oldin
    topshiriladi.
  - qayta foydalanish muvaffaqiyatsiz yopilgan: agar nomlangan xizmat/kvartira allaqachon mavjud bo'lsa, lekin mavjud bo'lsa
    bu kanonik manba, HF uchun kutilgan ishlab chiqarilgan HF joylashuvi emas
    lizingni aloqasi bo'lmaganlarga jimgina bog'lash o'rniga mutatsiya rad etiladi
    Soracloud ob'ektlari.
  - o'rnatilgan ish vaqti menejeri biriktirilganda, endi `hf-status` ham
    kanonik manba uchun ish vaqti proyeksiyasini qaytaradi, shu jumladan bog'langan
    xizmatlar/kvartiralar, navbatda turgan keyingi oynaning koʻrinishi va mahalliy toʻplam/
    artefakt keshini o'tkazib yuborish; `importer_pending` ushbu ish vaqti proyeksiyasini kuzatib boradi
    faqat nufuzli manba enumiga tayanish o'rniga.
  - `hf-deploy` yoki `hf-lease-renew` ishlab chiqarilgan HF xizmatini qabul qilganda
    umumiy ijara mutatsiyasi bilan bir xil operatsiya, vakolatli HF
    manba darhol `Ready` ga aylanadi va `importer_pending` qoladi
    Javobda `false`.
  - HF ijarasi holati va mutatsiyaga javoblar endi har qanday vakolatli shaxslarni ham ochib beradi
    joylashtirish surati allaqachon faol ijara oynasiga biriktirilgan, shu jumladantayinlangan xostlar, muvofiq-host soni, iliq xostlar soni va alohida
    saqlash-hisoblash to'lovi maydonlari.
  - `hf-deploy` va `hf-lease-renew` endi kanonik HF manbasini olishadi
    Hugging Face repo metamaʼlumotlaridan profilni ular yuborishdan oldin
    mutatsiya:
    - Torii repo `siblings` ni tekshiradi, `.gguf` ni afzal ko'radi
      PyTorch vazn sxemalarida `.safetensors`, tanlangan fayllarni HEADs
      `required_model_bytes` ni chiqaring va uni birinchi reliz bilan taqqoslang
      backend/format va RAM/disk qavatlari;
    - jonli validator xost reklamasi mumkin bo'lmasa, ijaraga qabul qilish yopilmaydi
      ushbu profilni qondirish; va
    - xost to'plami mavjud bo'lganda, faol oyna endi a yozadi
      deterministik stavka bo'yicha joylashtirish va alohida hisoblash bron qilish
      mavjud saqlash ijarasi hisobi bilan birga to'lov.
  - faol HF oynasiga qo'shilgan keyingi a'zolar endi proporsional saqlash va to'laydi
    faqat qolgan oyna uchun aktsiyalarni hisoblash, oldingi a'zolar esa
    bir xil deterministik saqlash qaytarib olish va hisoblash-qaytarib buxgalteriya hisobi
    kech qo'shilishdan.
  - o'rnatilgan ish vaqti menejeri yaratilgan HF stub to'plamini sintez qilishi mumkin
    mahalliy, shuning uchun o'sha yaratilgan xizmatlar a kutmasdan amalga oshirilishi mumkin
    faqat toʻldiruvchi xulosalar toʻplami uchun SoraFS foydali yukni belgiladi.- o'rnatilgan ish vaqti menejeri endi ruxsat etilgan Hugging Face repo ham import qiladi
    fayllarni `soracloud_runtime.state_dir/hf_sources/<source_id>/files/` va
    hal qilingan majburiyat bilan mahalliy `import_manifest.json` davom etadi, import qilinadi
    fayllar, o'tkazib yuborilgan fayllar va har qanday importer xatosi.
  - yaratilgan HF `metadata` mahalliy o'qishlar endi mahalliy import manifestini qaytaradi,
    import qilingan fayl inventarini, jumladan, mahalliy ijro va
    tugun uchun ko'prikni qaytarish yoqilgan.
  - hosil qilingan HF `infer` mahalliy o'qishlar endi tugunda bajarilishini afzal ko'radi
    import qilingan umumiy baytlar:
    - `irohad` mahalliy dastur ostida o'rnatilgan Python adapter skriptini amalga oshiradi
      Soracloud ish vaqti davlat katalogi va uni chaqiradi
      `soracloud_runtime.hf.local_runner_program`;
    - o'rnatilgan yuguruvchi avval deterministik armatura bandini tekshiradi
      `config.json` (sinovlar tomonidan ishlatiladi), keyin aks holda import qilingan manbani yuklaydi
      `transformers.pipeline(..., local_files_only=True)` orqali katalog shunday
      model tortish o'rniga umumiy mahalliy importga qarshi ishlaydi
      yangi Hub baytlari; va
    - agar `soracloud_runtime.hf.allow_inference_bridge_fallback = true` va
      `soracloud_runtime.hf.inference_token` sozlangan, ish vaqti tushadi
      faqat mahalliy bajarilganda konfiguratsiya qilingan HF Inference asosiy URL manziliga qayting
      mavjud emas yoki bajarilmaydi va qo'ng'iroq qiluvchi uni aniq tanlaydi
      `x-soracloud-hf-allow-bridge-fallback: 1`, `true` yoki `yes`.
  - ish vaqti proyeksiyasi endi HF manbasini `PendingImport` da saqlaydi.Muvaffaqiyatli mahalliy import manifest mavjud va import qiluvchining nosozliklari yuzaga keladi
    ish vaqti `Ready` jimgina xabar berish o'rniga `Failed` plus `last_error`.
  - ishlab chiqarilgan HF kvartiralar endi tasdiqlangan avtonomiya orqali iste'mol qiladi
    tugun-mahalliy ish vaqti yo'li:
    - `agent-autonomy-run` endi ikki bosqichli oqimga amal qiladi: birinchi imzolangan
      mutatsiya vakolatli tasdiqlashni qayd qiladi va deterministikni qaytaradi
      tranzaksiya loyihasi, keyin ikkinchi imzolangan yakuniy so'rov so'raydi
      chegaraga qarshi tasdiqlangan ishga tushirish uchun o'rnatilgan ish vaqti menejeri
      HF `/infer` xizmatini yaratdi va har qanday vakolatli kuzatuvni qaytaradi
      boshqa deterministik loyiha sifatida ko'rsatmalar;
    - tasdiqlangan yugurish rekordi endi kanonik bo'lib qoladi
      `request_commitment`, shuning uchun keyinroq yaratilgan xizmat kvitansiyasi bo'lishi mumkin
      aniq vakolatli avtonomiyani tasdiqlash bilan bog'liq;
    - tasdiqlashlar endi ixtiyoriy kanonik `workflow_input_json` davom etishi mumkin
      tanasi; mavjud bo'lganda, o'rnatilgan ish vaqti aynan JSON yuklamasini oldinga siljitadi
      yaratilgan HF `/infer` ishlov beruvchisiga va yo'q bo'lganda u qaytadan tushadi.
      vakolatli eski `run_label`-as-`inputs` konvert
      `artifact_hash` / `provenance_hash` / `budget_units` / `run_id` olib borilgan
      tuzilgan parametrlar sifatida;
    - `workflow_input_json` endi deterministik ketma-ketlikni ham tanlashi mumkinbilan ko'p bosqichli bajarish
      `{ "workflow_version": 1, "steps": [...] }`, bu erda har bir qadam ishlaydi a
      yaratilgan HF `/infer` so'rovi va keyingi qadamlar oldingi natijalarga murojaat qilishi mumkin
      `${run.*}`, `${previous.text|json|result_commitment}` va orqali
      `${steps.<step_id>.text|json|result_commitment}` to'ldiruvchilar; va
    - Mutatsiya javobi ham, `agent-autonomy-status` ham endi yuzaga chiqadi
      mavjud bo'lganda tugun-mahalliy bajarish xulosasi, shu jumladan muvaffaqiyat/muvaffaqiyatsizlik,
      bog'langan xizmatni qayta ko'rib chiqish, deterministik natija majburiyatlari, nazorat punkti
      / jurnal artefakt xeshlari, yaratilgan xizmat `AuditReceipt` va
      tahlil qilingan JSON javob tanasi.
    - yaratilgan xizmat kvitansiyasi mavjud bo'lganda, Torii uni qayd qiladi
      vakolatli `soracloud_runtime_receipts` va natijada paydo bo'ladi
      bilan birga oxirgi ish holati bo'yicha vakolatli ish vaqti kvitansiyasi
      tugun-mahalliy bajarilish xulosasi.
    - yaratilgan-HF avtonomiya yo'li endi ham maxsus vakolatli qayd qiladi
      kvartira `AutonomyRunExecuted` audit hodisasi va yaqinda ishlagan holati
      vakolatli ish vaqti kvitansiyasi bilan birga ijro auditini qaytaradi.
  - `hf-lease-renew` endi ikkita rejimga ega:
    - agar joriy oyna muddati tugagan yoki drenajlangan bo'lsa, u darhol yangi oynani ochadi
      oyna;
    - agar joriy oyna hali ham faol bo'lsa, u qo'ng'iroq qiluvchini navbatga qo'yadi
      keyingi oyna homiysi toʻliq keyingi oyna xotirasi va hisobini toʻlaydioldindan bron to'lovlari, deterministik keyingi oyna davom etadi
      joylashtirish rejasi va `hf-status` orqali navbatda turgan homiylikni fosh qiladi
      keyingi mutatsiya hovuzni oldinga siljitmaguncha.
  - yaratilgan HF ommaviy `/infer` kirish endi vakolatli muammoni hal qiladi
    joylashtirish va qabul qiluvchi tugun issiq birlamchi bo'lmaganda, proksi
    tayinlangan asosiy xostga Soracloud P2P boshqaruv xabarlari orqali so'rov;
    o'rnatilgan ish vaqti hali ham to'g'ridan-to'g'ri nusxada yopilmaydi/tayinlanmagan mahalliy
    ijro va ishlab chiqarilgan HF ish vaqti kvitantsiyalari `placement_id` ni o'z ichiga oladi,
    validator va nufuzli joylashtirish yozuvidan tengdosh atributi.
  - proksi-asosiy yo'l vaqti tugagach, javobdan oldin yopilganda yoki
    vakolatli tomonidan mijoz bo'lmagan ish vaqti xatosi bilan qaytib keladi
    birlamchi, kirish tuguni endi buning uchun `AssignedHeartbeatMiss` xabar beradi
    birlamchi va bir xil orqali `ReconcileSoracloudModelHosts` qatorlarini qo'yadi
    ichki mutatsiya chizig'i.
  - vakolatli muddati o'tgan xostni yarashtirish endi qaydlar saqlanib qoldi
    model-host buzilishining dalillari, umumiy chiziqli validator slash yo'lini qayta ishlatadi,
    va standart HF umumiy ijara jarima siyosatini qo'llaydi:
    `warmup_no_show_slash_bps=500`,
    `assigned_heartbeat_miss_slash_bps=250`,
    `assigned_heartbeat_miss_strike_threshold=3`, va
    `advert_contradiction_slash_bps=1000`.
  - mahalliy sifatida tayinlangan HF ish vaqtining holati endi xuddi shu dalil yo'lini ta'minlaydi:mahalliy `Warming` xost emitida import/isinish vaqtidagi nosozliklarni yarashtirish
    `WarmupNoShow` va mahalliy issiq boshlang'ichda rezident-ishchi nosozliklari
    normal orqali tormozlangan `AssignedHeartbeatMiss` hisobotlarini chiqaradi
    tranzaksiya navbati.
  - hozir ham ish boshlashdan oldin yarashtiring va mahalliy HF ishchilarini tekshirib ko'ring
    replikalarni o'z ichiga olgan issiq/isituvchi xostlar tayinlangan, shuning uchun replika muvaffaqiyatsiz bo'lishi mumkin
    har qanday oldin vakolatli `AssignedHeartbeatMiss` yo'liga yopiq
    ommaviy `/infer` so'rovi har doim birlamchiga tushadi.
  - bu mahalliy prob muvaffaqiyatli bo'lganda, ish vaqti endi bittasini chiqaradi
    mahalliy validator uchun vakolatli `model-host-heartbeat` mutatsiyasi qachon
    tayinlangan xost hali ham `Warming` yoki faol xost reklamasi TTL talab qiladi
    yangilash, shuning uchun muvaffaqiyatli mahalliy tayyorgarlik bir xil vakolatli targ'ib qiladi
    joylashtirish/reklama qo'lda yurak urishi yangilanishini bildiradi.
  - ish vaqti mahalliy `WarmupNoShow` yoki chiqarganda
    `AssignedHeartbeatMiss`, endi u ham navbatda turibdi
    `ReconcileSoracloudModelHosts` bir xil ichki mutatsiya chizig'i orqali shunday
    vakolatli yuklash/to'ldirish kutish o'rniga darhol boshlanadi
    keyinroq davriy xost muddatini tekshirish.
  - Qachon ommaviy ishlab chiqarilgan-HF kirish amalga oshirilgan, chunki hatto erta muvaffaqiyatsiz
    joylashtirishda proksi-server uchun issiq asosiy yo'q, Torii endi ish vaqtini so'raydi
    o'sha vakolatli shaxsni navbatga qo'yishKutish o'rniga darhol `ReconcileSoracloudModelHosts` ko'rsatmasi
    keyinroq muddati tugashi yoki ishchining ishlamay qolishi signali uchun.
  - ommaviy ishlab chiqarilgan HF kirish proksi-muvaffaqiyatli javob olganida,
    Torii endi kiritilgan ish vaqti kvitansiyasini hali ham bajarilganligini tasdiqlaydi
    faol joylashtirish uchun qilingan iliq boshlang'ich; etishmayotgan yoki mos kelmaydigan
    joylashtirish atributi endi yopilmaydi va xuddi shu vakolatga ishora qiladi
    a qaytish o'rniga `ReconcileSoracloudModelHosts` yo'li
    vakolatli bo'lmagan javob. Torii ham endi proksilangan muvaffaqiyatni rad etadi
    ish vaqti qabul qilish majburiyatlari yoki sertifikatlashtirish siyosati bajarilganda javoblar
    qaytarmoqchi bo'lgan javobga mos kelmasligi va o'sha yomon kvitansiya
    yo'l shuningdek, masofaviy birlamchi `AssignedHeartbeatMiss` hisobotini ham ta'minlaydi
    ilgak.
  - proksi-hosil qilingan-HF bajarilishdagi nosozliklar endi xuddi shunday talab qiladi
    haqida xabar bergandan keyin vakolatli `ReconcileSoracloudModelHosts` yo'li
    keyinroq muddatini kutish o'rniga, uzoqdan birlamchi sog'liqni saqlash xatosi
    supurish.
  - Torii endi kutilayotgan har bir HF proksi-server so'rovini bog'laydi
    obro'li asosiy tengdoshi u maqsadli. Noto'g'ri proksi javobi
    peer endi kutilayotgan so'rovni zaharlash o'rniga e'tiborga olinmaydi, shuning uchun faqat
    vakolatli birlamchi so'rovni bajarishi yoki bajarmasligi mumkin. Proksi-server
    qo'llab-quvvatlanmaydigan proksi-javob sxemasi bilan kutilgan tengdoshning javobiversiya faqat uning tufayli qabul qilinish o'rniga hali ham yopilmaydi
    `request_id` kutilayotgan so‘rovga mos keldi. Agar noto'g'ri javob bergan bo'lsa
    o'zi hamon o'sha joylashtirish uchun tayinlangan yaratilgan-HF xost, the
    runtime endi mavjud orqali xost haqida xabar beradi
    `WarmupNoShow` / `AssignedHeartbeatMiss` dalillarga asoslangan yo'l
    vakolatli topshiriq holati va shuningdek, vakolatli ishoralar
    `ReconcileSoracloudModelHosts`, shuning uchun eskirgan asosiy/replika vakolatlari
    faqat kirishda e'tiborga olinmaslik o'rniga boshqaruv tsiklini oziqlantiradi.
  - kiruvchi Soracloud proksi-serverining bajarilishi endi mo'ljallangan bilan cheklangan
    yaratilgan-HF `infer` so'rov ishi sozlangan iliq birlamchi. HF bo'lmagan
    umumiy mahalliy o'qiladigan marshrutlar va tugunga etkazib berilgan HF so'rovlari
    bu endi nufuzli issiq birlamchi emas, endi muvaffaqiyatsiz o'rniga yopiq
    P2P proksi yo'li orqali bajarish. Nufuzli boshlang'ich ham hozir
    kanonik yaratilgan-HF so'rov majburiyatini bajarishdan oldin qayta hisoblab chiqadi,
    shuning uchun soxta yoki mos kelmaydigan proksi konvertlar yopilmaydi.
  - tayinlangan replika yoki eskirgan birlamchi ushbu kiruvchini rad etganda
    yaratilgan-HF proksi-serverning bajarilishi, chunki u endi vakolatli emas
    issiq birlamchi, qabul qiluvchi tomoni ish vaqti endi ham ishora qiladi
    `ReconcileSoracloudModelHosts` faqat qo'ng'iroq qiluvchi tomoniga ishonish o'rniga
    marshrutlash ko'rinishi.- o'sha kiruvchi hosil-HF proksi avtoritet xatosi sodir bo'lganda
    mahalliy vakolatli birlamchining o'zi, ish vaqti endi uni a sifatida ko'radi
    birinchi darajali mezbon-sog'liqni saqlash signali: issiq primerlar o'z-o'zidan hisobot
    `AssignedHeartbeatMiss`, isitishning birlamchi o'z-o'zidan hisoboti `WarmupNoShow`,
    va ikkala yo'l darhol bir xil vakolatli qayta foydalanadi
    `ReconcileSoracloudModelHosts` boshqaruv zanjiri.
  - o'sha birlamchi bo'lmagan qabul qiluvchi hali ham vakolatli shaxslardan biri bo'lsa
    tayinlangan xostlar va belgilangan zanjir holatidan issiq birlamchini hal qila oladi,
    endi u yaratilgan HF so'rovini o'sha asosiyga qayta proksi qiladi
    darhol muvaffaqiyatsizlik. Tayinlanmagan validatorlar ishlash o‘rniga yopilmadi
    umumiy vositachi HF proksi-hops sifatida va asl kirish tugun hali ham
    qaytarilgan ish vaqti kvitansiyasini vakolatli joylashtirishga qarshi tasdiqlaydi
    davlat. Agar o'sha tayinlangan-replika oldinga birlamchiga o'tishdan keyin muvaffaqiyatsiz bo'lsa
    so'rov haqiqatda yuborilgan bo'lsa, qabul qiluvchi tomoni ish vaqti haqida xabar beradi
    masofaviy birlamchi sog'liqni saqlash xatosi va vakolatli maslahatlar
    `ReconcileSoracloudModelHosts`; mahalliy tayinlangan replika ham qila olmasa
    o'z proksi-transporti/ishlash vaqti yo'qligi sababli oldinga o'tishga harakat qiling,
    nosozlik endi o'rniga mahalliy tayinlangan xost xatosi sifatida qaraladi
    asosiyni ayblash.
  - yarashtirish endi ham avtomatik ravishda qachon `AdvertContradiction` chiqaradimahalliy validatorning sozlangan ish vaqti peer identifikatori bilan rozi emas
    ushbu validator uchun vakolatli `model-host-advertise` tengdosh identifikatori.
  - joriy model-xost qayta reklama mutatsiyalar endi ham vakolatli sinxronlash
    tayinlangan xost `peer_id` / `host_class` metadata va joriyni qayta hisoblash
    mezbon sinf o'zgarganda joylashtirish uchun to'lovlar.
  - qarama-qarshi model-xost qayta reklama mutatsiyalar endi darhol chiqaradi
    `AdvertContradiction` dalil, mavjud validator slash/evictni qo'llang
    yo'l va tekshirish muvaffaqiyatsiz o'rniga ta'sirlangan joylashtirishlarni yangilang.
  - qolgan HF hosting ishi hozir:
    - mahalliydan tashqari kengroq o'zaro faoliyat tugunlar/ish vaqti-klasterli sog'liq signallari
      validatorning to'g'ridan-to'g'ri ishchi/issiqlik kuzatuvlari va tayinlangan xost
      masofaviy tengdoshning ichki sog'lig'i kerak bo'lganda, qabul qiluvchi tomonidagi vakolatdagi nosozliklar
      shuningdek, nufuzli qayta muvozanat / slash yo'lini oziqlantirish.
  - yaratilgan HF mahalliy ijrosi endi har bir manba Python ishchisini saqlab qoladi
    `irohad` ostida tirik, takroriy `/infer` bo'ylab yuklangan modelni qayta ishlatadi
    qo'ng'iroq qiladi va mahalliy import bo'lsa, o'sha ishchini aniq qayta ishga tushiradi
    manifest o'zgarishlar yoki jarayon tugaydi.
  - bu marshrutlar shaxsiy yuklangan namunali yo'l emas. HF umumiy ijaralari qoladi
    zanjirda shifrlangandan ko'ra umumiy manba/import a'zoligiga e'tibor qaratildi
    xususiy model baytlari.- ishlab chiqarilgan HF avtonomiyasini tasdiqlash endi deterministik ketma-ketlikni qo'llab-quvvatlaydi
    ko'p bosqichli so'rov konvertlari, lekin kengroq chiziqli bo'lmagan/asbobdan foydalanish
    orkestratsiya va artefakt-grafik ijrosi hali ham keyingi ish bo'lib qolmoqda
    zanjirlangan `/infer` qadamlaridan tashqari.

## Status semantikasi

`/v1/soracloud/status` va tegishli agent/trening/model holati so'nggi nuqtalari hozir
vakolatli ish vaqti holatini aks ettiradi:

- qabul qilingan jahon davlatidan xizmat ko'rsatishning ruxsat etilgan tahrirlari;
- o'rnatilgan ish vaqti menejeridan ish vaqti hidratsiyasi/materializatsiya holati;
- real pochta qutisi ijro tushumlari va muvaffaqiyatsizlik holati;
- nashr etilgan jurnal/nazorat punkti artefaktlari;
- to'ldiruvchi holati o'rniga kesh va ish vaqti holati.

Agar vakolatli ish vaqti materiali eskirgan yoki mavjud bo'lmasa, o'qishlar yopilmaydi
mahalliy davlat ko'zgulariga qaytib tushish o'rniga.

`/v1/soracloud/status` - v1 versiyasida hujjatlashtirilgan yagona Soracloud holati so'nggi nuqtasi.
Alohida `/v1/soracloud/registry` marshruti yo'q.

## Mahalliy iskala olib tashlandi

Ushbu eski mahalliy simulyatsiya tushunchalari endi v1 da mavjud emas:

- CLI-mahalliy ro'yxatga olish kitobi/davlat fayllari yoki ro'yxatga olish kitobi yo'li variantlari
- Torii-mahalliy faylga asoslangan boshqaruv tekislik oynalari

## Misol

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## Eslatmalar- Mahalliy tekshirish hali so'rovlar imzolanishi va yuborilishidan oldin ishlaydi.
- Standart Soracloud mutatsion so'nggi nuqtalari endi xom `authority` ni qabul qilmaydi /
  Joylashtirish, yangilash, orqaga qaytarish, ishga tushirish, agent uchun `private_key` JSON maydonlari
  hayot aylanishi, o'qitish, model-host va model-vazn yo'llari; Torii autentifikatsiya qiladi
  o'rniga kanonik HTTP imzo sarlavhalaridan olingan so'rovlar.
- Multisig tomonidan boshqariladigan Soracloud egalari endi `X-Iroha-Witness` dan foydalanadilar; nuqta
  `soracloud.http_witness_file`, siz CLI-ni xohlagan JSON guvohi
  keyingi mutatsiya so'rovi uchun takrorlang va agar Torii yopilmaydi
  guvoh sub'ekti hisobi yoki kanonik so'rov xeshi mos kelmaydi.
- `hf-deploy` va `hf-lease-renew` endi mijoz tomonidan imzolangan yordamchini o'z ichiga oladi
  deterministik yaratilgan HF xizmati/kvartira artefaktlari uchun kelib chiqishi,
  shuning uchun Torii endi bu kuzatuvni qabul qilish uchun qo'ng'iroq qiluvchining shaxsiy kalitlariga muhtoj emas
  ob'ektlar.
- `agent-autonomy-run` va `model/run-private` endi qoralamadan, keyin yakunlashdan foydalanmoqda
  oqim: birinchi imzolangan mutatsiya vakolatli tasdiqlash/boshlashni qayd etadi,
  va ikkinchi imzolangan yakuniy so'rov ish vaqti yo'lini bajaradi va qaytaradi
  deterministik loyiha sifatida har qanday vakolatli kuzatish ko'rsatmalari
  operatsiyalar.
- `model/decrypt-output` endi nufuzli shaxsiy xulosani qaytaradi
  nazorat punkti faqat tashqi tomonidan imzolangan deterministik tranzaksiya loyihasi sifatidao'rnatilgan Torii shaxsiy kaliti orqali emas, balki tranzaksiya.
- ZK ilovasi CRUD endi imzolangan Iroha hisobini ijaraga olish huquqini beradi.
  yoqilganda API tokenlarini qoʻshimcha kirish eshigi sifatida koʻradi.
- Ommaviy Soracloud mahalliy o'qish kirish endi IP uchun aniq tarif va amal qiladi
  parallellik chegaralari va mahalliy yoki oldin umumiy marshrut ko'rinishini qayta tekshiradi
  proksilangan ijro.
- Soracloud xost ABI ichida shaxsiy ish vaqti qobiliyatini qo'llash amalga oshiriladi,
  CLI yoki Torii-mahalliy iskala ichida emas.
- `ram_lfe` alohida yashirin funksiyali quyi tizim bo'lib qoladi. Foydalanuvchi tomonidan yuklangan shaxsiy
  transformatorning bajarilishi Soracloud FHE/shifrni hal qilish boshqaruvini qayta ishlatishi kerak va
  model registrlari, `ram_lfe` so'rov yo'li emas.
- Ish vaqti sog'lig'i, hidratsiya va bajarilish manbalaridan olinadi
  `[soracloud_runtime]` konfiguratsiyasi va belgilangan holat, atrof-muhit emas
  almashtiradi.
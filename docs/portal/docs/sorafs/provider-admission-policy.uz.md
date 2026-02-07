---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17fcb22d5be25f601d4096c3a3488b7be2dd92dcf27019b678634590cd3bdde4
source_last_modified: "2025-12-29T18:16:35.197199+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) dan moslashtirilgan.

# SoraFS Provayderni qabul qilish va shaxsni aniqlash siyosati (SF-2b loyihasi)

Ushbu eslatma **SF-2b** uchun amalda bo'ladigan natijalarni qamrab oladi: aniqlash va
qabul ish jarayonini, shaxsni tasdiqlovchi talablarni va attestatsiyani amalga oshirish
SoraFS saqlash provayderlari uchun foydali yuklar. Bu yuqori darajadagi jarayonni kengaytiradi
SoraFS Arxitektura RFC-da tasvirlangan va qolgan ishni qismlarga ajratadi
kuzatilishi mumkin bo'lgan muhandislik vazifalari.

## Siyosat maqsadlari

- Faqat tekshirilgan operatorlar `ProviderAdvertV1` yozuvlarini nashr qilishiga ishonch hosil qiling.
  tarmoq qabul qiladi.
- Har bir reklama kalitini hukumat tomonidan tasdiqlangan shaxsni tasdiqlovchi hujjatga bog'lash;
  tasdiqlangan yakuniy nuqtalar va minimal ulush hissasi.
- Torii, shlyuzlar va deterministik tekshirish vositalarini taqdim eting
  `sorafs-node` bir xil tekshiruvlarni amalga oshiradi.
- Determinizmni buzmasdan yangilanish va favqulodda bekor qilishni qo'llab-quvvatlash yoki
  asboblar ergonomikasi.

## Identifikatsiya va stavka talablari

| Talab | Tavsif | Yetkazib beriladi |
|-------------|-------------|-------------|
| Reklama asosiy kelib chiqishi | Provayderlar har bir reklamani imzolaydigan Ed25519 tugmachalarini ro'yxatdan o'tkazishlari kerak. Qabul qilish to'plami umumiy kalitni boshqaruv imzosi bilan birga saqlaydi. | `ProviderAdmissionProposalV1` sxemasini `advert_key` (32 bayt) bilan kengaytiring va uni registrdan (`sorafs_manifest::provider_admission`) havola qiling. |
| Stake pointer | Qabul qilish uchun faol staking hovuziga ishora qiluvchi nolga teng bo'lmagan `StakePointer` talab qilinadi. | `sorafs_manifest::provider_advert::StakePointer::validate()` da tekshirishni va CLI/testlarda yuzaki xatolarni qo'shing. |
| Yurisdiktsiya teglari | Provayderlar yurisdiktsiyani e'lon qiladi + yuridik aloqa. | Taklif sxemasini `jurisdiction_code` (ISO 3166-1 alfa-2) va ixtiyoriy `contact_uri` bilan kengaytiring. |
| Yakuniy nuqta attestatsiyasi | Har bir e'lon qilingan so'nggi nuqta mTLS yoki QUIC sertifikat hisoboti bilan ta'minlanishi kerak. | `EndpointAttestationV1` Norito foydali yukini aniqlang va har bir so'nggi nuqtani qabul qilish to'plami ichida saqlang. |

## Qabul ish jarayoni

1. **Taklif yaratish**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …` qo'shing
     `ProviderAdmissionProposalV1` + attestatsiya to'plamini ishlab chiqarish.
   - Tasdiqlash: `profile_id` da kerakli maydonlar, stavka > 0, kanonik chunker tutqichiga ishonch hosil qiling.
2. **Boshqaruvni tasdiqlash**
   - Kengash imzolari `blake3("sorafs-provider-admission-v1" || canonical_bytes)` mavjud yordamida
     konvert asboblari (`sorafs_manifest::governance` moduli).
   - Konvert `governance/providers/<provider_id>/admission.json` da saqlanadi.
3. **Registrni kiritish**
   - Umumiy tekshirgichni amalga oshirish (`sorafs_manifest::provider_admission::validate_envelope`)
     Torii/shlyuzlar/CLI qayta foydalanish.
   - Qabul qilish yo'lini Torii yangilang, bu esa konvertdan farqli bo'lgan reklamalarni rad etish.
4. **Uzaytirish va bekor qilish**
   - `ProviderAdmissionRenewalV1` ni ixtiyoriy so'nggi nuqta/stake yangilanishlari bilan qo'shing.
   - `--revoke` CLI yo'lini bekor qilish sababini qayd eting va boshqaruv hodisasini ko'rsating.

## Amalga oshirish vazifalari

| Hudud | Vazifa | Ega(lar)i | Holati |
|------|------|----------|--------|
| Sxema | `crates/sorafs_manifest/src/provider_admission.rs` ostida `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) ni aniqlang. `sorafs_manifest::provider_admission` da tekshirish yordamchilari bilan amalga oshirilgan.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Saqlash / Boshqarish | ✅ Tugallandi |
| CLI asboblari | `sorafs_manifest_stub` ni quyi buyruqlar bilan kengaytiring: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Asboblar WG | ✅ |

CLI oqimi endi oraliq sertifikat paketlarini (`--endpoint-attestation-intermediate`) qabul qiladi, chiqaradi
kanonik taklif/konvert baytlari va `sign`/`verify` davrida kengash imzolarini tasdiqlaydi. Operatorlar mumkin
to'g'ridan-to'g'ri reklama organlarini taqdim eting yoki imzolangan reklamalarni qayta ishlating va imzo fayllari juftlashtirish orqali ta'minlanishi mumkin
Avtomatlashtirish qulayligi uchun `--council-signature-public-key` `--council-signature-file` bilan.

### CLI ma'lumotnomasi

Har bir buyruqni `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …` orqali bajaring.

- `proposal`
  - Kerakli bayroqlar: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>` va kamida bitta `--endpoint=<kind:host>`.
  - Bitta yakuniy attestatsiya `--endpoint-attestation-attested-at=<secs>` ni kutadi,
    `--endpoint-attestation-expires-at=<secs>`, orqali sertifikat
    `--endpoint-attestation-leaf=<path>` (plyus ixtiyoriy `--endpoint-attestation-intermediate=<path>`
    har bir zanjir elementi uchun) va har qanday kelishilgan ALPN identifikatorlari
    (`--endpoint-attestation-alpn=<token>`). QUIC so'nggi nuqtalari transport hisobotlarini taqdim etishi mumkin
    `--endpoint-attestation-report[-hex]=…`.
  - Chiqish: kanonik Norito taklif baytlari (`--proposal-out`) va JSON xulosasi
    (standart stdout yoki `--json-out`).
- `sign`
  - Kirishlar: taklif (`--proposal`), imzolangan e'lon (`--advert`), ixtiyoriy reklama organi
    (`--advert-body`), saqlash davri va kamida bitta kengash imzosi. Imzolar taqdim etilishi mumkin
    inline (`--council-signature=<signer_hex:signature_hex>`) yoki fayllar orqali birlashtirish orqali
    `--council-signature-public-key`, `--council-signature-file=<path>` bilan.
  - Tasdiqlangan konvertni (`--envelope-out`) va dayjest ulanishlarini ko'rsatuvchi JSON hisobotini ishlab chiqaradi,
    imzolovchilar soni va kirish yo'llari.
- `verify`
  - Mavjud konvertni (`--envelope`) tasdiqlaydi, ixtiyoriy ravishda mos keladigan taklifni tekshiradi,
    reklama yoki reklama tanasi. JSON hisobotida dayjest qiymatlari, imzoni tekshirish holati,
    va qaysi ixtiyoriy artefaktlar mos kelishi.
- `renewal`
  - Yangi tasdiqlangan konvertni ilgari ratifikatsiya qilingan dayjest bilan bog'laydi. Talab qiladi
    `--previous-envelope=<path>` va vorisi `--envelope=<path>` (ikkalasi ham Norito foydali yuklari).
    CLI profil taxalluslari, imkoniyatlar va reklama kalitlari o'zgarishsiz qolishini tasdiqlaydi
    stavkalar, so'nggi nuqtalar va metama'lumotlar yangilanishiga ruxsat berish. Kanonikni chiqaradi
    `ProviderAdmissionRenewalV1` bayt (`--renewal-out`) va JSON xulosasi.
- `revoke`
  - Konverti kerak bo'lgan provayder uchun favqulodda `ProviderAdmissionRevocationV1` to'plamini chiqaradi
    tortib olinsin. `--envelope=<path>`, `--reason=<text>`, kamida bitta talab qilinadi
    `--council-signature` va ixtiyoriy `--revoked-at`/`--notes`. CLI imzolaydi va tasdiqlaydi
    bekor qilish dayjesti, Norito foydali yukini `--revocation-out` orqali yozadi va JSON hisobotini chop etadi
    dayjest va imzolar sonini olish.
| Tekshirish | Torii, shlyuzlar va `sorafs-node` tomonidan foydalaniladigan umumiy tekshirgichni amalga oshiring. Birlik + CLI integratsiya testlarini taqdim eting.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Tarmoq TL / Saqlash | ✅ Tugallandi |
| Torii integratsiyasi | Tasdiqlagichni Torii reklama qabuliga o'tkazing, siyosatdan tashqari reklamalarni rad eting, telemetriyani chiqaring. | Networking TL | ✅ Tugallandi | Torii endi boshqaruv konvertlarini (`torii.sorafs.admission_envelopes_dir`) yuklaydi, qabul qilish vaqtida dayjest/imzo mosligini tekshiradi va kirishni yuzaga chiqaradi telemetriya.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api. |
| Yangilash | Yangilash/bekor qilish sxemasi + CLI yordamchilarini qo‘shing, hujjatlarda hayot aylanishi bo‘yicha qo‘llanmani nashr eting (quyida runbook va CLI buyruqlariga qarang). `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md: |120】 Saqlash / Boshqarish | ✅ Tugallandi |
| Telemetriya | `provider_admission` asboblar paneli va ogohlantirishlarni aniqlang (yangilash etishmayotgan, konvertning amal qilish muddati). | Kuzatish mumkinligi | 🟠 Davom etmoqda | `torii_sorafs_admission_total{result,reason}` hisoblagichi mavjud; asboblar paneli/ogohlantirishlar kutilmoqda.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### Yangilash va bekor qilish kitobi

#### Rejalashtirilgan yangilash (ulush/topologiya yangilanishlari)
1. `provider-admission proposal` va `provider-admission sign` bilan voris taklifi/reklama juftligini yarating, `--retention-epoch` ni oshiring va kerak bo'lganda ulush/so'nggi nuqtalarni yangilang.
2. Bajarmoq  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Buyruq orqali o'zgarmagan imkoniyatlar/profil maydonlarini tasdiqlaydi
   `AdmissionRecord::apply_renewal`, `ProviderAdmissionRenewalV1` chiqaradi va uchun dayjestlarni chop etadi
   boshqaruv jurnali.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Oldingi konvertni `torii.sorafs.admission_envelopes_dir` bilan almashtiring, Norito/JSON yangilanishini boshqaruv omboriga topshiring va `docs/source/sorafs/migration_ledger.md` ga yangilash xesh + saqlash davrini qo'shing.
4. Operatorlarga yangi konvertning faol ekanligi haqida xabar bering va qabul qilishni tasdiqlash uchun `torii_sorafs_admission_total{result="accepted",reason="stored"}` ni kuzatib boring.
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` orqali kanonik moslamalarni qayta tiklang va bajaring; CI (`ci/check_sorafs_fixtures.sh`) Norito chiqishlarining barqarorligini tasdiqlaydi.

#### Favqulodda bekor qilish
1. Buzilgan konvertni aniqlang va bekor qilish to'g'risida qaror chiqaring:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI `ProviderAdmissionRevocationV1` ga imzo qo'yadi, imzo to'plamini tasdiqlaydi
   `verify_revocation_signatures` va bekor qilish dayjestini xabar qiladi.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#48】L
2. `torii.sorafs.admission_envelopes_dir` dan konvertni olib tashlang, bekor qilish Norito/JSON ni qabul keshlariga tarqating va boshqaruv bayonnomasida xesh sababini yozib oling.
3. Keshlar bekor qilingan reklamani tashlab yuborishini tasdiqlash uchun `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` ni tomosha qiling; bekor qilish artefaktlarini voqea retrospektivlarida saqlang.

## Sinov va telemetriya- Qabul qilish takliflari va konvertlar uchun oltin moslamalarni qo'shing
  `fixtures/sorafs_manifest/provider_admission/`.
- Takliflarni qayta tiklash va konvertlarni tekshirish uchun CI (`ci/check_sorafs_fixtures.sh`) ni kengaytiring.
- Yaratilgan moslamalarga kanonik dayjestlarga ega `metadata.json` kiradi; quyi oqim sinovlari tasdiqlaydi
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Integratsiya testlarini taqdim eting:
  - Torii qabul qilish konvertlari etishmayotgan yoki muddati o'tgan reklamalarni rad etadi.
  - CLI taklifni → konvert → tekshirishni aylanib chiqadi.
  - Boshqaruvni yangilash provayder identifikatorini o'zgartirmasdan oxirgi nuqta attestatsiyasini aylantiradi.
- Telemetriya talablari:
  - Torii da `provider_admission_envelope_{accepted,rejected}` hisoblagichlarini chiqaradi. ✅ `torii_sorafs_admission_total{result,reason}` endi qabul qilingan/rad etilgan natijalarni ko'rsatadi.
  - Kuzatuv paneliga amal qilish muddati tugashi haqidagi ogohlantirishlarni qo'shing (yangilash 7 kun ichida amalga oshiriladi).

## Keyingi qadamlar

1. ✅ Norito sxemasiga oʻzgartirishlar yakunlandi va tasdiqlovchi yordamchilar
   `sorafs_manifest::provider_admission`. Hech qanday xususiyat bayroqlari shart emas.
2. ✅ CLI ish oqimlari (`proposal`, `sign`, `verify`, `renewal`, `revoke`) integratsiya testlari orqali hujjatlashtiriladi va amalga oshiriladi; boshqaruv skriptlarini runbook bilan sinxronlashtiring.
3. ✅ Torii qabul/kashfiyot konvertlarni qabul qilish va qabul qilish/rad etish uchun telemetriya hisoblagichlarini ochish.
4. Kuzatish mumkinligiga e'tibor qarating: qabul qilish panellarini/ogohlantirishlarni tugating, shunda etti kun ichida yangilanishlar ogohlantirishlarni oshiradi (`torii_sorafs_admission_total`, amal qilish muddati o'lchagichlari).
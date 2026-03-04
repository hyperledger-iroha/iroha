---
id: pin-registry-plan
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

# SoraFS Pin registrini amalga oshirish rejasi (SF-4)

SF-4 Pin Registry shartnomasini va saqlaydigan yordamchi xizmatlarni taqdim etadi
manifest majburiyatlari, pin siyosatlarini amalga oshirish va API'larni Torii, shlyuzlar,
va orkestrlar. Ushbu hujjat tasdiqlash rejasini beton bilan kengaytiradi
zanjirdagi mantiqni, xost tomonidagi xizmatlarni, moslamalarni qamrab oluvchi amalga oshirish vazifalari,
va operatsion talablar.

## Qo'llash doirasi

1. **Registr holati mashinasi**: manifestlar, taxalluslar uchun Norito tomonidan belgilangan yozuvlar,
   voris zanjirlari, ushlab turish davrlari va boshqaruv metama'lumotlari.
2. **Shartnomani amalga oshirish**: pinning hayot aylanishi uchun deterministik CRUD operatsiyalari
   (`ReplicationOrder`, `Precommit`, `Completion`, ko'chirish).
3. **Xizmat fasad**: Torii registr tomonidan quvvatlangan gRPC/REST oxirgi nuqtalari
   va SDK'lar, jumladan, sahifalash va attestatsiyadan foydalanadi.
4. **Asboblar va jihozlar**: CLI yordamchilari, sinov vektorlari va saqlanishi kerak bo'lgan hujjatlar
   manifestlar, taxalluslar va boshqaruv konvertlari sinxronlashtiriladi.
5. **Telemetriya va operatsiyalar**: registr salomatligi uchun ko'rsatkichlar, ogohlantirishlar va ish kitoblari.

## Ma'lumotlar modeli

### Asosiy yozuvlar (Norito)

| Struktura | Tavsif | Maydonlar |
|--------|-------------|--------|
| `PinRecordV1` | Kanonik manifest yozuvi. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, I18NI000000030X, I18NI0000103102, I18NI0000108102 `governance_envelope_hash`. |
| `AliasBindingV1` | Xaritalar taxallus -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Manifestni pin qilish bo'yicha provayderlar uchun ko'rsatma. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Provayderni tasdiqlash. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Boshqaruv siyosatining surati. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Amalga oshirish uchun ma'lumotnoma: uchun `crates/sorafs_manifest/src/pin_registry.rs` ga qarang
Rust Norito sxemalari va ushbu yozuvlarni qo'llab-quvvatlaydigan tekshirish yordamchilari. Tasdiqlash
manifest vositalarini aks ettiradi (chunker registrini qidirish, pin siyosati gating) shuning uchun
kontrakt, Torii jabhalari va CLI bir xil o'zgarmasliklarga ega.

Vazifalar:
- `crates/sorafs_manifest/src/pin_registry.rs` da Norito sxemalarini yakunlang.
- Norito makroslari yordamida kod yarating (Rust + boshqa SDK).
- Sxemalar tushgandan keyin hujjatlarni yangilang (`sorafs_architecture_rfc.md`).

## Shartnomani amalga oshirish

| Vazifa | Ega(lar)i | Eslatmalar |
|------|----------|-------|
| Ro'yxatga olish kitobini saqlash (sled/sqlite/off-chain) yoki aqlli kontrakt modulini amalga oshiring. | Asosiy Infra / Aqlli shartnoma jamoasi | Deterministik xeshni ta'minlang, suzuvchi nuqtadan qoching. |
| Kirish nuqtalari: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Yadro infra | Tekshirish rejasidan `ManifestValidator` dan foydalaning. Taxallusni ulash endi `RegisterPinManifest` (Torii DTO sirti) orqali o'tadi, `bind_alias` esa ketma-ket yangilanishlar uchun rejalashtirilgan. |
| Holat o'tishlari: ketma-ketlikni ta'minlash (manifest A -> B), saqlash davrlari, taxallusning o'ziga xosligi. | Boshqaruv Kengashi / Asosiy Infra | Taxallusning o'ziga xosligi, saqlash cheklovlari va avvalgi tasdiqlash/pensiya tekshiruvlari endi `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` da mavjud; multi-hop ketma-ketligini aniqlash va replikatsiya buxgalteriya hisobi ochiq qoladi. |
| Boshqariladigan parametrlar: konfiguratsiya/boshqaruv holatidan `ManifestPolicyV1` yuklash; boshqaruv tadbirlari orqali yangilanishlarga ruxsat berish. | Boshqaruv Kengashi | Siyosat yangilanishlari uchun CLI taqdim eting. |
| Hodisa emissiyasi: telemetriya uchun Norito hodisalarini chiqaradi (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Kuzatish mumkinligi | Voqealar sxemasini + jurnalga yozishni aniqlang. |

Sinov:
- Har bir kirish nuqtasi uchun birlik testlari (ijobiy + rad etish).
- ketma-ketlik zanjiri uchun xossa testlari (sikllar, monotonik davrlar yo'q).
- Tasodifiy manifestlarni (chegaralangan) yaratish orqali noaniqlikni tekshirish.

## Xizmat fasad (Torii/SDK integratsiyasi)

| Komponent | Vazifa | Ega(lar)i |
|----------|------|----------|
| Torii Xizmat | `/v1/sorafs/pin` (yuborish), `/v1/sorafs/pin/{cid}` (qidiruv), `/v1/sorafs/aliases` (roʻyxat/bogʻlash), `/v1/sorafs/replication` (buyurtmalar/kvitansiyalarni) oching. Sahifalar + filtrlashni ta'minlang. | Networking TL / Core Infra |
| Attestatsiya | Javoblarga registr balandligi/xeshini qo'shing; SDK tomonidan iste'mol qilinadigan Norito attestatsiya tuzilmasini qo'shing. | Yadro infra |
| CLI | `sorafs_manifest_stub` yoki yangi `sorafs_pin` CLI ni `pin submit`, `alias bind`, `order issue`, `registry export` bilan kengaytiring. | Asboblar WG |
| SDK | Norito sxemasidan mijoz ulanishlarini (Rust/Go/TS) yaratish; integratsiya testlarini qo'shing. | SDK jamoalari |

Operatsiyalar:
- GET so'nggi nuqtalari uchun keshlash qatlami/ETag qo'shing.
- Torii siyosatiga mos keladigan tezlikni cheklash / autentifikatsiyani taqdim eting.

## Armatura va CI

- Armatura katalogi: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` imzolangan manifest/taxallus/buyurtma oniy tasvirlarini `cargo run -p iroha_core --example gen_pin_snapshot` tomonidan qayta tiklangan holda saqlaydi.
- CI qadami: `ci/check_sorafs_fixtures.sh` suratni qayta tiklaydi va agar farqlar paydo bo'lsa, CI moslamalarini bir xilda ushlab turgan holda ishlamay qoladi.
- Integratsiya testlari (`crates/iroha_core/tests/pin_registry.rs`) baxtli yo'lni qo'llaydi, shuningdek, takroriy taxallusni rad etish, taxallusni tasdiqlash/saqlash himoyasi, mos kelmaydigan chunker tutqichlari, replikatsiyalar sonini tekshirish va voris qo'riqlash xatosi (noma'lum/oldindan tasdiqlangan/nafaqaga chiqqan/o'z-o'zidan ko'rsatkichlar); qamrov tafsilotlari uchun `register_manifest_rejects_*` holatlariga qarang.
- Birlik testlari endi `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` da taxallusni tekshirish, saqlash himoyasi va voris tekshiruvlarini qamrab oladi; davlat mashinasi erga tushgandan so'ng multi-hop ketma-ketligini aniqlash.
- Kuzatuv quvurlari tomonidan ishlatiladigan hodisalar uchun oltin JSON.

## Telemetriya va kuzatuvchanlik

Ko'rsatkichlar (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Mavjud provayder telemetriyasi (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) end-to-end asboblar paneli uchun qo'llaniladi.

Jurnallar:
- Boshqaruv auditlari uchun tuzilgan Norito hodisalar oqimi (imzolanganmi?).

Ogohlantirishlar:
- SLA dan ortiq kutilayotgan replikatsiya buyurtmalari.
- taxallusning amal qilish muddati < pol.
- saqlash qoidalarini buzish (manifest muddati tugashidan oldin yangilanmagan).

Boshqaruv paneli:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` manifest hayotiy siklining yakunlari, taxalluslar qamrovi, toʻyinganlik toʻyinganligi, SLA nisbati, kechikish va boʻshashmaslik va qoʻngʻiroq boʻyicha koʻrib chiqish uchun oʻtkazib yuborilgan buyurtma stavkalarini kuzatadi.

## Runbooks va hujjatlar

- Ro'yxatga olish kitobi holatini yangilash uchun `docs/source/sorafs/migration_ledger.md` ni yangilang.
- Operator uchun qo'llanma: `docs/source/sorafs/runbooks/pin_registry_ops.md` (hozir nashr etilgan) ko'rsatkichlar, ogohlantirishlar, joylashtirish, zaxiralash va tiklash oqimlarini qamrab oladi.
- Boshqaruv bo'yicha qo'llanma: siyosat parametrlarini, tasdiqlash ish jarayonini, nizolarni ko'rib chiqishni tavsiflang.
- Har bir so'nggi nuqta uchun API mos yozuvlar sahifalari (Docusaurus docs).

## Bog'liqlar va ketma-ketlik

1. To'liq tekshirish rejasi vazifalari (ManifestValidator integratsiyasi).
2. Norito sxemasi + standart parametrlarini yakunlang.
3. Shartnoma + xizmat ko'rsatish, simli telemetriyani amalga oshirish.
4. Armaturalarni qayta tiklash, integratsiya to'plamlarini ishga tushirish.
5. Docs/runbook-larni yangilang va yo'l xaritasi elementlarini tugallangan deb belgilang.

SF-4 ostidagi yo'l xaritasi nazorat ro'yxatining har bir bandi taraqqiyotga erishilganda ushbu rejaga havola qilishi kerak.
REST jabhasi endi tasdiqlangan ro'yxatning so'nggi nuqtalari bilan jo'natiladi:

- `GET /v1/sorafs/pin` va `GET /v1/sorafs/pin/{digest}` qaytish manifestlari bilan
  taxallus bog'lashlari, replikatsiya buyurtmalari va attestatsiya ob'ektidan olingan
  oxirgi blok xeshi.
- `GET /v1/sorafs/aliases` va `GET /v1/sorafs/replication` faol moddalarni ochib beradi.
  taxallus katalogi va izchil sahifalash bilan replikatsiya tartibi to'plami va
  holat filtrlari.

CLI ushbu qo'ng'iroqlarni o'radi (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) shuning uchun operatorlar ro'yxatga olish kitobi tekshiruvlarini teginmasdan yozishi mumkin
quyi darajadagi API.
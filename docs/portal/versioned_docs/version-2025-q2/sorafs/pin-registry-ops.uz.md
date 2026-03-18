---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T14:35:36.898296+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops-uz
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
slug: /sorafs/pin-registry-ops-uz
---

::: Eslatma Kanonik manba
Nometall `docs/source/sorafs/runbooks/pin_registry_ops.md`. Ikkala versiyani ham relizlar bo'ylab tekislang.
:::

## Umumiy ko'rinish

Ushbu runbook SoraFS pin registrini va uning replikatsiya xizmati darajasidagi kelishuvlarini (SLAs) qanday kuzatish va triajlashni hujjatlashtiradi. Ko'rsatkichlar `iroha_torii` dan kelib chiqadi va `torii_sorafs_*` nom maydoni ostida Prometheus orqali eksport qilinadi. Torii fonda 30 soniya oralig'ida ro'yxatga olish kitobi holatidan namuna oladi, shuning uchun hech bir operator `/v1/sorafs/pin/*` so'nggi nuqtalarini so'ramasa ham asboblar paneli joriy bo'lib qoladi. Foydalanishga tayyor Grafana tartibi uchun tanlangan asboblar panelini (`docs/source/grafana_sorafs_pin_registry.json`) import qiling, u toʻgʻridan-toʻgʻri quyidagi boʻlimlarga mos keladi.

## Metrik havola

| Metrik | Yorliqlar | Tavsif |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Zanjirdagi manifest inventarizatsiyasi hayot aylanishi holati bo'yicha. |
| `torii_sorafs_registry_aliases_total` | — | Ro'yxatga olish kitobida qayd etilgan faol manifest taxalluslari soni. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Replikatsiya tartibi bo‘yicha maqom bo‘yicha segmentlangan. |
| `torii_sorafs_replication_backlog_total` | — | `pending` buyurtmalarini aks ettiruvchi qulaylik o'lchagichi. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA hisobi: `met` belgilangan muddatda bajarilgan buyurtmalarni hisoblaydi, `missed` kech bajarilganlarni + amal qilish muddatini jamlaydi, `pending` bajarilmagan buyurtmalarni aks ettiradi. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Yakunlangan yakunlash kechikishi (berilish va tugallanish o'rtasidagi davrlar). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Kutilayotgan buyurtma bo'sh derazalar (muddati minus chiqarilgan davr). |

Barcha o‘lchagichlar har bir suratga olinganda asliga qaytariladi, shuning uchun asboblar paneli `1m` kadansda yoki tezroq namuna olishi kerak.

## Grafana asboblar paneli

JSON asboblar paneli operator ish jarayonlarini qamrab oluvchi yettita panel bilan birga keladi. Agar siz buyurtma jadvallarini yaratishni afzal ko'rsangiz, so'rovlar qisqacha ma'lumot olish uchun quyida keltirilgan.

1. **Manifest hayotiy sikli** – `torii_sorafs_registry_manifests_total` (`status` tomonidan guruhlangan).
2. **Taxallus katalogi trend** – `torii_sorafs_registry_aliases_total`.
3. **Holat bo‘yicha buyurtma navbati** – `torii_sorafs_registry_orders_total` (`status` bo‘yicha guruhlangan).
4. **Backlog vs muddati o'tgan buyurtmalar** - sirt to'yinganligi uchun `torii_sorafs_replication_backlog_total` va `torii_sorafs_registry_orders_total{status="expired"}` ni birlashtiradi.
5. **SLA muvaffaqiyati nisbati** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Kutilish va oxirgi muddatning susayishi** – qoplama `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` va `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Mutlaq bo'sh qavat kerak bo'lganda `min_over_time` ko'rinishlarini qo'shish uchun Grafana o'zgarishlaridan foydalaning, masalan:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **O‘tkazib yuborilgan buyurtmalar (1 soatlik tarif)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Ogohlantirish chegaralari- **SLA muvaffaqiyati  0**
  - Eshik chegarasi: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Harakat: provayderning ishlamay qolganligini tasdiqlash uchun boshqaruv manifestlarini tekshiring.
- **Tugallash p95 > oxirgi muddatning o‘rtacha bo‘shligi**
  - Eshik chegarasi: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Harakat: provayderlar belgilangan muddatdan oldin majburiyatlarni bajarayotganligini tekshiring; qayta tayinlash masalasini ko'rib chiqing.

### Misol Prometheus Qoidalar

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Triaj ish jarayoni

1. **Sababini aniqlang**
   - Agar SLA ko'tarilishni o'tkazib yuborsa, kechikish pastligicha qolsa, provayderning ishlashiga e'tibor qarating (PoR nosozliklari, kech yakunlash).
   - Agar kechikish barqaror o'tkazib yuborishlar bilan o'ssa, kengash ma'qullanishini kutayotgan manifestlarni tasdiqlash uchun qabulni tekshiring (`/v1/sorafs/pin/*`).
2. **Provayder holatini tekshirish**
   - `iroha app sorafs providers list` ni ishga tushiring va reklama qilingan imkoniyatlar replikatsiya talablariga mos kelishini tekshiring.
   - Ta'minlangan GiB va PoR muvaffaqiyatini tasdiqlash uchun `torii_sorafs_capacity_*` o'lchagichlarini tekshiring.
3. **Replikatsiyani qayta tayinlash**
   - Kechikish sustligi (`stat="avg"`) 5 davrdan pastga tushganda (manifest/CAR qadoqlashda `iroha app sorafs toolkit pack` ishlatiladi) `sorafs_manifest_stub capacity replication-order` orqali yangi buyurtmalar bering.
   - Agar taxalluslarda faol manifest bog'lanishlari bo'lmasa, boshqaruvni xabardor qiling (`torii_sorafs_registry_aliases_total` kutilmaganda tushib ketadi).
4. **Hujjat natijasi**
   - Voqea qaydlarini SoraFS operatsiyalar jurnaliga vaqt belgilari va ta'sirlangan manifest dayjestlari bilan yozib oling.
   - Agar yangi ishlamay qolish rejimlari yoki asboblar paneli joriy etilsa, ushbu ish kitobini yangilang.

## Chiqarish rejasi

Ishlab chiqarishda taxallus kesh siyosatini yoqish yoki kuchaytirishda ushbu bosqichli tartibni bajaring:1. **Konfiguratsiyani tayyorlang**
   - `torii.sorafs_alias_cache` ni `iroha_config` (foydalanuvchi → haqiqiy) da kelishilgan TTL va imtiyozli oynalar bilan yangilang: `positive_ttl`, `refresh_window`, `hard_expiry`, I0808030, I18080 `revocation_ttl`, `rotation_max_age`, `successor_grace` va `governance_grace`. Standartlar `docs/source/sorafs_alias_policy.md` siyosatiga mos keladi.
   - SDK uchun bir xil qiymatlarni ularning konfiguratsiya qatlamlari orqali taqsimlang (Rust / NAPI / Python bog'lashlarida `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)`), shuning uchun mijozni qo'llash shlyuzga mos keladi.
2. **Sahnadagi quruq yugurish**
   - Konfiguratsiya o'zgarishini ishlab chiqarish topologiyasini aks ettiruvchi bosqichli klasterga o'rnating.
   - `cargo xtask sorafs-pin-fixtures` ni ishga tushiring va kanonik taxallus moslamalari hali ham dekodlanishi va aylanmasini tasdiqlash uchun; har qanday nomuvofiqlik birinchi navbatda hal qilinishi kerak bo'lgan yuqori oqimdagi manifest driftni nazarda tutadi.
   - `/v1/sorafs/pin/{digest}` va `/v1/sorafs/aliases` so'nggi nuqtalarini yangi, yangilash oynasi, muddati o'tgan va muddati o'tgan holatlarni qamrab oluvchi sintetik dalillar bilan ishlating. HTTP holat kodlari, sarlavhalar (`Sora-Proof-Status`, `Retry-After`, `Warning`) va JSON asosiy maydonlarini ushbu runbook bilan taqqoslang.
3. **Ishlab chiqarishni yoqish**
   - Yangi konfiguratsiyani standart o'zgartirish oynasi orqali chiqaring. Avval uni Torii ga qo‘llang, so‘ngra tugun jurnallardagi yangi siyosatni tasdiqlaganidan so‘ng shlyuzlar/SDK xizmatlarini qayta ishga tushiring.
   - `docs/source/grafana_sorafs_pin_registry.json` ni Grafana ga import qiling (yoki mavjud asboblar panelini yangilang) va taxallus kesh yangilash panellarini NOC ish maydoniga mahkamlang.
4. **O'rnatishdan keyingi tekshirish**
   - `torii_sorafs_alias_cache_refresh_total` va `torii_sorafs_alias_cache_age_seconds` ni 30 daqiqa davomida kuzatib boring. `error`/`expired` egri chiziqlaridagi tikanlar siyosatni yangilash oynalari bilan o'zaro bog'liq bo'lishi kerak; kutilmagan o'sish operatorlar davom ettirishdan oldin taxallus isbotlari va provayder salomatligini tekshirish kerak, degan ma'noni anglatadi.
   - Mijoz jurnallari bir xil siyosat qarorlarini ko'rsatishini tasdiqlang (SDKlar isbot eskirgan yoki muddati o'tgan bo'lsa, xatolar yuzaga keladi). Mijoz ogohlantirishlarining yo'qligi noto'g'ri konfiguratsiyani ko'rsatadi.
5. ** Qayta tiklash**
   - Agar taxallusning chiqarilishi ortda qolsa va yangilash oynasi tez-tez ishlamasa, konfiguratsiyada `refresh_window` va `positive_ttl` ni oshirish orqali siyosatni vaqtincha yumshatib, keyin qayta joylashtiring. `hard_expiry` ni saqlanib qoling, shunda haqiqiy eskirgan dalillar hali ham rad etiladi.
   - Agar telemetriya yuqori `error` sonlarini ko'rsatishda davom etsa, oldingi `iroha_config` suratini tiklash orqali oldingi konfiguratsiyaga qayting, keyin taxallus yaratish kechikishlarini kuzatish uchun hodisani oching.

## Tegishli materiallar

- `docs/source/sorafs/pin_registry_plan.md` - amalga oshirish yo'l xaritasi va boshqaruv konteksti.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - saqlash ishchi operatsiyalari, bu ro'yxatga olish kitobini to'ldiradi.

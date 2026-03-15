---
lang: az
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T14:35:37.691079+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Məlumat Əlçatımlılığı İcarəsi və Təşviq Siyasəti (DA-7)

_Status: Layihənin hazırlanması — Sahiblər: İqtisadiyyat üzrə İş Qrupu / Xəzinədarlıq / Saxlama Qrupu_

Yol xəritəsi elementi **DA-7** hər bir blok üçün açıq XOR ilə ifadə olunan icarə haqqı təqdim edir
`/v2/da/ingest`-ə təqdim olunur, üstəgəl PDP/PoTR icrasını mükafatlandıran bonuslar və
egress müştəriləri cəlb etməyə xidmət edirdi. Bu sənəd ilkin parametrləri müəyyən edir,
onların məlumat modeli təmsili və Torii tərəfindən istifadə edilən hesablama iş axını,
SDK-lar və Xəzinədarlığın idarə panelləri.

## Siyasət strukturu

Siyasət [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs) kimi kodlaşdırılıb
məlumat modeli daxilində. Torii və idarəetmə alətləri siyasətini davam etdirir
Norito faydalı yüklər ki, icarə qiymətləri və həvəsləndirici kitablar yenidən hesablana bilsin
deterministik olaraq. Sxem beş düyməni göstərir:

| Sahə | Təsvir | Defolt |
|-------|-------------|---------|
| `base_rate_per_gib_month` | XOR saxlama ayı üçün GiB başına tutulur. | `250_000` mikro-XOR (0.25 XOR) |
| `protocol_reserve_bps` | Protokol ehtiyatına yönəldilən icarə haqqının payı (əsas nöqtələr). | `2_000` (20%) |
| `pdp_bonus_bps` | Uğurlu PDP qiymətləndirməsinə görə bonus faizi. | `500` (5%) |
| `potr_bonus_bps` | Uğurlu PoTR qiymətləndirməsi üçün bonus faizi. | `250` (2,5%) |
| `egress_credit_per_gib` | Provayder 1GiB DA datasına xidmət etdikdə ödənilən kredit. | `1_500` mikro-XOR |

Bütün əsas nöqtə dəyərləri `BASIS_POINTS_PER_UNIT` (10000) ilə təsdiqlənir.
Siyasət yeniləmələri idarəetmə vasitəsilə keçməlidir və hər Torii qovşağı
`torii.da_ingest.rent_policy` konfiqurasiya bölməsi vasitəsilə aktiv siyasət
(`iroha_config`). Operatorlar `config.toml`-də defoltları ləğv edə bilər:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

CLI alətləri (`iroha app da rent-quote`) eyni Norito/JSON siyasət daxiletmələrini qəbul edir
və aktiv `DaRentPolicyV1`-ə çatmadan əks etdirən artefaktlar yayır.
Torii vəziyyətinə qayıdın. Qəbul etmə üçün istifadə edilən siyasət snapshotunu təmin edin
sitat təkrarlana bilən olaraq qalır.

### Davamlı icarə qiymətləri artefaktları

`iroha app da rent-quote --gib <size> --months <months> --quote-out <path>`-i işə salın
həm ekrandakı xülasəni, həm də olduqca çap olunmuş JSON artefaktını yayır. Fayl
qeydlər `policy_source`, daxili `DaRentPolicyV1` snapshot, hesablanmış
`DaRentQuote` və törəmə `ledger_projection` (seriyalı
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) onu xəzinə tabloları və kitab ISI üçün uyğun edir
boru kəmərləri. `--quote-out` daxili kataloqa işarə etdikdə CLI hər hansı bir fayl yaradır
itkin valideynlər, beləliklə operatorlar kimi yerləri standartlaşdıra bilər
`artifacts/da/rent_quotes/<timestamp>.json` digər DA sübut paketləri ilə birlikdə.
Artefaktı XOR üçün icarə təsdiqlərinə və ya uzlaşmaya əlavə edin
bölgüsü (əsas icarə, ehtiyat, PDP/PoTR bonusları və çıxış kreditləri).
təkrarlana bilən. Avtomatik olaraq ləğv etmək üçün `--policy-label "<text>"`-i keçin
əldə edilmiş `policy_source` təsviri (fayl yolları, quraşdırılmış standart və s.)
idarəetmə bileti və ya manifest hash kimi insan tərəfindən oxuna bilən etiket; CLI kəsir
bu dəyər və boş/yalnız boşluq sətirlərini rədd edir, beləliklə qeydə alınmış sübut
audit edilə bilən olaraq qalır.

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```Kitabın proyeksiyası bölməsi birbaşa DA icarə kitabçası ISI-lərə daxil olur: o
protokol ehtiyatı, provayder ödənişləri və üçün nəzərdə tutulmuş XOR deltalarını müəyyən edir
sifarişli orkestrasiya kodu tələb etmədən hər sübut üçün bonus hovuzları.

### Kirayə dəftəri planlarının yaradılması

`iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora`-i işə salın
davamlı icarə təklifini icra edilə bilən kitab köçürmələrinə çevirmək. Əmr
daxil edilmiş `ledger_projection`-i təhlil edir, Norito `Transfer` təlimatlarını yayır
əsas icarə haqqını xəzinəyə toplayan, ehtiyatı/provayderi yönləndirir
hissələrə bölünür və PDP/PoTR bonus hovuzlarını birbaşa ödəyicidən əvvəlcədən maliyyələşdirir. The
çıxış JSON sitat metadatasını əks etdirir ki, CI və xəzinə alətləri əsaslandırsın
eyni artefakt haqqında:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

Son `egress_credit_per_gib_micro_xor` sahəsi tablosuna və ödəməyə imkan verir
planlaşdırıcılar çıxış kompensasiyalarını istehsal edən icarə siyasəti ilə uyğunlaşdırır
skript yapışqanında siyasət riyaziyyatını yenidən hesablamadan sitat gətirin.

## Sitat nümunəsi

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

Sitat Torii qovşaqlarında, SDK-larda və Xəzinədarlıq hesabatlarında təkrarlana bilər, çünki
ad-hoc riyaziyyat əvəzinə deterministik Norito strukturlarından istifadə edir. Operatorlar bilər
idarəetmə təkliflərinə və ya icarəyə JSON/CBOR kodlu `DaRentPolicyV1` əlavə edin
hər hansı verilmiş blob üçün hansı parametrlərin qüvvədə olduğunu sübut etmək üçün auditlər.

## Bonuslar və ehtiyatlar

- **Protokol ehtiyatı:** `protocol_reserve_bps` dəstəkləyən XOR ehtiyatını maliyyələşdirir
  fövqəladə replikasiya və geri qaytarmaların kəsilməsi. Xəzinədarlıq bu vedrəni izləyir
  mühasibat balansının konfiqurasiya edilmiş tarifə uyğun olmasını təmin etmək üçün ayrıca.
- **PDP/PoTR bonusları:** Hər bir uğurlu sübut qiymətləndirməsi əlavə mükafat alır
  `base_rent × bonus_bps`-dən əldə edilən ödəniş. DA planlaşdırıcı sübut yaydıqda
  qəbzlərə əsas nöqtə teqləri daxildir, beləliklə, təşviqlər təkrar oynaya bilsin.
- **Çıxış krediti:** Təchizatçılar hər manifestdə göstərilən GiB-ni qeyd edir, çoxaldır
  `egress_credit_per_gib` və qəbzləri `iroha app da prove-availability` vasitəsilə təqdim edin.
  İcarə siyasəti GiB başına məbləği idarəetmə ilə sinxronlaşdırır.

## Əməliyyat axını

1. **İngest:** `/v2/da/ingest` aktiv `DaRentPolicyV1` yükləyir, kotirovkalar icarəyə verilir
   blob ölçüsünə və saxlanmasına əsaslanır və təklifi Norito-ə daxil edir
   aşkar. Təqdimatçı icarə hashına istinad edən bəyanatı imzalayır və
   saxlama bileti id.
2. **Mühasibatlıq:** Xəzinədarlıq qəbulu skriptləri manifesti deşifrə edir, zəng edin
   `DaRentPolicyV1::quote` və icarə dəftərlərini doldurun (əsas icarə, ehtiyat,
   bonuslar və gözlənilən çıxış kreditləri). Qeydə alınmış icarə arasında hər hansı uyğunsuzluq
   və yenidən hesablanmış kotirovkalar CI-də uğursuz olur.
3. **Sübut mükafatları:** PDP/PoTR planlaşdırıcıları uğur qeyd etdikdə qəbz verirlər.
   manifest həzm, sübut növü və əldə edilən XOR bonusunu ehtiva edir
   siyasət. İdarəetmə eyni təklifi yenidən hesablamaqla ödənişləri yoxlaya bilər.
4. **Çıxış kompensasiyası:** Alma orkestrləri imzalanmış çıxış xülasələrini təqdim edir.
   Torii GiB sayını `egress_credit_per_gib`-ə vurur və ödəniş verir
   icarə əmanətinə qarşı göstərişlər.

## TelemetriyaTorii qovşaqları icarə istifadəsini aşağıdakı Prometheus ölçüləri (etiketlər:
`cluster`, `storage_class`):

- `torii_da_rent_gib_months_total` — GiB-aylar `/v2/da/ingest` tərəfindən sitat gətirilib.
- `torii_da_rent_base_micro_total` — qəbul zamanı hesablanmış əsas icarə haqqı (mikro XOR).
- `torii_da_protocol_reserve_micro_total` — protokol ehtiyatı töhfələri.
- `torii_da_provider_reward_micro_total` — provayder tərəfindən icarə ödənişləri.
- `torii_da_pdp_bonus_micro_total` və `torii_da_potr_bonus_micro_total` —
  PDP/PoTR bonus hovuzları qəbul sitatından qaynaqlanır.

İqtisadiyyat panelləri bu sayğaclara etibar edir ki, bu sayğaclar ISI-ləri, ehtiyat kranları,
və PDP/PoTR bonus cədvəllərinin hamısı hər biri üçün qüvvədə olan siyasət parametrlərinə uyğun gəlir
klaster və saxlama sinfi. SoraFS Capacity Health Grafana lövhəsi
(`dashboards/grafana/sorafs_capacity_health.json`) indi xüsusi panelləri təqdim edir
icarə paylanması, PDP/PoTR bonus hesablanması və GiB-ay tutulması üçün imkan verir
Xəzinədarlıq, qəbulu nəzərdən keçirərkən Torii klaster və ya yaddaş sinfinə görə filtrasiya edəcək
həcmi və ödənişləri.

## Növbəti addımlar

- ✅ `/v2/da/ingest` qəbzləri indi `rent_quote`-i yerləşdirir və CLI/SDK səthləri sitat gətirilənləri göstərir
  baza icarəsi, ehtiyat payı və PDP/PoTR bonusları beləliklə, təqdim edənlər XOR öhdəliklərini əvvəlcədən nəzərdən keçirə bilsinlər.
  yüklərin verilməsi.
- İcarəyə götürmə kitabını qarşıdakı DA reputasiyası/sifariş kitabçası ilə inteqrasiya edin
  yüksək mövcud provayderlərin düzgün ödənişləri aldığını sübut etmək.
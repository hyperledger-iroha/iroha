---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Node ↔ Müştəri Protokolu

Bu təlimatda kanonik protokol tərifini ümumiləşdirir
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Bayt səviyyəli Norito tərtibatları və dəyişiklik qeydləri üçün yuxarı axın spesifikasiyasından istifadə edin; portal
surəti əməliyyat məqamlarını SoraFS runbook-larının qalan hissəsinə yaxın saxlayır.

## Provayder Reklamları və Təsdiqləmə

SoraFS provayderləri qeybət edir `ProviderAdvertV1` faydalı yükləri (bax.
`crates/sorafs_manifest::provider_advert`) idarə olunan operator tərəfindən imzalanmışdır.
Reklamlar kəşf metadatasını və qoruyucuları çox mənbəyə bağlayır
orkestr icra zamanı tətbiq edir.

- **Ömür boyu** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Provayderlər
  hər 12 saatdan bir yenilənməlidir.
- **Capability TLVs** — TLV siyahısı nəqliyyat xüsusiyyətlərini reklam edir (Torii,
  QUIC+Noise, SoraNet releləri, satıcı genişləndirmələri). Naməlum kodlar atlana bilər
  zaman `allow_unknown_capabilities = true`, YAĞ təlimatına uyğun olaraq.
- **QoS göstərişləri** — `availability` səviyyəsi (İsti/İsti/Soyuq), maksimum axtarış
  gecikmə, paralellik limiti və əlavə yayım büdcəsi. QoS uyğun olmalıdır
  telemetriya müşahidə edilir və qəbul tərəfindən yoxlanılır.
- **Son nöqtələr və görüş mövzuları** — TLS/ALPN ilə konkret xidmət URL-ləri
  metadata və müştərilərin tikinti zamanı abunə olmalı olduqları kəşf mövzuları
  mühafizə dəstləri.
- **Yol müxtəlifliyi siyasəti** — `min_guard_weight`, AS/hovuz havalandırma qapaqları və
  `provider_failure_threshold` deterministik multi-peer gətirmələri mümkün edir.
- **Profil identifikatorları** — provayderlər kanonik sapı ifşa etməlidirlər (məs.
  `sorafs.sf1@1.0.0`); isteğe bağlı `profile_aliases` yaşlı müştərilərin köçməsinə kömək edir.

Doğrulama qaydaları sıfır payı, boş imkanları/son nöqtələri/mövzu siyahılarını rədd edir,
səhv ömür müddətləri və ya çatışmayan QoS hədəfləri. Qəbul zərfləri müqayisə edir
qeybət yeniləmələrindən əvvəl reklam və təklif orqanları (`compare_core_fields`).

### Range Fetch Extensions

Aralığı bilən provayderlərə aşağıdakı metadata daxildir:

| Sahə | Məqsəd |
|-------|---------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`, `min_granularity` və hizalanma/sübut bayraqlarını elan edir. |
| `StreamBudgetV1` | Könüllü paralellik/keçirmə qabiliyyəti zərfi (`max_in_flight`, `max_bytes_per_sec`, isteğe bağlı `burst`). Bir sıra qabiliyyəti tələb edir. |
| `TransportHintV1` | Sifarişli nəqliyyat tərcihləri (məsələn, `torii_http_range`, `quic_stream`, `soranet_relay`). Prioritetlər `0–15`-dir və dublikatlar rədd edilir. |

Alət dəstəyi:

- Provayderin reklam boru kəmərləri diapazon qabiliyyətini, axın büdcəsini və
  auditlər üçün deterministik faydalı yükləri yaymazdan əvvəl nəqliyyat göstərişləri.
- `cargo xtask sorafs-admission-fixtures` kanonik çoxmənbəli paketlər
  aşağı səviyyəli qurğularla yanaşı reklamlar
  `fixtures/sorafs_manifest/provider_admission/`.
- `stream_budget` və ya `transport_hints`-i buraxan diapazona malik reklamlar
  çox mənbəni saxlayaraq planlaşdırmadan əvvəl CLI/SDK yükləyiciləri tərəfindən rədd edilir
  qoşqu Torii qəbul gözləntilərinə uyğunlaşdırılıb.

## Gateway Range Endpoints

Şlüzlər reklam metadatasını əks etdirən deterministik HTTP sorğularını qəbul edir.

### `GET /v2/sorafs/storage/car/{manifest_id}`

| Tələb | Təfərrüatlar |
|-------------|---------|
| **Başlıqlar** | `Range` (yığılmış ofsetlərə uyğunlaşdırılmış tək pəncərə), `dag-scope: block`, `X-SoraFS-Chunker`, isteğe bağlı `X-SoraFS-Nonce` və məcburi baza64 `X-SoraFS-Stream-Token`. |
| **Cavablar** | `206` ilə `Content-Type: application/vnd.ipld.car`, xidmət göstərilən pəncərəni təsvir edən `Content-Range`, `X-Sora-Chunk-Range` metadata və əks-sədalanan chunker/token başlıqları. |
| **Uğursuzluq rejimləri** | Yanlış düzülmüş diapazonlar üçün `416`, çatışmayan/etibarsız nişanlar üçün `401`, axın/bayt büdcələri aşıldığında `429`. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Eyni başlıqlar və deterministik yığın həzmi ilə tək yığın gətirmə.
CAR dilimləri lazımsız olduqda təkrar cəhdlər və ya məhkəmə endirmələri üçün faydalıdır.

## Çox Mənbəli Orkestrator İş Akışı

SF-6 çox mənbəli gətirmə aktiv olduqda (`sorafs_fetch` vasitəsilə Rust CLI,
`sorafs_orchestrator` vasitəsilə SDK-lar):

1. **Girişləri toplayın** — manifest yığın planını deşifrə edin, ən son reklamları çəkin,
   və isteğe bağlı olaraq telemetriya şəklini (`--telemetry-json` və ya
   `TelemetrySnapshot`).
2. **Hesab lövhəsi yaradın** — `Orchestrator::build_scoreboard` qiymətləndirir
   uyğunluq və imtina səbəblərini qeyd edir; `sorafs_fetch --scoreboard-out`
   JSON-u davam etdirir.
3. **Cədvəl hissələri** — `fetch_with_scoreboard` (və ya `--plan`) diapazonu tətbiq edir
   məhdudiyyətlər, axın büdcələri, təkrar cəhd/peer hədləri (`--retry-budget`,
   `--max-peers`) və hər sorğu üçün manifest əhatəli axın işarəsi verir.
4. **Qəbzləri yoxlayın** — çıxışlara `chunk_receipts` və
   `provider_reports`; CLI xülasələri davam edir `provider_reports`,
   Sübut paketləri üçün `chunk_receipts` və `ineligible_providers`.

Operatorlara/SDK-lara verilən ümumi səhvlər:

| Səhv | Təsvir |
|-------|-------------|
| `no providers were supplied` | Filtrdən sonra uyğun giriş yoxdur. |
| `no compatible providers available for chunk {index}` | Müəyyən bir hissə üçün diapazon və ya büdcə uyğunsuzluğu. |
| `retry budget exhausted after {attempts}` | `--retry-budget` artırın və ya uğursuz həmyaşıdları çıxarın. |
| `no healthy providers remaining` | Təkrarlanan uğursuzluqlardan sonra bütün provayderlər əlil oldu. |
| `streaming observer failed` | Aşağı axın CAR yazıçısı dayandırıldı. |
| `orchestrator invariant violated` | Triaj üçün manifest, skorbord, telemetriya snapşotunu və CLI JSON-u çəkin. |

## Telemetriya və Sübut

- Orkestr tərəfindən yayılan ölçülər:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (manifest/region/provayder tərəfindən işarələnib). `telemetry_region` konfiqurasiyasında və ya vasitəsilə təyin edin
  CLI bayraqları beləliklə idarə panellərini donanma üzrə bölməyə imkan verir.
- CLI/SDK gətirmə xülasələrinə davamlı hesab lövhəsi JSON, yığın qəbzləri,
  və SF-6/SF-7 qapıları üçün buraxılış paketlərində göndərilməli olan provayder hesabatları.
- Gateway işləyiciləri `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`-i ifşa edir
  beləliklə, SRE panelləri orkestrator qərarlarını server davranışı ilə əlaqələndirə bilər.

## CLI & REST Köməkçiləri

- `iroha app sorafs pin list|show`, `alias list` və `replication list`
  pin-reyestr REST son nöqtələri və attestasiya blokları ilə xam Norito JSON çap edin
  audit sübutları üçün.
- `iroha app sorafs storage pin` və `torii /v2/sorafs/pin/register` Norito qəbul edir
  və ya JSON manifestləri üstəgəl əlavə ləqəb sübutları və varisləri; qüsurlu sübutlar
  `400` qaldırın, köhnəlmiş səthi `503` ilə `Warning: 110` və
  müddəti bitmiş sübutlar `412` qaytarır.
- `iroha app sorafs repair list` güzgüləri isə növbə filtrlərini təmir edir
  `repair claim|complete|fail|escalate` imzalanmış işçi hərəkətləri və ya slash göndərin
  təkliflər Torii. Slash təkliflərinə idarəetmənin təsdiqi xülasəsi daxil ola bilər
  (təsdiq/rədd/səslərin sayılmasından imtina üstəgəl təsdiqlənmiş_at/finalized_at
  vaxt ştampları); mövcud olduqda o, kvorum və mübahisə/apellyasiya pəncərələrini təmin etməlidir,
  əks halda təklif son tarixdə səslər həll olunana qədər mübahisədə qalır.
- Təmir siyahıları və işçi növbəsinin seçimi SLA-nın son tarixi, nasazlığın şiddəti və deterministik əlaqə kəsiciləri (növbəyə qoyulmuş vaxt, manifest həzm, bilet id) ilə təminatçının geridə qalması ilə sıralanır.
- Təmir statusu cavablarına base64 Norito ehtiva edən `events` massivi daxildir
  `RepairTaskEventV1` girişləri audit yolları üçün baş verməsi ilə sıralanır; siyahı
  ən son keçidlərlə məhdudlaşır.
- `iroha app sorafs gc inspect|dry-run --data-dir=/var/lib/sorafs` yalnız oxumaq üçün buraxılır
  audit sübutları üçün yerli manifest mağazasından saxlama hesabatları.
- REST son nöqtələri (`/v2/sorafs/pin`, `/v2/sorafs/aliases`,
  `/v2/sorafs/replication`) müştərilərin edə bilməsi üçün attestasiya strukturlarını ehtiva edir
  tədbir görməzdən əvvəl məlumatları ən son blok başlıqları ilə yoxlayın.

## İstinadlar

- Kanonik spesifikasiya:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito növləri: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI köməkçiləri: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orkestr qutusu: `crates/sorafs_orchestrator`
- İdarə paneli paketi: `dashboards/grafana/sorafs_fetch_observability.json`
---
id: orchestrator-config
lang: az
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Orchestrator Configuration
sidebar_label: Orchestrator Configuration
description: Configure the multi-source fetch orchestrator, interpret failures, and debug telemetry output.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

# Çox Mənbəli Alma Orkestratoru Bələdçisi

SoraFS çoxmənbəli gətirmə orkestratoru deterministik, paralel sürücülər
idarəetmə tərəfindən dəstəklənən reklamlarda dərc edilmiş provayder dəstindən endirmələr. Bu
bələdçi orkestri necə konfiqurasiya edəcəyini, hansı uğursuzluq siqnallarını gözlədiyini izah edir
yayım zamanı və hansı telemetriya axınları sağlamlıq göstəricilərini ifşa edir.

## 1. Konfiqurasiyaya baxış

Orkestr üç konfiqurasiya mənbəyini birləşdirir:

| Mənbə | Məqsəd | Qeydlər |
|--------|---------|-------|
| `OrchestratorConfig.scoreboard` | Provayder çəkilərini normallaşdırır, telemetriyanın təzəliyini təsdiq edir və auditlər üçün istifadə edilən JSON skorbordunu saxlayır. | `crates/sorafs_car::scoreboard::ScoreboardConfig` tərəfindən dəstəklənir. |
| `OrchestratorConfig.fetch` | İş vaxtı limitlərini tətbiq edir (yenidən cəhd büdcələri, paralellik sərhədləri, yoxlama keçidləri). | `crates/sorafs_car::multi_fetch`-də `FetchOptions` xəritələri. |
| CLI / SDK parametrləri | Həmyaşıdların sayını məhdudlaşdırın, telemetriya bölgələrini əlavə edin və inkar/gücləndirici siyasətləri yerləşdirin. | `sorafs_cli fetch` bu bayraqları birbaşa ifşa edir; SDK-lar onları `OrchestratorConfig` vasitəsilə ötürür. |

`crates/sorafs_orchestrator::bindings`-dəki JSON köməkçiləri bütövlükdə seriyaya çevrilir
Norito JSON-da konfiqurasiya, onu SDK bağlamaları arasında portativ edir və
avtomatlaşdırma.

### 1.1 Nümunə JSON Konfiqurasiyası

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Faylı adi `iroha_config` təbəqələşdirmə (`defaults/`, istifadəçi,
faktiki) beləliklə, deterministik yerləşdirmələr qovşaqlar arasında eyni məhdudiyyətləri miras alır.
SNNet-5a təqdimatı ilə uyğunlaşan yalnız birbaşa ehtiyat profili üçün,
`docs/examples/sorafs_direct_mode_policy.json` və yoldaşı ilə məsləhətləşin
`docs/source/sorafs/direct_mode_pack.md`-də təlimat.

### 1.2 Uyğunluğun ləğv edilməsi

SNNet-9 idarəetməyə əsaslanan uyğunluğu orkestratora ötürür. Yeni
`compliance` JSON konfiqurasiyasındakı `compliance` obyekti kəsikləri çəkir
gətirmə boru kəmərini yalnız birbaşa rejimə məcbur edən:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` ISO‑3166 alfa‑2 kodlarını elan edir, burada bu
  orkestr instansiyası işləyir. Kodlar zamanı böyük hərflə normallaşdırılır
  təhlil.
- `jurisdiction_opt_outs` idarəetmə reyestrini əks etdirir. İstənilən operator zaman
  yurisdiksiya siyahıda görünür, orkestr icra edir
  `transport_policy=direct-only` və siyasətin geri qaytarılma səbəbini verir
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` manifest həzmləri siyahıya alır (korlanmış CID-lər, aşağıdakı kimi kodlanır
  böyük hex). Uyğun yüklər də yalnız birbaşa planlaşdırma və
  telemetriyada `compliance_blinded_cid_opt_out` geri dönüşünü üzə çıxarın.
- `audit_contacts` URI-lərin idarə edilməsinin operatorların dərc etməyi gözlədiyini qeyd edir.
  onların GAR oyun kitabları.
- `attestations` siyasəti dəstəkləyən imzalanmış uyğunluq paketlərini çəkir.
  Hər bir giriş isteğe bağlı `jurisdiction` (ISO-3166 alfa-2 kodu), bir
  `document_uri`, kanonik 64 simvollu `digest_hex`, buraxılış
  vaxt damgası `issued_at_ms` və isteğe bağlı `expires_at_ms`. Bu artefaktlar
  orkestratorun audit yoxlama siyahısına daxil olun ki, idarəetmə alətləri əlaqələndirə bilsin
  imzalanmış sənədləri ləğv edir.

Uyğunluq blokunu operatorlar üçün adi konfiqurasiya təbəqəsi vasitəsilə təmin edin
deterministik overrides almaq. Orkestr uyğunluğu _after_ tətbiq edir
yazma rejimi göstərişləri: hətta SDK `upload-pq-only`, yurisdiksiya və ya tələb etsə belə
manifest imtinaları hələ də yalnız birbaşa nəqliyyata qayıdır və olmadıqda sürətlə uğursuz olur
uyğun provayderlər mövcuddur.

Canonical imtina kataloqları altında yaşayır
`governance/compliance/soranet_opt_outs.json`; İdarəetmə Şurası dərc edir
etiketli buraxılışlar vasitəsilə yeniləmələr. Tam nümunə konfiqurasiya (o cümlədən
sertifikatlar) `docs/examples/sorafs_compliance_policy.json`-də mövcuddur və
əməliyyat prosesi ələ keçirilir
[GAR uyğunluğu üzrə dərs kitabı](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 CLI və SDK Düymələri

| Bayraq / Sahə | Effekt |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Hesab tablosu filtrindən neçə provayderin sağ qalmasını məhdudlaşdırır. Hər uyğun provayderdən istifadə etmək üçün `None` olaraq təyin edin. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Hər parça üçün yenidən cəhdlər. Həddini aşmaq `MultiSourceError::ExhaustedRetries`-i artırır. |
| `--telemetry-json` | Hesab lövhəsi qurucusuna gecikmə/uğursuzluq snapshotlarını daxil edir. `telemetry_grace_secs` xaricində köhnəlmiş telemetriya provayderlərin uyğun olmadığını göstərir. |
| `--scoreboard-out` | Qaçışdan sonrakı yoxlama üçün hesablanmış xal tablosunu (uyğun + uyğun olmayan provayderlər) saxlayır. |
| `--scoreboard-now` | Hesab lövhəsinin vaxt möhürünü ləğv edir (Unix saniyə) beləliklə, fikstür çəkilişləri deterministik olaraq qalır. |
| `--deny-provider` / hesab siyasəti çəngəl | Reklamları silmədən provayderləri planlaşdırmadan qəti şəkildə istisna edin. Tez cavab qara siyahıya salmaq üçün faydalıdır. |
| `--boost-provider=name:delta` | İdarəetmə çəkilərinə toxunulmadan provayder üçün çəkili dəyirmi kreditləri tənzimləyin. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Etiketlər ölçülər və strukturlaşdırılmış jurnallar buraxdı ki, tablolar coğrafiyaya və ya yayım dalğasına görə dönə bilsin. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Çoxmənbəli orkestrator bazaya uyğun olduğundan defolt olaraq `soranet-first`. Qiyməti endirərkən və ya uyğunluq direktivinə əməl edərkən `direct-only` istifadə edin və `soranet-strict`-i yalnız PQ pilotları üçün ehtiyatda saxlayın; uyğunluğu ləğv etmələri hələ də sərt tavan rolunu oynayır. |

SoraNet-first indi defolt göndərmədir və geri çəkilmələr müvafiq SNNet blokerinə istinad etməlidir. SNNet-4/5/5a/5b/6a/7/8/12/13 məzunu olduqdan sonra idarəetmə tələb olunan duruşu irəli aparacaq (`soranet-strict` istiqamətində); o vaxta qədər yalnız insidentlə idarə olunan ləğvetmələr `direct-only`-ə üstünlük verməli və onlar buraxılış jurnalında qeyd edilməlidir.

Yuxarıdakı bütün bayraqlar həm `sorafs_cli fetch`, həm də `--` üslublu sintaksisi qəbul edir.
geliştirici ilə üzləşən `sorafs_fetch` ikili. SDK-lar eyni variantları yazılan vasitəsilə ifşa edir
inşaatçılar.

### 1.4 Mühafizə Keşinin İdarə Edilməsi

CLI indi SoraNet qoruyucu seçicisinə qoşulur ki, operatorlar girişi sanclaya bilsinlər
tam SNNet-5 nəqliyyatının buraxılışından determinist şəkildə irəliləyir. üç
yeni bayraqlar iş axınına nəzarət edir:

| Bayraq | Məqsəd |
|------|---------|
| `--guard-directory <PATH>` | Ən son relay konsensusunu təsvir edən JSON faylına işarə edir (alt dəst aşağıda göstərilmişdir). Kataloqdan keçmək, gətirməni yerinə yetirməzdən əvvəl qoruyucu önbelleği yeniləyir. |
| `--guard-cache <PATH>` | Norito kodlu `GuardSet`-i davam etdirir. Heç bir yeni kataloq təmin edilmədikdə belə, sonrakı qaçışlar keşi təkrar istifadə edir. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Saxlanılacaq giriş qoruyucularının sayı (defolt 3) və saxlama pəncərəsi (defolt 30 gün) üçün isteğe bağlı ləğvetmələr. |
| `--guard-cache-key <HEX>` | Blake3 MAC ilə qoruyucu keşləri etiketləmək üçün istifadə olunan isteğe bağlı 32 baytlıq açar beləliklə, fayl təkrar istifadə edilməzdən əvvəl yoxlanıla bilər. |

Mühafizə kataloqunun faydalı yükləri yığcam sxemdən istifadə edir:

`--guard-directory` bayrağı indi Norito kodlu olmasını gözləyir
`GuardDirectorySnapshotV2` faydalı yük. İkili snapshot ehtiva edir:

- `version` — sxem versiyası (hazırda `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` — konsensus
  hər bir daxil edilmiş sertifikata uyğun gəlməli olan metadata.
- `validation_phase` — sertifikat siyasəti qapısı (`1` = tək Ed25519 imzasına icazə verin,
  `2` = ikili imzaya üstünlük verilir, `3` = ikili imza tələb olunur).
- `issuers` — `fingerprint`, `ed25519_public` və `mldsa65_public` ilə idarəetmə emitentləri.
  Barmaq izləri kimi hesablanır
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — SRCv2 paketlərinin siyahısı (`RelayCertificateBundleV2::to_cbor()` çıxışı). Hər bir paket
  rele deskriptorunu, qabiliyyət bayraqlarını, ML-KEM siyasətini və ikili Ed25519/ML-DSA-65-i daşıyır
  imzalar.

CLI kataloqu birləşdirməzdən əvvəl elan edilmiş emitent açarları ilə hər bir paketi yoxlayır

Ən son konsensusu birləşdirmək üçün `--guard-directory` ilə CLI-yə müraciət edin.
mövcud önbellek. Selektor hələ də içəridə olan bərkidilmiş qoruyucuları qoruyur
saxlama pəncərəsi və kataloqda uyğundur; müddəti bitmiş yeni releləri əvəz edir
girişlər. Uğurlu əldə edildikdən sonra yenilənmiş keş yenidən yola yazılır
`--guard-cache` vasitəsilə təmin edilir, sonrakı sessiyaları deterministik saxlayır. SDK-lar
zəng etməklə eyni davranışı təkrarlaya bilər
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` və
nəticədə yaranan `GuardSet`-i `SorafsGatewayFetchOptions` vasitəsilə bağlayın.

`ml_kem_public_hex` selektora PQ qabiliyyətli qoruyucuları prioritetləşdirməyə imkan verir.
SNNet-5 buraxılışı. Mərhələ keçidləri (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) indi klassik releləri avtomatik olaraq azaldır: PQ qoruyucusu olduqda
mövcud selektor artıq klassik sancaqlar düşür, belə ki, sonrakı seanslar üstünlük təşkil edir
hibrid əl sıxma. CLI/SDK xülasələri vasitəsilə ortaya çıxan qarışığı üzə çıxarır
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`,
`anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` və yoldaş namizəd/defisit/təchizat deltası
sahələr, qəhvəyi və klassik geri dönmələri aydın edir.

Mühafizə kataloqları indi vasitəsilə tam SRCv2 paketini yerləşdirə bilər
`certificate_base64`. Orkestrator hər paketi deşifrə edir, onu yenidən təsdiqləyir
Ed25519/ML-DSA imzaları və təhlil edilmiş sertifikatı yanında saxlayır
önbelleği qoruyun. Sertifikat mövcud olduqda o, kanonik mənbəyə çevrilir
PQ düymələri, əl sıxma dəsti seçimləri və çəki; müddəti bitmiş sertifikatlardır
dövrə həyat dövrünün idarə edilməsi vasitəsilə yayılır və vasitəsilə üzə çıxır
qeyd edən `telemetry::sorafs.guard` və `telemetry::sorafs.circuit`
etibarlılıq pəncərəsi, əl sıxma paketləri və ikili imzalar üçün müşahidə edilib
hər bir gözətçi.

Snapshotları naşirlərlə sinxronlaşdırmaq üçün CLI köməkçilərindən istifadə edin:```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` SRCv2 snapşotunu diskə yazmadan əvvəl yükləyir və yoxlayır,
`verify` isə digər mənbələrdən alınan artefaktlar üçün doğrulama boru xəttini təkrarlayır
komandalar, CLI/SDK qoruyucu seçici çıxışını əks etdirən JSON xülasəsi yayır.

### 1.5 Circuit Lifecycle Manager

Həm relay kataloqu, həm də qoruyucu keş təmin edildikdə, orkestr
SoraNet sxemlərini əvvəlcədən qurmaq və yeniləmək üçün dövrənin həyat dövrü menecerini aktivləşdirir
hər almadan əvvəl. Konfiqurasiya `OrchestratorConfig`-də yaşayır
(`crates/sorafs_orchestrator/src/lib.rs:305`) iki yeni sahə vasitəsilə:

- `relay_directory`: SNNet-3 qovluğunun şəklini daşıyır, beləliklə orta/çıxış hops
  deterministik şəkildə seçilə bilər.
- `circuit_manager`: isteğe bağlı konfiqurasiya (defolt olaraq aktivdir)
  dövrə TTL.

Norito JSON indi `circuit_manager` blokunu qəbul edir:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK-lar kataloq məlumatlarını ötürür
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) və CLI istənilən vaxt onu avtomatik olaraq bağlayır
`--guard-directory` verilir (`crates/iroha_cli/src/commands/sorafs.rs:365`).

Mühafizə metadatası dəyişdikdə menecer sxemləri yeniləyir (son nöqtə, PQ açarı,
və ya bərkidilmiş vaxt damğası) və ya TTL keçir. Köməkçi `refresh_circuits`
hər qəbuldan əvvəl çağırılır (`crates/sorafs_orchestrator/src/lib.rs:1346`)
operatorların həyat dövrü qərarlarını izləyə bilməsi üçün `CircuitEvent` qeydləri buraxır. Islatmaq
test `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) sabit gecikməni nümayiş etdirir
üç qoruyucu fırlanma üzrə; əlavə edilən hesabata baxın
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Yerli QUIC Proksi

Orkestrator isteğe bağlı olaraq yerli QUIC proksisini yarada bilər ki, brauzer uzantıları olsun
və SDK adapterləri sertifikatları idarə etməli və ya keş açarlarını qorumalı deyil. The
proksi geri dönmə ünvanına bağlanır, QUIC bağlantılarını dayandırır və a
Sertifikatı və əlavə qoruyucu keş açarını təsvir edən Norito manifest
müştəri. Proxy tərəfindən yayılan nəqliyyat hadisələri vasitəsilə sayılır
`sorafs_orchestrator_transport_events_total`.

JSON orkestrində yeni `local_proxy` bloku vasitəsilə proksi-ni aktivləşdirin:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` proksinin harada qulaq asdığına nəzarət edir (istəmək üçün `0` portundan istifadə edin
  efemer liman).
- `telemetry_label` ölçülərə yayılır ki, tablosuna görə fərqlənə bilsin
  gətirmə sessiyalarından proksilər.
- `guard_cache_key_hex` (isteğe bağlı) proksiyə eyni açarlı qoruyucu səthi təmin etməyə imkan verir
  CLI/SDK-ların etibar etdiyi önbellek, brauzer uzantılarını sinxronizasiyada saxlayır.
- `emit_browser_manifest` əl sıxma manifestini qaytarıb-qaytarmamasını dəyişdirir
  uzantılar saxlaya və təsdiq edə bilər.
- `proxy_mode` proksinin yerli trafiki (`bridge`) körpü edib-etmədiyini seçir
  SDK-ların özləri SoraNet sxemlərini aça bilməsi üçün yalnız metadata yayır
  (`metadata-only`). Proksi standart olaraq `bridge`; `metadata-only` təyin edərkən a
  iş stansiyası manifesti axınları ötürmədən ifşa etməlidir.
- `prewarm_circuits`, `max_streams_per_circuit` və `circuit_ttl_hint_secs`
  Brauzerə əlavə göstərişlər yerləşdirin ki, paralel axınları büdcə edə bilsin və
  proksinin sxemləri necə aqressiv şəkildə təkrar istifadə etdiyini anlayın.
- `car_bridge` (isteğe bağlı) yerli CAR arxiv keşində nöqtələr. `extension`
  axın hədəfi `*.car` buraxdıqda sahə əlavə edilmiş şəkilçiyə nəzarət edir; təyin edin
  Əvvəlcədən sıxılmış `*.car.zst` faydalı yüklərə birbaşa xidmət etmək üçün `allow_zst = true`.
- `kaigi_bridge` (isteğe bağlı) proksiyə yığılmış Kaigi marşrutlarını ifşa edir. The
  `room_policy` sahəsi körpünün `public` və ya işlədiyini elan edir
  `authenticated` rejimi beləliklə brauzer müştəriləri düzgün GAR etiketlərini əvvəlcədən seçə bilsinlər.
- `sorafs_cli fetch` `--local-proxy-mode=bridge|metadata-only` və
  `--local-proxy-norito-spool=PATH` operatorlara keçid imkanını ləğv edir
  icra vaxtı rejimi və ya JSON siyasətini dəyişdirmədən alternativ makaralara işarə edin.
- `downgrade_remediation` (isteğe bağlı) avtomatik endirmə çəngəlini konfiqurasiya edir.
  Aktivləşdirildikdə, orkestr endirmə partlayışları üçün relay telemetriyasını izləyir
  və `window_secs` daxilində konfiqurasiya edilmiş `threshold`-dən sonra yerli
  proxy-yə `target_mode` (defolt `metadata-only`). Bir dəfə endirmələr dayanır
  proxy `cooldown_secs`-dən sonra `resume_mode`-ə qayıdır. `modes` istifadə edin
  Tətiyi xüsusi relay rollarına əhatə etmək üçün massiv (giriş releləri üçün standart).

Proksi körpü rejimində işlədikdə o, iki proqram xidmətinə xidmət edir:

- **`norito`** – müştərinin axın hədəfi ilə müqayisədə həll edilir
  `norito_bridge.spool_dir`. Hədəflər sanitarlaşdırılıb (keçmə, mütləq deyil
  yollar) və faylda genişlənmə olmadıqda konfiqurasiya edilmiş şəkilçi tətbiq olunur
  faydalı yük brauzerə hərfi olaraq ötürülməzdən əvvəl.
- **`car`** – axın hədəfləri `car_bridge.cache_dir` daxilində həll olunur, miras alır
  konfiqurasiya edilmiş standart genişləndirmə və sıxılmış yükləri rədd etmə
  `allow_zst` təyin edilib. Uğurlu körpülər əvvəllər `STREAM_ACK_OK` ilə cavab verir
  Arxiv baytlarının ötürülməsi, belə ki, müştərilər boru kəmərinin yoxlanışını həyata keçirə bilsinlər.

Hər iki halda proksi HMAC keş-teqini təmin edir (qoruyucu keş açarı olduğu zaman
əl sıxma zamanı mövcuddur) və qeydlər `norito_*` / `car_*` telemetriya səbəbini
kodlar beləliklə idarə panelləri uğurları, çatışmayan faylları və sanitarizasiyanı fərqləndirə bilər
bir baxışda uğursuzluqlar.

`Orchestrator::local_proxy().await` işləyən tutacaqı ifşa edir ki, zəng edənlər edə bilsinlər
PEM sertifikatını oxuyun, brauzer manifestini götürün və ya zərif tələb edin
proqram çıxdıqda bağlanır.

Aktiv edildikdə, proksi indi **manifest v2** qeydlərinə xidmət edir. Mövcud olandan başqa
sertifikat və qoruyucu keş açarı, v2 əlavə edir:

- `alpn` (`"sorafs-proxy/1"`) və `capabilities` massivi, beləliklə müştərilər təsdiq edə bilsinlər
  danışmalı olduqları axın protokolu.
- Hər əl sıxma `session_id` və önbellek etiketleme duzu (`cache_tagging` bloku)
  sessiya başına mühafizə yaxınlıqlarını və HMAC teqlərini əldə edin.
- Dövrə və qoruyucu seçim göstərişləri (`circuit`, `guard_selection`,
  `route_hints`) beləliklə brauzer inteqrasiyaları axınlar başlamazdan əvvəl daha zəngin UI-ni ifşa edə bilər.
  açıldı.
- Yerli cihazlar üçün seçmə və məxfilik düymələri ilə `telemetry_v2`.
- Hər bir `STREAM_ACK_OK`-ə `cache_tag_hex` daxildir. Müştərilər dəyəri əks etdirir
  `x-sorafs-cache-tag` başlığı HTTP və ya TCP sorğularını bu cür keşlənmişdir
  mühafizə seçimləri istirahətdə şifrələnmiş qalır.

v1 alt dəstinə etibar etməyə davam edin.

## 2. Uğursuzluğun Semantikası

Orkestrator təkdən əvvəl ciddi qabiliyyət və büdcə yoxlamalarını həyata keçirir
bayt ötürülür. Uğursuzluqlar üç kateqoriyaya bölünür:

1. **Uyğunluq uğursuzluqları (uçuşdan əvvəl).** Təchizatçıların uçuş məsafəsi imkanlarından məhrum olması,
   vaxtı keçmiş reklamlar və ya köhnəlmiş telemetriya tablosu artefaktına daxil edilir və
   planlaşdırmadan çıxarıldı. CLI xülasələri `ineligible_providers`-i doldurur
   operatorların idarəetmənin sürüşməsini kazımadan yoxlaya bilməsi üçün səbəblərlə sıralayın
   loglar.
2. **İş vaxtının tükənməsi.** Hər bir provayder ardıcıl uğursuzluqları izləyir. Bir dəfə
   konfiqurasiya edilmiş `provider_failure_threshold` əldə olundu, provayder qeyd edildi
   Sessiyanın qalan hissəsi üçün `disabled`. Əgər hər provayder keçid edirsə
   `disabled`, orkestr qayıdır
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Deterministik abortlar.** Sərt səthi strukturlaşdırılmış səhvlər kimi məhdudlaşdırır:
   - `MultiSourceError::NoCompatibleProviders` — manifest bir parça tələb edir
     span və ya uyğunlaşdırma qalan provayderlər hörmət edə bilməz.
   - `MultiSourceError::ExhaustedRetries` — hər hissəyə yenidən cəhd büdcəsi idi
     istehlak edilmişdir.
   - `MultiSourceError::ObserverFailed` - aşağı axın müşahidəçiləri (axın qarmaqları)
     təsdiqlənmiş parçanı rədd etdi.

Hər bir səhv xəta törədən yığın indeksini və mövcud olduqda yekun indeksi özündə cəmləşdirir
provayderin uğursuzluğu səbəbi. Bunları sərbəst buraxma blokerləri kimi qəbul edin - eyni şəkildə yenidən cəhd edin
giriş əsas reklam, telemetriya və ya qədər uğursuzluğu təkrar edəcək
provayderin sağlamlığında dəyişikliklər.

### 2.1 Hesab Tablosunun Davamlılığı

`persist_path` konfiqurasiya edildikdə, orkestr yekun tablonu yazır
hər qaçışdan sonra. JSON sənədinə daxildir:

- `eligibility` (`eligible` və ya `ineligible::<reason>`).
- `weight` (bu qaçış üçün normallaşdırılmış çəki təyin edilmişdir).
- `provider` metadata (identifikator, son nöqtələr, paralellik büdcəsi).

Buraxılış artefaktları ilə yanaşı skorbord snapshotlarını arxivləşdirin və qara siyahıya salın
tətbiqetmə qərarları yoxlanıla bilər.

## 3. Telemetriya və Sazlama

### 3.1 Prometheus Metriklər

Orkestrator `iroha_telemetry` vasitəsilə aşağıdakı göstəriciləri yayır:| Metrik | Etiketlər | Təsvir |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Uçuşda təşkil edilmiş gətirmələrin ölçüsü. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histoqramın qeydi başdan sona gətirmə gecikməsi. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Terminal uğursuzluqlarının sayğacı (yenidən cəhdlər tükəndi, provayder yoxdur, müşahidəçi nasazlığı). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Provayderə görə təkrar cəhdlərin sayı. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Seans səviyyəli provayder xətalarının sayğacını söndürməyə gətirib çıxaran. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Yayımlanma mərhələsi və geri qaytarılma səbəbi ilə qruplaşdırılan anonimlik siyasəti qərarlarının sayı (qarşılaşdı və ləğv edildi). |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Seçilmiş SoraNet dəsti arasında PQ relay payının histoqramı. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Hesab lövhəsinin görüntüsündə PQ relay təchizatı nisbətlərinin histoqramı. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Siyasət çatışmazlığının histoqramı (hədəf və faktiki PQ payı arasındakı boşluq). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Hər sessiyada istifadə olunan klassik relay payının histoqramı. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Hər seans üçün seçilmiş klassik relay saylarının histoqramı. |

İstehsal düymələrini çevirməzdən əvvəl ölçüləri səhnələşdirmə panellərinə inteqrasiya edin.
Tövsiyə olunan tərtibat SF-6 müşahidə planını əks etdirir:

1. **Aktiv gətirmələr** — ölçü cihazı uyğun tamamlamalar olmadan dırmaşdıqda xəbərdarlıq edir.
2. **Yenidən cəhd nisbəti** — `retry` sayğacları tarixi əsas göstəriciləri aşdıqda xəbərdarlıq edir.
3. **Provayder xətaları** — hər hansı bir provayder keçdikdə peycer xəbərdarlıqlarını işə salır
   15 dəqiqə ərzində `session_failure > 0`.

### 3.2 Strukturlaşdırılmış Giriş Hədəfləri

Orkestr deterministik hədəflər üçün strukturlaşdırılmış hadisələri dərc edir:

- `telemetry::sorafs.fetch.lifecycle` — `start` və `complete` həyat dövrü
  yığın sayları, təkrar cəhdlər və ümumi müddəti olan markerlər.
- `telemetry::sorafs.fetch.retry` — təkrar cəhd hadisələri (`provider`, `reason`,
  `attempts`) əl triajına qidalandırmaq üçün.
- `telemetry::sorafs.fetch.provider_failure` — provayderlər səbəbiylə deaktiv edilib
  təkrarlanan səhvlər.
- `telemetry::sorafs.fetch.error` — terminal xətaları ilə ümumiləşdirilmişdir
  `reason` və əlavə provayder metadatası.

Bu axınları mövcud Norito log boru kəmərinə yönləndirin ki, insident cavabı alınsın
yeganə həqiqət mənbəyinə malikdir. Həyat dövrü hadisələri vasitəsilə PQ/klassik qarışığı ifşa edir
`anonymity_effective_policy`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` və onların köməkçi sayğacları,
ölçüləri sıyırmadan tablosuna tel düzəltməyi asanlaşdırır. ərzində
GA yayımları, həyat dövrü/yenidən cəhd hadisələri üçün log səviyyəsini `info`-ə bağlayın və etibar edin
Terminal xətaları üçün `warn`.

### 3.3 JSON Xülasələri

Həm `sorafs_cli fetch`, həm də Rust SDK aşağıdakıları ehtiva edən strukturlaşdırılmış xülasə qaytarır:

- `provider_reports` müvəffəqiyyət/uğursuzluq sayıları və provayderin olub-olmaması
  əlil.
- `chunk_receipts`, hansı provayderin hər bir parçanı qane etdiyini təfərrüatlandırır.
- `retry_stats` və `ineligible_providers` massivləri.

Səhv fəaliyyət göstərən provayderləri sazlayarkən xülasə faylını arxivləşdirin - qəbzlər xəritəsi
birbaşa yuxarıdakı log metadatasına.

## 4. Əməliyyat yoxlama siyahısı

1. **CI-də mərhələ konfiqurasiyası.** `sorafs_fetch`-i hədəflə işə salın
   konfiqurasiya, uyğunluq görünüşünü çəkmək üçün `--scoreboard-out`-i keçin və
   əvvəlki buraxılışdan fərqlidir. Hər hansı gözlənilməz uyğun olmayan provayder fəaliyyətini dayandırır
   promosyon.
2. **Telemetriyanı doğrulayın.** Yerləşdirmə ixracını təmin edin `sorafs.fetch.*`
   istifadəçilər üçün çox mənbəli gətirmələri işə salmadan əvvəl ölçülər və strukturlaşdırılmış qeydlər.
   Metriklərin olmaması adətən orkestr fasadının olmadığını göstərir
   çağırdı.
3. **Sənəd ləğv edilir.** Təcili `--deny-provider` və ya tətbiq edərkən
   `--boost-provider` parametrləri, JSON-u (və ya CLI çağırışını) yerinə yetirin
   dəyişiklik jurnalı. Geri çəkilmələr ləğvetməni geri qaytarmalı və yeni hesab tablosunu tutmalıdır
   snapshot.
4. **Tüstü sınaqlarını yenidən həyata keçirin.** Yenidən cəhd büdcələrini və ya provayder limitlərini dəyişdirdikdən sonra,
   kanonik qurğunu (`fixtures/sorafs_manifest/ci_sample/`) yenidən gətirin və
   yığın qəbzlərinin deterministik olaraq qaldığını yoxlayın.

Yuxarıdakı addımları izləmək orkestratorun davranışını təkrarlana bilir
mərhələli buraxılışlar və insidentlərə cavab vermək üçün lazım olan telemetriyanı təmin edir.

### 4.1 Siyasət Qaydaları

Operatorlar aktiv nəqliyyat/anonimlik mərhələsini redaktə etmədən təyin edə bilərlər
`policy_override.transport_policy` və təyin etməklə əsas konfiqurasiya
`policy_override.anonymity_policy` öz `orchestrator` JSON (və ya təchiz
`--transport-policy-override=` / `--anonymity-policy-override=`
`sorafs_cli fetch`). Hər hansı ləğvetmə mövcud olduqda, orkestrator onu atlayır
adi işləmə geriliyi: tələb olunan PQ səviyyəsi təmin edilə bilməzsə,
alma səssizcə endirmək əvəzinə `no providers` ilə uğursuz olur. -a geri qayıt
defolt davranış ləğvetmə sahələrini təmizləmək qədər sadədir.

Standart `iroha_cli app sorafs fetch` əmri eyni ləğv bayraqlarını ifşa edir,
onları şlüz müştərisinə ötürür ki, ad-hoc alınır və avtomatlaşdırma skriptləri olsun
eyni mərhələ pinning davranışını paylaşın.

Cross-SDK qurğuları `fixtures/sorafs_gateway/policy_override/` altında yaşayır. The
CLI, Rust müştəri, JavaScript bağlamaları və Swift qoşqu deşifrəsi
`override.json` öz paritet dəstlərində beləliklə, yüklərin ləğv edilməsində istənilən dəyişiklik
bu qurğu yeniləməli və `cargo test -p iroha`, `npm test` və
SDK-ları uyğunlaşdırmaq üçün `swift test`. Həmişə bərpa edilmiş armaturu əlavə edin
baxışı dəyişdirin ki, aşağı axın istehlakçıları ləğvetmə müqaviləsini fərqləndirə bilsinlər.

İdarəetmə hər ləğv üçün runbook girişini tələb edir. Səbəbini qeyd edin,
gözlənilən müddət və dəyişiklik jurnalınızdakı geri qaytarma tetiği, PQ-ya bildirin
ratchet fırlanma kanalını açın və imzalanmış təsdiqi eyni artefakt üçün əlavə edin
hesab tablosunun anlık görüntüsünü saxlayan paket. Üstünlüklər qısa müddət üçün nəzərdə tutulub
fövqəladə hallar (məsələn, PQ gözətçisinin söndürülməsi); uzunmüddətli siyasət dəyişiklikləri getməlidir
normal şura səsverməsi vasitəsilə qovşaqlar yeni defoltda birləşir.

### 4.2 PQ Ratchet Yanğın Qazması

- **Runbook:** Üçün `docs/source/soranet/pq_ratchet_runbook.md`-i izləyin
  mühafizəçi-kataloqla işləmə və geri çəkilmə daxil olmaqla, yüksəliş/azaltma məşqi.
- **Dashboard:** Monitorinq üçün `dashboards/grafana/soranet_pq_ratchet.json` idxal edin
  `sorafs_orchestrator_policy_events_total`, tükənmə dərəcəsi və PQ nisbəti orta
  qazma zamanı.
- **Avtomatlaşdırma:** `cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics`
  eyni keçidləri həyata keçirir və metriklərin artımını yoxlayır
  operatorlar canlı məşqi keçirməzdən əvvəl gözlənilir.

## 5. Yayım kitabçaları

SNNet-5 nəqliyyatının təqdimatı yeni mühafizə seçimini, idarəetməni təqdim edir
sertifikatlar və siyasətin geri dönmələri. Aşağıdakı oyun kitabları ardıcıllığı kodlaşdırır
son istifadəçilər üçün çox mənbəli əldəetmələri, üstəlik aşağı səviyyəni aktivləşdirmədən əvvəl edin
birbaşa rejimə qayıdır.

### 5.1 Tərtibatçı Uçuşdan əvvəl (CI / Mərhələ)

1. **Cİ-də hesab lövhələrini bərpa edin.** `sorafs_cli fetch` (və ya SDK) işə salın
   ekvivalent) ilə `fixtures/sorafs_manifest/ci_sample/` manifestinə qarşı
   namizəd konfiqurasiyası. vasitəsilə hesab tablosunu davam etdirin
   `--scoreboard-out=artifacts/sorafs/scoreboard.json` və təsdiq edin:
   - `anonymity_status=="met"` və `anonymity_pq_ratio` hədəfə cavab verir
     mərhələ (`anon-guard-pq`, `anon-majority-pq` və ya `anon-strict-pq`).
   - Deterministik yığın qəbzləri hələ də sadiq qalan qızıl dəstlə uyğun gəlir
     anbar.
2. **Açıq idarəçiliyi yoxlayın.** CLI/SDK xülasəsini yoxlayın və
   yeni üzə çıxan `manifest_governance.council_signatures` serialı ehtiva edir
   gözlənilən Şuranın barmaq izləri. Bu, şlüz cavablarının GAR-ı göndərdiyini təsdiqləyir
   zərf və `validate_manifest` onu qəbul etdi.
3. **Uyğunluğu ləğv edin.** Hər bir yurisdiksiya profilini buradan yükləyin
   `docs/examples/sorafs_compliance_policy.json` və orkestratoru təsdiqləyin
   düzgün siyasət ehtiyatını verir (`compliance_jurisdiction_opt_out` və ya
   `compliance_blinded_cid_opt_out`). Xeyr olduqda nəticə çıxarma uğursuzluğunu qeyd edin
   uyğun nəqliyyat mövcuddur.
4. **Endirməni simulyasiya edin.** `transport_policy`-i `direct-only`-ə çevirin.
   konfiqurasiyanı sınaqdan keçirin və orkestratoru təmin etmək üçün gətirməni yenidən işə salın
   SoraNet relelərinə toxunmadan Torii/QUIC-ə qayıdır. Bu JSON-u saxlayın
   Variant versiya nəzarəti altındadır ki, bir müddət ərzində sürətlə irəlilənə bilsin
   hadisə.

### 5.2 Operator Yayımı (İstehsal Dalğaları)1. **`iroha_config` vasitəsilə konfiqurasiyanı mərhələləndirin.** İstifadə olunan dəqiq JSON-u dərc edin
   CI-də `actual` təbəqənin ləğvi kimi. Orkestr podunu / binarını təsdiqləyin
   başlanğıcda yeni konfiqurasiya hashini qeyd edir.
2. **Əsas qoruyucu keşlər.** `--guard-directory` vasitəsilə relay kataloqunu yeniləyin
   və `--guard-cache` ilə Norito qoruyucu keşini davam etdirin. Keşin olduğunu yoxlayın
   imzalanmışdır (`--guard-cache-key` konfiqurasiya olunubsa) və versiya altında saxlanılır
   nəzarəti dəyişdirin.
3. **Telemetriya tablosunu aktivləşdirin.** İstifadəçi trafikinə xidmət etməzdən əvvəl
   mühit `sorafs.fetch.*`, `sorafs_orchestrator_policy_events_total` və
   proksi ölçüləri (yerli QUIC proksi istifadə edərkən). Siqnallar bağlanmalıdır
   `anonymity_brownout_effective` və uyğunluq ehtiyat sayğacları.
4. **Canlı tüstü testləri keçirin.** Hər biri vasitəsilə idarəetmə tərəfindən təsdiqlənmiş manifest əldə edin
   provayder kohortu (PQ, klassik və birbaşa) və yığın qəbzlərini təsdiqləyin,
   CAR həzmləri və şura imzaları CI bazasına uyğun gəlir.
5. **Ünsiyyət aktivləşdirmə.** Yayım izləyicisini ilə yeniləyin
   `scoreboard.json` artefaktı, qoruyucu keş barmaq izi və
   ilk istehsal gətirilməsi üçün manifest idarəetmə yoxlanışını göstərən qeydlər.

### 5.3 Endirmə / Geriyə Qaytarma Proseduru

Hadisələr, PQ defisitləri və ya tənzimləyici tələblər geri çəkilməyə məcbur olduqda, əməl edin
Bu deterministik ardıcıllıq:

1. **Nəqliyyat siyasətini dəyişdirin.** `transport_policy=direct-only` tətbiq edin (və əgər varsa
   dərhal yeni SoraNet sxeminin tikintisini dayandırır.
2. **Qoruyucu vəziyyəti yuyun.** İstinad etdiyi qoruyucu keş faylını silin və ya arxivləşdirin
   `--guard-cache` belə ki, sonrakı qaçışlar bərkidilmiş relelərdən təkrar istifadə etməyə cəhd göstərməsin.
   Bu addımı yalnız sürətli yenidən aktivləşdirmə planlaşdırıldıqda və önbellek qaldıqda keçin
   etibarlıdır.
3. **Yerli proksiləri deaktiv edin.** Əgər yerli QUIC proksi `bridge` rejimində idisə,
   orkestratoru `proxy_mode="metadata-only"` ilə yenidən başladın və ya çıxarın
   `local_proxy` tamamilə bloklanır. Port buraxılışını sənədləşdirin, belə ki, iş stansiyası və
   brauzer inteqrasiyaları birbaşa Torii girişinə qayıdır.
4. **Uyğunluğu ləğv edin.** Yurisdiksiyadan imtina girişi əlavə edin (və ya
   kor-CID girişi) təsirə məruz qalan faydalı yüklər üçün uyğunluq siyasətinə
   avtomatlaşdırma və tablosunda məqsədli birbaşa rejimli əməliyyat əks olunur.
5. **Audit sübutunu əldə edin.** `--scoreboard-out` ilə dəyişiklikdən sonra əldəetməni işə salın
   və CLI JSON xülasəsini (`manifest_governance` daxil olmaqla) yanında saxlayın
   hadisə bileti.

### 5.4 Tənzimlənən Yerləşdirmə Yoxlama Siyahısı

| Yoxlama məntəqəsi | Məqsəd | Tövsiyə olunan sübut |
|------------|---------|----------------------|
| Uyğunluq siyasəti mərhələli | Yurisdiksiyanın kəsilməsinin GAR sənədləri ilə uyğunluğunu təsdiq edir. | İmzalanmış `soranet_opt_outs.json` snapshot + orkestr konfiqurasiya fərqi. |
| Manifest idarəçiliyi qeydə alınıb | Sübut edir Şuranın imzaları hər bir şlüz manifestini müşayiət edir. | `sorafs_cli fetch ... --output /dev/null --summary out.json`, `manifest_governance.council_signatures` arxivləşdirilmişdir. |
| Attestasiya inventar | `compliance.attestations`-də istinad edilən sənədləri izləyir. | PDF-ləri/JSON artefaktlarını sertifikatlaşdırma həzmi və istifadə müddəti ilə birlikdə saxlayın. |
| Aşağı səviyyəli qazma qeyd edildi | Geri çəkilmənin deterministik qalmasını təmin edir. | Yalnız birbaşa siyasətin tətbiq edildiyini və qoruyucu keşinin təmizləndiyini göstərən rüblük quru iş rekordu. |
| Telemetriyanın saxlanması | Tənzimləyicilər üçün məhkəmə məlumatları təqdim edir. | `sorafs.fetch.*` və uyğunluq ehtiyatlarını təsdiqləyən tablosunun ixracı və ya OTEL snapşotu siyasətə uyğun olaraq saxlanılır. |

Operatorlar hər buraxılış pəncərəsindən əvvəl yoxlama siyahısını nəzərdən keçirməli və təqdim etməlidirlər
sorğu əsasında idarəetmə və ya tənzimləyici orqanlara sübut paketi. Tərtibatçılar təkrar istifadə edə bilərlər
Qəzalar və ya uyğunluq ləğv edildikdə postmortem paketlər üçün eyni artefaktlar
sınaq zamanı işə salınır.
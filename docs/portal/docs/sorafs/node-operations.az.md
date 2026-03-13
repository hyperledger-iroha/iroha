---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0d99cea6198ef7ea6c75d7823854237983f17c3341cea2b3e491bb03e54531f2
source_last_modified: "2026-01-22T14:35:36.797300+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
Güzgülər `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Sfenks dəsti çıxarılana qədər hər iki versiyanı sinxronlaşdırın.
:::

## Baxış

Bu runbook operatorları Torii daxilində quraşdırılmış `sorafs-node` yerləşdirməsini təsdiqləməkdən keçir. Hər bir bölmə birbaşa SF-3 çatdırılmalarına uyğunlaşdırılır: pin/gəlmə gedişləri, bərpanı yenidən başladın, kvotadan imtina və PoR seçmə.

## 1. İlkin şərtlər

- `torii.sorafs.storage`-də saxlama işçisini aktivləşdirin:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Torii prosesinin `data_dir`-ə oxumaq/yazmaq imkanına malik olduğundan əmin olun.
- Bəyannamə qeydə alındıqdan sonra qovşağın gözlənilən tutumu `GET /v2/sorafs/capacity/state` vasitəsilə elan etdiyini təsdiqləyin.
- Hamarlaşdırma işə salındıqda, tablolar həm xam, həm də hamarlanmış GiB·saat/PoR sayğaclarını ifşa edərək spot dəyərlərlə yanaşı titrəməsiz tendensiyaları vurğulayır.

### CLI Dry Run (İsteğe bağlı)

HTTP son nöqtələrini ifşa etməzdən əvvəl siz yığılmış CLI ilə yaddaşın arxa ucunu yoxlaya bilərsiniz.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

Əmrlər Norito JSON xülasələrini çap edir və yığın profilindən və ya həzm uyğunsuzluqlarından imtina edərək, onları Torii naqilindən əvvəl CI tüstü yoxlamaları üçün faydalı edir.【crates/sorafs_node/tests/cli.rs#L1】

### PoR Proof Məşqi

Operatorlar indi Torii-ə yükləməzdən əvvəl idarəetmə tərəfindən verilmiş PoR artefaktlarını yerli olaraq təkrarlaya bilər. CLI eyni `sorafs-node` qəbul yolundan yenidən istifadə edir, beləliklə, yerli qaçışlar HTTP API-nin qaytaracağı dəqiq yoxlama xətalarını üzə çıxarır.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Əmr JSON xülasəsi (manifest həzm, provayder identifikatoru, sübut digesti, nümunə sayı, isteğe bağlı qərar nəticəsi) verir. Saxlanılan manifestin çağırış həzminə uyğun olmasını təmin etmək üçün `--manifest-id=<hex>` və audit sübutu üçün xülasəni orijinal artefaktlarla arxivləşdirmək istədiyiniz zaman `--json-out=<path>` təmin edin. `--verdict` daxil olmaqla, HTTP API-yə zəng etməzdən əvvəl bütün problem → sübut → hökm döngəsini oflayn rejimdə məşq etməyə imkan verir.

Torii canlı olduqdan sonra HTTP vasitəsilə eyni artefaktları əldə edə bilərsiniz:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Hər iki son nöqtəyə daxili yaddaş işçisi xidmət edir, buna görə də CLI tüstü testləri və şlüz zondları sinxronizasiyada qalır.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#1】L

## 2. Sancaq → Gediş-gəliş

1. Manifest + faydalı yük paketi yaradın (məsələn, `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` ilə).
2. Manifesti base64 kodlaşdırması ilə təqdim edin:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON sorğusunda `manifest_b64` və `payload_b64` olmalıdır. Uğurlu cavab `manifest_id_hex` və faydalı yük həzmini qaytarır.
3. Saxlanmış məlumatları əldə edin:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-`data_b64` sahəsini deşifrə edin və onun orijinal baytlara uyğun olduğunu yoxlayın.

## 3. Bərpa Drillini yenidən başladın

1. Ən azı bir manifest yuxarıdakı kimi bərkidin.
2. Torii prosesini (və ya bütün node) yenidən başladın.
3. Alma sorğusunu yenidən təqdim edin. Faydalı yük hələ də bərpa edilə bilən olmalıdır və qaytarılmış həzm yenidən başlamadan əvvəlki dəyərə uyğun olmalıdır.
4. `bytes_used` rebootdan sonra davamlı manifestləri əks etdirdiyini təsdiqləmək üçün `GET /v2/sorafs/storage/state` yoxlayın.

## 4. Kvotadan imtina testi

1. `torii.sorafs.storage.max_capacity_bytes`-i müvəqqəti olaraq kiçik bir dəyərə endirin (məsələn, tək manifestin ölçüsü).
2. Bir manifest işarələyin; sorğu uğur qazanmalıdır.
3. Oxşar ölçüdə ikinci manifesti sancmağa cəhd edin. Torii HTTP `400` və tərkibində `storage capacity exceeded` olan xəta mesajı ilə sorğunu rədd etməlidir.
4. Bitirdikdən sonra normal tutum limitini bərpa edin.

## 5. Saxlama / GC Təftişi (yalnız oxumaq üçün)

1. Yaddaş qovluğuna qarşı yerli saxlama skanını həyata keçirin:

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. Yalnız vaxtı keçmiş manifestləri yoxlayın (yalnız quru əməliyyat, silinmə yoxdur):

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. Hostlar və ya hadisələr arasında hesabatları müqayisə edərkən qiymətləndirmə pəncərəsini bərkitmək üçün `--now` və ya `--grace-secs` istifadə edin.

GC CLI qəsdən yalnız oxumaq üçündür. Audit izləri üçün saxlama müddətlərini və vaxtı keçmiş manifest inventarını tutmaq üçün ondan istifadə edin; istehsalda məlumatları əl ilə silməyin.

## 6. PoR Sampling Probu

1. Manifesti sancaqlayın.
2. PoR nümunəsi tələb edin:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Cavabın tələb olunan sayla `samples` olduğunu və hər sübutun saxlanılan manifest kökünə qarşı doğrulandığını yoxlayın.

## 7. Avtomatlaşdırma qarmaqları

- CI / tüstü testləri əlavə edilmiş hədəf yoxlamaları təkrar istifadə edə bilər:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` və `por_sampling_returns_verified_proofs` əhatə edir.
- Panellər izləməlidir:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` və `torii_sorafs_storage_fetch_inflight`
  - PoR müvəffəqiyyət/uğursuzluq sayğacları `/v2/sorafs/capacity/state` vasitəsilə ortaya çıxdı
  - Hesablaşma `sorafs_node_deal_publish_total{result=success|failure}` vasitəsilə dərc cəhdləri

Bu məşğələlərin ardınca daxil edilmiş yaddaş işçisi məlumatları qəbul edə, yenidən işə salındıqda sağ qala, konfiqurasiya edilmiş kvotalara əməl edə və qovşaq daha geniş şəbəkəyə tutumunu reklam etməzdən əvvəl deterministik PoR sübutları yarada bilər.
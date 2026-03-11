---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c9acf8a8d9c298ad2fe95e5480942441aa790346f90c5b5e1a8c1ff638c5e73
source_last_modified: "2026-01-22T16:26:46.522695+00:00"
translation_last_reviewed: 2026-02-07
id: developer-cli
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

Konsolidə edilmiş `sorafs_cli` səthi (`sorafs_car` qutusu ilə təmin edilmişdir.
`cli` funksiyası aktivləşdirilib) SoraFS hazırlamaq üçün tələb olunan hər addımı ifşa edir
artefaktlar. Birbaşa ümumi iş axınlarına keçmək üçün bu yemək kitabından istifadə edin; ilə cütləşdirin
əməliyyat konteksti üçün manifest boru kəməri və orkestrator runbooks.

## Paket yükləri

Deterministik CAR arxivləri və yığın planları yaratmaq üçün `car pack` istifadə edin. The
komanda avtomatik olaraq SF-1 parçasını seçir, əgər tutacaq təmin olunmursa.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Defolt chunker sapı: `sorafs.sf1@1.0.0`.
- Kataloq girişləri leksikoqrafik qaydada aparılır ki, yoxlama məbləğləri sabit olsun
  platformalar arasında.
- JSON xülasəsinə faydalı yük həzmləri, hər bir parça üçün metadata və kök daxildir
  CID reyestr və orkestr tərəfindən tanınır.

## Manifestləri qurun

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` variantları birbaşa `PinPolicy` sahələrinə xəritə verir
  `sorafs_manifest::ManifestBuilder`.
- CLI-nin SHA3 yığınını yenidən hesablamasını istədiyiniz zaman `--chunk-plan` təmin edin
  təqdim etməzdən əvvəl həzm etmək; əks halda daxil edilmiş həzmdən yenidən istifadə edir
  xülasə.
- JSON çıxışı zamanı sadə fərqlər üçün Norito faydalı yükünü əks etdirir.
  rəylər.

## İşarə uzunömürlü açarlar olmadan özünü göstərir

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Daxili nişanları, mühit dəyişənlərini və ya fayl əsaslı mənbələri qəbul edir.
- Mənbə metadatasını əlavə edir (`token_source`, `token_hash_hex`, yığın həzm)
  `--include-token=true` istisna olmaqla, xam JWT-ni davam etdirmədən.
- CI-də yaxşı işləyir: təyin etməklə GitHub Actions OIDC ilə birləşdirin
  `--identity-token-provider=github-actions`.

## Manifestləri Torii-ə göndərin

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Ləqəb sübutları üçün Norito deşifrəsini həyata keçirir və onların uyğunluğunu yoxlayır
  Torii-ə POST etmədən əvvəl manifest həzm.
- Uyğunsuzluq hücumlarının qarşısını almaq üçün plandan SHA3 həzmini yenidən hesablayır.
- Cavab xülasələri HTTP statusunu, başlıqları və reyestr yüklərini ələ keçirir
  sonra audit.

## Avtomobilin məzmununu və sübutlarını yoxlayın

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- PoR ağacını yenidən qurur və faydalı yük həzmlərini manifest xülasəsi ilə müqayisə edir.
- Replikasiya sübutlarını təqdim edərkən tələb olunan sayları və identifikatorları çəkir
  idarəçiliyə.

## Axın sübut telemetriya

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Hər bir axın sübutu üçün NDJSON elementləri yayır (təkrarla
  `--emit-events=false`).
- Müvəffəqiyyət/uğursuzluq saylarını, gecikmə histoqramlarını və nümunəvi uğursuzluqları birləşdirir
  Xülasə JSON-u beləliklə idarə panelləri qeydləri silmədən nəticələri tərtib edə bilsin.
- Şlüz uğursuzluqlar və ya yerli PoR yoxlaması barədə məlumat verdikdə sıfırdan çıxır
  (`--por-root-hex` vasitəsilə) sübutları rədd edir. ilə eşikləri tənzimləyin
  Məşq qaçışları üçün `--max-failures` və `--max-verification-failures`.
- Bu gün PoR dəstəkləyir; PDP və PoTR eyni zərfdən bir dəfə SF-13/SF-14 istifadə edir
  torpaq.
- `--governance-evidence-dir` göstərilən xülasə, metadata (zaman möhürü,
  CLI versiyası, şlüz URL-i, manifest həzmi) və manifestin surəti
  təchiz olunmuş kataloq, beləliklə idarəetmə paketləri sübut axını arxivləşdirə bilər
  qaçışı təkrar etmədən sübut.

## Əlavə istinadlar

- `docs/source/sorafs_cli.md` - hərtərəfli bayraq sənədləri.
- `docs/source/sorafs_proof_streaming.md` - sübut telemetriya sxemi və Grafana
  tablosuna şablon.
- `docs/source/sorafs/manifest_pipeline.md` - parçalanmaya dərin dalış, manifest
  kompozisiya və CAR idarəetməsi.
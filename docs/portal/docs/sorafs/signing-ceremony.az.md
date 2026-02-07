---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a4be274ad087f292559c4b83a120ad20316a1e1dfe0ccbfb9aad42235ac136b
source_last_modified: "2026-01-05T09:28:11.909870+00:00"
translation_last_reviewed: 2026-02-07
id: signing-ceremony
title: Signing Ceremony Replacement
description: How the Sora Parliament approves and distributes SoraFS chunker fixtures (SF-1b).
sidebar_label: Signing Ceremony
translator: machine-google-reviewed
---

> Yol xəritəsi: **SF-1b — Sora Parlamentinin fikstür təsdiqləri.**

SoraFS chunker armaturları üçün istifadə edilən əl ilə imzalama ritualı köhnəlib. Hamısı
təsdiqlər indi **Sora Parlamenti**, çeşidləmə əsaslı DAO vasitəsilə axır
Nexus-i idarə edir. Parlament üzvləri vətəndaşlıq əldə etmək üçün XOR bağlayır, fırlanır
panellər və armaturu təsdiqləyən, rədd edən və ya geri qaytaran zəncirdə səslər verir
relizlər. Bu təlimat prosesi və tərtibatçı alətlərini izah edir.

## Parlamentə baxış

- **Vətəndaşlıq** — Operatorlar vətəndaş kimi qeydiyyatdan keçmək üçün tələb olunan XOR-u bağlayırlar
  çeşidləmə hüququ qazanır.
- **Panellər** — Məsuliyyətlər fırlanan panellər arasında bölünür (İnfrastruktur,
  Moderasiya, Xəzinədarlıq, ...). İnfrastruktur Paneli SoraFS qurğusuna malikdir
  təsdiqlər.
- **Sorsifikasiya və fırlanma** — Panel oturacaqları bənddə göstərilən kadansda yenidən çəkilir
  Parlament konstitusiyasına görə heç bir tək qrup təkəbbürünü təsdiqləməz.

## Quraşdırma təsdiq axını

1. **Təklifin təqdim edilməsi**
   - Tooling WG namizəd `manifest_blake3.json` paketi plus yükləyir
     qurğu `sorafs.fixtureProposal` vasitəsilə zəncirvari reyestrdən fərqlənir.
   - Təklif BLAKE3 həzmini, semantik versiyasını və dəyişiklik qeydlərini qeyd edir.
2. **İnceləmə və səsvermə**
   - İnfrastruktur Paneli tapşırığı Parlamentin tapşırığı vasitəsilə alır
     növbə.
   - Panel üzvləri CI artefaktlarını yoxlayır, paritet testlərini həyata keçirir və çəkisi ölçülür
     zəncirvari səslər.
3. ** Yekunlaşdırma**
   - Kvorum təmin edildikdən sonra, icra müddəti daxil olan təsdiq hadisəsi yayır
     kanonik manifest həzmi və qurğunun faydalı yükü ilə bağlı Merkle öhdəliyi.
   - Tədbir SoraFS reyestrinə əks olunub ki, müştərilər əldə edə bilsinlər.
     Parlament tərəfindən təsdiqlənmiş son manifest.
4. **Paylaşım**
   - CLI köməkçiləri (`cargo xtask sorafs-fetch-fixture`) təsdiq edilmiş manifesti çəkir
     Nexus RPC-dən. Repozitoriyanın JSON/TS/Go sabitləri sinxronizasiyada qalır
     `export_vectors`-i yenidən işə salmaq və zəncirvari zəncirə qarşı həzmi təsdiqləmək
     rekord.

## Tərtibatçı iş axını

- Armaturları aşağıdakılarla bərpa edin:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Təsdiqlənmiş zərfi endirmək, təsdiqləmək üçün Parlament almaq köməkçisindən istifadə edin
  imzalar və yerli qurğuları yeniləyin. `--signatures` nöqtəsində
  Parlament tərəfindən nəşr olunan zərf; köməkçi müşayiət olunan manifestləri həll edir,
  BLAKE3 həzmini yenidən hesablayır və kanonikləri tətbiq edir
  `sorafs.sf1@1.0.0` profili.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Manifest başqa URL-də yaşayırsa, `--manifest` keçin. İmzasız zərflər
yerli tüstü axını üçün `--allow-unsigned` təyin edilmədiyi halda rədd edilir.

- Təhlil şlüzü vasitəsilə manifest təsdiq edərkən, əvəzinə Torii hədəf alın
  yerli yüklər:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Yerli CI artıq `signer.json` siyahısı tələb etmir.
  `ci/check_sorafs_fixtures.sh` repo vəziyyətini sonuncu ilə müqayisə edir
  zəncirvari öhdəlik və bir-birindən ayrıldıqda uğursuz olur.

## İdarəetmə qeydləri

- Parlament konstitusiyası kvorumu, rotasiyanı və eskalasiyanı tənzimləyir - yox
  sandıq səviyyəsində konfiqurasiya tələb olunur.
- Fövqəladə hallarda geri çəkilmələr Parlamentin moderasiya paneli vasitəsilə idarə olunur. The
  İnfrastruktur Paneli əvvəlki manifestə istinad edərək geri qaytarma təklifi verir
  təsdiq edildikdən sonra buraxılışı əvəz edən həzm.
- Tarixi təsdiqlər məhkəmə ekspertizası üçün SoraFS reyestrində mövcuddur
  təkrar.

## Tez-tez verilən suallar

- **`signer.json` hara getdi?**  
  Silindi. Bütün imzalayan atributları zəncirdə yaşayır; `manifest_signatures.json`
  depoda yalnız ən son uyğunlaşmalı olan bir tərtibatçı qurğusudur
  təsdiq hadisəsi.

- **Biz hələ də yerli Ed25519 imzaları tələb edirik?**  
  Xeyr. Parlament təsdiqləri zəncir üzərindəki artefaktlar kimi saxlanılır. Yerli qurğular mövcuddur
  reproduktivlik üçün lakin Parlament həzminə qarşı təsdiq edilmişdir.

- **Komandalar təsdiqlərə necə nəzarət edir?**  
  `ParliamentFixtureApproved` hadisəsinə abunə olun və ya reyestrdən sorğu keçirin
  Nexus RPC cari manifest həzmini və panel çağırışını əldə etmək üçün.
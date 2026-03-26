---
id: deploy-guide
lang: az
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Baxış

Bu dərslik **DOCS-7** (SoraFS nəşri) və **DOCS-8** yol xəritəsi elementlərini çevirir
(CI/CD pin avtomatlaşdırılması) inkişaf etdirici portalı üçün təsirli bir prosedura çevirin.
O, qurma/lint mərhələsini, SoraFS qablaşdırmanı, Sigstore dəstəkli manifestini əhatə edir.
imzalama, ləqəb təşviqi, doğrulama və geri qaytarma təlimləri beləliklə hər bir önizləmə və
buraxılış artefaktı təkrarlana bilər və yoxlanıla bilər.

Axın güman edir ki, sizdə `sorafs_cli` binar (quraşdırılmışdır)
`--features cli`), pin-reyestr icazələri ilə Torii son nöqtəsinə giriş və
OIDC Sigstore üçün etimadnamələr. Uzunömürlü sirləri saxla (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii nişanları) CI anbarınızda; yerli qaçışlar onları mənbə edə bilər
qabıq ixracından.

## İlkin şərtlər

- `npm` və ya `pnpm` ilə qovşaq 18.18+.
- `cargo run -p sorafs_car --features cli --bin sorafs_cli`-dən `sorafs_cli`.
- Torii URL-i `/v1/sorafs/*` üstəgəl səlahiyyət hesabı/şəxsi açarı ifşa edir
  manifestlər və ləqəblər təqdim edə bilər.
- OIDC emitenti (GitHub Actions, GitLab, iş yükünün identifikasiyası və s.)
  `SIGSTORE_ID_TOKEN`.
- İsteğe bağlı: quru qaçışlar üçün `examples/sorafs_cli_quickstart.sh` və
  GitHub/GitLab iş axını iskele üçün `docs/source/sorafs_ci_templates.md`.
- Try it OAuth dəyişənlərini (`DOCS_OAUTH_*`) konfiqurasiya edin və
  Quraşdırmanı təşviq etməzdən əvvəl [təhlükəsizliyi gücləndirən yoxlama siyahısı](./security-hardening.md)
  laboratoriyadan kənarda. Bu dəyişənlər əskik olduqda portal quruluşu indi uğursuz olur
  və ya TTL/səsvermə düymələri məcburi pəncərələrdən kənara düşdükdə; ixrac
  `DOCS_OAUTH_ALLOW_INSECURE=1` yalnız birdəfəlik yerli önizləmələr üçün. əlavə edin
  buraxılış biletinə qələm-test sübutu.

## Addım 0 — Sınaq proksi paketini çəkin

Netlify və ya şlüz üçün önizləməni təşviq etməzdən əvvəl, Sınaq proksisini möhürləyin
mənbələri və imzalı OpenAPI manifest həzmini deterministik paketə çevirir:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` proksi/zond/geri qaytarma köməkçilərini kopyalayır,
OpenAPI imzasını yoxlayır və `release.json` plus yazır
`checksums.sha256`. Bu paketi Netlify/SoraFS şlüz tanıtımına əlavə edin
Rəyçilər dəqiq proksi mənbələri və Torii hədəf göstərişlərini təkrar oxuya bilsinlər
yenidən qurmadan. Paket həmçinin müştəri tərəfindən təmin edilmiş daşıyıcıların olub-olmadığını qeyd edir
işə salınma planını və CSP qaydalarını sinxronlaşdırmaq üçün aktivləşdirildi (`allow_client_auth`).

## Addım 1 — Portalı qurun və tündləyin

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` avtomatik olaraq `scripts/write-checksums.mjs`-i yerinə yetirir, istehsal edir:

- `build/checksums.sha256` — SHA256 manifest `sha256sum -c` üçün uyğundur.
- `build/release.json` — metadata (`tag`, `generated_at`, `source`)
  hər CAR/manifest.

Hər iki faylı CAR xülasəsi ilə birlikdə arxivləşdirin ki, rəyçilər önizləməni fərqləndirə bilsinlər
bərpa edilmədən artefaktlar.

## Addım 2 — Statik aktivləri paketləyin

CAR paketləyicisini Docusaurus çıxış qovluğuna qarşı işə salın. Aşağıdakı nümunə
`artifacts/devportal/` altında bütün artefaktları yazır.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

Xülasə JSON yığın saylarını, həzmləri və sübut planlaşdırma göstərişlərini çəkir
`manifest build` və CI panelləri daha sonra yenidən istifadə olunur.

## Addım 2b — Paket OpenAPI və SBOM yoldaşları

DOCS-7 portal saytının, OpenAPI snapşotunun və SBOM yüklərinin dərc edilməsini tələb edir
fərqli şəkildə şlüzlər `Sora-Proof`/`Sora-Content-CID`-i zımbalaya bilər
hər artefakt üçün başlıqlar. Buraxılış köməkçisi
(`scripts/sorafs-pin-release.sh`) artıq OpenAPI kataloqunu paketləyir
(`static/openapi/`) və `syft` vasitəsilə yayılan SBOM-lar ayrı-ayrı
`openapi.*`/`*-sbom.*` CAR-ları və metadatanı qeyd edir
`artifacts/sorafs/portal.additional_assets.json`. Əl axını işləyərkən,
öz prefiksləri və metadata etiketləri ilə hər bir faydalı yük üçün 2-4-cü addımları təkrarlayın
(məsələn, `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`). Hər manifest/ləqəb qeydiyyatdan keçin
DNS-i dəyişməzdən əvvəl Torii (sayt, OpenAPI, portal SBOM, OpenAPI SBOM) ilə cütləşdirin.
şlüz bütün nəşr olunmuş artefaktlar üçün zımbalanmış sübutlara xidmət edə bilər.

## Addım 3 — Manifest yaradın

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

Buraxılış pəncərənizdə pin siyasəti bayraqlarını tənzimləyin (məsələn, `--pin-storage-class
kanareykalar üçün isti). JSON variantı isteğe bağlıdır, lakin kodu nəzərdən keçirmək üçün əlverişlidir.

## Addım 4 — Sigstore ilə imzalayın

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Paket açıq-aşkar həzmləri, yığın həzmləri və BLAKE3 hashini qeyd edir.
JWT-ni davam etdirmədən OIDC nişanı. Həm paketi, həm də ayrı saxlayın
imza; istehsal təşviqləri istefa vermək əvəzinə eyni artefaktlardan yenidən istifadə edə bilər.
Yerli qaçışlar provayder bayraqlarını `--identity-token-env` (və ya təyin) ilə əvəz edə bilər
`SIGSTORE_ID_TOKEN` mühitdə) xarici OIDC köməkçisi
nişan.

## Addım 5 — PİN reyestrinə təqdim edin

İmzalanmış manifesti (və yığın planını) Torii nömrəsinə təqdim edin. Həmişə xülasə tələb edin
nəticədə reyestr girişi/ləqəb sübutu yoxlanıla bilər.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Önizləmə və ya kanarya ləqəbini (`docs-preview.sora`) yayarkən, təkrarlayın
QA istehsaldan əvvəl məzmunu yoxlaya bilməsi üçün unikal ləqəblə təqdim edin
təşviq.

Alias bağlamaq üçün üç sahə tələb olunur: `--alias-namespace`, `--alias-name` və
`--alias-proof`. İdarəetmə sübut paketini istehsal edir (base64 və ya Norito bayt)
ləqəb sorğusu təsdiq edildikdə; onu CI sirrlərində saxlayın və onu a kimi üzə çıxarın
`manifest submit`-i çağırmadan əvvəl fayl. Siz zaman ləqəb bayraqları ayarlanmamış buraxın
yalnız DNS-ə toxunmadan manifestə yapışdırmaq niyyətindədir.

## Addım 5b — İdarəetmə təklifi yaradın

Hər hansı bir Sora Məclisə hazır bir təkliflə səfər etməlidir ki, hər manifest
vətəndaş imtiyazlı etimadnaməsini borc almadan dəyişikliyi tətbiq edə bilər.
Göndərmə/imzalama addımlarından sonra işə salın:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` kanonik `RegisterPinManifest`-i çəkir
təlimat, yığın həzm, siyasət və ləqəb işarəsi. Onu idarəetməyə əlavə edin
bilet və ya Parlament portalı, beləliklə, nümayəndələr yenidən qurmadan yükü fərqləndirə bilsinlər
artefaktlar. Çünki əmr heç vaxt Torii səlahiyyət açarına toxunmur, hər hansı
vətəndaş yerli olaraq təklifini tərtib edə bilər.

## Addım 6 — Sübutları və telemetriyanı yoxlayın

Saxladıqdan sonra deterministik yoxlama addımlarını yerinə yetirin:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- `torii_sorafs_gateway_refusals_total` və yoxlayın
  Anomaliyalar üçün `torii_sorafs_replication_sla_total{outcome="missed"}`.
- Try-It proksisini və qeydə alınmış bağlantıları istifadə etmək üçün `npm run probe:portal`-i işə salın
  yeni bərkidilmiş məzmuna qarşı.
--də təsvir olunan monitorinq sübutlarını əldə edin
  [Nəşriyyat və Monitorinq](./publishing-monitoring.md) belə ki, DOCS-3c
  Nəşretmə addımları ilə yanaşı müşahidə olunma qapısı da təmin edilir. Köməkçi
  indi çoxsaylı `bindings` girişlərini qəbul edir (sayt, OpenAPI, portal SBOM, OpenAPI
  SBOM) və `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`-i hədəfə tətbiq edir
  isteğe bağlı `hostname` qoruyucu vasitəsilə host. Aşağıdakı çağırış həm a yazır
  tək JSON xülasəsi və sübut paketi (`portal.json`, `tryit.json`,
  `binding.json` və `checksums.sha256`) buraxılış kataloqu altında:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Addım 6a — Şlüz sertifikatlarını planlaşdırın

GAR paketləri yaratmadan əvvəl TLS SAN/çağırış planını əldə edin
komanda və DNS təsdiqləyiciləri eyni sübutları nəzərdən keçirirlər. Yeni köməkçi əks etdirir
Kanonik joker hostları sadalamaqla DG-3 avtomatlaşdırma girişləri,
olduqca host SAN'ları, DNS-01 etiketləri və tövsiyə olunan ACME problemləri:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Buraxılış paketi ilə yanaşı JSON-u təhvil verin (və ya dəyişikliklə birlikdə yükləyin
bilet) operatorlar SAN dəyərlərini Torii-lərə yapışdıra bilsinlər
`torii.sorafs_gateway.acme` konfiqurasiyası və GAR rəyçiləri təsdiq edə bilər
host törəmələrini yenidən işə salmadan kanonik/gözəl xəritələr. Əlavə əlavə edin
Eyni buraxılışda irəli sürülən hər bir şəkilçi üçün `--name` arqumentləri.

## Addım 6b — Kanonik host xəritələrini əldə edin

GAR yüklərini şablon etməzdən əvvəl, hər biri üçün deterministik host xəritəsini qeyd edin
ləqəb. `cargo xtask soradns-hosts` hər bir `--name`-ni öz kanonikinə heş edir
etiket (`<base32>.gw.sora.id`), tələb olunan joker işarəni verir
(`*.gw.sora.id`) və gözəl host əldə edir (`<alias>.gw.sora.name`). Davam et
buraxılış artefaktlarındakı çıxış DG-3 rəyçiləri xəritələşdirməni fərqləndirə bilsinlər
GAR təqdimatı ilə yanaşı:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

GAR və ya şlüz zamanı sürətli uğursuzluq üçün `--verify-host-patterns <file>` istifadə edin
məcburi JSON tələb olunan hostlardan birini buraxır. Köməkçi çoxluğu qəbul edir
doğrulama faylları, həm GAR şablonunu, həm də
eyni çağırışda zımbalanmış `portal.gateway.binding.json`:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Xülasə JSON və yoxlama jurnalını DNS/şlüz dəyişikliyi biletinə əlavə edin
auditorlar kanonik, wildcard və yaraşıqlı hostları təkrar işə salmadan təsdiq edə bilərlər
törəmə skriptləri. Yeni ləqəblər əlavə edildikdə əmri yenidən işlədin
belə ki, sonrakı GAR yeniləmələri eyni sübut izini miras alır.

## Addım 7 — DNS kəsmə deskriptorunu yaradın

İstehsalın kəsilməsi üçün yoxlanıla bilən dəyişiklik paketi tələb olunur. Uğur qazandıqdan sonra
təqdim (ləqəb bağlama), köməkçi yayır
`artifacts/sorafs/portal.dns-cutover.json`, çəkmək:- ləqəb bağlayan metadata (ad sahəsi/ad/sübut, manifest digest, Torii URL,
  təqdim olunan dövr, səlahiyyət);
- buraxılış konteksti (teq, ləqəb etiketi, manifest/CAR yolları, yığın planı, Sigstore
  paket);
- yoxlama göstəriciləri (zond əmri, ləqəb + Torii son nöqtəsi); və
- isteğe bağlı dəyişiklik-nəzarət sahələri (bilet id, kəsmə pəncərəsi, əməliyyat əlaqəsi,
  istehsal host adı/zonası);
- zımbalanmış `Sora-Route-Binding`-dən əldə edilmiş marşrut təşviqi metadatası
  başlıq (kanonik host/CID, başlıq + bağlama yolları, yoxlama əmrləri),
  GAR təşviqi və ehtiyat təlimlərinin eyni sübuta istinad etməsini təmin etmək;
- yaradılan marşrut planı artefaktları (`gateway.route_plan.json`,
  başlıq şablonları və isteğe geri qaytarma başlıqları) buna görə də biletləri və CI-ni dəyişdirin
  lint qarmaqlar hər DG-3 paketinin kanonik paketə istinad etdiyini yoxlaya bilər
  təsdiqdən əvvəl təşviq/geri qaytarma planları;
- isteğe bağlı keşin etibarsızlığının metadatası (təmizləmə son nöqtəsi, auth dəyişəni, JSON
  faydalı yük və misal `curl` əmri); və
- əvvəlki deskriptora işarə edən geri qaytarma göstərişləri (buraxılış etiketi və manifest
  həzm edin) beləliklə, dəyişdirmə biletləri deterministik geri dönüş yolunu tutur.

Buraxılış önbelleğin təmizlənməsini tələb etdikdə, ilə yanaşı kanonik plan yaradın
kəsmə təsviri:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Nəticədə `portal.cache_plan.json`-i DG-3 paketinə əlavə edin ki, operatorlar
verən zaman deterministik hostlara/yollara (və uyğun auth göstərişlərinə) sahib olun
`PURGE` sorğuları. Deskriptorun isteğe bağlı keş metadata bölməsi istinad edə bilər
bu faylı birbaşa olaraq dəyişdirməyə nəzarət edən rəyçiləri məhz hansına uyğunlaşdırır
kəsmə zamanı son nöqtələr yuyulur.

Hər bir DG-3 paketi də təşviq + geri qaytarma siyahısına ehtiyac duyur. vasitəsilə yaradın
`cargo xtask soradns-route-plan` beləliklə, dəyişikliklərə nəzarət edən rəyçilər dəqiqliyi izləyə bilərlər
ləqəb üçün əvvəlcədən uçuş, kəsmə və geri qaytarma addımları:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Emissiya edilmiş `gateway.route_plan.json` səhnələşdirilmiş kanonik/yaraşıqlı hostları çəkir
sağlamlıq yoxlanışı xatırlatmaları, GAR məcburi yeniləmələri, keş təmizləmələri və geri qaytarma hərəkətləri.
Dəyişikliyi təqdim etməzdən əvvəl onu GAR/bağlayıcı/kəsmə artefaktları ilə birləşdirin
Bilet, belə ki, Əməliyyatçılar eyni skriptlə yazılmış addımlarda məşq edib imza atsınlar.

`scripts/generate-dns-cutover-plan.mjs` bu deskriptoru gücləndirir və işləyir
avtomatik olaraq `sorafs-pin-release.sh`-dən. Onu bərpa etmək və ya fərdiləşdirmək üçün
əl ilə:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

Pini işə salmazdan əvvəl ətraf mühit dəyişənləri vasitəsilə isteğe bağlı metadatanı doldurun
köməkçi:

| Dəyişən | Məqsəd |
|----------|---------|
| `DNS_CHANGE_TICKET` | Bilet ID-si deskriptorda saxlanılır. |
| `DNS_CUTOVER_WINDOW` | ISO8601 kəsmə pəncərəsi (məsələn, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | İstehsal host adı + nüfuzlu zona. |
| `DNS_OPS_CONTACT` | Zəng ləqəbi və ya eskalasiya əlaqəsi. |
| `DNS_CACHE_PURGE_ENDPOINT` | Deskriptorda qeyd olunan önbelleğin təmizlənməsi son nöqtəsi. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Təmizləmə nişanını ehtiva edən env var (defolt olaraq `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Geri qaytarma metadata üçün əvvəlki kəsmə deskriptoruna yol. |

JSON-u DNS dəyişikliyinin nəzərdən keçirilməsinə əlavə edin ki, təsdiqləyənlər manifesti doğrulaya bilsinlər
CI qeydlərini silmədən həzmlər, ləqəb bağlamaları və araşdırma əmrləri.
CLI bayraqları `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env` və `--previous-dns-plan` eyni ləğvetmələri təmin edir
köməkçini CI-dən kənarda işlədərkən.

## Addım 8 — Həlledici zona faylı skeletini buraxın (isteğe bağlı)

İstehsalın kəsilməsi pəncərəsi məlum olduqda, buraxılış skripti onu yaya bilər
Avtomatik olaraq SNS zona faylı skeleti və həlledici parça. İstədiyiniz DNS-i keçin
mühit dəyişənləri və ya CLI seçimləri vasitəsilə qeydlər və metadata; köməkçi
kəsildikdən dərhal sonra `scripts/sns_zonefile_skeleton.py`-ə zəng edəcək
deskriptor yaradılır. Ən azı bir A/AAAA/CNAME dəyəri və GAR təmin edin
həzm (imzalanmış GAR yükünün BLAKE3-256). Zona/host adı məlumdursa
və `--dns-zonefile-out` buraxıldı, köməkçi ona yazır
`artifacts/sns/zonefiles/<zone>/<hostname>.json` və yaşayır
`ops/soradns/static_zones.<hostname>.json` həlledici parça kimi.

| Dəyişən / bayraq | Məqsəd |
|----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Yaradılmış zona faylı skeleti üçün yol. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Həlledici fraqment yolu (buraxıldıqda defolt olaraq `ops/soradns/static_zones.<hostname>.json` olur). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | Yaradılmış qeydlərə tətbiq edilən TTL (defolt: 600 saniyə). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 ünvanları (vergüllə ayrılmış env və ya təkrarlana bilən CLI bayrağı). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 ünvanları. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Könüllü CNAME hədəfi. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI sancaqları (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Əlavə TXT qeydləri (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Hesablanmış zona faylı versiyası etiketini ləğv edin. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Kəsmə pəncərəsinin başlaması əvəzinə `effective_at` vaxt damgasını (RFC3339) məcbur edin. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Metadatada qeydə alınmış sübut hərfini ləğv edin. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Metadatada qeydə alınmış CID-i ləğv edin. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Guardian dondurma vəziyyəti (yumşaq, sərt, ərimə, monitorinq, fövqəladə). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Dondurulmalar üçün qəyyum/məclis bileti arayışı. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 ərimə üçün vaxt möhürü. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Əlavə dondurma qeydləri (vergüllə ayrılmış env və ya təkrarlanan bayraq). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | İmzalanmış GAR yükünün BLAKE3-256 həzmi (hex). Şlüz bağlamaları mövcud olduqda tələb olunur. |

GitHub Actions iş prosesi bu dəyərləri repozitor sirrlərindən oxuyur ki, hər istehsal pin avtomatik olaraq zona faylı artefaktlarını yaysın. Aşağıdakı sirləri konfiqurasiya edin (sətirlərdə çox dəyərli sahələr üçün vergüllə ayrılmış siyahılar ola bilər):

| Gizli | Məqsəd |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | İstehsal host adı/zonası köməkçiyə keçdi. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Deskriptorda saxlanılan zəng ləqəbi. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Nəşr etmək üçün IPv4/IPv6 qeydləri. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Könüllü CNAME hədəfi. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI sancaqları. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Əlavə TXT qeydləri. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Skeletdə qeydə alınmış metadatanı dondurun. |
| `DOCS_SORAFS_GAR_DIGEST` | İmzalanmış GAR yükünün hex kodlu BLAKE3 həzmi. |

`.github/workflows/docs-portal-sorafs-pin.yml`-i işə salarkən, `dns_change_ticket` və `dns_cutover_window` daxiletmələrini təmin edin ki, deskriptor/zonefil düzgün dəyişiklik pəncərəsi metadatasını miras alsın. Yalnız quru qaçış zamanı onları boş buraxın.

Tipik çağırış (SN-7 sahibi runbook ilə uyğun gəlir):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  …other flags…
```

Köməkçi avtomatik olaraq dəyişiklik biletini TXT girişi kimi daşıyır və
kəsilmə pəncərəsinin başlanğıcını `effective_at` vaxt damğası kimi miras alır
ləğv edildi. Tam əməliyyat iş axını üçün baxın
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### İctimai DNS nümayəndə heyəti qeydi

Zonefile skeleti yalnız zona üçün səlahiyyətli qeydləri müəyyən edir. sən
hələ də qeydiyyatçınızda və ya DNS-də ana zona NS/DS nümayəndə heyətini konfiqurasiya etməlisiniz
provayderdir ki, adi internet ad serverlərini kəşf edə bilsin.

- Apeks/TLD kəsimləri üçün ALIAS/ANAME (provayderə məxsus) istifadə edin və ya A/AAAA dərc edin
  şluz hər hansı bir yayım IP-yə işarə edən qeydlər.
- Subdomenlər üçün, əldə edilmiş gözəl hosta CNAME dərc edin
  (`<fqdn>.gw.sora.name`).
- Kanonik host (`<hash>.gw.sora.id`) şluz domeni altında qalır və
  ictimai zonanızda dərc olunmur.

### Şluz başlığı şablonu

Yerləşdirmə köməkçisi həmçinin `portal.gateway.headers.txt` və yayır
`portal.gateway.binding.json`, DG-3 tələblərini qane edən iki artefakt
gateway-content-məcburi tələb:

- `portal.gateway.headers.txt` tam HTTP başlıq blokunu ehtiva edir (o cümlədən
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS və
  `Sora-Route-Binding` deskriptoru) kənar şlüzlərin hər bir şlüz üzərində bərkidilməsi lazımdır.
  cavab.
- `portal.gateway.binding.json` eyni məlumatı maşın tərəfindən oxuna bilən şəkildə qeyd edir
  forma dəyişdirin ki, biletləri dəyişdirin və avtomatlaşdırma host/cid bağlamaları olmadan fərqlənə bilər
  kazıma qabığı çıxışı.

Onlar vasitəsilə avtomatik olaraq yaradılır
`cargo xtask soradns-binding-template`
və tədarük edilən ləqəb, manifest həzm və şlüz host adını ələ keçirin
`sorafs-pin-release.sh`-ə. Başlıq blokunu yenidən yaratmaq və ya fərdiləşdirmək üçün işlədin:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Ləğv etmək üçün `--csp-template`, `--permissions-template` və ya `--hsts-template` keçin
xüsusi yerləşdirməyə əlavə ehtiyac olduqda standart başlıq şablonları
direktivlər; başlığı buraxmaq üçün onları mövcud `--no-*` açarları ilə birləşdirin
tamamilə.

Başlıq parçasını CDN dəyişmə sorğusuna əlavə edin və JSON sənədini qidalandırın
şlüz avtomatlaşdırma boru kəmərinə daxil olun ki, faktiki host təşviqi ilə uyğun olsun
sübut buraxın.

Buraxılış skripti DG-3 biletlərini avtomatik olaraq yoxlama köməkçisini işə salır
həmişə son sübutları daxil edin. Hər dəfə düzəliş etdiyiniz zaman onu əl ilə yenidən işə salın
JSON-u əl ilə bağlamaq:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Komanda bağlama paketində tutulan `Sora-Proof` faydalı yükünü təsdiqləyir,
`Sora-Route-Binding` metadatasının manifest CID + host adına uyğun olmasını təmin edir,
və hər hansı bir başlıq sürüşürsə, tez uğursuz olur. yanındakı konsol çıxışını arxivləşdirin
CI xaricində əmri işlətdiyiniz zaman digər yerləşdirmə artefaktları belə DG-3
Rəyçilər bağlamanın kəsilməzdən əvvəl təsdiq edildiyinə dair sübuta malikdirlər.> **DNS deskriptor inteqrasiyası:** `portal.dns-cutover.json` indi
> Bu artefaktlara işarə edən `gateway_binding` bölməsi (yollar, məzmun CID,
> sübut statusu və hərfi başlıq şablonu) **və** `route_plan` misrası
> `gateway.route_plan.json` və əsas + geri qaytarma başlığına istinad edir
> şablonlar. Bu blokları hər bir DG-3 dəyişiklik biletinə daxil edin ki, rəyçilər bilsin
> dəqiq `Sora-Name/Sora-Proof/CSP` dəyərlərini fərqləndirin və marşrutu təsdiqləyin
> təşviq/geri qaytarma planları quruluşu açmadan sübut paketinə uyğun gəlir
> arxiv.

## Addım 9 — Nəşriyyat monitorlarını işə salın

Yol xəritəsi tapşırığı **DOCS-3c** portaldan davamlı sübut tələb edir, Sınayın
proksi və şlüz bağlamaları buraxıldıqdan sonra sağlam qalır. Konsolidasiyanı işə salın
Addım 7-8-dən dərhal sonra nəzarət edin və onu planlaşdırılmış zondlarınıza qoşun:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` konfiqurasiya faylını yükləyir (bax
  Sxe üçün `docs/portal/docs/devportal/publishing-monitoring.md`) və
  üç yoxlama həyata keçirir: portal yolu araşdırmaları + CSP/İcazələr-Siyasət doğrulaması,
  Proksi zondlarını sınayın (istəyə görə onun `/metrics` son nöqtəsinə toxunur) və
  yoxlayan şlüz bağlama yoxlayıcısı (`cargo xtask soradns-verify-binding`).
  gözlənilən ləqəb, host, sübut statusuna qarşı tutulan bağlama paketi,
  və manifest JSON.
- Hər hansı bir prob uğursuz olduqda əmr sıfırdan çıxır, beləliklə CI, cron işləri və ya
  runbook operatorları ləqəbləri təşviq etməzdən əvvəl buraxılışı dayandıra bilər.
- `--json-out`-dən keçmək hər hədəflə tək xülasə JSON yükünü yazır
  status; `--evidence-dir` `summary.json`, `portal.json`, `tryit.json`,
  `binding.json` və `checksums.sha256`, beləliklə idarəetmə rəyçiləri fərqlənə bilər.
  monitorları yenidən işə salmadan nəticələr verir. Bu kataloqu altında arxivləşdirin
  `artifacts/sorafs/<tag>/monitoring/`, Sigstore paketi və DNS ilə birlikdə
  kəsmə təsviri.
- Monitor çıxışını daxil edin, Grafana ixracı (`dashboards/grafana/docs_portal.json`),
  və DOCS-3c SLO ola bilsin ki, buraxılış biletində Alertmanager drill ID
  sonra yoxlanılır. Xüsusi nəşriyyat monitoru kitabı burada yaşayır
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Portal araşdırmaları HTTPS tələb edir və `http://` əsas URL-lərini rədd edir.
`allowInsecureHttp` monitor konfiqurasiyasında quraşdırılıb; istehsalı / səhnələşdirməni davam etdirin
TLS-də hədəflər və yalnız yerli önizləmələr üçün ləğvi aktivləşdirin.

Bir dəfə Buildkite/cron-da `npm run monitor:publishing` vasitəsilə monitoru avtomatlaşdırın.
portal canlıdır. İstehsal URL-lərinə işarə edən eyni əmr, davam edənləri qidalandırır
SRE/Sənədlərin buraxılışlar arasında etibar etdiyi sağlamlıq yoxlamaları.

## `sorafs-pin-release.sh` ilə avtomatlaşdırma

`docs/portal/scripts/sorafs-pin-release.sh` 2-6 Addımları əhatə edir. O:

1. `build/` arxivini deterministik tarballa çevirin,
2. çalışır `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   və `proof verify`,
3. Torii olduqda isteğe bağlı olaraq `manifest submit` (o cümlədən ləqəb bağlama) yerinə yetirir
   etimadnamələr mövcuddur və
4. `artifacts/sorafs/portal.pin.report.json` yazır, isteğe bağlıdır
  `portal.pin.proposal.json`, DNS kəsmə deskriptoru (təqdimatdan sonra),
  və şlüz bağlama paketi (`portal.gateway.binding.json` plus
  mətn başlığı bloku) beləliklə idarəetmə, şəbəkə və əməliyyat qrupları fərqlənə bilər
  CI qeydlərini qırmadan sübut paketi.

Set `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` və (istəyə görə)
Skripti çağırmadan əvvəl `PIN_ALIAS_PROOF_PATH`. Quru üçün `--skip-submit` istifadə edin
qaçış; Aşağıda təsvir edilən GitHub iş axını bunu `perform_submit` vasitəsilə dəyişdirir
giriş.

## Addım 8 — OpenAPI xüsusiyyətləri və SBOM paketlərini dərc edin

DOCS-7 səyahət etmək üçün portal quruluşunu, OpenAPI spesifikasiyasını və SBOM artefaktlarını tələb edir
eyni deterministik boru kəməri vasitəsilə. Mövcud köməkçilər hər üçü əhatə edir:

1. **Spesifikasiyanı bərpa edin və imzalayın.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Saxlamaq istədiyiniz zaman buraxılış etiketini `--version=<label>` vasitəsilə ötürün a
   tarixi snapshot (məsələn, `2025-q3`). Köməkçi snapshot yazır
   `static/openapi/versions/<label>/torii.json`-ə, onu əks etdirir
   `versions/current` və metadata qeyd edir (SHA-256, manifest statusu və
   yenilənmiş vaxt damğası) `static/openapi/versions.json`. Tərtibatçı portalı
   həmin indeksi oxuyur ki, Swagger/RapiDoc panelləri versiya seçicisini təqdim edə bilsin
   və əlaqəli həzm/imza məlumatını sətirdə göstərin. İtirmək
   `--version` əvvəlki buraxılış etiketlərini toxunulmaz saxlayır və yalnız
   `current` + `latest` göstəriciləri.

   Manifest SHA-256/BLAKE3 həzmlərini ələ keçirir, beləliklə, şlüz şlüzlə bağlana bilsin
   `Sora-Proof` `/reference/torii-swagger` üçün başlıqlar.

2. **CycloneDX SBOM-ları buraxın.** Buraxılış boru kəməri artıq syft-əsaslı gözləyir
   `docs/source/sorafs_release_pipeline_plan.md` üçün SBOM-lar. Çıxışı saxlayın
   tikinti artefaktlarının yanında:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Hər bir faydalı yükü avtomobilə yığın.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   Əsas sayt kimi eyni `manifest build` / `manifest sign` addımlarını izləyin,
   aktiv üçün ləqəbləri tənzimləmək (məsələn, spesifikasiya üçün `docs-openapi.sora` və
   İmzalanmış SBOM paketi üçün `docs-sbom.sora`). Fərqli ləqəblərin saxlanması
   SoraDNS sübutlarını, GAR-ları və geri qaytarma biletlərini dəqiq yükə uyğun olaraq saxlayır.

4. **Təqdim edin və bağlayın.** Mövcud səlahiyyət + Sigstore paketindən yenidən istifadə edin, lakin
   auditorların hansını izləyə bilməsi üçün buraxılış yoxlama siyahısında təxəllüsü qeyd edin
   Sora adı xəritələr hansı manifest həzm edir.

Portal quruluşu ilə yanaşı spesifikasiya/SBOM təzahürlərinin arxivləşdirilməsi hər şeyi təmin edir
buraxılış bileti paketləyicini yenidən işə salmadan tam artefakt dəstini ehtiva edir.

### Avtomatlaşdırma köməkçisi (CI/paket skripti)

`./ci/package_docs_portal_sorafs.sh` 1-8 addımlarını kodlaşdırır, beləliklə yol xəritəsi elementi
**DOCS‑7** bir əmrlə həyata keçirilə bilər. Köməkçi:

- tələb olunan portal hazırlığını həyata keçirir (`npm ci`, OpenAPI/norito sinxronizasiyası, vidjet testləri);
- `sorafs_cli` vasitəsilə portal, OpenAPI və SBOM CARs + manifest cütlərini yayır;
- isteğe bağlı olaraq `sorafs_cli proof verify` (`--proof`) və Sigstore imzalaması ilə işləyir
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- hər artefaktı `artifacts/devportal/sorafs/<timestamp>/` altına düşür və
  `package_summary.json` yazır ki, CI/buraxılış alətləri paketi qəbul edə bilsin; və
- ən son qaçışa işarə etmək üçün `artifacts/devportal/sorafs/latest`-i yeniləyir.

Nümunə (Sigstore + PoR ilə tam boru kəməri):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Bilməyə dəyər bayraqlar:

- `--out <dir>` – artefakt kökünü ləğv edin (defolt vaxt damğası qoyulmuş qovluqları saxlayır).
- `--skip-build` – mövcud `docs/portal/build`-dən yenidən istifadə edin (CI mümkün olmayanda əlverişlidir
  oflayn güzgülər səbəbindən yenidən qurulur).
- `--skip-sync-openapi` – `cargo xtask openapi` olduqda `npm run sync-openapi`-i keçin
  crates.io-ya daxil ola bilməz.
- `--skip-sbom` – binar quraşdırılmadıqda `syft`-ə zəng etməyin (
  skript əvəzinə xəbərdarlıq çap edir).
- `--proof` – hər CAR/manifest cütü üçün `sorafs_cli proof verify`-i işə salın. çox-
  fayl yükləri hələ də CLI-də yığın plan dəstəyi tələb edir, ona görə də bu bayrağı tərk edin
  `plan chunk count` səhvlərinə toxunsanız və bir dəfə əl ilə yoxlasanız, parametrləri ləğv edin.
  yuxarı axar qapı torpaqları.
- `--sign` - `sorafs_cli manifest sign` çağırın. Bir nişan təqdim edin
  `SIGSTORE_ID_TOKEN` (və ya `--sigstore-token-env`) və ya istifadə edərək CLI-nin onu götürməsinə icazə verin
  `--sigstore-provider/--sigstore-audience`.

İstehsal artefaktlarını göndərərkən `docs/portal/scripts/sorafs-pin-release.sh` istifadə edin.
İndi o, portalı, OpenAPI və SBOM yüklərini paketləyir, hər bir manifestə imza atır və
`portal.additional_assets.json`-də əlavə aktiv metadatasını qeyd edir. Köməkçi
CI paketləyicisi və yenisi tərəfindən istifadə edilən eyni isteğe bağlı düymələri başa düşür
`--openapi-*`, `--portal-sbom-*` və `--openapi-sbom-*` açarları ilə
artefakt başına ləqəb dəstləri təyin edin, vasitəsilə SBOM mənbəyini ləğv edin
`--openapi-sbom-source`, müəyyən faydalı yükləri atlayın (`--skip-openapi`/`--skip-sbom`),
və `--syft-bin` ilə qeyri-defolt `syft` binarına işarə edin.

Skript işlətdiyi hər əmri üzə çıxarır; jurnalı buraxılış biletinə köçürün
`package_summary.json` ilə yanaşı rəyçilər CAR həzmlərini, planlarını fərqləndirə bilsinlər
metadata və xüsusi qabıq çıxışı olmadan Sigstore paket heşləri.

## Addım 9 — Gateway + SoraDNS yoxlanışı

Kəsmə elan etməzdən əvvəl, yeni ləqəbin SoraDNS vasitəsilə həll olunduğunu sübut edin
şlüzlər təzə sübutları əsaslandırır:

1. **Zod qapısını işə salın.** `ci/check_sorafs_gateway_probe.sh` məşqləri
   `cargo xtask sorafs-gateway-probe` demo qurğulara qarşı
   `fixtures/sorafs_gateway/probe_demo/`. Həqiqi yerləşdirmələr üçün zondu istiqamətləndirin
   hədəf host adında:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Zond `Sora-Name`, `Sora-Proof` və `Sora-Proof-Status` kodunu deşifrə edir.
   `docs/source/sorafs_alias_policy.md` və manifest həzm edildikdə uğursuz olur,
   TTL-lər və ya GAR bağlamalarının sürüşməsi.

   Yüngül ləkə yoxlamaları üçün (məsələn, yalnız bağlama paketi olduqda
   dəyişdirildi), `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` işə salın.
   Köməkçi tutulan bağlama paketini təsdiqləyir və azad etmək üçün əlverişlidir
   tam yoxlama qazma yerinə yalnız məcburi təsdiq tələb edən biletlər.

2. **Qazma dəlillərini çəkin.** Operator qazmaları və ya PagerDuty quru qaçışları üçün sarın
   `scripts/telemetry/run_sorafs_gateway_probe.sh --ssenari ilə zond
   devportal-rollout -- …`. Sarğı başlıqları/logları altında saxlayır
   `artifacts/sorafs_gateway_probe/<stamp>/`, `ops/drill-log.md` yeniləmələri və
   (isteğe bağlı olaraq) geri qaytarma qarmaqlarını və ya PagerDuty yüklərini tetikler. Set
   `--host docs.sora` IP-ni sərt kodlaşdırmaq əvəzinə SoraDNS yolunu təsdiqləmək üçün.3. **DNS bağlamalarını yoxlayın.** İdarəetmə ləqəb sübutunu dərc etdikdə, qeyd edin
   zondda istinad edilən GAR faylı (`--gar`) və onu buraxılışa əlavə edin
   sübut. Həlledici sahibləri eyni girişi əks etdirə bilər
   `tools/soradns-resolver` keşlənmiş girişlərin yeni manifestə hörmət etməsini təmin etmək üçün.
   JSON-u əlavə etməzdən əvvəl işə salın
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Beləliklə, deterministik host xəritələmə, manifest metadata və telemetriya etiketləri
   offline olaraq təsdiqlənmişdir. Köməkçi ilə yanaşı `--json-out` xülasəsi də yaya bilər
   İmzalanmış GAR, belə ki, rəyçilər binar faylı açmadan yoxlanıla bilən sübuta malik olsunlar.
  Yeni GAR layihəsini hazırlayarkən üstünlük verin
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (yalnız manifest faylı olmadıqda `--manifest-cid <cid>`-ə qayıdın
  mövcuddur). Köməkçi indi CID **və** BLAKE3 həzmini birbaşa ondan əldə edir
  manifest JSON, boşluqları kəsir, təkrarlanan `--telemetry-label` təkrarlanır
  bayraqlar, etiketləri çeşidləyir və defolt CSP/HSTS/Permissions-Policy-ni yayır
  JSON-u yazmadan əvvəl şablonlar hazırlayın ki, faydalı yük belə olduqda belə deterministik qalır
  operatorlar müxtəlif qabıqlardan etiketlər çəkirlər.

4. **Lütfən ölçülərə baxın.** `torii_sorafs_alias_cache_refresh_duration_ms` saxlayın
   və ekranda isə `torii_sorafs_gateway_refusals_total{profile="docs"}`
   zond işləyir; hər iki serialda yer alıb
   `dashboards/grafana/docs_portal.json`.

## Addım 10 — Monitorinq və sübutların yığılması

- **İdarə panelləri.** Eksport `dashboards/grafana/docs_portal.json` (portal SLO),
  `dashboards/grafana/sorafs_gateway_observability.json` (şluz gecikməsi +
  sübut sağlamlıq) və `dashboards/grafana/sorafs_fetch_observability.json`
  hər buraxılış üçün (orkestr sağlamlığı). JSON ixracını əlavə edin
  buraxılış bileti beləliklə rəyçilər Prometheus sorğularını təkrarlaya bilsinlər.
- **Arxivləri yoxlayın.** `artifacts/sorafs_gateway_probe/<stamp>/`-i git-annex-də saxlayın
  və ya sübut qabınız. Prob xülasəsini, başlıqları və PagerDuty-ni daxil edin
  telemetriya skripti tərəfindən tutulan faydalı yük.
- **Paketi buraxın.** Portalı/SBOM/OpenAPI CAR xülasəsini, manifesti saxlayın
  paketlər, Sigstore imzaları, `portal.pin.report.json`, Try-It araşdırma jurnalları və
  bir vaxt damğası olan bir qovluq altında hesabatları yoxlayın (məsələn,
  `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Qazma jurnalı.** Zondlar qazmağın bir hissəsi olduqda icazə verin
  `scripts/telemetry/run_sorafs_gateway_probe.sh` `ops/drill-log.md`-a əlavə edin
  buna görə də eyni sübut SNNet-5 xaos tələbini ödəyir.
- **Bilet bağlantıları.** Grafana panel ID-lərinə və ya əlavə edilmiş PNG ixracına istinad edin
  dəyişiklik bileti, araşdırma hesabat yolu ilə birlikdə, beləliklə, dəyişiklik-rəyçilər
  qabıq girişi olmadan SLO-ları çarpaz yoxlaya bilər.

## Addım 11 - Çox mənbəli əldə etmə qazması və skorbord sübutu

SoraFS-də dərc etmək indi çox mənbəli əldəetmə sübutunu tələb edir (DOCS-7/SF-6)
yuxarıdakı DNS/gateway sübutları ilə yanaşı. Manifesti bağladıqdan sonra:

1. **Canlı manifestə qarşı `sorafs_fetch` işlədin.** Eyni plan/manifestdən istifadə edin
   Addım 2-3-də hazırlanmış artefaktlar və hər biri üçün verilmiş şlüz etimadnaməsi
   provayder. Auditorların orkestri təkrar oxuya bilməsi üçün hər çıxışı davam etdirin
   qərar yolu:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - Əvvəlcə manifestdə istinad edilən provayder reklamlarını alın (məsələn
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     və onları `--provider-advert name=path` vasitəsilə ötürün ki, tabloya baxa bilsin
     imkan pəncərələrini deterministik olaraq qiymətləndirin. istifadə edin
     `--allow-implicit-provider-metadata` **yalnız** armaturları təkrar oxuduqda
     CI; istehsalat matkapları ilə eniş edən imzalanmış elanlara istinad etməlidir
     sancaq.
   - Manifest əlavə bölgələrə istinad etdikdə əmri ilə təkrarlayın
     müvafiq provayder tənzimləyir ki, hər bir keş/ləqəb uyğun gəlir
     artefakt gətirmək.

2. **Çıxışları arxivləşdirin.** `scoreboard.json` saxla,
   `providers.ndjson`, `fetch.json` və `chunk_receipts.ndjson`
   sübut qovluğunu buraxın. Bu fayllar həmyaşıdların çəkisini ələ keçirir, yenidən cəhd edin
   büdcə, gecikmə EWMA və idarəetmə paketinin təqdim etməli olduğu hər bir hissəyə daxilolmalar
   SF-7 üçün saxlamaq.

3. **Telemetriyanı yeniləyin.** Gəlmə nəticələrini **SoraFS Fetch-ə idxal edin
   Müşahidə oluna bilənlik** idarə paneli (`dashboards/grafana/sorafs_fetch_observability.json`),
   `torii_sorafs_fetch_duration_ms`/`_failures_total` və
   anomaliyalar üçün provayder diapazonu panelləri. Grafana panel şəkillərini ilə əlaqələndirin
   Tablo yolunun yanında bilet buraxın.

4. **Xəbərdarlıq qaydalarını çəkin.** `scripts/telemetry/test_sorafs_fetch_alerts.sh`-i işə salın
   buraxılışı bağlamadan əvvəl Prometheus xəbərdarlıq paketini doğrulamaq üçün. əlavə edin
   promtool çıxışı biletə beləliklə, DOCS-7 rəyçiləri tövləni təsdiq edə bilsinlər
   və yavaş provayder siqnalları silahlı olaraq qalır.

5. **Cİ-yə naqil edin.** Portal pin iş axını `sorafs_fetch` addımını geridə saxlayır
   `perform_fetch_probe` girişi; səhnələşdirmə/istehsal işləri üçün onu aktivləşdirin
   gətirmə sübutu manifest paketi ilə yanaşı təlimatsız hazırlanır
   müdaxilə. Yerli təlimlər eyni skripti ixrac etməklə təkrar istifadə edə bilər
   şlüz nişanları və vergüllə ayrılmış `PIN_FETCH_PROVIDERS` təyin edilməsi
   provayder siyahısı.

## Təbliğat, müşahidə oluna bilmə və geri çəkilmə

1. **Təqdimat:** ayrı-ayrı səhnələşdirmə və istehsal ləqəblərini saxlayın. tərəfindən təşviq edin
   eyni manifest/paketlə `manifest submit`-i yenidən işə salmaq, dəyişdirmək
   İstehsal ləqəbini göstərmək üçün `--alias-namespace/--alias-name`. Bu
   QA quruluş pinini təsdiqlədikdən sonra yenidən qurmaqdan və ya istefa verməkdən yayınır.
2. **Monitorinq:** pin reyestrinin idarə panelini idxal edin
   (`docs/source/grafana_sorafs_pin_registry.json`) üstəgəl portala xas
   zondlar (bax `docs/portal/docs/devportal/observability.md`). Yoxlama məbləği haqqında xəbərdarlıq
   sürüşmə, uğursuz zondlar və ya təkrar cəhd sıçrayışlarını sübut edin.
3. **Geri qaytarma:** geri qaytarmaq, əvvəlki manifesti yenidən göndərmək (və ya manifestdən çıxmaq)
   cari ləqəb) `sorafs_cli manifest submit --alias ... --retire` istifadə edərək.
   Həmişə məlum olan ən son paketi və CAR xülasəsini saxlayın ki, geri çəkilmə sübutları mümkün olsun
   CI jurnalları dönərsə, yenidən yaradıla bilər.

## CI iş axını şablonu

Boru kəməriniz ən azı:

1. Build + lint (`npm ci`, `npm run build`, checksum nəsil).
2. Paket (`car pack`) və manifestləri hesablayın.
3. İşə aid olan OIDC nişanı (`manifest sign`) ilə imzalayın.
4. Audit üçün artefaktları (CAR, manifest, paket, plan, xülasələr) yükləyin.
5. PİN reyestrinə təqdim edin:
   - Sorğuları çəkin → `docs-preview.sora`.
   - Teqlər / qorunan filiallar → istehsal ləqəbinin təşviqi.
6. Çıxmazdan əvvəl probları + sübut yoxlama qapılarını işə salın.

`.github/workflows/docs-portal-sorafs-pin.yml` bütün bu addımları birləşdirir
əl ilə buraxılışlar üçün. İş axını:

- portalı qurur/sınaq edir,
- quruluşu `scripts/sorafs-pin-release.sh` vasitəsilə paketləyir,
- GitHub OIDC istifadə edərək manifest paketini imzalayır/yoxlayır,
- CAR/manifest/paket/plan/sübut xülasələrini artefakt kimi yükləyir və
- (isteğe bağlı olaraq) sirlər mövcud olduqda manifest + ləqəb bağlamasını təqdim edir.

İşi işə salmazdan əvvəl aşağıdakı repozitor sirlərini/dəyişənlərini konfiqurasiya edin:

| Adı | Məqsəd |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | `/v1/sorafs/pin/register`-i ifşa edən Torii host. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Təqdimatlarla qeydə alınmış dövr identifikatoru. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Manifest təqdim etmək üçün imzalama səlahiyyəti. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | `perform_submit` `true` olduqda manifestə bağlanan ləqəb dəsti. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64 ilə kodlanmış ləqəb sübut paketi (isteğe bağlıdır; ləqəb bağlamasını keçmək üçün buraxın). |
| `DOCS_ANALYTICS_*` | Digər iş axınları tərəfindən təkrar istifadə edilən mövcud analitik/tədqiqat son nöqtələri. |

Fəaliyyətlər UI vasitəsilə iş axını işə salın:

1. `alias_label` (məsələn, `docs.sora.link`), isteğe bağlı `proposal_alias`,
   və isteğe bağlı `release_tag` ləğvi.
2. Torii-ə toxunmadan artefakt yaratmaq üçün `perform_submit` işarəsini işarəsiz qoyun
   (quru qaçışlar üçün faydalıdır) və ya konfiqurasiya edilmiş birbaşa dərc etməyə imkan verin
   ləqəb.

`docs/source/sorafs_ci_templates.md` hələ də ümumi CI köməkçilərini sənədləşdirir
bu depodan kənar layihələr, lakin portal iş axınına üstünlük verilməlidir
gündəlik buraxılışlar üçün.

## Yoxlama siyahısı

- [ ] `npm run build`, `npm run test:*` və `npm run check:links` yaşıldır.
- [ ] Artefaktlarda ələ keçirilmiş `build/checksums.sha256` və `build/release.json`.
- [ ] CAR, plan, manifest və xülasə `artifacts/` altında yaradılıb.
- [ ] Sigstore paketi + qeydlərlə saxlanılan ayrılmış imza.
- [ ] `portal.manifest.submit.summary.json` və `portal.manifest.submit.response.json`
      təqdimatlar baş verdikdə tutulur.
- [ ] `portal.pin.report.json` (və isteğe bağlı `portal.pin.proposal.json`)
      CAR/manifest artefaktları ilə birlikdə arxivləşdirilmişdir.
- [ ] `proof verify` və `manifest verify-signature` qeydləri arxivləşdirilib.
- [ ] Grafana idarə panelləri yeniləndi + Sınaq sınaqları uğurlu oldu.
- [ ] Geri qaytarma qeydləri (əvvəlki manifest identifikatoru + təxəllüs həzm) əlavə olunur
      buraxılış bileti.
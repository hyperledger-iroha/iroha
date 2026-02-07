---
lang: az
direction: ltr
source: docs/portal/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c0320c0372f182add4c290a339bc4a0598ec00d0e95dd6601c811bf8134e96c0
source_last_modified: "2026-02-07T00:38:43.595197+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SORA Nexus Tərtibatçı Portalı

Bu kataloq interaktiv tərtibatçı üçün Docusaurus iş sahəsinə sahibdir.
portal. Portal Norito təlimatlarını, SDK sürətli başlanğıclarını və OpenAPI-i birləşdirir
`cargo xtask openapi` tərəfindən yaradılan arayış, onları SORA Nexus-ə bükərək
docs proqramında istifadə olunan brendinq.

## İlkin şərtlər

- Node.js 18.18 və ya daha yeni (Docusaurus v3 bazası).
- Paketin idarə olunması üçün iplik 1.x və ya npm ≥ 9.
- Rust alətlər silsiləsi (OpenAPI sinxronizasiya skripti tərəfindən istifadə olunur).

## Bootstrap

```bash
cd docs/portal
npm install    # or yarn install
```

## Mövcud skriptlər

| Komanda | Təsvir |
|---------|-------------|
| `npm run start` / `yarn start` | Canlı yenidən yükləmə ilə yerli inkişaf serverini işə salın (defolt olaraq `http://localhost:3000`). |
| `npm run build` / `yarn build` | `build/`-də istehsal quruluşu hazırlayın. |
| `npm run serve` / `yarn serve` | Ən son quruluşu yerli olaraq xidmət edin (tüstü testləri üçün faydalıdır). |
| `npm run docs:version -- <label>` | Cari sənədləri `versioned_docs/version-<label>` (`docusaurus docs:version` ətrafında sarğı) daxil edin. |
| `npm run sync-openapi` / `yarn sync-openapi` | `static/openapi/torii.json`-i `cargo xtask openapi` vasitəsilə bərpa edin (spesifikasiyanı əlavə versiya anlıq görüntülərinə köçürmək üçün `--mirror=<label>`-i keçin). |
| `npm run tryit-proxy` | “Sınayın” konsolunu gücləndirən staging proxy-ni işə salın (konfiqurasiya üçün aşağıya baxın). |
| `npm run probe:tryit-proxy` | Proksiyə (CI/monitorinq köməkçisi) qarşı `/healthz` + nümunə sorğu araşdırması verin. |
| `npm run manage:tryit-proxy -- <update|rollback>` | `.env` proksi hədəfini yedəkləmə ilə yeniləyin və ya bərpa edin. |
| `npm run sync-i18n` | Yapon, İvrit, İspan, Portuqal, Fransız, Rus, Ərəb, Urdu, Birma, Gürcü, Erməni, Azərbaycan, Qazax, Başqırd, Amhar, Dzonqa, Özbək, Monqol, Ənənəvi Çin və Sadələşdirilmiş Çin dillərinə tərcümə köpəklərinin `i18n/` altında mövcud olduğundan əmin olun. |
| `npm run sync-norito-snippets` | Kurasiya edilmiş Kotodama nümunə sənədləri + endirilə bilən fraqmentləri bərpa edin (həmçinin dev server plagini tərəfindən avtomatik işə salınır). |
| `npm run test:tryit-proxy` | Proksi vahid testlərini Node-un test proqramı (`node --test`) vasitəsilə həyata keçirin. |

OpenAPI sinxronizasiya skripti tələb edir ki, `cargo xtask openapi` buradan əldə edilə bilər
anbar kökü; `static/openapi/`-ə deterministik JSON faylı verir
və indi Torii marşrutlaşdırıcısının canlı spesifikasiyanı ifşa etməsini gözləyir (istifadə edin
`cargo xtask openapi --allow-stub` yalnız təcili yer tutucu çıxışı üçün).

## Sənəd versiyaları və OpenAPI anlıq görüntülər

- **Sənəd versiyasının kəsilməsi:** `npm run docs:version -- 2025-q3` (və ya razılaşdırılmış hər hansı etiket) işlədin.
  Yaradılmış `versioned_docs/version-<label>`, `versioned_sidebars`,
  və `versions.json`. Navbar versiyası açılan menyu avtomatik olaraq üzə çıxır
  yeni snapshot.
- **OpenAPI artefaktlarının sinxronlaşdırılması:** versiyanı kəsdikdən sonra kanonikləri yeniləyin
  spesifikasiya və `cargo xtask openapi --sign <path-to-ed25519-key>` vasitəsilə manifest, sonra
  ilə uyğun snapshot çəkin
  `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest`. The
  skript `static/openapi/versions/2025-q3/torii.json` yazır, spesifikasiyanı əks etdirir
  `versions/current/torii.json`-ə daxil olur, `versions.json` yeniləyir, yeniləyir
  `/openapi/torii.json` və imzalanmış `manifest.json`-i hər versiyaya klonlayır
  kataloqu belə ki, tarixi xüsusiyyətlər eyni mənşəli metadata daşıyır. İstənilən tədarük
  yeni yaradılmış spesifikasiyanı kopyalamaq üçün `--mirror=<label>` bayraqlarının sayı
  digər tarixi görüntülər.
- **CI gözləntiləri:** toxunan sənədlərə versiya zərbəsi daxil edilməlidir
  (mümkün olduqda) üstəgəl yenilənmiş OpenAPI anlıq görüntülər, beləliklə Swagger, RapiDoc,
  və Redoc panelləri əldə etmə xətası olmadan tarixi xüsusiyyətlər arasında keçid edə bilər.
- **Manifest icrası:** imzalanan zaman `sync-openapi` skripti indi uğursuz olur
  `manifest.json` əskikdir, qüsurludur və ya yeni yaradılmışa uyğun gəlmir
  spec belə ki, imzasız snapshotlar defolt olaraq dərc edilə bilməz. Yenidən qaç
  Kanonik manifesti yeniləmək üçün `cargo xtask openapi --sign <key>` və
  sinxronizasiyanı yenidən işə salın ki, versiyalı snapshotlar imzalanmış metadatanı götürsün.
  `--allow-unsigned`-i yalnız yerli önizləmələr üçün keçin (CI hələ də işləyir
  `ci/check_openapi_spec.sh`, spesifikasiyanı bərpa edir və onu yoxlayır
  öhdəliklərin birləşməsinə icazə verməzdən əvvəl manifest).

## Struktur

```
docs/portal/
├── docs/                 # Markdown/MDX content for the portal
├── i18n/                 # Locale overrides generated by sync-i18n
├── src/                  # React pages/components (placeholder scaffolding)
├── static/               # Static assets served verbatim (includes OpenAPI JSON)
├── scripts/              # Helper scripts (OpenAPI synchronisation)
├── docusaurus.config.js  # Core site configuration
└── sidebars.js           # Sidebar / navigation model
```

### Proksi konfiqurasiyasını sınayın

“Cəhd edin” sandbox sorğuları `scripts/tryit-proxy.mjs` vasitəsilə yönləndirir. Konfiqurasiya edin
onu işə salmazdan əvvəl mühit dəyişənləri ilə proksi:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_BEARER="sora-dev-token"          # optional
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run tryit-proxy
```

- `TRYIT_PROXY_LISTEN` (standart `127.0.0.1:8787`) bağlama ünvanına nəzarət edir.
- `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` yaddaşdaxili konfiqurasiya edin
  sürət məhdudlaşdırıcısı (60 saniyədə 60 sorğu üçün standart).
- Zəng edənin `Authorization` nömrəsini yönləndirmək üçün `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` seçin
  defolt daşıyıcı nişanına etibar etmək əvəzinə başlıq.
- `static/openapi/torii.json` proqramdan kənara çıxarsa, proxy işə başlamaqdan imtina edir.
  `static/openapi/manifest.json`-də imzalanmış manifest. Qaç
  Spesifikasiyanı yeniləmək üçün `npm run sync-openapi -- --latest`; ixrac
  `TRYIT_PROXY_ALLOW_STALE_SPEC=1` yalnız fövqəladə hallar üçün (xəbərdarlıq
  daxil oldu və proksi hər halda başlayır).
- Səhnə proksisini tüstü sınamaq üçün `npm run probe:tryit-proxy`-i işə salın və
  monitorinq işlərinə əmr; `npm run manage:tryit-proxy -- update` sadələşdirir
  geri qaytarmaq üçün `.env` ehtiyat nüsxəsini saxlayarkən Torii son nöqtələrini fırladın.
- `TRYIT_PROXY_PROBE_METRICS_FILE` və `TRYIT_PROXY_PROBE_LABELS` proba imkan verir
  `probe_success`/`probe_duration_seconds` Prometheus mətn faylı formatında emit
  sağlamlıq yoxlanışını node_exporter və ya hər hansı toplu kollektora bağlaya bilərsiniz.
- Prometheus sıyrıntıları ifşa edir `tryit_proxy_request_duration_ms_bucket`/`_count`/`_sum`
  histoqram seriyası; sənədlər portalı Grafana idarə paneli onları p95/p99 gecikməsi üçün istehlak edir
  SLO izləmə (`dashboards/grafana/docs_portal.json`).

### OAuth cihaz koduna giriş

Portal etibarlı şəbəkələrdən kənara çıxdıqda, OAuth cihazını konfiqurasiya edin
avtorizasiya ki, rəyçilər heç vaxt uzunömürlü Torii tokenlərinə toxunmurlar. İxrac et
`npm run start` və ya `npm run build`-i işə salmadan əvvəl aşağıdakı dəyişənlər:

| Dəyişən | Qeydlər |
|----------|-------|
| `DOCS_OAUTH_DEVICE_CODE_URL` / `DOCS_OAUTH_TOKEN_URL` | Cihaz Avtorizasiya Qrantını həyata keçirən OAuth son nöqtələri. |
| `DOCS_OAUTH_CLIENT_ID` | Müştəri ID-si sənədlərin önizləməsi üçün qeydiyyatdan keçib. |
| `DOCS_OAUTH_SCOPE` / `DOCS_OAUTH_AUDIENCE` | Buraxılmış nişanı məhdudlaşdırmaq üçün əlavə əhatə dairəsi/auditoriya sətirləri. |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Portal vidceti tərəfindən tətbiq edilən minimum səsvermə intervalı (defolt olaraq 5000 ms; daha aşağı dəyərlər rədd edilir). |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` / `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Server `expires_in`-i buraxdıqda istifadə edilən geri qaytarma müddətinin bitməsi pəncərələri. |
| `DOCS_OAUTH_ALLOW_INSECURE` | Quraşdırma vaxtı mühafizəsini keçmək üçün yalnız yerli inkişaf üçün `1` olaraq təyin edin. Tələb olunan dəyişənlər əskik olduqda və ya TTL-lər məcburi diapazondan kənara düşərsə, istehsal qurulması uğursuz olur. |

Misal:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
```

Quraşdırma bu dəyərləri götürdükdən sonra Sınaq konsolu girişi göstərir
cihaz kodunu göstərən, işarənin son nöqtəsini sorğulayan, Daşıyıcını dolduran panel
sahə avtomatik olaraq və müddəti bitdikdə nişanı təmizləyir. Portal bundan imtina edir
OAuth konfiqurasiyası natamam olduqda və ya token/cihaz TTL-ləri düşdükdə başlayın
təhlükəsizlik büdcəsindən kənarda; yalnız `DOCS_OAUTH_ALLOW_INSECURE=1` ixrac edin
anonim girişin məqbul olduğu birdəfəlik yerli önizləmələr. Proksi hələ də
onur kitabçası `X-TryIt-Auth` ləğv edir, beləliklə siz OAuth dəyişənlərini silə bilərsiniz
ad-hoc tokenləri yapışdırmağa üstünlük verdiyiniz zaman.

[`docs/devportal/security-hardening.md`](docs/devportal/security-hardening.md) üçün baxın
tam təhlükə modeli və qələm test qapıları.

### Təhlükəsizlik başlıqları

`docusaurus.config.js` indi deterministik təhlükəsizlik başlıqlarını - məzmun təhlükəsizliyini yayır
siyasət, Etibarlı Növlər, İcazələr-Siyasət və İstinad Siyasəti—belə statik hostlar
dev server kimi eyni qoruyucu relsləri miras alır. Varsayılanlar yalnız skriptlərə icazə verir
portal mənşəli xidmət göstərir və cross-mənşəli `connect-src` trafikini məhdudlaşdırır
konfiqurasiya edilmiş analitik son nöqtəni və Proksi ilə sınayın. İstehsal qurmaları rədd edilir
qeyri-HTTPS analitikası/sınayın son nöqtələri; `http://`-ə qarşı yerli önizləmələr üçün
hədəfləri, `DOCS_SECURITY_ALLOW_INSECURE=1`-i açıq şəkildə təyin edin ki, qoruyucu
endirilməsini qəbul edir.

### Lokallaşdırma iş axını

Portal mənbə dili olaraq ingilis və yapon, ivrit,
İspan, Portuqal, Fransız, Rus, Ərəb, Urdu, Birma, Gürcü,
Erməni, Azərbaycan, Qazax, Başqırd, Amhar, Dzonqa, Özbək, Monqol,
Ənənəvi Çin və Sadələşdirilmiş Çin dili. Nə vaxt yeni sənədlər düşsə,
qaçmaq:

```bash
npm run sync-i18n
```

Skript repozitoriya miqyasında `scripts/sync_docs_i18n.py` davranışını əks etdirir.
`i18n/<lang>/docusaurus-plugin-content-docs/current/...` altında qaralama tərcümələrinin yaradılması.
Redaktorlar daha sonra yer tutucu mətni əvəz edə və ön məsələni yeniləyə bilərlər
metadata (`status`, `translation_last_reviewed` və s.).

Varsayılan olaraq, istehsal quruluşlarına yalnız sadalanan yerlilər daxildir
`docs/i18n/published_locales.json` (hazırda yalnız ingilis dilində) belə qaralama yertutanları
göndərilmir. Öncədən baxmaq lazım olduqda `DOCS_I18N_INCLUDE_STUBS=1` təyin edin
yerli olaraq davam edən yerlilər.

### Məzmun xəritəsi (MVP hədəfləri)| Bölmə | Status | Qeydlər |
|---------|--------|-------|
| Norito Quickstart (`docs/norito/quickstart.md`) | Nəşr olundu | Norito faydalı yükü təqdim etmək üçün uçdan-uca Docker + Kotodama + CLI təlimatı. |
| Norito Mühasibat kitabçası (`docs/norito/ledger-walkthrough.md`) | Nəşr olundu | CLI ilə idarə olunan registr → nanə → əməliyyat/status yoxlanışı və SDK pariteti bağlantıları ilə transfer axını. |
| SDK reseptləri (`docs/sdk/recipes/`) | 🟡 Yayılır | Rust/Python/JS/Swift/Java kitab axını reseptləri dərc edilmişdir; qalan SDK-lar nümunəni əks etdirməlidir. |
| API arayışı | 📢 Avtomatlaşdırılmış | `yarn sync-openapi` `static/openapi/torii.json` nəşr edir; Hər buraxılışdan sonra yoxlamaq üçün DevRel. |
| Axın yol xəritəsi | 🢢 Kəsilmiş | Arxa plan inteqrasiyası üçün `docs/norito-streaming-roadmap.md`-ə baxın. |
| SoraFS çoxmənbəli planlaşdırma (`docs/sorafs/provider-advert-multisource.md`) | Nəşr olundu | Çox provayder əldə etmək üçün diapazon qabiliyyəti TLV-lərini, idarəetmənin yoxlanmasını, CLI qurğularını və telemetriya arayışlarını ümumiləşdirir. |

> 📌 03-05-2026 yoxlama məntəqəsi: sürətli başlanğıc və ən azı bir resept dərc edildikdən sonra, bu cədvəli silin və əvəzinə portal töhfəçiləri bələdçisinə keçid edin.

## CI və yerləşdirmə

- `ci/check_docs_portal.sh` əvvəlcə SDK kitabçası reseptlərinin onların kanonik mənbə fayllarına uyğunluğunu yoxlamaq üçün `node scripts/check-sdk-recipes.mjs`-i işə salır, sonra `npm ci|install`, `npm run build`-i yerinə yetirir və əsas bölmələri təsdiq edir (ana səhifə, I0NT02, I010 Norito, nəşr yoxlama siyahısı) yaradılan HTML-də mövcuddur.
- `ci/check_openapi_spec.sh`, `cargo xtask openapi` vasitəsilə Torii spesifikasiyasını bərpa edir,
  onu `static/openapi/torii.json` və `versions/current/torii.json` ilə müqayisə edir,
  və yoxlama cəmi manifestini yoxlayır, beləliklə, izlənən spesifikasiya və ya hash zamanı PR-lər uğursuz olur
  köhnəlmişdir.
- `.github/workflows/check-docs.yml` məzmun reqressiyalarını tutmaq üçün hər bir PR-də portal quruluşunu idarə edir.
- `.github/workflows/docs-portal-preview.yml` çəkmə sorğuları üçün sayt qurur, `build/checksums.sha256` yazır, onu `sha256sum -c` vasitəsilə təsdiqləyir, çıxışı `artifacts/preview-site.tar.gz` kimi paketləyir, `scripts/generate-preview-descriptor.mjs` vasitəsilə deskriptor yaradır, həm sayt, həm də metastatic, həm də artifda, yüklənir. rəyçilər quruluşu yenidən işə salmadan dəqiq görüntünü yoxlaya bilərlər. İş axını həm də manifest/arxiv həzmlərini (və mövcud olduqda SoraFS paketini) ümumiləşdirən çəkmə sorğusu şərhi göndərir ki, rəyçilər Fəaliyyət jurnallarını açmadan yoxlama nəticəsini görürlər.
- `.github/workflows/docs-portal-deploy.yml`, `github-pages` mühit URL-də önizləmə təmin edərək, `main`/`master`-ə təkanlarla GitHub Səhifələrində statik quruluşu dərc edir. İş axını yerləşdirmədən əvvəl açılış səhifəsinin tüstü yoxlamasını həyata keçirir.
- `scripts/sorafs-package-preview.sh` önizləmə saytını deterministik SoraFS paketinə (CAR, plan, manifest) çevirir və etimadnamələr JSON konfiqurasiyası vasitəsilə təmin edildikdə (bax: `docs/examples/sorafs_preview_publish.json`), manifest I180040.
- `scripts/preview_verify.sh` nəzərdən keçirənlərin əvvəllər əl ilə icra etdiyi yoxlama məbləği + deskriptorun doğrulama axınını həyata keçirir. Önizləmə artefaktlarının dəyişdirilmədiyini təsdiqləmək üçün onu çıxarılmış `build/` kataloquna (və əlavə deskriptor/arxiv yollarına) yönəldin.
- `scripts/sorafs-pin-release.sh` plus `.github/workflows/docs-portal-sorafs-pin.yml` istehsalı SoraFS boru kəmərini avtomatlaşdırın: qurma/sınaq, CAR + manifest generasiyası, Sigstore imzalama, yoxlama, isteğe bağlı ləqəb bağlama və idarəetmənin nəzərdən keçirilməsi üçün artefakt yükləmələri. Buraxılışı təşviq edərkən `workflow_dispatch` vasitəsilə iş axını işə salın.
- `cargo xtask soradns-verify-gar --gar <path> --name <fqdn> [...]` Gateway Avtorizasiya Qeydlərini imzalanmadan və ya əməliyyatlara göndərilməmişdən əvvəl təsdiqləyir. Köməkçi kanonik/gözəl hostları, manifest metadatasını və telemetriya etiketlərinin deterministik siyasətə uyğun olmasını təmin edir və `--json-out` vasitəsilə DG-3 sübutu üçün JSON xülasəsi göndərə bilər.
- Təqdimat baş verdikdə, pin köməkçisi də `artifacts/sorafs/portal.dns-cutover.json` istehsal etmək üçün `scripts/generate-dns-cutover-plan.mjs`-i çağırır. `DNS_CHANGE_TICKET`, `DNS_CUTOVER_WINDOW`, `DNS_HOSTNAME`, `DNS_ZONE` və `DNS_OPS_CONTACT` təyin edin (yaxud müvafiq `--dns-*` bayraqlarını keçin) DNS istehsalı üçün kəsilmiş avtomobillər üçün metadata ehtiyacı var. Keşin etibarsızlaşdırılması və geri qaytarma qarmaqları tələb olunduqda, `DNS_CACHE_PURGE_ENDPOINT`, `DNS_CACHE_PURGE_AUTH_ENV` və `DNS_PREVIOUS_PLAN` (və ya `--cache-purge-*` / `--previous-dns-plan` bayraqları) əlavə edin, beləliklə deskript və ya deskript çağırışı üçün API qeydləri çevrilmələr. Deskriptor indi də zımbalanmış `Sora-Route-Binding`-i (host, CID, başlıq/məcburi yollar, yoxlama əmrləri) sənədləşdirir, beləliklə, GAR təşviqi və ehtiyat planının nəzərdən keçirilməsi kənarda göstərilən dəqiq başlıqlara istinad edir.
- Eyni iş axını `scripts/sns_zonefile_skeleton.py` vasitəsilə SNS zonası skeleti/resolver parçasını yaya bilər. IPv4/IPv6/CNAME/SPKI/TXT metadatasını təmin edin (ya `DNS_ZONEFILE_IPV4` kimi env varyasyonları və ya `--dns-zonefile-ipv4` kimi CLI bayraqları kimi), `DNS_GAR_DIGEST` ilə GAR həzmini keçin və köməkçi I019us (I019us2) yazacaq. snippet) avtomatik olaraq SN-7 sübutu kəsici deskriptorun yanında yer alır.
- DNS deskriptoru indi yuxarıdakı artefaktlara (yollar, məzmun CID-si, sübut statusu və hərfi başlıq şablonu) istinad edən `gateway_binding` bölməsini daxil edir, beləliklə, DG-3 dəyişikliyinin təsdiqlənməsi şlüzdə gözlənilən dəqiq `Sora-Name/Sora-Proof/CSP/HSTS` paketini ehtiva edir.

Hər `npm run build` çağırışı `postbuild` çəngəlini işə salır.
`build/checksums.sha256`. Quraşdırdıqdan (və ya CI artefaktlarını endirdikdən sonra) işə salın
`./docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build`-dən
manifesti təsdiq edin. Zaman `--descriptor <path>`/`--archive <path>` keçin
CI-dən metadata paketini yoxlamaq ki, skript qeydə alınmışları çarpaz yoxlaya bilsin
həzmlər və fayl adları.
- Yerli önizləməni təhlükəsiz şəkildə paylaşmaq üçün `npm run serve`-i işə salın; komanda indi sarılır
  `scripts/serve-verified-preview.mjs`, `preview_verify.sh` yerinə yetirir
  `docusaurus serve`-i işə salmazdan əvvəl və manifest yoxlaması uğursuz olarsa və ya dayandırılır
  yoxdur, hətta CI-dən kənarda da yoxlama cəminə qapalı ön baxışları saxlayır. The
  `npm run serve:verified` ləqəbi açıq zənglər üçün əlçatan olaraq qalır.

### URL-ləri və buraxılış qeydlərini nəzərdən keçirin

- İctimai beta baxışı: `https://docs.iroha.tech/`
- GitHub həmçinin hər bir yerləşdirmə üçün **github-səhifələr** mühiti altında quruluşu ifşa edir.
- Portal məzmununa toxunan çəkmə sorğularına qurulmuş saytı, yoxlama cəmi manifestini, sıxılmış arxivi və deskriptoru ehtiva edən Fəaliyyət artefaktları (`docs-portal-preview`, `docs-portal-preview-metadata`) daxildir; rəyçilər yerli olaraq `index.html`-i endirə və aça və önizləmələri paylaşmazdan əvvəl yoxlama məbləğlərini yoxlaya bilərlər. İş axını hər bir PR-də xülasə şərhini (manifest/arxiv heşləri üstəgəl SoraFS statusu) buraxır, beləliklə rəyçilər yoxlamanın keçdiyi barədə sürətli siqnal əldə etsinlər.
- Linki xaricdə paylaşmazdan əvvəl artefaktların CI-nin istehsal etdiklərinə uyğunluğunu təsdiqləmək üçün ilkin baxış paketini endirdikdən sonra `./docs/portal/scripts/preview_verify.sh --build-dir <extracted build> --descriptor <descriptor> --archive <archive>` istifadə edin.
- Buraxılış qeydlərini və ya status yeniləmələrini hazırlayarkən ilkin baxış URL-inə istinad edin ki, xarici rəyçilər anbarı klonlaşdırmadan ən son portal snapşotunu nəzərdən keçirə bilsinlər.
- `docs/portal/docs/devportal/preview-invite-flow.md` vasitəsilə önizləmə dalğalarını əlaqələndirin
  və onu hər dəfə `docs/portal/docs/devportal/reviewer-onboarding.md` ilə cütləşdirin
  dəvət, telemetriya ixracı və kənara çıxma addımı eyni sübut izini təkrar istifadə edir.
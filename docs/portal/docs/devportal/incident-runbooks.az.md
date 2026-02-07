---
lang: az
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8599dbc1a8e4fe846965eed90af128deb5950f83dc61838fea583b326b92a011
source_last_modified: "2025-12-29T18:16:35.104300+00:00"
translation_last_reviewed: 2026-02-07
id: incident-runbooks
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
---

## Məqsəd

Yol xəritəsi elementi **DOCS-9** işləyə bilən oyun kitablarını və məşq planını tələb edir.
portal operatorları təxmin etmədən göndərmə uğursuzluqlarını bərpa edə bilər. Bu qeyd
üç yüksək siqnal hadisəsini əhatə edir - uğursuz yerləşdirmə, təkrarlama
deqradasiya və analitik kəsintilər - və rüblük təlimləri sənədləşdirir
ləqəbin geri qaytarılmasını sübut etmək və sintetik doğrulama hələ də sona qədər işləyir.

### Əlaqədar material

- [`devportal/deploy-guide`](./deploy-guide) — qablaşdırma, imzalama və ləqəb
  təşviq iş prosesi.
- [`devportal/observability`](./observability) — buraxılış teqləri, analitika və
  zondlar aşağıda istinad edilir.
- `docs/source/sorafs_node_client_protocol.md`
  və [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — reyestr telemetriyası və eskalasiya hədləri.
- `docs/portal/scripts/sorafs-pin-release.sh` və `npm run probe:*` köməkçiləri
  yoxlama siyahıları boyunca istinad edilir.

### Paylaşılan telemetriya və alətlər

| Siqnal / Alət | Məqsəd |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (qarşılaşdı/buraxıldı/gözlənilir) | Replikasiya dayanacaqlarını və SLA pozuntularını aşkar edir. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Triaj üçün geriləmə dərinliyini və tamamlama gecikməsini kəmiyyətləşdirir. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Çox vaxt pis yerləşdirməni izləyən şlüz tərəfindəki uğursuzluqları göstərir. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Qapıları buraxan və geri dönmələri təsdiqləyən sintetik zondlar. |
| `npm run check:links` | Sınıq keçid qapısı; hər yumşaldılmadan sonra istifadə olunur. |
| `sorafs_cli manifest submit … --alias-*` (`scripts/sorafs-pin-release.sh` ilə bükülmüş) | Ləqəb təşviqi/reversiya mexanizmi. |
| `Docs Portal Publishing` Grafana lövhəsi (`dashboards/grafana/docs_portal.json`) | İmtina/ləqəb/TLS/replikasiya telemetriyasını cəmləşdirir. PagerDuty xəbərdarlıqları sübut üçün bu panellərə istinad edir. |

## Runbook — Uğursuz yerləşdirmə və ya pis artefakt

### Tətik şərtləri

- Önizləmə/istehsal zondları uğursuz oldu (`npm run probe:portal -- --expect-release=…`).
- Grafana xəbərdarlıqları `torii_sorafs_gateway_refusals_total` və ya
  Yayımdan sonra `torii_sorafs_manifest_submit_total{status="error"}`.
- Manual QA dərhal sonra pozulmuş marşrutları və ya Try-It proksi uğursuzluqlarını bildirir
  ləqəb tanıtımı.

### Dərhal mühafizə

1. **Yerləşdirmələri dondurun:** CI boru kəmərini `DEPLOY_FREEZE=1` (GitHub) ilə qeyd edin
   iş axını girişi) və ya Jenkins işini dayandırın ki, heç bir əlavə artefakt sönməsin.
2. **Artefaktları ələ keçirin:** uğursuz quruluşun `build/checksums.sha256`-ni endirin,
   `portal.manifest*.{json,to,bundle,sig}` və prob çıxışı beləliklə geriyə dönə bilər
   dəqiq həzmlərə istinad edin.
3. **Maraqlı tərəfləri xəbərdar edin:** yaddaş SRE, Sənədlər/DevRel rəhbəri və idarəetmə
   məlumatlandırma üçün növbətçi (xüsusilə `docs.sora` təsirləndikdə).

### Geri qaytarma proseduru

1. Son tanınmış yaxşı (LKG) manifestini müəyyənləşdirin. İstehsal iş axını saxlayır
   onları `artifacts/devportal/<release>/sorafs/portal.manifest.to` altında.
2. Göndərmə köməkçisi ilə həmin manifestə ləqəbi yenidən bağlayın:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. LKG və ilə birlikdə hadisə biletində geri qaytarma xülasəsini qeyd edin
   uğursuz manifest həzmlər.

### Doğrulama

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature …` və `sorafs_cli proof verify …`
   (yerləşdirmə təlimatına baxın) yenidən irəli sürülən manifestin hələ də uyğun olduğunu təsdiqləmək üçün
   arxivləşdirilmiş CAR.
4. `npm run probe:tryit-proxy` Try-It staging proxy-nin geri qayıtmasını təmin etmək üçün.

### Hadisədən sonra

1. Yalnız əsas səbəb başa düşüldükdən sonra yerləşdirmə boru kəmərini yenidən aktivləşdirin.
2. Doldurma [`devportal/deploy-guide`](./deploy-guide) “Öyrənilən dərslər”
   yeni əldə edilmiş girişlər, əgər varsa.
3. Uğursuz test paketi üçün fayl qüsurları (zond, keçid yoxlayıcı və s.).

## Runbook — Replikasiya deqradasiyası

### Tətik şərtləri

- Xəbərdarlıq: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{nəticə=~"qarşılaşdı|buraxıldı"}), 1) <
  10 dəqiqə üçün 0,95`.
- 10 dəqiqə ərzində `torii_sorafs_replication_backlog_total > 10` (bax
  `pin-registry-ops.md`).
- İdarəetmə, buraxılışdan sonra ləqəblərin əlçatanlığının yavaş olduğunu bildirir.

### Triaj

1. Təsdiq etmək üçün [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) idarə panellərini yoxlayın
   geriləmənin saxlama sinifinə və ya provayder donanmasına lokallaşdırılıb- lokallaşdırılmaması.
2. `sorafs_registry::submit_manifest` xəbərdarlıqları üçün Torii qeydlərini çarpaz yoxlayın
   təqdimatların özlərinin uğursuz olub-olmadığını müəyyənləşdirin.
3. `sorafs_cli manifest status --manifest …` (siyahılar
   provayderə görə təkrarlama nəticələri).

### Təsirlərin azaldılması

1. Daha yüksək replika sayı (`--pin-min-replicas 7`) ilə manifesti yenidən buraxın
   `scripts/sorafs-pin-release.sh` beləliklə, planlaşdırıcı yükü daha böyük yerə yayır
   provayder dəsti. Hadisə jurnalında yeni manifest həzmini qeyd edin.
2. Əgər gecikmə bir provayderə bağlıdırsa, onu müvəqqəti olaraq deaktiv edin
   replikasiya planlaşdırıcısı (`pin-registry-ops.md`-də sənədləşdirilmiş) və yenisini təqdim edin
   manifest digər provayderləri ləqəbi yeniləməyə məcbur edir.
3. Təxəllüsün təravəti replikasiya paritetindən daha kritik olduqda, yenidən bağlayın
   artıq səhnələşdirilmiş (`docs-preview`) isti manifestə ləqəb verin, sonra dərc edin
   SRE geridə qalanı təmizlədikdən sonra təqib manifestidir.

### Bərpa və bağlanma

1. Təmin etmək üçün `torii_sorafs_replication_sla_total{outcome="missed"}` monitoru
   yaylaları saymaq.
2. Hər replikanın olduğunu sübut kimi `sorafs_cli manifest status` çıxışını çəkin
   uyğun olaraq geri qayıdır.
3. Növbəti addımlarla ölümdən sonra təkrarlama ehtiyatını fayllayın və ya yeniləyin
   (provayder miqyası, chunker tuning və s.).

## Runbook — Analitika və ya telemetriya kəsilməsi

### Tətik şərtləri

- `npm run probe:portal` uğur qazanır, lakin idarə panelləri qəbul etməyi dayandırır
  >15 dəqiqə ərzində `AnalyticsTracker` hadisələri.
- Məxfilik nəzərdən keçirilməsi azalan hadisələrdə gözlənilməz artımı qeyd edir.
- `npm run probe:tryit-proxy` `/probe/analytics` yollarında uğursuz olur.

### Cavab

1. Quraşdırma vaxtı daxiletmələrini yoxlayın: `DOCS_ANALYTICS_ENDPOINT` və
   Uğursuz buraxılış artefaktında `DOCS_ANALYTICS_SAMPLE_RATE` (`build/release.json`).
2. `DOCS_ANALYTICS_ENDPOINT` işarəsi ilə `npm run probe:portal`-i yenidən işə salın
   izləyicinin hələ də faydalı yüklər yaydığını təsdiqləmək üçün quruluş kollektoru.
3. Kollektorlar işləmirsə, `DOCS_ANALYTICS_ENDPOINT=""` seçin və yenidən qurun.
   izləyicinin qısa qapanması; hadisə qrafikində kəsilmə pəncərəsini qeyd edin.
4. `scripts/check-links.mjs` hələ də barmaq izlərini təsdiq edin `checksums.sha256`
   (analitik kəsintilər sayt xəritəsinin təsdiqini * blok etməməlidir).
5. Kollektor bərpa edildikdən sonra onu həyata keçirmək üçün `npm run test:widgets`-i işə salın
   yenidən nəşr etməzdən əvvəl analitik köməkçi bölmə testləri.

### Hadisədən sonra

1. İstənilən yeni kollektorla [`devportal/observability`](./observability) yeniləyin
   məhdudiyyətlər və ya nümunə tələbləri.
2. Hər hansı analitik məlumat kənarda atılıbsa və ya redaktə edilibsə, fayl idarəçiliyi bildirişi
   siyasət.

## Rüblük davamlılıq məşqləri

Hər iki məşqi **hər rübün ilk çərşənbə axşamı** (yanvar/aprel/iyul/oktyabr) ərzində yerinə yetirin
və ya hər hansı əsas infrastruktur dəyişikliyindən dərhal sonra. Artefaktları altında saxlayın
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Qazma | Addımlar | Sübut |
| ----- | ----- | -------- |
| Ləqəb geri qaytarma məşqi | 1. Ən son istehsal manifestindən istifadə edərək "Uğursuz yerləşdirmə" geri qaytarılmasını təkrarlayın.<br/>2. Zondlar keçdikdən sonra istehsala yenidən qoşulun.<br/>3. `portal.manifest.submit.summary.json` yazın və qazma qovluğuna zond qeydləri yazın. | `rollback.submit.json`, zond çıxışı və məşqin buraxılış etiketi. |
| Sintetik doğrulama auditi | 1. İstehsal və quruluşa qarşı `npm run probe:portal` və `npm run probe:tryit-proxy`-i işə salın.<br/>2. `npm run check:links`-i işə salın və `build/link-report.json`-i arxivləşdirin.<br/>3. Probun müvəffəqiyyətini təsdiqləyən Grafana panellərinin skrinşotlarını/eksportlarını əlavə edin. | Zond qeydləri + `link-report.json` manifest barmaq izinə istinad edir. |

Buraxılmış təlimləri Sənədlər/DevRel menecerinə və SRE idarəetmə icmalına çatdırın,
çünki yol xəritəsi hər iki ləqəbin olduğunu göstərən deterministik, rüblük sübut tələb edir
geri qaytarma və portal probları sağlam qalır.

## PagerDuty və zəng üzrə koordinasiya

- PagerDuty xidməti **Docs Portal Publishing**-dən yaradılan xəbərdarlıqlara sahibdir.
  `dashboards/grafana/docs_portal.json`. Qaydalar `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` və `DocsPortal/TLSExpiry` Sənədlər/DevRel səhifəsi
  ikincil olaraq Storage SRE ilə əsas.
- Səhifə edilən zaman, `DOCS_RELEASE_TAG` daxil edin, təsirlənənlərin skrinşotlarını əlavə edin
  Grafana panelləri və əvvəl hadisə qeydlərində prob/bağlantı yoxlayın çıxışı
  yumşaldılması başlayır.
- Yumşaldıldıqdan sonra (geri qaytarma və ya yenidən yerləşdirmə), `npm run probe:portal`-i yenidən işə salın,
  `npm run check:links` və ölçüləri göstərən yeni Grafana anlıq görüntüləri çəkin
  eşiklər daxilində. PagerDuty hadisəsindən əvvəl bütün sübutları əlavə edin
  həll edir.
- İki xəbərdarlıq eyni vaxtda işə salınarsa (məsələn, TLS-nin bitmə vaxtı və gecikmə), triaj
  əvvəlcə imtina edir (nəşri dayandırın), geri qaytarma prosedurunu yerinə yetirin, sonra silin
  Körpüdə Saxlama SRE ilə TLS/arxa siyahı elementləri.
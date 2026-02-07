---
lang: az
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
translator: machine-google-reviewed
---

# Red-Team Drill — Əməliyyat SeaGlass

- **Drill ID:** `20260818-operation-seaglass`
- **Tarix və pəncərə:** `2026-08-18 09:00Z – 11:00Z`
- **Ssenari sinfi:** `smuggling`
- **Operatorlar:** `Miyu Sato, Liam O'Connor`
- **İdarəetmə panelləri icradan dondurulub:** `364f9573b`
- **Dəlil paketi:** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID (isteğe bağlı):** `not pinned (local bundle only)`
- **Əlaqədar yol xəritəsi elementləri:** `MINFO-9`, üstəgəl əlaqəli izləmələr `MINFO-RT-17` / `MINFO-RT-18`.

## 1. Məqsədlər və Giriş Şərtləri

- **Əsas məqsədlər**
  - Yük atma xəbərdarlıqları zamanı qaçaqmalçılıq cəhdi zamanı rədd edilmiş TTL tətbiqini və şlüz karantinini təsdiqləyin.
  - İdarəetmənin təkrar oxunmasının aşkar edilməsini təsdiqləyin və moderasiya runbook-da səhvlərin idarə edilməsini xəbərdar edin.
- **İlkin şərtlər təsdiqləndi**
  - `emergency_canon_policy.md` versiyası `v2026-08-seaglass`.
  - `dashboards/grafana/ministry_moderation_overview.json` həzm `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`.
  - Zəng zamanı səlahiyyəti ləğv edin: `Kenji Ito (GovOps pager)`.

## 2. İcra qrafiki

| Vaxt möhürü (UTC) | Aktyor | Fəaliyyət / Əmr | Nəticə / Qeydlər |
|----------------|-------|------------------|----------------|
| 09:00:12 | Miyu Sato | `scripts/ministry/export_red_team_evidence.py --freeze-only` vasitəsilə `364f9573b`-də dondurulmuş idarə panelləri/xəbərdarlıqları | Əsas xətt çəkilib və `dashboards/` | altında saxlanılır
| 09:07:44 | Liam O'Connor | Yayımlanan rədd siyahısı snapshot + `sorafs_cli ... gateway update-denylist --policy-tier emergency` ilə səhnələşdirmə üçün GAR ləğvi | Snapshot qəbul edildi; Alertmanager |-də qeydə alınan pəncərəni ləğv et
| 09:17:03 | Miyu Sato | `moderation_payload_tool.py --scenario seaglass` istifadə edərək enjekte edilmiş qaçaqmalçılıq yükü + idarəetmə təkrarı | 3m12s sonra siqnal verildi; idarəçilik təkrarı işarələndi |
| 09:31:47 | Liam O'Connor | Sübut ixracını həyata keçirdi və möhürlənmiş manifest `seaglass_evidence_manifest.json` | Sübut paketi və `manifests/` | altında saxlanılan heşlər

## 3. Müşahidələr və Metriklər

| Metrik | Hədəf | Müşahidə | Keçdi/Uğursuz | Qeydlər |
|--------|--------|----------|-----------|-------|
| Xəbərdarlığa cavab gecikməsi | = 0,98 | 0,992 | ✅ | Həm qaçaqmalçılıq, həm də təkrar yüklənmə aşkarlandı |
| Gateway anomaliyasının aşkarlanması | Xəbərdarlıq verildi | Xəbərdarlıq atıldı + avtomatik karantin | ✅ | Yenidən cəhd büdcəsi tükənməzdən əvvəl tətbiq olunan karantin |

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. Tapıntılar və Təmir

| Ciddilik | Tapmaq | Sahibi | Hədəf Tarixi | Status / Link |
|----------|---------|-------|-------------|---------------|
| Yüksək | İdarəetmə təkrar siqnalı işə salındı, lakin gözləmə siyahısı işə salındıqda SoraFS möhürü 2m gecikdi | İdarəetmə Əməliyyatları (Liam O'Connor) | 2026-09-05 | `MINFO-RT-17` açıq — əvəzetmə yoluna təkrar möhür avtomatlaşdırmasını əlavə edin |
| Orta | İdarə panelinin dondurulması SoraFS-ə bağlanmayıb; operatorlar yerli paketə güvənirdi | Müşahidə qabiliyyəti (Miyu Sato) | 25-08-2026 | `MINFO-RT-18` açıq — növbəti məşqdən əvvəl imzalanmış CID ilə `dashboards/*` - SoraFS pin |
| Aşağı | CLI jurnalı ilk keçiddə Norito manifest hash buraxıldı | Nazirlik Əməliyyatları (Kenji Ito) | 22-08-2026 | Qazma zamanı sabit; şablon jurnalda yeniləndi |Kalibrləmənin necə təzahür etdiyini sənədləşdirin, ləğvetmə siyasətləri və ya SDK/alətlər dəyişməlidir. GitHub/Jira problemləri ilə əlaqə saxlayın və bloklanmış/blokdan çıxarılan vəziyyətləri qeyd edin.

## 5. İdarəetmə və Təsdiqlər

- **Hadisə komandirinin imzalanması:** `Miyu Sato @ 2026-08-18T11:22Z`
- **İdarəetmə şurasının nəzərdən keçirilmə tarixi:** `GovOps-2026-08-22`
- **İzləmə yoxlama siyahısı:** `[x] status.md updated`, `[x] roadmap row updated`, `[x] transparency packet annotated`

## 6. Qoşmalar

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

Hər bir qoşmanı sübut paketinə və SoraFS anlıq şəklinə yüklədikdən sonra `[x]` ilə qeyd edin.

---

_Son yenilənmə: 2026-08-18_
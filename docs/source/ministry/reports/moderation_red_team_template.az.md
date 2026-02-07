---
lang: az
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **İstifadə qaydası:** hər qazmadan dərhal sonra bu şablonu `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md`-ə kopyalayın. Fayl adlarını kiçik hərflərlə, defislə və Alertmanager-ə daxil edilmiş məlumat identifikatoru ilə uyğunlaşdırın.

# Qırmızı Komanda Drill Hesabatı — `<SCENARIO NAME>`

- **Drill ID:** `<YYYYMMDD>-<scenario>`
- **Tarix və pəncərə:** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **Ssenari sinfi:** `smuggling | bribery | gateway | ...`
- **Operatorlar:** `<names / handles>`
- **İdarəetmə panelləri icradan dondurulub:** `<git SHA>`
- **Dəlil paketi:** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (isteğe bağlı):** `<cid>`  
- **Əlaqədar yol xəritəsi elementləri:** `MINFO-9`, üstəgəl hər hansı əlaqəli biletlər.

## 1. Məqsədlər və Giriş Şərtləri

- **Əsas məqsədlər**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **İlkin şərtlər təsdiqləndi**
  - `emergency_canon_policy.md` versiyası `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` həzm `<sha256>`
  - Zəng zamanı səlahiyyəti ləğv edin: `<name>`

## 2. İcra qrafiki

| Vaxt möhürü (UTC) | Aktyor | Fəaliyyət / Əmr | Nəticə / Qeydlər |
|----------------|-------|------------------|----------------|
|  |  |  |  |

> Torii sorğu identifikatorları, yığın hashləri, ləğvetmə təsdiqləri və Alertmanager keçidlərini daxil edin.

## 3. Müşahidələr və Metriklər

| Metrik | Hədəf | Müşahidə | Keçdi/Uğursuz | Qeydlər |
|--------|--------|----------|-----------|-------|
| Xəbərdarlığa cavab gecikməsi | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| Moderasiya aşkarlama dərəcəsi | `>= <value>` |  |  |  |
| Gateway anomaliyasının aşkarlanması | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. Tapıntılar və Təmir

| Ciddilik | Tapmaq | Sahibi | Hədəf Tarixi | Status / Link |
|----------|---------|-------|-------------|---------------|
| Yüksək |  |  |  |  |

Kalibrləmənin necə təzahür etdiyini sənədləşdirin, ləğvetmə siyasətləri və ya SDK/alətlər dəyişməlidir. GitHub/Jira problemləri ilə əlaqə saxlayın və bloklanmış/blokdan çıxarılan vəziyyətləri qeyd edin.

## 5. İdarəetmə və Təsdiqlər

- **Hadisə komandirinin imzalanması:** `<name / timestamp>`
- **İdarəetmə şurasının nəzərdən keçirilmə tarixi:** `<meeting id>`
- **İzləmə yoxlama siyahısı:** `[ ] status.md updated`, `[ ] roadmap row updated`, `[ ] transparency packet annotated`

## 6. Qoşmalar

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

Sübut paketinə və SoraFS snapşotuna yükləndikdən sonra hər bir qoşmanı `[x]` ilə qeyd edin.

---

_Son yenilənmə: {{tarix | default("2026-02-20") }}_
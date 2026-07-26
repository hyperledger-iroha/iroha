---
lang: az
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ca51ec624ebbb4b3760d5f2265d31047cd3b6492e21bdb10a3aa61655ccca69
source_last_modified: "2025-12-29T18:16:35.069181+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Partner SLA Discovery Notes — Şablon

Hər AND8 SLA kəşf sessiyası üçün bu şablondan istifadə edin. Doldurulmuş nüsxəni saxlayın
`docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` altında
və dəstəkləyici artefaktları əlavə edin (anket cavabları, təşəkkürlər,
əlavələr) eyni kataloqda.

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. Gündəlik və Kontekst

- Sessiyanın məqsədi (pilot əhatə dairəsi, buraxılış pəncərəsi, telemetriya gözləntiləri).
- Zəngdən əvvəl paylaşılan istinad sənədləri (dəstək kitabçası, buraxılış təqvimi,
  telemetriya panelləri).

## 2. İş yükünün icmalı

| Mövzu | Qeydlər |
|-------|-------|
| Hədəf iş yükləri / zəncirlər | |
| Gözlənilən əməliyyat həcmi | |
| Kritik iş pəncərələri / qaralma dövrləri | |
| Tənzimləmə rejimləri (GDPR, MAS, FISC və s.) | |
| Tələb olunan dillər / lokalizasiya | |

## 3. SLA Müzakirəsi

| SLA Class | Tərəfdaş gözləntisi | Başlanğıcdan Delta? | Tədbir tələb olunur |
|----------|--------------------|----------------------|-----------------|
| Kritik düzəliş (48 saat) | | Bəli/Xeyr | |
| Yüksək ciddilik (5 iş günü) | | Bəli/Xeyr | |
| Baxım (30 gün) | | Bəli/Xeyr | |
| Kəsmə bildirişi (60 gün) | | Bəli/Xeyr | |
| Hadisə rabitə kadansı | | Bəli/Xeyr | |

Tərəfdaşın tələb etdiyi hər hansı əlavə SLA müddəalarını sənədləşdirin (məsələn, ayrılmış
telefon körpüsü, əlavə telemetriya ixracı).

## 4. Telemetriya və Giriş Tələbləri

- Grafana / Prometheus giriş tələbləri:
- İxrac tələblərini qeyd edin/izləyin:
- Offline sübut və ya dosye gözləntiləri:

## 5. Uyğunluq və Hüquqi Qeydlər

- Yurisdiksiyaya aid bildiriş tələbləri (qanun + vaxt).
- Hadisə yeniləmələri üçün tələb olunan hüquqi əlaqə.
- Məlumat rezidentliyi məhdudiyyətləri / saxlama tələbləri.

## 6. Qərarlar və Fəaliyyət Maddələri

| Maddə | Sahibi | Vaxtı | Qeydlər |
|------|-------|-----|-------|
| | | | |

## 7. Təşəkkür

- Tərəfdaş əsas SLA-nı qəbul etdi? (Y/N)
- Təqibin təsdiqi üsulu (e-poçt / bilet / imza):
- Bağlamadan əvvəl təsdiq e-poçtunu və ya görüş protokolunu bu kataloqa əlavə edin.
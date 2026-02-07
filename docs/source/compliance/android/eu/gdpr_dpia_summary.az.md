---
lang: az
direction: ltr
source: docs/source/compliance/android/eu/gdpr_dpia_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5
source_last_modified: "2025-12-29T18:16:35.925474+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GDPR DPIA Xülasəsi — Android SDK Telemetriyası (AND7)

| Sahə | Dəyər |
|-------|-------|
| Qiymətləndirmə Tarixi | 2026-02-12 |
| Emal Fəaliyyəti | Android SDK telemetriyasının paylaşılan OTLP backendlərinə ixracı |
| Nəzarətçilər / Prosessorlar | SORA Nexus Əməliyyatlar (nəzarətçi), tərəfdaş operatorlar (birgə nəzarətçilər), Hyperledger Iroha İştirakçılar (prosessor) |
| İstinad Sənədləri | `docs/source/sdk/android/telemetry_redaction.md`, `docs/source/android_support_playbook.md`, `docs/source/android_runbook.md` |

## 1. Emal təsviri

- **Məqsəd:** Rust node sxemini əks etdirərkən (`telemetry_redaction.md` §2) AND7 müşahidəsini (gecikmə, təkrar cəhdlər, attestasiya sağlamlığı) dəstəkləmək üçün lazım olan operativ telemetriyanı təmin edin.
- **Sistemlər:** Android SDK cihazları -> OTLP ixracatçısı -> SRE tərəfindən idarə olunan paylaşılan telemetriya kollektoru (bax Dəstək Kitabı §8).
- **Məlumat subyektləri:** Android SDK əsaslı proqramlardan istifadə edən operator heyəti; aşağı axın Torii son nöqtələri (səlahiyyət sətirləri telemetriya siyasətinə uyğun heşlənmişdir).

## 2. Məlumatların İnventarlaşdırılması və Azaldılması

| Kanal | Sahələr | PII Riski | Azaldılması | Saxlama |
|---------|--------|----------|------------|-----------|
| İzlər (`android.torii.http.request`, `android.torii.http.retry`) | Marşrut, status, gecikmə | Aşağı (PII yoxdur) | Authority Blake2b + fırlanan duz ilə hashed; heç bir faydalı yük gövdəsi ixrac edilməmişdir. | 7-30 gün (telemetriya sənədinə görə). |
| Hadisələr (`android.keystore.attestation.result`) | Ləqəb etiketi, təhlükəsizlik səviyyəsi, attestasiya həzmi | Orta (əməliyyat məlumatları) | `actor_role_masked`, `actor_role_masked`, Norito audit tokenləri ilə ləğvetmələr üçün qeydə alınmış ləqəb (`alias_label`). | Uğur üçün 90 gün, ləğv/uğursuzluq üçün 365 gün. |
| Metriklər (`android.pending_queue.depth`, `android.telemetry.export.status`) | Növbə sayları, ixracatçı statusu | Aşağı | Yalnız ümumi hesablar. | 90 gün. |
| Cihaz profil ölçüləri (`android.telemetry.device_profile`) | SDK əsas, hardware səviyyəsi | Orta | Bucketing (emulator/istehlakçı/müəssisə), OEM və ya seriya nömrələri yoxdur. | 30 gün. |
| Şəbəkə kontekst hadisələri (`android.telemetry.network_context`) | Şəbəkə növü, rouminq bayrağı | Orta | Operatorun adı tamamilə silindi; abunəçi məlumatlarından qaçmaq üçün uyğunluq tələbini dəstəkləyir. | 7 gün. |

## 3. Qanuni Əsaslar və Hüquqlar

- **Qanuni əsas:** Qanuni maraq (Maddə 6(1)(f)) — tənzimlənən mühasibat kitab müştərilərinin etibarlı fəaliyyətinin təmin edilməsi.
- **Zəruriyyət testi:** Əməliyyat sağlamlığı ilə məhdudlaşan ölçülər (istifadəçi məzmunu yoxdur); hashed səlahiyyəti yalnız səlahiyyətli dəstək işçiləri üçün əlçatan olan geri çevrilə bilən xəritələmə vasitəsilə Rust qovşaqları ilə pariteti təmin edir (iş axınını ləğv etməklə).
- **Balansın testi:** Telemetriya son istifadəçi məlumatlarına deyil, operator tərəfindən idarə olunan cihazlara şamil edilir. Dəstək + Uyğunluq (Support Playbook §3 + §9) tərəfindən nəzərdən keçirilən Norito imzalı artefaktları ləğv etmək tələb olunur.
- **Məlumat subyektinin hüquqları:** Operatorlar telemetriyanın ixracını/silməsini tələb etmək üçün Dəstək Mühəndisliyi ilə əlaqə saxlayın (çalışma kitabı §2). Redaksiyanın ləğvi və qeydlər (telemetriya sənədi §Siqnal İnventarizasiyası) 30 gün ərzində yerinə yetirilməyə imkan verir.

## 4. Riskin Qiymətləndirilməsi| Risk | Ehtimal | Təsir | Qalıq azaldılması |
|------|------------|--------|---------------------|
| Hashed orqanları vasitəsilə yenidən identifikasiya | Aşağı | Orta | `android.telemetry.redaction.salt_version` vasitəsilə qeydə alınan duz fırlanması; təhlükəsiz anbarda saxlanılan duzlar; rüblük yoxlanılır. |
| Profil kovaları vasitəsilə cihaz barmaq izi | Aşağı | Orta | Yalnız səviyyə + SDK əsas ixrac edildi; Support Playbook OEM/seriya datası üçün eskalasiya sorğularını qadağan edir. |
| PII sızan sui-istifadəni ləğv edin | Very Low | Yüksək | Norito ləğvetmə sorğuları daxil edilib, müddəti 24 saat ərzində bitir, SRE təsdiqi tələb olunur (`docs/source/android_runbook.md` §3). |
| AB xaricində telemetriya saxlama | Orta | Orta | AB + JP bölgələrində yerləşdirilən kollektorlar; OTLP backend konfiqurasiyası vasitəsilə tətbiq edilən saxlama siyasəti (Dəstək Playbook §8-də sənədləşdirilib). |

Yuxarıdakı nəzarətlər və davam edən monitorinq nəzərə alınmaqla, qalıq risk məqbul hesab edilir.

## 5. Fəaliyyətlər və Təqiblər

1. **Rüblük nəzərdən keçirmə:** Telemetriya sxemlərini, duz fırlanmalarını təsdiqləyin və qeydləri ləğv edin; `docs/source/sdk/android/telemetry_redaction_minutes_YYYYMMDD.md`-də sənəd.
2. ** Çarpaz SDK uyğunlaşdırılması:** Ardıcıl hashing/baqlama qaydalarına riayət etmək üçün Swift/JS xidmətçiləri ilə koordinasiya edin (AND7 yol xəritəsində izlənilir).
3. **Tərəfdaş ünsiyyətləri:** DPIA xülasəsini partnyor daxiletmə dəstlərinə daxil edin (Dəstək Playbook §9) və `status.md`-dən bu sənədə keçid edin.
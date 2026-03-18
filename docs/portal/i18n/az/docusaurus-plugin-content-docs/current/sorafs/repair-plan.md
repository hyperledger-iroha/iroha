---
id: repair-plan
lang: az
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
Güzgülər `docs/source/sorafs_repair_plan.md`. Sfenks dəsti çıxarılana qədər hər iki versiyanı sinxronlaşdırın.
:::

## İdarəetmə Qərarının Həyat Dövrü
1. Artırılmış təmirlər slash təklif layihəsi yaradır və mübahisə pəncərəsini açın.
2. İdarəetmə seçiciləri mübahisə pəncərəsi zamanı təsdiq/rədd səslərini verirlər.
3. `escalated_at_unix + dispute_window_secs`-də qərar deterministik şəkildə hesablanır: minimum seçicilər, təsdiqlər rəddləri üstələyir və təsdiq nisbəti kvorum həddinə cavab verir.
4. Təsdiq edilmiş qərarlar apellyasiya pəncərəsini açır; `approved_at_unix + appeal_window_secs`-dən əvvəl qeydə alınmış şikayətlər qərarı şikayət edilmiş kimi qeyd edir.
5. Bütün təkliflərə cərimə həddi tətbiq edilir; həddən yuxarı təqdimatlar rədd edilir.

## İdarəetmənin Eskalasiya Siyasəti
Eskalasiya siyasəti `iroha_config`-də `governance.sorafs_repair_escalation`-dən qaynaqlanır və hər təmir slash təklifi üçün tətbiq edilir.

| Parametrlər | Defolt | Məna |
|---------|---------|---------|
| `quorum_bps` | 6667 | Hesablanmış səslər arasında minimum təsdiq nisbəti (əsas bal). |
| `minimum_voters` | 3 | Qərarın həlli üçün tələb olunan fərqli seçicilərin minimum sayı. |
| `dispute_window_secs` | 86400 | Səsvermənin yekunlaşmasına qədər eskalasiyadan sonrakı vaxt (saniyələr). |
| `appeal_window_secs` | 604800 | Təsdiqdən sonra müraciətlərin qəbul edildiyi vaxt (saniyələr). |
| `max_penalty_nano` | 1.000.000.000 | Təmir eskalasiyası üçün icazə verilən maksimum kəsik cəzası (nano-XOR). |

- Planlayıcı tərəfindən yaradılan təkliflər `max_penalty_nano` ilə məhdudlaşır; yuxarı həddən yuxarı auditor təqdimatları rədd edilir.
- Səs qeydləri deterministik sifarişlə (`voter_id` çeşidlənməsi) `repair_state.to`-də saxlanılır, beləliklə, bütün qovşaqlar eyni qərar vaxt möhürü və nəticəsi əldə edir.
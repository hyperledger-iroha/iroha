---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c11fe861e7052b113b91249eb9e39adca67a3b3cc20acf497f0785e37498504c
source_last_modified: "2025-12-29T18:16:35.196700+00:00"
translation_last_reviewed: 2026-02-07
id: priority-snapshot-2025-03
title: Priority Snapshot — March 2025 (Beta)
description: Mirror of the 2025-03 Nexus steering snapshot; pending ACKs before public rollout.
translator: machine-google-reviewed
---

> Каноник эх сурвалж: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Статус: **Бета / удирдах ACK-г хүлээж байна** (Сүлжээ, Хадгалах, Доксын удирдамж).

## Тойм

Гуравдугаар сарын хормын хувилбар нь баримт бичиг/контент-сүлжээний санаачилгатай нийцүүлэн хадгалдаг
SoraFS хүргэх замууд (SF‑3, SF‑6b, SF‑9). Бүх удирдагчид хүлээн зөвшөөрсний дараа
Nexus жолооны суваг дахь агшин зуурын зураг, дээрх "Бета" тэмдэглэлийг устгана уу.

### Утаснуудыг төвлөрүүл

1. **Тэргүүлэх агшин зуурын зургийг эргүүлэх** — талархал цуглуулж, тэдгээрийг бүртгэлд оруулна уу.
   2025-03-05 Зөвлөлийн протокол.
2. **Gateway/DNS kickoff close-out** — шинэ хөнгөвчлөх иж бүрдлийг давт (6-р хэсэг)
   runbook-д) 2025-03-03 семинарын өмнө.
3. **Операторын runbook шилжилт** — `Runbook Index` портал шууд ажиллаж байна; бета хувилбарыг ил гаргах
   Шүүгч элссэний дараа URL-г урьдчилан харах.
4. **SoraFS хүргэх утас** — SF‑3/6b/9 үлдсэн ажлыг төлөвлөгөө/замын зурагтай уялдуулна уу:
   - `sorafs-node` PoR залгих ажилтан + төлөвийн төгсгөлийн цэг.
   - Rust/JS/Swift найруулагчийн интеграцид CLI/SDK холбох өнгөлгөө.
   - PoR зохицуулагчийн ажиллах цагийн утас болон GovernanceLog үйл явдлууд.

Бүрэн хүснэгт, түгээлтийн хяналтын хуудас, бүртгэлийн оруулгуудыг эх файлаас харна уу.
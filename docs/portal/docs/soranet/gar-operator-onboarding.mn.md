---
lang: mn
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

Энэ товчлолыг ашиглан SNNet-9 нийцлийн тохиргоог давтагдах боломжтой,
аудитад ээлтэй үйл явц. Үүнийг оператор бүртэй харъяаллын хяналттай хослуулаарай
ижил тойм болон нотлох баримтын бүдүүвчийг ашигладаг.

## Алхам

1. **Тохиргоог угсарна**
   - `governance/compliance/soranet_opt_outs.json` импортлох.
   - Өөрийн `operator_jurisdictions`-г нийтлэгдсэн баталгаажуулалтын тоймтой нэгтгэнэ үү
     [харьяаллын хяналт](gar-jurisdictional-review).
2. **Баталгаажуулах**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Нэмэлт: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Нотлох баримтыг барих**
   - `artifacts/soranet/compliance/<YYYYMMDD>/` дор дэлгүүр:
     - `config.json` (эцсийн дагаж мөрдөх блок)
     - `attestations.json` (URI + задаргаа)
     - баталгаажуулалтын бүртгэлүүд
     - гарын үсэг зурсан PDF/Norito дугтуйны лавлагаа
4. **Идэвхжүүлэх**
   - Нэвтрүүлэлтийг шошголох (`gar-opt-out-<date>`), найруулагч/SDK тохиргоог дахин байршуулах,
     болон `compliance_*` үйл явдлууд хүлээгдэж буй логонд гарч байгааг баталгаажуулна уу.
5. **Хаах**
   - Нотлох баримтын багцаа Засаглалын зөвлөлд ирүүлнэ үү.
   - Идэвхжүүлэлтийн цонх + зөвшөөрчдийг GAR бүртгэлийн дэвтэрт бүртгэнэ.
   - Шүүхийн хяналтын хүснэгтээс дараагийн хяналтын огноог төлөвлө.

## Шуурхай шалгах хуудас

- [ ] `jurisdiction_opt_outs` каноник каталогтой таарч байна.
- [ ] Баталгаажуулалтын тоймыг яг хуулбарласан.
- [ ] Баталгаажуулах командуудыг ажиллуулж архивласан.
- [ ] `artifacts/soranet/compliance/<date>/`-д хадгалагдсан нотлох баримт.
- [ ] Дамжуулах шошго + GAR дэвтэр шинэчлэгдсэн.
- [ ] Дараагийн хяналтын сануулагчийг тохируулсан.

## Мөн үзнэ үү

- [ГАР-ын харьяаллын хяналт](gar-jurisdictional-review)
- [GAR Compliance Playbook (эх сурвалж)](../../../source/soranet/gar_compliance_playbook.md)
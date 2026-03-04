---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c3d1e36d99e18b5986e911a6b240393a92140324142f9edb778d2f966b1712e
source_last_modified: "2026-01-05T09:28:11.909605+00:00"
translation_last_reviewed: 2026-02-07
id: runbooks-index
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
translator: machine-google-reviewed
---

> `docs/source/sorafs/runbooks/` altında yaşayan sahibin kitabçasını əks etdirir.
> Hər yeni SoraFS əməliyyat təlimatı burada dərc edildikdən sonra burada əlaqələndirilməlidir.
> portal qurmaq.

Hansı runbook-ların miqrasiyanı tamamladığını yoxlamaq üçün bu səhifədən istifadə edin
mənbə yolu və portal nüsxəsi beləliklə, rəyçilər birbaşa istədiyinizə keçə bilsinlər
beta önizləmə zamanı bələdçi.

## Beta önizləmə aparıcısı

DocOps dalğası indi rəyçi tərəfindən təsdiqlənmiş beta önizləmə hostunu təqdim etdi
`https://docs.iroha.tech/`. Operatorları və ya rəyçiləri miqrasiyaya yönəldərkən
runbook, həmin host adına istinad edin ki, onlar yoxlama cəminə qapalı portaldan istifadə etsinlər
snapshot. Nəşriyyat/geri qaytarma prosedurları yaşayır
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Sahib(lər) | Portal surəti | Mənbə |
|---------|----------|-------------|--------|
| Gateway & DNS başlanğıcı | Şəbəkə TL, Ops Avtomatlaşdırma, Sənədlər/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS əməliyyat kitabçası | Sənədlər/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Bacarıqların uzlaşdırılması | Xəzinədarlıq / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Pin reyestr əməliyyatları | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Node əməliyyatlarının yoxlanış siyahısı | Saxlama Komandası, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Mübahisə və ləğvetmə runbook | İdarəetmə Şurası | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Səhnələşdirmə manifest oyun kitabı | Sənədlər/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Taikai anker müşahidəsi | Media Platforması WG / DA Proqramı / Şəbəkə TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Doğrulama yoxlama siyahısı

- [x] Portal bu indeksə bağlantılar qurur (yan panel girişi).
- [x] Hər köçürülmüş runbook rəyçiləri saxlamaq üçün kanonik mənbə yolunu sadalayır
  doc baxışları zamanı uyğunlaşdırılmışdır.
- [x] DocOps önizləmə boru kəməri blokları sadalanan runbook olmadıqda birləşir
  portal çıxışından.

Gələcək miqrasiyalar (məsələn, yeni xaos təlimləri və ya idarəetmə əlavələri)
yuxarıdakı cədvələ sətir çəkin və daxil edilmiş DocOps yoxlama siyahısını yeniləyin
`docs/examples/docs_preview_request_template.md`.
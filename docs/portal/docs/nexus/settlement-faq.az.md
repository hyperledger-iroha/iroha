---
lang: az
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-12-29T18:16:35.146583+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-settlement-faq
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
---

Bu səhifə daxili hesablaşma haqqında FAQ-ı əks etdirir (`docs/source/nexus_settlement_faq.md`)
Beləliklə, portal oxucuları eyni təlimatı qazmadan nəzərdən keçirə bilərlər
mono-repo. Bu, Hesablaşma Routerinin ödənişləri necə emal etdiyini, hansı ölçüləri izah edir
nəzarət etmək və SDK-ların Norito faydalı yüklərini necə inteqrasiya etməsi lazımdır.

## Əsas məqamlar

1. **Şerli xəritəçəkmə** — hər bir məlumat məkanı `settlement_handle` elan edir
   (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody`, və ya
   `xor_dual_fund`). Aşağıdakı ən son zolaq kataloqu ilə tanış olun
   `docs/source/project_tracker/nexus_config_deltas/`.
2. **Deterministik konversiya** — marşrutlaşdırıcı vasitəsilə bütün hesablaşmaları XOR-a çevirir
   idarəetmə tərəfindən təsdiq edilmiş likvidlik mənbələri. Şəxsi zolaqlar XOR buferlərini əvvəlcədən maliyyələşdirir;
   saç düzümü yalnız tamponlar siyasətdən kənara çıxdıqda tətbiq edilir.
3. **Telemetri** — saat `nexus_settlement_latency_seconds`, konversiya sayğacları,
   və saç düzümü ölçüləri. Panellər `dashboards/grafana/nexus_settlement.json`-də yaşayır
   və `dashboards/alerts/nexus_audit_rules.yml`-də xəbərdarlıqlar.
4. **Dəlil** — arxiv konfiqurasiyaları, marşrutlaşdırıcı qeydləri, telemetriya ixracı və
   auditlər üçün tutuşdurma hesabatları.
5. **SDK məsuliyyətləri** — hər bir SDK məskunlaşma köməkçilərini, zolaq identifikatorlarını,
   və marşrutlaşdırıcı ilə paritet saxlamaq üçün Norito faydalı yük kodlayıcıları.

## Nümunə axınları

| Zolaq növü | Tutmaq üçün sübut | Nəyi sübut edir |
|----------|--------------------|----------------|
| Şəxsi `xor_hosted_custody` | Router jurnalı + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC buferləri debet deterministik XOR və saç düzümü siyasət daxilində qalır. |
| İctimai `xor_global` | Router jurnalı + DEX/TWAP arayışı + gecikmə/çevirmə ölçüləri | Paylaşılan likvidlik yolu, sıfır saç düzümü ilə nəşr edilmiş TWAP-da transferi qiymətləndirdi. |
| Hibrid `xor_dual_fund` | İctimai və qorunan split + telemetriya sayğaclarını göstərən marşrutlaşdırıcı jurnal | Qorunan/ictimai qarışıq idarəetmə nisbətlərinə hörmətlə yanaşır və hər bir ayağa tətbiq olunan saç düzümü qeyd olunur. |

## Daha ətraflı məlumat lazımdır?

- Tam tez-tez verilən suallar: `docs/source/nexus_settlement_faq.md`
- Hesablaşma marşrutlaşdırıcısının xüsusiyyətləri: `docs/source/settlement_router.md`
- CBDC siyasət kitabı: `docs/source/cbdc_lane_playbook.md`
- Əməliyyat kitabçası: [Nexus əməliyyatları](./nexus-operations)
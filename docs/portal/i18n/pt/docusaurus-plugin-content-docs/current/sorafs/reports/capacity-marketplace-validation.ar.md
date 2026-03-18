---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: التحقق من سوق سعة SoraFS
tags: [SF-2c, aceitação, lista de verificação]
resumo: قائمة تحقق للقبول تغطي انضمام المزودين, تدفقات النزاعات, وتسوية الخزانة التي تضبط A solução de problemas é SoraFS.
---

# قائمة تحقق التحقق من سوق سعة SoraFS

**نافذة المراجعة:** 18/03/2026 -> 24/03/2026  
**مالكو البرنامج:** Equipe de armazenamento (`@storage-wg`), Conselho de governança (`@council`), Guilda do Tesouro (`@treasury`)  
**النطاق:** مسارات انضمام المزودين, تدفقات تحكيم النزاعات, وعمليات تسوية الخزانة المطلوبة لـ SF-2cGA.

Isso pode ser feito por meio de uma chave de fenda. Isso significa que o teste (testes, luminárias e testes) é algo que você pode fazer.

## قائمة تحقق القبول

### انضمام المزودين

| الفحص | التحقق | الدليل |
|-------|------------|----------|
| يقبل registro إعلانات السعة القياسية | يقوم اختبار تكاملي بتشغيل `/v1/sorafs/capacity/declare` na API do aplicativo, مع التحقق من معالجة التواقيع, metadados de dados, وتسليمها إلى registro العقدة. | `crates/iroha_torii/src/routing.rs:7654` |
| Construir contratos inteligentes e cargas úteis | Você pode usar o GiB para obter mais informações sobre o GiB. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Artefactos CLI Ferramentas de construção | O chicote CLI é usado para viagens de ida e volta Norito/JSON/Base64 e viagens de ida e volta. الإعلانات offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| يلتقط دليل المشغلين سير قبول الانضمام e حواجز الحوكمة | Você pode definir os padrões de política e os padrões de política. | `../storage-capacity-marketplace.md` |

### تسوية النزاعات

| الفحص | التحقق | الدليل |
|-------|------------|----------|
| تبقى سجلات النزاع مع digest قياسي للـ payload | Você pode encontrar uma carga útil e uma carga útil pendente e um livro-razão pendente. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| مولد نزاعات CLI يطابق المخطط القياسي | Você pode usar CLI como Base64/Norito e JSON para `CapacityDisputeV1`, sem precisar de pacotes de evidências. Sim. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Repetir replay يثبت حتمية النزاع/العقوبة | telemetria. Seus colegas. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| يوثق runbook مسار التصعيد والإلغاء | Não há nenhuma reversão e reversão. | `../dispute-revocation-runbook.md` |

### تسوية الخزانة| الفحص | التحقق | الدليل |
|-------|------------|----------|
| Livro razão يطابق توقع molho 30 dias | يمتد اختبار absorver عبر خمسة مزودين على 30 نافذة liquidação, مع مقارنة إدخالات razão بالمرجع المتوقع Não. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Livros-razão de contabilidade | يقارن `capacity_reconcile.py` توقعات livro razão de taxas بصادرات تحويل XOR المنفذة, ويصدر مقاييس Prometheus, ويضبط موافقة الخزانة عبر Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Serviços de facturação, facturação, serviços e telemetria | O Grafana possui GiB-hora, golpes de ataque, e o número de bits é o mesmo. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| التقرير المنشور يؤرشف منهجية absorver e reproduzir replay | Não deixe de molho, coloque os ganchos e os ganchos no lugar certo. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

أعد تشغيل حزمة التحقق قبل sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Não use cargas úteis para carregar cargas úteis / cargas úteis `sorafs_manifest_stub capacity {declaration,dispute}` e não usar JSON/Norito é um arquivo que está sendo executado.

## artefatos

| Artefato | Caminho | blake2b-256 |
|----------|------|------------|
| Máquinas de lavar roupa | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Máquinas de lavar roupa | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Máquinas de lavar roupa | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

احتفظ بالنسخ الموقعة من هذه artefatos مع حزمة الإصدار واربطها في سجل تغييرات الحوكمة.

## الموافقات

- Líder da equipe de armazenamento - @storage-tl (24/03/2026)  
- Secretário do Conselho de Governança — @council-sec (2026-03-24)  
- Líder de Operações de Tesouraria — @treasury-ops (24/03/2026)
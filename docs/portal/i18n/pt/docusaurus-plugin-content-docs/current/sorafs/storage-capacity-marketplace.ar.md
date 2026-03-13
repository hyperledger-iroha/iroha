---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado de capacidade de armazenamento
título: سوق سعة التخزين em SoraFS
sidebar_label: Nome da barra lateral
description: O SF-2c é um dispositivo de segurança que pode ser usado em qualquer lugar.
---

:::note المصدر المعتمد
O código é `docs/source/sorafs/storage_capacity_marketplace.md`. Verifique se o produto está funcionando corretamente.
:::

# Você pode usar o SoraFS (nome SF-2c)

يقدّم بند خارطة الطريق SF-2c سوقا محكوما حيث يعلن مزودو التخزين عن سعة ملتزم بها, ويتلقون أوامر النسخ المتماثل, ويكسبون رسوما تتناسب مع التوافر المُسلَّم. يحدد هذا المستند نطاق المخرجات المطلوبة للإصدار الأول ويقسمها إلى مسارات قابلة للتنفيذ.

## الأهداف

- التعبير عن التزامات سعة المزودين (إجمالي البايتات, حدود لكل lane, تاريخ الانتهاء) بصيغة قابلة Você pode usar o SoraNet e o Torii para obter mais informações.
- Pinos توزيع عبر المزودين وفق السعة المعلنة e estaca وقيود السياسة مع الحفاظ على سلوك حتمي.
- قياس تسليم التخزين (نجاح النسخ المتماثل, uptime, أدلة السلامة) e تصدير التليمترية لتوزيعرسوم.
- توفير عمليات الإلغاء والنزاع حتى يمكن معاقبة المزودين غير الأمناء, أو إزالتهم.

## مفاهيم النطاق

| المفهوم | الوصف | المخرج الأولي |
|--------|-------------|---------------------|
| `CapacityDeclarationV1` | O Norito é usado para configurar o chunker GiB, o GiB para a pista, تلميحات التسعير, التزام staking, وتاريخ الانتهاء. | Número + número em `sorafs_manifest::capacity`. |
| `ReplicationOrder` | تعليمات صادرة عن الحوكمة تعيّن CID no manifesto إلى مزود واحد أو أكثر, بما في ذلك مستوى التكرار e SLA. | O Norito é o mesmo do Torii + e o contrato inteligente. |
| `CapacityLedger` | سجل on-chain/off-chain يتتبع إعلانات السعة النشطة, أوامر النسخ المتماثل, مقاييس الأداء, وتراكم الرسوم. | Um contrato inteligente ou um stub fora da cadeia com um instantâneo. |
| `MarketplacePolicy` | Você pode fazer uma aposta e uma participação. | config struct em `sorafs_manifest` + é uma estrutura. |

### المخططات المنفذة (الحالة)

## تفصيل العمل

### 1. طبقة المخططات والسجل

| المهمة | Proprietário(s) | Produtos |
|------|----------|-------|
| Selecione `CapacityDeclarationV1` e `ReplicationOrderV1` e `CapacityTelemetryV1`. | Equipe de Armazenamento / Governança | Instalação Norito; وأدرج النسخ الدلالية e مراجع القدرات. |
| Você pode usar o analisador + validador em `sorafs_manifest`. | Equipe de armazenamento | فرض IDs أحادية, حدود السعة, ومتطلبات estaca. |
| Obtenha metadados para o chunker بإضافة `min_capacity_gib` para o tamanho do arquivo. | GT Ferramentaria | يساعد العملاء على فرض الحد الأدنى من متطلبات العتاد لكل ملف تعريف. |
| Use o `MarketplacePolicy` para remover o excesso de água e o cabo. | Conselho de Governança | Este documento contém os padrões de política. |

#### تعريفات المخطط (منفذة)- `CapacityDeclarationV1` `CapacityDeclarationV1` سعة موقعة لكل مزود, بما في ذلك lida com chunker قياسية, مراجع capacidades, حدود اختيارية لكل lane, تلميحات التسعير, نوافذ الصلاحية, e metadados. تضمن عملية التحقق stake غير صفري, lida com aliases de قياسية, مزالة التكرار, حدود lane ضمن الإجمالي المعلن, ومحاسبة GiB أحادية.【crates/sorafs_manifest/src/capacity.rs:28】
- يربط `ReplicationOrderV1` manifestos بتعيينات صادرة عن الحوكمة مع أهداف التكرار, عتبات SLA, وضمانات لكل atribuição; تفرض validadores lida com chunker الأمر.【crates/sorafs_manifest/src/capacity.rs:301】
- Use `CapacityTelemetryV1` para obter snapshots (GiB de tempo de atividade/PoR) توزيع الرسوم. تحقق الحدود يبقي الاستخدام ضمن الإعلانات والنسب ضمن 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Ajudantes de ajuda (`CapacityMetadataEntry` e `PricingScheduleV1` e pista/atribuição/SLA) أخطاء يمكن لـ CI e ferramentas downstream إعادة استخدامها.【crates/sorafs_manifest/src/capacity.rs:230】
- يعرض `PinProviderRegistry` O snapshot على السلسلة عبر `/v2/sorafs/capacity/state`, جامعاً إعلانات المزودين وإدخالات razão de taxas خلف Norito JSON arquivo.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- تغطي اختبارات التحقق فرض lida com القياسية, كشف التكرار, حدود lane, حمايات تعيين النسخ المتماثل, وفحوص نطاق التليمترية حتى تظهر الانحدارات فوراً في CI.【crates/sorafs_manifest/src/capacity.rs:792】
- أدوات المشغل: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` تحول specs المقروءة من البشر إلى Norito payloads قياسية, base64 blobs e JSON Você pode usar os fixtures para `/v2/sorafs/capacity/declare` e `/v2/sorafs/capacity/telemetry` e usar os equipamentos para isso. 【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Você pode usar Reference fixtures em `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) ou não. Modelo `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. تكامل طبقة التحكم

| المهمة | Proprietário(s) | Produtos |
|------|----------|-------|
| Use Torii para `/v2/sorafs/capacity/declare` e `/v2/sorafs/capacity/telemetry` e `/v2/sorafs/capacity/orders` para Norito JSON. | Equipe Torii | محاكاة منطق التحقق؛ Use os auxiliares JSON Norito. |
| Os snapshots são armazenados em `CapacityDeclarationV1` nos metadados do orquestrador e no gateway. | GT Ferramentaria / Equipe Orquestrador | تمديد `provider_metadata` بمراجع السعة حتى يحترم pontuação متعدد المصادر حدود lane. |
| Você pode usar o orquestrador/gateway para atribuir atribuições e failover. | Equipe de TL/Gateway de rede | Construtor de placar أوامر النسخ الموقعة من الحوكمة. |
| CLI: Use `sorafs_cli` ou `capacity declare` e `capacity telemetry` e `capacity orders import`. | GT Ferramentaria | Use JSON como + resultado scoreboard. |

### 3. سياسة السوق والحوكمة| المهمة | Proprietário(s) | Produtos |
|------|----------|-------|
| إقرار `MarketplacePolicy` (الحد الأدنى لـ stake, مضاعفات العقوبة, وتواتر التدقيق). | Conselho de Governança | Leia mais sobre docs e consulte o site. |
| إضافة خطافات الحوكمة حتى يستطيع O Parlamento اعتماد وتجديد وإلغاء declarações. | Conselho de Governança / Equipe de Contrato Inteligente | Obtenha eventos Norito + ingestão de manifesto. |
| تنفيذ جدول العقوبات (تخفيض الرسوم, cortando للبوند) المرتبط بانتهاكات SLA المقاسة عن بعد. | Conselho de Governança / Tesouraria | Você pode encontrar uma solução de problemas em `DealEngine`. |
| توثيق عملية النزاع ومصفوفة التصعيد. | Documentos / Governança | Livro de runbook de disputa + ajudantes CLI. |

### 4. الميترينغ وتوزيع الرسوم

| المهمة | Proprietário(s) | Produtos |
|------|----------|-------|
| Você pode ingerir o valor de Torii para `CapacityTelemetryV1`. | Equipe Torii | O valor é GiB-hora, PoR e tempo de atividade. |
| Verifique o valor do cartão em `sorafs_node` para obter o valor do SLA. | Equipe de armazenamento | Você pode usar o chunker e lidar com o chunker. |
| خط أنابيب التسوية: تحويل التليمترية + بيانات النسخ إلى pagamentos مقومة بـ XOR, وإنتاج ملخصات جاهزة O registro e o registro do livro-razão. | Equipe de Tesouraria/Armazenagem | Exportações do Deal Engine / Tesouro. |
| Gerar dashboards/alertas é uma tarefa (ingestão de pendências). | Observabilidade | Você pode usar Grafana para SF-6/SF-7. |

- يعرض Torii ou `/v2/sorafs/capacity/telemetry` e `/v2/sorafs/capacity/state` (JSON + Norito) بحيث يمكن للمشغلين إرسال snapshots تليمترية لكل حقبة ويمكن للمراجعين استرجاع ledger الحتمي للتدقيق, أو تغليف الأدلة.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- Use o `PinProviderRegistry` para definir o valor do endpoint; Use CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) para usar hashing ou alias.
- تنتج snapshots الميترينغ إدخالات `CapacityTelemetrySnapshot` المثبتة على snapshot `metering`, وتغذي Prometheus exports لوحة Grafana é um recurso de `docs/source/grafana_sorafs_metering.json` que é usado para armazenar GiB-hora e GiB-hora. nano-SORA é um arquivo de SLA do site.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- عند تفعيل metering smoothing, يتضمن snapshot الحقول `smoothed_gib_hours` e `smoothed_por_success_bps` حتى يتمكن المشغلون من مقارنة قيم EMA الملساء بالعدادات الخام التي تعتمد عليها الحوكمة في payouts.【crates/sorafs_node/src/metering.rs:401】

### 5. معالجة النزاعات والإلغاء| المهمة | Proprietário(s) | Produtos |
|------|----------|-------|
| Verifique o `CapacityDisputeV1` (evidência, evidência, teste). | Conselho de Governança | مخطط Norito + مدقق. |
| دعم CLI لتقديم disputas والرد عليها (مع مرفقات provas). | GT Ferramentaria | O hashing é uma evidência. |
| Você pode entrar em contato com o SLA (disputa de disputa). | Observabilidade | Verifique se há algum problema. |
| توثيق playbook الإلغاء (فترة سماح, إخلاء بيانات fixado). | Documentos / Equipe de armazenamento | O runbook do operador é o runbook do operador. |

## متطلبات الاختبار e CI

- Verifique e verifique o número de telefone (`sorafs_manifest`).
- اختبارات تكامل تحاكي: إعلان → أمر نسخ → medição → pagamento.
- fluxo de trabalho no CI que está sendo executado no sistema de gerenciamento de dados (ou seja, `ci/check_sorafs_fixtures.sh`).
- Altere a API do registro (entre 10k de tamanho e 100k de tamanho).

## التليمترية ولوحات المعلومات

- Painel de controle:
  - السعة المعلنة مقابل المستخدمة لكل مزود.
  - backlog é uma tarefa que pode ser executada.
  - Melhorar SLA (tempo de atividade, menos PoR).
  - تراكم الرسوم والغرامات لكل حقبة.
- Alertas:
  - مزود أقل من الحد الأدنى للسعة الملتزم بها.
  - Não use o SLA.
  - إخفاقات خط أنابيب الميترينغ.

## مخرجات التوثيق

- دليل المشغل لإعلان السعة وتجديد الالتزامات ومراقبة الاستخدام.
- دليل الحوكمة للموافقة على الإعلانات وإصدار الأوامر ومعالجة النزاعات.
- A API da API não está disponível e não está disponível.
- أسئلة شائعة للـ marketplace موجهة للمطورين.

## قائمة تحقق جاهزية GA

بند خارطة الطريق **SF-2c** يبوّب إطلاق الإنتاج على أدلة ملموسة عبر المحاسبة ومعالجة النزاعات والالتحاق. Certifique-se de que o produto esteja funcionando corretamente.

### Contabilidade noturna e reconciliação XOR
- صدّر snapshot حالة السعة وملف XOR ledger للفترة نفسها, ثم شغّل:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  ينهي المساعد التنفيذ بكود غير صفري عند وجود تسويات, أو غرامات مفقودة/زائدة, ويصدر Selecione o arquivo de texto Prometheus.
- Modelo `SoraFSCapacityReconciliationMismatch` (ضمن `dashboards/alerts/sorafs_capacity_rules.yml`)
  يطلق عندما تشير مقاييس reconciliação إلى فجوات؛ لوحات المعلومات موجودة تحت
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Insira JSON e hashes em `docs/examples/sorafs_capacity_marketplace_validation/`
  بجانب pacotes de governança.

### Disputa e redução de evidências
- قدّم disputas عبر `sorafs_manifest_stub capacity dispute` (اختبارات:
  `cargo test -p sorafs_car --test capacity_cli`) reduz as cargas úteis.
- شغّل `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` وحزم العقوبات
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) Não há disputas e cortes.
- اتبع `docs/source/sorafs/dispute_revocation_runbook.md` لالتقاط الأدلة والتصعيد؛ اربط موافقات greve em تقرير التحقق.

### Testes de fumaça de integração e saída do provedor
- أعد توليد artefatos للإعلان/التليمترية باستخدام `sorafs_manifest_stub capacity ...` وأعد تشغيل اختبارات CLI قبل الإرسال (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Para substituir Torii (`/v2/sorafs/capacity/declare`) ou `/v2/sorafs/capacity/state`, use Grafana. Verifique o valor do arquivo em `docs/source/sorafs/capacity_onboarding_runbook.md`.
- أرشِف artefatos الموقعة ومخرجات reconciliação داخل `docs/examples/sorafs_capacity_marketplace_validation/`.

## الاعتماديات والتسلسل1. إكمال SF-2b (política de admissão) - يعتمد marketplace على مزودين مدققين.
2. Pressione o botão + botão (referência) para Torii.
3. A tubulação de medição deve ser ajustada para o medidor.
4. Método de medição: Use o método de medição para medir na preparação.

يجب تتبع التقدم في roadmap مع الإحالات إلى هذا المستند. Roteiro de حدّث بمجرد وصول كل قسم رئيسي (المخططات, avião de controle, medição, medição, معالجة النزاعات) e recurso completo.
---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: Pin Registry em SoraFS
sidebar_label: خطة Pin Registry
description: Você pode usar o SF-4 para registrar o registro e o Torii e o registro.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/pin_registry_plan.md`. Não se preocupe, você pode fazer isso sem parar.
:::

# خطة تنفيذ Pin Registry em SoraFS (SF-4)

يقدّم SF-4 عقد Pin Registry والخدمات المساندة التي تخزن التزامات manifesto,
Crie um pino, uma API e uma API para Torii e uma API.
يوسّع هذا المستند خطة التحقق بمهام تنفيذية ملموسة تغطي المنطق on-chain,
وخدمات المضيف, والـ fixtures, والمتطلبات التشغيلية.

## النطاق

1. **Registro de registro**: Norito para manifestos e aliases e aliases
   وعصور الاحتفاظ وبيانات الحوكمة الوصفية.
2. **تنفيذ العقد**: عمليات CRUD حتمية لدورة حياة pin (`ReplicationOrder`, `Precommit`,
   `Completion`, despejo).
3. **واجهة الخدمة**: نقاط نهاية gRPC/REST مدعومة بالـ registro Torii وSDKs,
   وتشمل الترقيم والاتستاشن.
4. **التولنغ والـ fixtures**: CLI مساعدات ومتجهات اختبار ووثائق تحافظ على تزامن
   manifestos e aliases e envelopes الخاصة بالحوكمة.
5. **التليمتري وعمليات التشغيل**: Registro de aplicativos e runbooks para registro.

## نموذج البيانات

### Número de telefone (Norito)

| البنية | الوصف | الحقول |
|----|-------|----|
| `PinRecordV1` | Este é o manifesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | ربط alias -> CID é o manifesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Ative o manifesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | اقرار المزوّد. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة سياسة الحوكمة. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Nome do produto: Use `crates/sorafs_manifest/src/pin_registry.rs` para Norito com ferrugem
ومساعدات التحقق التي تدعم هذه السجلات. Ferramentas de ferramentas para manifesto
(pesquisa para registro de chunker e controle de política de pin) Torii e CLI
Existem invariantes.

Nome:
- Instale Norito em `crates/sorafs_manifest/src/pin_registry.rs`.
- A solução (Rust + SDKs está disponível) é usada como Norito.
- تحديث الوثائق (`sorafs_architecture_rfc.md`) بعد تثبيت المخططات.

## تنفيذ العقد| المهمة | Produtos/serviços | الملاحظات |
|--------|------------------|-----------|
| Você pode usar o registro (sled/sqlite/off-chain) e o contrato inteligente. | Equipe Core de Infra/Contrato Inteligente | O hashing é um recurso que não funciona. |
| Nome do modelo: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infra principal | Use `ManifestValidator` no lugar certo. ربط alias يمر الان عبر `RegisterPinManifest` (DTO من Torii) بينما يبقى `bind_alias` المخصص مخططا لتحديثات Não. |
| انتقالات الحالة: فرض التعاقب (manifesto A -> B), عصور الاحتفاظ, تفرد alias. | Conselho de Governança / Core Infra | تفرد alias وحدود الاحتفاظ وفحوصات اعتماد/سحب السلف موجودة em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; كشف التعاقب متعدد القفزات ودفاتر التكرار ما زالت مفتوحة. |
| Solução de problemas: Instale `ManifestPolicyV1` em config/حالة الحوكمة؛ Você pode fazer isso com mais frequência. | Conselho de Governança | Clique em CLI para obter mais informações. |
| Solução de problemas: Use o código Norito para (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidade | تعريف مخطط الاحداث + التسجيل. |

Informações:
- اختبارات وحدة لكل نقطة دخول (ايجابي + رفض).
- اختبارات خصائص لسلسلة التعاقب (بدون دورات, عصور متصاعدة).
- Fuzz للتحقق عبر توليد manifesta عشوائية (مقيدة).

## Método de configuração (Torii/SDK)

| المكون | المهمة | Produtos/serviços |
|--------|--------|------------------|
| Modelo Torii | كشف `/v2/sorafs/pin` (enviar), `/v2/sorafs/pin/{cid}` (pesquisa), `/v2/sorafs/aliases` (listar/vincular), `/v2/sorafs/replication` (pedidos/recebimentos). توفير ترقيم + ترشيح. | Rede TL / Core Infra |
| الاتستاشن | تضمين ارتفاع/هاش registro no site Use Norito para configurar SDKs. | Infra principal |
| CLI | `sorafs_manifest_stub` e CLI são `sorafs_pin` como `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Ferramentaria |
| SDK | Ligações de segurança para (Rust/Go/TS) com Norito; Faça isso com cuidado. | Equipes SDK |

العمليات:
- Coloque cache/ETag em vez de GET.
- Limite de taxa / autenticação بما يتوافق مع سياسات Torii.

## Luminárias e CI

- Os fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` são usados para manifestar/alias/pedido por meio de `cargo run -p iroha_core --example gen_pin_snapshot`.
- خطوة CI: `ci/check_sorafs_fixtures.sh` تعيد توليد اللقطة وتفشل عند وجود اختلافات, لتحافظ على تماهي fixtures الخاصة بـ CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تغطي المسار السعيد مع رفض alias المكرر, وحمايات اعتماد/احتفاظ alias، وhandles غير متطابقة لـ chunker, والتحقق من عدد النسخ, وفشل حمايات التعاقب (مؤشرات مجهولة/موافَق عليها مسبقا/مسحوبة/ذاتية الاشارة), Verifique se o `register_manifest_rejects_*` está danificado.
- اختبارات الوحدة تغطي الان التحقق من alias وحمايات الاحتفاظ وفحوصات الخلف في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; كشف التعاقب متعدد القفزات عند وصول آلة الحالات.
- JSON é usado para definir o valor do arquivo.

## التليمتري e والرصد

Nome (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- A solução de problemas (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) é usada de ponta a ponta.Palavras:
- Verifique o Norito para remover o problema (referência).

Palavras:
- اوامر تكرار معلقة تتجاوز SLA.
- انتهاء صلاحية alias اقل من العتبة.
- مخالفات الاحتفاظ (manifesto لم يجدد قبل الانتهاء).

Leia mais:
- ملف Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` يتتبع اجمالي دورة حياة manifestos, تغطية aliases, تشبع backlog, نسبة SLA, Aumentar a latência e a folga é importante para aumentar a latência.

## Runbooks

- تحديث `docs/source/sorafs/migration_ledger.md` لتضمين تحديثات registro.
- Nome de usuário: `docs/source/sorafs/runbooks/pin_registry_ops.md` (não disponível) الخدمة.
- دليل الحوكمة: وصف معلمات السياسة وسير عمل الاعتماد ومعالجة النزاعات.
- A API da API não está disponível (ou seja, Docusaurus).

## الاعتماديات والتسلسل

1. Use o ManifestValidator (ou ManifestValidator).
2. Selecione Norito + قيم السياسة الافتراضية.
3. تنفيذ العقد + الخدمة وربط التليمتري.
4. Ajuste os dispositivos elétricos e instale-os.
5. Execute o runbooks/runbooks e instale-os no site.

Você não pode usar o SF-4 para obter mais informações.
واجهة REST توفر الان نقاط نهاية قائمة مع اتستاشن:

- `GET /v2/sorafs/pin` e `GET /v2/sorafs/pin/{digest}` manifestam manifestos
  ربط aliases واوامر التكرار وكائن اتستاشن مشتق من هاش اخر كتلة.
- `GET /v2/sorafs/aliases` e `GET /v2/sorafs/replication` تكشفان كتالوج alias
  A máquina de lavar roupa pode ser usada e usada.

Clique em CLI para obter mais informações (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) حتى يتمكن المشغلون من اتمتة تدقيقات registro
A API é útil.
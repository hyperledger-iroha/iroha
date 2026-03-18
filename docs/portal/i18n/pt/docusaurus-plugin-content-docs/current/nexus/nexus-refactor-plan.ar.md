---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de refatoração do nexo
título: خطة اعادة هيكلة دفتر Sora Nexus
description: نسخة مطابقة لـ `docs/source/nexus_refactor_plan.md` اعمال التنظيف المرحلية لقاعدة شفرة Iroha 3.
---

:::note المصدر القانوني
Verifique o valor `docs/source/nexus_refactor_plan.md`. A primeira coisa que você pode fazer é fazer com que a máquina funcione corretamente.
:::

# خطة اعادة هيكلة دفتر Sora Nexus

Você pode registrar o Sora Nexus Ledger ("Iroha 3"). Você pode usar o genesis/WSV e Sumeragi e Sumeragi O ponteiro-ABI do ponteiro é o Norito. A maneira mais fácil de fazer isso é que você pode fazer isso sem parar. واحد ضخم.

## 0. مبادئ موجهة
- الحفاظ على سلوك حتمي عبر عتاد متنوع؛ O recurso de flags de recurso é usado com fallbacks.
- Norito é um problema. Não há jogos de ida e volta para jogos de ida e volta e jogos de ida e volta Norito.
- يمر التكوين عبر `iroha_config` (usuário -> real -> padrões). ازل alterna a opção de menu do dispositivo.
- ABI ABI V1 está disponível para download. يجب على المضيفين رفض tipos de ponteiro/syscalls غير المعروفة بشكل حتمي.
- `cargo test --workspace` e testes de ouro (`ivm`, `norito`, `integration_tests`) تبقى بوابة الاساس لكل مرحلة.

## 1. لقطة من طوبولوجيا المستودع
- `crates/iroha_core`: ممثلو Sumeragi وWSV ومحمل genesis وخطوط الانابيب (query, overlay, zk lanes) وغراء مضيف العقود الذكية.
- `crates/iroha_data_model`: O código de segurança do arquivo e o código de barras estão bloqueados.
- `crates/iroha`: instale o arquivo no CLI, no CLI e no SDK.
- `crates/iroha_cli`: A CLI está disponível para uso com APIs no `iroha`.
- `crates/ivm`: Você pode usar o Kotodama para usar o ponteiro-ABI.
- `crates/norito`: codec criado para JSON e para AoS/NCB.
- `integration_tests`: تأكيدات عبر المكونات تغطي genesis/bootstrap e Sumeragi وtriggers وpagination وغيرها.
- تشرح اهداف Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), لكن التنفيذ مجزأ ومتهالك جزئيا مقارنة بالشفرة.

## 2. ركائز اعادة الهيكلة والمراحل

### المرحلة A - الاساسات والمراقبة
1. **Telemetria WSV + Instantâneos**
   - A API de API é definida em `state` (trait `WorldStateSnapshot`) e Sumeragi e CLI.
   - Use `scripts/iroha_state_dump.sh` para obter instantâneos de `iroha state dump --format norito`.
2. **Genesis/Bootstrap**
   - اعادة هيكلة ادخال genesis ليمر عبر مسار واحد يعتمد Norito (`iroha_core::genesis`).
   - اضافة تغطية تكامل/ارتداد تعيد تشغيل genesis مع اول كتلة وتؤكد تطابق جذور WSV بين arm64/x86_64 (recomendado em `integration_tests/tests/genesis_replay_determinism.rs`).
3. **اختبارات ثبات عبر الحزم**
   - Use `integration_tests/tests/genesis_json.rs` para conectar WSV e pipeline e ABI para chicote e chicote.
   - Use o scaffold `cargo xtask check-shape` para evitar o desvio do esquema (o backlog do DevEx é instalado no `scripts/xtask/README.md`).

### المرحلة B - WSV وسطح الاستعلام
1. **معاملات تخزين الحالة**
   - Use `state/storage_transactions.rs` para fazer o commit e o commit.
   - اختبارات الوحدة تتحقق الان من ان تعديلات ativos/mundo/gatilhos تتراجع عند الفشل.
2. **اعادة هيكلة نموذج الاستعلام**
   - Não há paginação/cursor no site `crates/iroha_core/src/query/`. Você pode usar Norito em `iroha_data_model`.
   - Faça consultas de snapshot para gatilhos, ativos e funções para obter informações (referentes a `crates/iroha_core/tests/snapshot_iterable.rs`).
3. **اتساق اللقطات**
   - Use CLI `iroha ledger query` para usar o Sumeragi/fetchers.
   - A regressão de regressão é realizada na CLI usando `tests/cli/state_snapshot.rs` (recurso fechado).

### Nome C - خط انابيب Sumeragi
1. **الطوبولوجيا وادارة الحقبات**
   - O `EpochRosterProvider` do trait pode ser usado para capturar snapshots da aposta no WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` é um dispositivo que pode ser usado em bancadas/testes.
2. **تبسيط تدفق الاجماع**
   - Nome do modelo `crates/iroha_core/src/sumeragi/*`: `pacemaker`, `aggregation`, `availability`, `witness`. مشتركة تحت `consensus`.
   - استبدال تمرير الرسائل العشوائي بأغلفة Norito نوعية وادخال testes de propriedade para تغيير العرض (متابعة في backlog (Referência Sumeragi).
3. **Faixa/prova**
   - As provas de pista são dadas e controladas pelo RBC.
   - Verifique o `integration_tests/tests/extra_functional/seven_peer_consistency.rs` de ponta a ponta no RBC.### المرحلة D - العقود الذكية ومضيفات ponteiro-ABI
1. **تدقيق حدود المضيف**
   - É um tipo de ponteiro (`ivm::pointer_abi`) e um tipo de ponteiro (`iroha_core::smartcontracts::ivm::host`).
   - توقعات جدول المؤشرات وربط manifest المضيف مغطاة في `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` و`ivm_host_mapping.rs` التي تمارس خرائط TLV dourado.
2. **Gatilhos de sandbox de segurança**
   - Os gatilhos de segurança do `TriggerExecutor` podem ser usados ​​para ativar e desativar o recurso.
   - A regressão de chamada/tempo de regressão é definida como `crates/iroha_core/tests/trigger_failure.rs`.
3. **Configuração CLI **
   - ضمان ان عمليات CLI (`audit`, `gov`, `sumeragi`, `ivm`) تعتمد على وظائف العميل A chave para `iroha` é o problema.
   - Crie snapshots JSON na CLI usando `tests/cli/json_snapshot.rs`; Você pode usar o formato JSON do arquivo.

### Código E - Codec Norito
1. **سجل المخططات**
   - Use o Norito para `crates/norito/src/schema/` para remover o problema.
   - تمت اضافة doc tests تتحقق من ترميز payloads نموذجية (`norito::schema::SamplePayload`).
2. **Acessórios dourados**
   - Use luminárias douradas em `crates/norito/tests/*` para obter informações sobre o WSV.
   - `scripts/norito_regen.sh` é um JSON dourado que é definido como `norito_regen_goldens`.
3. **IVM/Norito**
   - O nome do manifesto Kotodama é o ponteiro de metadados ABI.
   - `crates/ivm/tests/manifest_roundtrip.rs` é um código de codificação/decodificação Norito.

## 3. قضايا مشتركة
- **استراتيجية الاختبارات**: كل مرحلة ترقى testes unitários -> testes de caixa -> testes de integração. الاختبارات الفاشلة تلتقط الانتكاسات الحالية؛ Você pode fazer isso sem problemas.
- **التوثيق**: بعد كل مرحلة, حدث `status.md` وادفع العناصر المفتوحة الى `roadmap.md` مع حذف المهام المكتملة.
- **معايير الاداء**: Bancos de bancos em `iroha_core` و`ivm` و`norito`; Você pode fazer isso com regressões e regressões.
- **Sinalizadores de recursos**: الحفاظ على alterna على مستوى crate فقط للواجهات الخلفية التي تتطلب toolchains خارجية (`cuda`, `zk-verify-batch`). O cartão SIMD da CPU não é compatível com a CPU e o computador. E os substitutos podem ser usados ​​​​para isso.

## 4. الاجراءات الفورية
- Andaime للمرحلة A (traço instantâneo + ربط telemetria) - راجع المهام القابلة للتنفيذ في تحديثات roteiro.
- Verifique o valor de `sumeragi` e `state` e `ivm` para obter o seguinte valor:
  - `sumeragi`: permissões para código morto تحمي بث اثباتات view-change وحالة replay VRF وتصدير telemetria EMA. Verifique se o portão é fechado e o portão C e a pista/prova.
  - `state`: تنظيف `Cell` ومسار telemetria ينتقلان الى مسار telemetria WSV في المرحلة A, بينما تنضم ملاحظات SoA/parallel-apply no backlog do pipeline no processo C.
  - `ivm`: عرض alterna CUDA والتحقق من envelopes وتغطية Halo2/Metal تتطابق مع اعمال حدود المضيف في المرحلة D ومع موضوع تسريع GPU العرضي؛ Os kernels mais importantes estão no backlog da GPU.
- تحضير RFC عبر الفرق يلخص هذه الخطة من اجل sign-off قبل ادخال تغييرات شيفرة واسعة.

## 5. اسئلة مفتوحة
- Você pode usar o RBC para obter P1, mas não usar o Nexus; يتطلب قرار اصحاب المصلحة.
- Esta é a capacidade de composição do DS no P1 e a capacidade de composição das provas de pista.
- ما الموقع القانوني لمعاملات ML-DSA-87؟ Posição: caixa جديد `crates/fastpq_isi` (قيد الانشاء).

---

_Atualizado: 12/09/2025_
---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visão geral do nexo
título: نظرة عامة على Sora Nexus
description: ملخص عالي المستوى لمعمارية Iroha 3 (Sora Nexus) مع مؤشرات إلى وثائق المستودع الأحادي المعيارية.
---

Nexus (Iroha 3) Iroha 2 بتنفيذ متعدد المسارات, ومساحات بيانات محددة Você pode fazer isso com o SDK. Verifique o valor do código `docs/source/nexus_overview.md` no site da empresa. كيفية ترابط أجزاء المعمارية بسرعة.

## خطوط الإصدارات

- **Iroha 2** - Verifique se o dispositivo está conectado ou não.
- **Iroha 3 / Sora Nexus** - الشبكة العامة متعددة المسارات حيث يسجل المشغلون مساحات البيانات (DS) ويرثون أدوات مشتركة للحوكمة والتسوية والملاحظة.
- كلا الخطين يُبنيان من نفس مساحة العمل (IVM + conjunto de ferramentas Kotodama), لذا تبقى إصلاحات SDK Os dispositivos ABI e luminárias Norito são instalados. Você pode usar o `iroha3-<version>-<os>.tar.zst` para obter o Nexus; Use `docs/source/sora_nexus_operator_onboarding.md` para remover o problema.

## اللبنات الأساسية

| المكون | الملخص | روابط البوابة |
|-----------|---------|-------------|
| مساحة البيانات (DS) | Não deixe de usar/desenvolver um produto que não seja adequado para você e para quem você não sabe. وفئة الخصوصية وسياسة الرسوم + DA. | راجع [Nexus spec](./nexus-spec) é uma opção. |
| Pista | Verifique se o NPoS está funcionando corretamente. Verifique a pista `default_public` e `public_custom` e `private_permissioned` e `hybrid_confidential`. | يلتقط [نموذج Lane](./nexus-lane-model). |
| خطة الانتقال | معرفات placeholder ومراحل توجيه وتعبئة بملفين تتبع كيف تتطور عمليات النشر أحادية المسار إلى Nexus. | Verifique se o produto está funcionando corretamente (./nexus-transition-notes). |
| Diretório Espacial | Não use o DS e o DS. يقوم المشغلون بمطابقة إدخالات الكتالوج مع هذا الدليل قبل الانضمام. | A solução de problemas é `docs/source/project_tracker/nexus_config_deltas/`. |
| كتالوج المسارات | قسم الإعداد `[nexus]` يربط معرّفات Lane بالأسماء المستعارة وسياسات التوجيه وعتبات DA. Use `irohad --sora --config … --trace-config` para remover o problema. | Use `docs/source/sora_nexus_operator_onboarding.md` para remover o problema. |
| موجه التسوية | Você pode usar o XOR no banco de dados CBDC para obter mais informações. | Use `docs/source/cbdc_lane_playbook.md` para remover o problema e o problema. |
| Países/SLOs | A solução de problemas `dashboards/grafana/nexus_*.json` é a solução para o problema e o uso do produto. وعمق طابور الحوكمة. | يوضح [خطة معالجة القياس](./nexus-telemetry-remediation) اللوحات والتنبيهات وأدلة التدقيق. |

## لقطة الإطلاق| المرحلة | التركيز | معايير الخروج |
|-------|-------|---------------|
| N0 - بيتا مغلقة | Registrador يديره المجلس (`.sora`), انضمام يدوي للمشغلين, كتالوج مسارات ثابت. | بيانات DS موقعة + عمليات تسليم حوكمة مجربة. |
| N1 - إطلاق عام | Se você usar `.nexus`, o registrador deve usar o XOR. | Você pode usar o resolvedor/gateway, o que significa que você pode usar o resolvedor/gateway. |
| N2 - توسع | O `.dao` e a API do sistema operacional são configurados para serem usados. | Você pode usar o produto para obter mais informações sobre o produto. |
| Versão NX-12/13/14 | Não há nada que você possa fazer e que você possa usar para obter mais informações. | نشر [Visão geral Nexus](./nexus-overview) + [Operações Nexus](./nexus-operations), توصيل اللوحات, دمج محرك السياسات. |

## مسؤوليات المشغلين

1. **نظافة الإعداد** - أبقِ `config/config.toml` متزامنا مع كتالوج المسارات ومساحات البيانات منشور؛ Verifique se o `--trace-config` está funcionando corretamente.
2. **تتبع البيانات** - طابق إدخالات الكتالوج مع أحدث حزمة Space Directory قبل الانضمام, أو ترقية العقد.
3. **تغطية القياس** - Instale `nexus_lanes.json` e `nexus_settlement.json` e SDK no SDK. Você pode usar o PagerDuty para obter mais informações e obter mais informações sobre o PagerDuty.
4. **الإبلاغ عن الحوادث** - اتبع مصفوفة الشدة في [Nexus operações](./nexus-operations) e تقارير RCA خلال خمسة أيام عمل.
5. **الجاهزية للحوكمة** - احضر تصويتات مجلس Nexus التي تؤثر على مساراتك وتدرّب على Verifique o valor da chave (ou seja, `docs/source/project_tracker/nexus_config_deltas/`).

## انظر أيضا

- Nome do código: `docs/source/nexus_overview.md`
- Nome de usuário: [./nexus-spec](./nexus-spec)
- Configuração do modelo: [./nexus-lane-model](./nexus-lane-model)
- خطة الانتقال: [./nexus-transition-notes](./nexus-transition-notes)
- خطة معالجة القياس: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Configuração do sistema: [./nexus-operations](./nexus-operations)
- Nome de usuário: `docs/source/sora_nexus_operator_onboarding.md`
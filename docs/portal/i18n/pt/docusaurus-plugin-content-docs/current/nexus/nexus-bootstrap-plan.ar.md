---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano nexus-bootstrap
título: Sora Nexus e Sora Nexus
description: Você pode usar o Nexus para usar o SoraFS e SoraNet.
---

:::note المصدر القانوني
Verifique o valor `docs/source/soranexus_bootstrap_plan.md`. Verifique se o produto está funcionando corretamente.
:::

# خطة اقلاع ومراقبة Sora Nexus

## الاهداف
- تشغيل شبكة المدققين/المراقبين الاساسية لـ Sora Nexus مع مفاتيح الحوكمة وواجهات Torii ومراقبة اجماع.
- Torii, SoraFS/SoraNet المتراكبة.
- Fluxos de trabalho de fluxo de trabalho para CI/CD e/ou serviços de gerenciamento de fluxo de trabalho.

## المتطلبات المسبقة
- Você pode usar o multisig no HSM e no Vault.
- Você pode usar o Kubernetes e o bare-metal em uma plataforma/serviço.
- A inicialização do bootstrap (`configs/nexus/bootstrap/*.toml`) é necessária.

## بيئات الشبكة
- Verifique o valor do Nexus para obter mais informações:
- **Sora Nexus (mainnet)** - بادئة شبكة انتاج `nexus` تستضيف الحوكمة القانونية وخدمات Código SoraFS/SoraNet (ID de cadeia `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - بادئة شبكة staging `testus` تعكس تكوين mainnet لاختبارات التكامل والتحقق قبل الاصدار (cadeia UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- الحفاظ على ملفات genesis ومفاتيح حوكمة وبصمات بنية تحتية منفصلة لكل بيئة. Teste o Testus para obter o SoraFS/SoraNet para o Nexus.
- يجب ان تنشر خطوط CI/CD الى Testus اولا وتنفذ testes de fumaça تلقائية وتطلب ترقية يدوية الى Nexus بعد نجاح الفحوصات.
- Você pode usar `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet) e `config.toml` O `genesis.json` é o mesmo do Torii.

## الخطوة 1 - مراجعة التكوين
1. تدقيق التوثيق الموجود:
   - `docs/source/nexus/architecture.md` (referência Torii).
   - `docs/source/nexus/deployment_checklist.md` (não disponível).
   - `docs/source/nexus/governance_keys.md` (não disponível).
2. O nome do genesis (`configs/nexus/genesis/*.json`) é a escalação da lista e o staking.
3. Como fazer o download:
   - حجم لجنة الاجماع e quórum.
   - فاصل الكتل / عتبات finalidade.
   - Use Torii e TLS.

## Passo 2 - Inicialização do bootstrap
1. تجهيز عقد المدققين:
   - A chave `irohad` (recomendada) pode ser usada para evitar problemas.
   - ضمان ان قواعد الجدار الناري تسمح بحركة مرور الاجماع e Torii بين العقد.
2. Use Torii (REST/WebSocket) para configurar o TLS.
3. Deixe a máquina de lavar (قراءة فقط) sem fio.
4. Execute o bootstrap (`scripts/nexus_bootstrap.sh`) para genesis e genesis.
5. Testes de fumaça:
   - A chave de segurança é Torii (`iroha_cli tx submit`).
   - التحقق من انتاج/نهائية الكتل عبر التليمتري.
   - فحص تكرار السجل بين المدققين/المراقبين.

## الخطوة 3 - الحوكمة وادارة المفاتيح
1. تحميل تكوين multisig للمجلس; Você pode fazer isso com uma chave de fenda e uma chave de fenda.
2. تخزين مفاتيح الاجماع/اللجنة بشكل امن; Certifique-se de que você está usando um computador portátil e sem fio.
3. Faça o download do arquivo مفاتيح الطوارئ (`docs/source/nexus/key_rotation.md`) e use o runbook.

## Passo 4 - Configuração CI/CD
1. Como fazer isso:
   - Use o validator/Torii (GitHub Actions e GitLab CI).
   - التحقق التلقائي من التكوين (lint لـ genesis, تحقق من التواقيع).
   - خطوط نشر (Helm/Kustomize) لعناقيد staging والانتاج.
2. Faça testes de fumaça em CI (não há testes de fumaça e testes de fumaça).
3. Execute rollback em runbooks.

## الخطوة 5 - المراقبة والتنبيهات
1. Selecione a opção (Prometheus + Grafana + Alertmanager) para isso.
2. جمع المقاييس الاساسية:
  -`nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - O modelo Loki/ELK é Torii e também.
3. Métodos de pagamento:
   - صحة الاجماع (ارتفاع الكتلة, النهائية, حالة pares).
   - Verifique a API Torii e a API Torii.
   - معاملات الحوكمة وحالة المقترحات.
4. Passos:
   - توقف انتاج الكتل (>2 فواصل كتل).
   - هبوط عدد peers تحت quorum.
   - Use a chave Torii.
   - تراكم طابور مقترحات الحوكمة.

## الخطوة 6 - التحقق والتسليم
1. Avaliação de ponta a ponta:
   - ارسال مقترح حوكمة (مثل تغيير معلمة).
   - تمريره عبر موافقة المجلس لضمان عمل خط الحوكمة.
   - تنفيذ diff لحالة السجل لضمان الاتساق.
2. Use o runbook para executar (failover, escalonamento).
3. Instale o SoraFS/SoraNet; Você não pode pegar carona nas costas do Nexus.## قائمة تنفيذ
- [ ] تدقيق genesis/configuração مكتمل.
- [ ] نشر عقد المدققين والمراقبين مع اجماع سليم.
- [ ] تحميل مفاتيح الحوكمة واختبار المقترح.
- [ ] خطوط CI/CD تعمل (construir + implantar + testes de fumaça).
- [ ] لوحات المراقبة تعمل مع التنبيهات.
- [ ] تسليم توثيق handoff para downstream.
---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "خطة طرح وتوافق anúncios لمزودي SoraFS"
---

> مقتبس من [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة طرح وتوافق anúncios لمزودي SoraFS

تنسق هذه الخطة الانتقال من adverts المسموحة لمزودي التخزين إلى سطح `ProviderAdvertV1`
Cada pedaço é cortado e cortado em pedaços. وهي تركز على
ثلاثة مخرجات رئيسية:

- **دليل المشغل.** خطوات يجب على مزودي التخزين إكمالها قبل تفعيل كل gate.
- **تغطية التليمترية.** Recursos de observação e operações
  للتأكد من أن الشبكة تقبل anúncios المتوافقة فقط.
- **الجدول الزمني للتوافق.** تواريخ واضحة لرفض envelopes القديمة حتى تتمكن
  O SDK e as ferramentas estão disponíveis.

A melhor opção para SF-2b/2c é
[Você pode usar SoraFS](./migration-roadmap) para obter mais informações
[política de admissão do provedor](./provider-admission-policy) مطبقة بالفعل.

## الجدول الزمني للمراحل| المرحلة | النافذة (الهدف) | السلوك | إجراءات المشغل | تركيز الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 - Nome de usuário** | حتى **31/03/2025** | O Torii é um dos anúncios do `ProviderAdvertV1`. Para evitar a ingestão, você pode usar anúncios `chunk_range_fetch` ou `profile_aliases`. | - إعادة توليد adverts عبر pipeline نشر fornecedor advert (ProviderAdvertV1 + envelope de governança) مع ضمان `profile_id=sorafs.sf1@1.0.0` و`profile_aliases` قياسية و`signature_strict=true`. - تشغيل اختبارات `sorafs_fetch` محليا؛ يجب capacidades de triagem تحذيرات غير المعروفة. | Use o Grafana para obter informações sobre o produto. Não. |
| **R1 - بوابة التحذير** | **01/04/2025 → 15/05/2025** | Torii é usado para anúncios, mas também para `torii_sorafs_admission_total{result="warn"}`, capacidade de carga útil `chunk_range_fetch` e capacidades O modelo é `allow_unknown_capabilities=true`. As ferramentas CLI não são usadas para lidar com o problema. | - Anúncios de teste em testes e produção em cargas úteis `CapabilityType::ChunkRangeFetch` e testes de GREASE em `allow_unknown_capabilities=true`. - توثيق الاستعلامات الجديدة للتليمترية em runbooks. | Painéis de controle إلى دوران de plantão; Verifique o valor do produto `warn` com 5% de desconto em 15 dias. |
| **R2 - Aplicação** | **16/05/2025 → 30/06/2025** | يرفض Torii adverts التي تفتقد envelopes الحوكمة أو الـ handle القياسي للملف الشخصي أو capacidade `chunk_range_fetch`. Não há alças no `namespace-name`. Capacidades adicionais غير المعروفة دون GREASE opt-in بسبب `reason="unknown_capability"`. | - Envelopes para todos os tipos de envelopes, `torii.sorafs.admission_envelopes_dir` e anúncios, sem anúncios. - التحقق من أن SDKs تصدر handles قياسية فقط مع aliases اختيارية للتوافق العكسي. | Gere alertas de pager: `torii_sorafs_admission_total{result="reject"}` > 0 para 5 dias após a instalação. Verifique se há algum problema e como isso pode ser feito. |
| **R3 - إيقاف القديمة** | **Em 2025-07-01** | A descoberta do Discovery tem anúncios no site `signature_strict=true` ou `profile_aliases`. O cache de descoberta Torii é definido como o prazo final do prazo. | - جدولة نافذة descomission النهائية لمكدسات المزودين القديمة. - A graxa GREASE `--allow-unknown` é usada para perfurar brocas e perfurações. - تحديث playbooks للحوادث لاعتبار مخرجات تحذير `sorafs_fetch` مانعا قبل الإصدارات. | تشديد التنبيهات: No `warn` تنبه de plantão. O JSON de descoberta pode ser usado para definir capacidades. |

## قائمة تحقق المشغل1. **جرد anúncios.** احصر كل anúncio منشور وسجل:
   - مسار الـ envelope governamental (`defaults/nexus/sorafs_admission/...` أو ما يعادله في الإنتاج).
   - `profile_id` e `profile_aliases` para anúncio.
   - Recursos de recursos (definidos como `torii_gateway` e `chunk_range_fetch`).
   - Por `allow_unknown_capabilities` (é possível que os TLVs sejam fornecidos pelo fornecedor).
2. **إعادة التوليد باستخدام ferramental.**
   - أعد بناء payload عبر ناشر fornecedor anúncio, مع التأكد من:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` ou `max_span`
     - `allow_unknown_capabilities=<true|false>` e TLVs com GREASE
   - تحقق عبر `/v2/sorafs/providers` e `sorafs_fetch`; يجب triagem تحذيرات
     capacidades غير المعروفة.
3. **التحقق من جاهزية multi-fonte.**
   - Nome `sorafs_fetch` ou `--provider-advert=<path>`; يفشل CLI الآن عندما
     O `chunk_range_fetch` é um recurso que pode ser usado para aumentar os recursos.
     O JSON é definido como algo que não funciona.
4. **تجهيز التجديدات.**
   - Envelopes de envelopes de `ProviderAdmissionRenewalV1` por 30 dias por mês
     aplicação no gateway (R2). يجب أن تحافظ التجديدات على الـ handle القياسي
     Capacidades de ومجموعة; Não há participação, endpoints e metadados.
5. **التواصل مع الفرق المعتمدة.**
   - Não use o SDK para exibir anúncios.
   - يعلن DevRel كل انتقال مرحلة؛ Crie painéis de controle e painéis de controle.
6. **Painéis de controle e painéis.**
   - Implementar Grafana e implementar **SoraFS / Implementação de provedor** com UID
     `sorafs-provider-admission`.
   - تأكد من أن قواعد التنبيه تشير إلى قناة `sorafs-advert-rollout` المشتركة
     Na encenação e produção.

## التليمترية ولوحات المعلومات

O código de barras do dispositivo `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — يعد القبول والرفض ونتائج التحذير.
  Verifique o valor `missing_envelope`, `unknown_capability`, `stale` e `policy_violation`.

Escolha Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
قم باستيراد الملف إلى مستودع dashboards المشترك (`observability/dashboards`) مع
Verifique o UID do site no site.

Grafana **SoraFS / Implementação de Provedor** com UID ثابت
`sorafs-provider-admission`. قواعد التنبيه `sorafs-admission-warn` (aviso) e
`sorafs-admission-reject` (crítico)
`sorafs-advert-rollout`; عدل جهة الاتصال إذا تغيّرت قائمة الوجهات بدلا من تحرير
JSON é definido.

O código Grafana é o seguinte:

| الوحة | الاستعلام | الملاحظات |
|-------|-------|-------|
| **Taxa de resultado de admissão** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط مكدس لعرض aceitar vs avisar vs rejeitar. É necessário avisar > 0,05 * total (aviso) ou rejeitar > 0 (crítico). |
| **Taxa de alerta** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Você pode usar um pager de alta qualidade (5% em 15 dias). |
| **Motivos de rejeição** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Como fazer a triagem no runbook; أرفق روابط لخطوات التخفيف. |
| **Atualizar dívida** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | تشير إلى مزودين فاتتهم مهلة التجديد؛ Este é o cache de descoberta. |

Artefatos da CLI nos painéis de controle são:- `sorafs_fetch --provider-metrics-out` é compatível com `failures` e `successes` e
  `disabled` para a maioria. Criar painéis ad-hoc para simulações
  orquestrador قبل تبديل مزودي الإنتاج.
- É `chunk_retry_rate` e `provider_failure_rate` no formato JSON.
  estrangulamento ou estrangulamento de cargas úteis obsoletas.

### تخطيط لوحة Grafana

Observabilidade لوحة مخصصة — **SoraFS Admissão do Provedor
Implementação** (`sorafs-provider-admission`) — ضمن **SoraFS / Implementação do provedor**
مع معرفات اللوحات القياسية التالية:

- Painel 1 — *Taxa de resultado de admissão* (área empilhada, وحدة "ops/min").
- Painel 2 — *Taxa de advertência* (série única)، مع التعبير
  `sum(taxa(torii_sorafs_admission_total{result="warn"}[5m])) /
   soma(taxa(torii_sorafs_admission_total[5m]))`.
- Painel 3 — *Motivos de rejeição* (série temporal مجمعة حسب `reason`), مرتبة حسب
  `rate(...[5m])`.
- Painel 4 — *Atualizar dívida* (estatísticas), يعكس الاستعلام أعلاه ومُعلق بمهل atualizar
  O registro do livro de migração.

Esqueleto JSON (ou melhor) JSON que está disponível para download
`observability/dashboards/sorafs_provider_admission.json`, qual é o valor do UID
البيانات؛ معرفات اللوحات وقواعد التنبيه يُرجع إليها runbooks أدناه, لذا تجنب
Você pode fazer isso sem problemas.

للتسهيل, يوفر المستودع تعريفا مرجعيا للوحة في
`docs/source/grafana_sorafs_admission.json`; Use o Grafana para obter mais informações
كنقطة انطلاق للاختبارات المحلية.

### قواعد تنبيه Prometheus

أضف مجموعة القواعد التالية إلى
`observability/prometheus/sorafs_admission.rules.yml` (أنشئ الملف إن كانت هذه
Você pode usar o SoraFS) e o Prometheus. Modelo `<pagerduty>`
بعلامة التوجيه الفعلية لدوام المناوبة.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Modelo `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
Verifique se o dispositivo está conectado ao `promtool check rules`.

## مصفوفة التوافق

| Anúncio de anúncio | R0 | R1 | R2 | R3 |
|-----|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` Nomes alternativos, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| capacidade de operação `chunk_range_fetch` | ⚠️ Avisar (ingestão + telemetria) | ⚠️Avisar | ❌ Rejeitar (`reason="missing_capability"`) | ❌ Rejeitar |
| Capacidade de TLVs غير معروفة دون `allow_unknown_capabilities=true` | ✅ | ⚠️ Avisar (`reason="unknown_capability"`) | ❌ Rejeitar | ❌ Rejeitar |
| `refresh_deadline` Mecanismo | ❌ Rejeitar | ❌ Rejeitar | ❌ Rejeitar | ❌ Rejeitar |
| `signature_strict=false` (dispositivos de diagnóstico) | ✅ (للتطوير فقط) | ⚠️Avisar | ⚠️Avisar | ❌ Rejeitar |

É o horário UTC. A execução da aplicação é feita no registro de migração e no registro
بدون تصويت المجلس؛ Você pode usar o livro-razão no PR.

> **Configurações de uso:** R1 padrão `result="warn"` aqui
> `torii_sorafs_admission_total`. Ingerir ingest em Torii é uma tarefa simples
> ضمن مهام تليمترية SF-2; وحتى ذلك الحين استخدم أخذ عينات من السجلات لمراقبة

## التواصل ومعالجة الحوادث- **رسالة حالة أسبوعية.** يرسل DevRel ملخصا موجزا لمقاييس القبول والتحذيرات
  والمواعيد القادمة.
- **استجابة للحوادث.** إذا انطلقت تنبيهات `reject`، يقوم on-call بما يلي:
  1. Abra o anúncio de descoberta em Torii (`/v2/sorafs/providers`).
  2. Coloque um anúncio no pipeline e envie-o para o pipeline.
     `/v2/sorafs/providers` não funciona.
  3. O anúncio do anúncio pode ser atualizado para atualizar o site.
- **تجميد التغييرات.** para definir o esquema, os recursos, o R1/R2, mas não
  Implementação de يوافق عليها فريق; Use GREASE para usar com graxa
  É usado no registro de migração.

## المراجع

- [Protocolo de nó/cliente SoraFS] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de Admissão de Provedor](./provider-admission-policy)
- [Roteiro de migração](./migration-roadmap)
- [Extensões de múltiplas fontes de anúncio do provedor] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
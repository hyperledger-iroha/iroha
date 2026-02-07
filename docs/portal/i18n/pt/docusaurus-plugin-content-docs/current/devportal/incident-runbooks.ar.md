---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# كتيبات الحوادث وتمارين rollback

## الغرض

بند خارطة الطريق **DOCS-9** يتطلب كتيبات اجرائية وخطة تدريب حتى يتمكن مشغلو البوابة من
Não há nada que você possa fazer. تغطي هذه الملاحظة ثلاثة حوادث عالية الاشارة—فشل النشر,
تدهور النسخ, وانقطاع التحليلات—وتوثق تمارين ربع سنوية تثبت ان rollback للalias
والتحقق الاصطناعي ما زالا يعملان de ponta a ponta.

### مواد ذات صلة

- [`devportal/deploy-guide`](./deploy-guide) — Sem embalagem e assinatura e alias.
- [`devportal/observability`](./observability) — tags de liberação والتحليلات والprobes المذكورة ادناه.
-`docs/source/sorafs_node_client_protocol.md`
  e [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — telemetria السجل وحدود التصعيد.
- Ajudantes `docs/portal/scripts/sorafs-pin-release.sh` e `npm run probe:*`
  المشار اليها عبر قوائم التحقق.

### القياس عن بعد والادوات المشتركة

| Sinal / Ferramenta | الغرض |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (atendido/perdido/pendente) | Não use o SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Não há backlog e triagem de triagem. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | O gateway não pode ser implantado para ser implantado. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | probes اصطناعية تقوم بعمل gate للreleases وتتحقق من rollbacks. |
| `npm run check:links` | بوابة الروابط المكسورة؛ تستخدم بعد كل mitigação. |
| `sorafs_cli manifest submit ... --alias-*` (مغلفة عبر `scripts/sorafs-pin-release.sh`) | آلية ترقية/اعادة alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | تجمع recusas/alias/TLS/replicação de telemetria. Você pode usar o PagerDuty para obter mais informações. |

## Runbook - فشل نشر او artefato سيئ

### شروط الاطلاق

- Sondas para pré-visualização/produção (`npm run probe:portal -- --expect-release=...`).
- Altere Grafana para `torii_sorafs_gateway_refusals_total` ou
  Implementação do `torii_sorafs_manifest_submit_total{status="error"}`.
- QA يدوي يلاحظ مسارات مكسورة او فشل proxy Experimente مباشرة بعد ترقية alias.

### احتواء فوري

1. **Informações:** Use `DEPLOY_FREEZE=1` no pipeline do CI (entrada do fluxo de trabalho no GitHub)
   E o trabalho do Jenkins não envolve artefatos.
2. **Artefatos adicionais:** Modelo `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, testes de teste para build são usados para rollback
   الى digere الدقيقة.
3. **اخطار اصحاب المصلحة:** storage SRE وlead لـ Docs/DevRel وضابط الحوكمة المناوب
   (é o modelo `docs.sora`).

### اجراء reversão

1. Manifesto do تحديد الاخير المعروف انه جيد (LKG). O fluxo de trabalho não está disponível
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. اعادة ربط alias بهذا manifest عبر helper الشحن:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Execute o rollback no arquivo de resumos do manifesto LKG e do manifesto.

### التحقق

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` e `sorafs_cli proof verify ...`
   (انظر دليل النشر) لتاكيد ان manifest المعاد ترقيته ما زال يطابق CAR المؤرشف.
4. `npm run probe:tryit-proxy` use o proxy Try-It para teste.

### ما بعد الحادثة

1. Verifique o pipeline do sistema de controle de fluxo de ar.
2. Selecione a opção "Lições aprendidas" em [`devportal/deploy-guide`](./deploy-guide)
   بملاحظات جديدة عند الحاجة.
3. فتح defeitos لاختبارات فشلت (sonda, verificador de link, teste).

## Runbook - تدهور النسخ

### شروط الاطلاق

- Exemplo: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` por 10 segundos.
- `torii_sorafs_replication_backlog_total > 10` 10 polegadas (não disponível)
  `pin-registry-ops.md`).
- الحوكمة تبلغ عن بطء توفر alias بعد release.

### Triagem

1. Painéis de controle [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para que você possa usá-los
   backlog pode ser encontrado em provedores de serviços e fornecedores.
2. Coloque o Torii no lugar do `sorafs_registry::submit_manifest` para obter mais informações
   submissões aqui.
3. Instale o `sorafs_cli manifest status --manifest ...` (provedor do provedor).

### Mitigação

1. اعادة اصدار manifesto بعدد نسخ اعلى (`--pin-min-replicas 7`) عبر
   `scripts/sorafs-pin-release.sh` O agendador de tarefas está disponível para provedores.
   Você pode digerir o conteúdo do site.
2. O backlog do provedor e do provedor de backlog é o agendador de replicação
   (é encontrado em `pin-registry-ops.md`) e o manifesto é fornecido por provedores e por alias.
3. عندما تكون حداثة alias اهم من parity النسخ, اعد ربط alias الى manifest دافئ
   staged (`docs-preview`) , o manifesto não suporta o backlog do SRE.

### التعافي والاغلاق

1. Insira `torii_sorafs_replication_sla_total{outcome="missed"}` para remover o problema.
2. A chave `sorafs_cli manifest status` é usada para reproduzir a réplica.
3. Post-mortem e post-mortem do backlog do banco de dados
   (fornecedores de serviços, ضبط chunker, الخ).

## Runbook - انقطاع التحليلات او القياس عن بعد

### شروط الاطلاق

- `npm run probe:portal` é um painel de controle que não pode ser usado
  `AnalyticsTracker` está disponível há 15 dias.
- Revisão de privacidade.
- `npm run probe:tryit-proxy` é compatível com `/probe/analytics`.

### الاستجابة1. Insira as entradas e o código: `DOCS_ANALYTICS_ENDPOINT` e
   `DOCS_ANALYTICS_SAMPLE_RATE` é um artefato de artefato (`build/release.json`).
2. Use `npm run probe:portal` para substituir `DOCS_ANALYTICS_ENDPOINT`
   O coletor é o staging, o rastreador e o payloads.
3. Coletores de alta qualidade, `DOCS_ANALYTICS_ENDPOINT=""` e reconstruídos
   ليقوم rastreador بعمل curto-circuito; سجل نافذة الانقطاع na linha do tempo.
4. Use `scripts/check-links.mjs` para imprimir impressões digitais em `checksums.sha256`
   (Este site está *الا* no mapa do site).
5. Coletor de dados, usando `npm run test:widgets` para testes de unidade e auxiliar de análise
   قبل اعادة النشر.

### ما بعد الحادثة

1. Selecione [`devportal/observability`](./observability) no seu coletor e coletor
   amostragem.
2. Faça o download do seu telefone e verifique o valor do produto.

## تمارين المرونة الربع سنوية

شغل كلا التمرينين خلال **اول ثلاثاء من كل ربع** (janeiro/abril/julho/outubro)
E você pode fazer isso sem parar. خزّن artefatos تحت
`artifacts/devportal/drills/<YYYYMMDD>/`.

| التمرين | Produtos | الدليل |
| ----- | ----- | -------- |
| تمرين rollback للalias | 1. اعادة تشغيل rollback الخاص بـ "Falha na implantação" باستخدام احدث manifesto انتاجي.<br/>2. اعادة الربط الى الانتاج بعد نجاح sondas.<br/>3. Verifique as sondas `portal.manifest.submit.summary.json` e as sondas em qualquer lugar. | `rollback.submit.json`, sondas de teste, e tag de liberação são. |
| تدقيق التحقق الاصطناعي | 1. Use `npm run probe:portal` e `npm run probe:tryit-proxy` para teste e teste.<br/>2. Use `npm run check:links` e `build/link-report.json`.<br/>3. Faça capturas de tela/exportações no Grafana para testar os probes. | سجلات sondas + `link-report.json` تشير الى impressão digital paramanifesto. |

صعّد التمارين الفائتة الى مدير Docs/DevRel ومراجعة حوكمة SRE, لان خارطة الطريق تتطلب
Não há nenhum rollback para alias e probes que possam ser usados.

## تنسيق PagerDuty e de plantão

- خدمة PagerDuty **Docs Portal Publishing** تملك التنبيهات المولدة من
  `dashboards/grafana/docs_portal.json`. Nome `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, e `DocsPortal/TLSExpiry` na página de paginação do Docs/DevRel
  O armazenamento SRE está disponível.
- عند النداء, ارفق `DOCS_RELEASE_TAG`, e capturas de tela para Grafana متاثرة,
  Isso significa sondar/verificar link para reduzir a mitigação.
- Melhor mitigação (reversão e reimplantação), usando `npm run probe:portal`,
  `npm run check:links`, e os snapshots são criados em Grafana.
  ضمن العتبات. Você pode usar o PagerDuty para obter mais informações.
- اذا اطلق تنبيهان في نفس الوقت (como a expiração do TLS com o backlog), تعامل مع recusas e اولا
  (publicação), sem reversão, sem TLS/backlog com armazenamento SRE ou ponte.
---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# انسیڈنٹ رن بکس اور رول بیک ڈرلز

## مقصد

روڈمیپ آئٹم **DOCS-9** قابل عمل playbooks اور ensaio پلان کا تقاضا کرتا ہے تاکہ
پورٹل آپریٹرز ڈلیوری فیلئرز سے بغیر اندازے کے ریکور کر سکیں۔ یہ نوٹ تین ہائی سگنل
انسیڈنٹس — ناکام implantações, degradação de replicação, e interrupções de análise — کو کور کرتا ہے
اور treinos trimestrais کو دستاویزی بناتا ہے جو ثابت کرتے ہیں کہ alias rollback اور validação sintética
اب بھی de ponta a ponta کام کرتے ہیں۔

### متعلقہ مواد

- [`devportal/deploy-guide`](./deploy-guide) — embalagem, assinatura, e fluxo de trabalho de promoção de alias۔
- [`devportal/observability`](./observability) — tags de liberação, análises, sondas جن کا نیچے حوالہ ہے۔
-`docs/source/sorafs_node_client_protocol.md`
  E [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetria de registro e limites de escalonamento۔
- Ajudantes `docs/portal/scripts/sorafs-pin-release.sh` e `npm run probe:*`
  Qual é a melhor opção para você

### مشترکہ telemetria e ferramentas

| Sinal / Ferramenta | مقصد |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (atendido/perdido/pendente) | travamentos de replicação e detecção de violações de SLA |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | profundidade do backlog e latência de conclusão کو triagem کے لئے quantificar کرتا ہے۔ |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | falhas no lado do gateway |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | sondas sintéticas جو libera portão کرتے ہیں اور reversões validam کرتے ہیں۔ |
| `npm run check:links` | portão de link quebrado; ہر mitigação کے بعد استعمال ہوتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعے) | mecanismo de promoção/reversão de alias۔ |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | recusas/alias/TLS/telemetria de replicação کو agregado کرتا ہے۔ Alertas PagerDuty ان painéis کو evidências کے طور پر consulte کرتے ہیں۔ |

## Runbook - Implantação de ناکام یا خراب artefato

### شروع ہونے کی شرائط

- falha nas análises de visualização/produção (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana em `torii_sorafs_gateway_refusals_total` یا
  Implementação `torii_sorafs_manifest_submit_total{status="error"}`
- promoção manual de alias de controle de qualidade کے فوراً بعد rotas quebradas یا Tente falhas de proxy نوٹ کرے۔

### فوری روک تھام

1. **Implantações congeladas کریں:** Pipeline de CI کو `DEPLOY_FREEZE=1` سے marca کریں (entrada de fluxo de trabalho do GitHub)
   یا Pausa de trabalho do Jenkins کریں تاکہ مزید artefatos نہ نکلیں۔
2. **Captura de artefatos کریں:** falha na compilação کے `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, a saída da sonda é definida como rollback
   عین resumos کو referência کرے۔
3. **Stakeholders کو اطلاع دیں:** armazenamento SRE, líder do Docs/DevRel, e diretor de governança
   (خصوصاً جب `docs.sora` متاثر ہو)۔

### رول بیک طریقہ کار

1. Manifesto de última validade (LKG) کی شناخت کریں۔ fluxo de trabalho de produção
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` میں اسٹور کرتا ہے۔
2. auxiliar de remessa سے alias کو اس manifesto پر دوبارہ bind کریں:

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

3. resumo de reversão کو ticket de incidente میں LKG اور resumos de manifesto com falha کے ساتھ ریکارڈ کریں۔

###

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` e `sorafs_cli proof verify ...`
   (guia de implantação دیکھیں) تاکہ تصدیق ہو کہ manifesto repromovido arquivado CAR کے ساتھ correspondência کرتا ہے۔
4. `npm run probe:tryit-proxy` تاکہ Try-It staging proxy کی بحالی یقینی ہو۔

### واقعے کے بعد

1. causa raiz سمجھ میں آنے کے بعد ہی pipeline de implantação
2. [`devportal/deploy-guide`](./deploy-guide) میں Entradas "Lições aprendidas" کو نئے pontos سے بھر دیں، اگر ہوں۔
3. conjunto de testes com falha (sonda, verificador de link, وغیرہ) کے لئے defeitos فائل کریں۔

## Runbook - ریپلیکیشن میں گراوٹ

### شروع ہونے کی شرائط

- الرٹ: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` 10 minutos
- `torii_sorafs_replication_backlog_total > 10` 10 meses (`pin-registry-ops.md` دیکھیں)۔
- Governança ریلیز کے بعد alias کی دستیابی سست ہونے کی رپورٹ کرے۔

### ابتدائی جانچ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) dashboards دیکھیں تاکہ معلوم ہو سکے کہ backlog
   کسی classe de armazenamento یا frota do fornecedor تک محدود ہے یا نہیں۔
2. Registros Torii میں Avisos `sorafs_registry::submit_manifest` چیک کریں تاکہ معلوم ہو سکے کہ envios falham ہو رہی ہیں یا Não
3. `sorafs_cli manifest status --manifest ...` کے ذریعے amostra de integridade de réplica کریں (resultados por provedor دکھاتا ہے)۔

### تخفیفی اقدامات1. `scripts/sorafs-pin-release.sh` کے ذریعے زیادہ contagem de réplicas (`--pin-min-replicas 7`) کے ساتھ manifesto دوبارہ جاری کریں
   تاکہ carregamento do agendador کو زیادہ provedores پر پھیلا دے۔ Digest log de incidentes میں ریکارڈ کریں۔
2. O backlog do provedor é o seu provedor e o agendador de replicação é definido como desabilitar o recurso
   (`pin-registry-ops.md` documentado) Para enviar manifesto کریں جو دوسرے provedores کو atualização de alias کرنے پر مجبور کرے۔
3. Frescura do alias, paridade de replicação سے زیادہ crítico ہو تو alias کو پہلے سے manifesto گرم encenado (`docs-preview`) پر religar کریں،
   پھر SRE کے backlog claro کرنے کے بعد publicação de manifesto de acompanhamento کریں۔

### بحالی اور اختتام

1. `torii_sorafs_replication_sla_total{outcome="missed"}` کو monitor کریں تاکہ contagem platô ہو۔
2. Saída `sorafs_cli manifest status` کو evidência کے طور پر captura کریں کہ ہر réplica دوبارہ compatível ہے۔
3. post-mortem do backlog de replicação (escalonamento do provedor, ajuste do chunker, وغیرہ)۔

## Runbook - اینالیٹکس یا ٹیلیمیٹری آؤٹیج

### شروع ہونے کی شرائط

- `npm run probe:portal` کامیاب ہو لیکن dashboards Eventos `AnalyticsTracker` کو >15 minutos تک ingerir نہ کریں۔
- Eventos descartados de revisão de privacidade میں غیر متوقع اضافہ رپورٹ کرے۔
- Caminhos `npm run probe:tryit-proxy` `/probe/analytics` falharam

### جوابی اقدامات

1. Verificação de entradas em tempo de construção: `DOCS_ANALYTICS_ENDPOINT` e `DOCS_ANALYTICS_SAMPLE_RATE`
   artefato de liberação com falha (`build/release.json`) میں۔
2. `DOCS_ANALYTICS_ENDPOINT` کو coletor de teste پر ponto کر کے `npm run probe:portal` دوبارہ چلائیں تاکہ cargas úteis do rastreador emitem کرنا ثابت ہو۔
3. Coletores desligados ہوں تو `DOCS_ANALYTICS_ENDPOINT=""` set کریں اور reconstruir کریں تاکہ curto-circuito do rastreador کرے؛ cronograma de incidente da janela de interrupção میں ریکارڈ کریں۔
4. validar a impressão digital `scripts/check-links.mjs` e a impressão digital `checksums.sha256` کرتا ہے
   (interrupções de análise e validação do mapa do site *bloquear* نہیں کرنا چاہیے)۔
5. recuperação do coletor ہونے کے بعد `npm run test:widgets` چلائیں تاکہ testes de unidade auxiliares de análise executados ہوں پھر republicar کریں۔

### واقعے کے بعد

1. [`devportal/observability`](./observability) میں نئی limitações do coletor یا requisitos de amostragem اپ ڈیٹ کریں۔
2. Política de dados analíticos کے باہر descartar یا redigir ہوا ہو تو aviso de governança فائل کریں۔

## سہ ماہی استقامت کی مشقیں

دونوں treinos **ہر trimestre کے پہلے منگل** (janeiro/abril/julho/outubro) کو چلائیں
یا کسی بڑے mudança de infraestrutura کے فوراً بعد۔ artefatos
`artifacts/devportal/drills/<YYYYMMDD>/` کے تحت محفوظ کریں۔

| مشق | مراحل | شواہد |
| ----- | ----- | -------- |
| Reversão de alias کی مشق | 1. تازہ ترین manifesto de produção کے ساتھ Reversão de "implantação com falha" دوبارہ چلائیں۔<br/>2. sondas passam ہونے کے بعد produção پر religar کریں۔<br/>3. `portal.manifest.submit.summary.json` Para logs de sonda e broca فولڈر میں ریکارڈ کریں۔ | `rollback.submit.json`, saída da sonda, ensaio e etiqueta de liberação۔ |
| مصنوعی توثیق کا آڈٹ | 1. produção e preparação کے خلاف `npm run probe:portal` اور `npm run probe:tryit-proxy` چلائیں۔<br/>2. `npm run check:links` چلائیں اور `build/link-report.json` arquivo کریں۔<br/>3. Painéis Grafana کے capturas de tela/exportações anexadas کریں جو sucesso da sonda confirmar کریں۔ | Logs de sonda + `link-report.json` ou impressão digital manifestada |

Exercícios perdidos کو Gerente de Docs/DevRel اور Revisão de governança SRE تک escalar کریں، کیونکہ roteiro یہ تقاضا کرتا ہے کہ
alias rollback اور portal probes کے صحت مند رہنے کا determinístico, evidência trimestral موجود ہو۔

## PagerDuty e coordenação de plantão

- Serviço PagerDuty **Publicação no Portal de Documentos** `dashboards/grafana/docs_portal.json` سے پیدا ہونے والے alertas کی مالک ہے۔
  قواعد `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, اور `DocsPortal/TLSExpiry` Docs/DevRel página primária کرتے ہیں
  Para armazenamento SRE secundário ہوتا ہے۔
- página آنے پر `DOCS_RELEASE_TAG` شامل کریں, متاثرہ Grafana painéis کے capturas de tela anexadas کریں, اور mitigação شروع کرنے سے پہلے
  saída de sondagem/verificação de link کو notas de incidente میں link کریں۔
- mitigação (reversão ou reimplantação) کے بعد `npm run probe:portal`, `npm run check:links` دوبارہ چلائیں, اور تازہ Grafana
  captura de instantâneos کریں جو métricas کو limites کے اندر دکھائیں۔ تمام evidência incidente PagerDuty کے ساتھ anexar کریں
  قبل ازیں کہ اسے resolver کیا جائے۔
- اگر دو alertas ایک ساتھ fogo ہوں (مثال کے طور پر expiração TLS اور backlog), تو recusas کو پہلے triagem کریں
  (publicação روکیں), procedimento de reversão چلائیں, پھر Armazenamento SRE کے ساتھ ponte پر TLS/backlog صاف کریں۔
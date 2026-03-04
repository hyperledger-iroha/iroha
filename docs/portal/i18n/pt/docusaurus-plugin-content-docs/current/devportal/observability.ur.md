---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پورٹل آبزرویبیلٹی اور اینالٹکس

DOCS-SORA روڈمیپ ہر preview build کے لئے analytics, sondas sintéticas, اور automação de link quebrado کا تقاضا کرتا ہے۔
یہ نوٹ وہ encanamento بیان کرتا ہے جو اب پورٹل کے ساتھ navio ہوتی ہے تاکہ monitoramento de operadores جوڑ سکیں
Dados de visitantes بغیر لیک کئے۔

## Liberar marcação

- `DOCS_RELEASE_TAG=<identifier>` سیٹ کریں (fallback `GIT_COMMIT` یا `dev`) جب پورٹل build ہو۔
  ویلیو `<meta name="sora-release">` میں injetar ہوتی ہے تاکہ probes e implantações de painéis کو distinguir کر سکیں۔
- `npm run build` `build/release.json` emite کرتا ہے (جسے `scripts/write-checksums.mjs` کھتا ہے)
  A tag, carimbo de data / hora, ou opcional `DOCS_RELEASE_SOURCE` e descrição do arquivo یہی فائل visualização de artefatos میں pacote
  ہوتی ہے اور link checker رپورٹ میں consulte ہوتی ہے۔

## Análises que preservam a privacidade

- `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` configurar کریں تاکہ rastreador leve ativar ہو۔
  cargas úteis میں `{ event, path, locale, release, ts }` شامل ہوتے ہیں بغیر referenciador یا metadados IP کے, اور
  `navigator.sendBeacon` جہاں ممکن ہو استعمال ہوتا ہے تاکہ bloco de navegação نہ ہو۔
- `DOCS_ANALYTICS_SAMPLE_RATE` (0-1) کے ساتھ controle de amostragem کریں۔ último caminho enviado do rastreador ذخیرہ کرتا ہے
  اور ایک ہی navegação کے لئے eventos duplicados emitem نہیں کرتا۔
- implementação `src/components/AnalyticsTracker.jsx` میں ہے اور `src/theme/Root.js` کے ذریعے montagem global ہوتی ہے۔

## Sondas sintéticas

- `npm run probe:portal` عام rotas کے خلاف solicitações GET بھیجتا ہے
  (`/`, `/norito/overview`, `/reference/torii-swagger`, وغیرہ) Para verificar o valor do cartão
  `sora-release` meta tag `--expect-release` (یا `DOCS_RELEASE_TAG`) سے correspondência کرتا ہے۔ Exemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Falhas ہر caminho کے حساب سے relatório ہوتے ہیں، جس سے CD gate کرنا آسان ہو جاتا ہے۔

## Automação de link quebrado

- `npm run check:links` `build/sitemap.xml` scan کرتا ہے، ہر entrada کو arquivo local سے mapa ہونا یقینی بناتا ہے
  (`index.html` fallbacks چیک کرتا ہے), e `build/link-report.json` کھتا ہے جس میں liberar metadados, totais, falhas,
  اور `checksums.sha256` کا SHA-256 impressão digital شامل ہوتا ہے (جو `manifest.id` کے طور پر expor ہوتا ہے) تاکہ ہر رپورٹ
  manifesto do artefato
- script diferente de zero پر exit کرتا ہے جب کوئی صفحہ ausente ہو، اس لئے CI پرانی یا rotas quebradas پر liberações روک سکتا ہے۔
  relatórios ان caminhos candidatos کو citar کرتے ہیں جو tentar کئے گئے تھے, جو regressões de roteamento کو árvore de documentos تک rastreamento کرنے میں مدد دیتا ہے۔

## Painel Grafana e alertas

- Placa `dashboards/grafana/docs_portal.json` Grafana **Publicação no Portal de Documentos** publicar کرتا ہے۔
  Quais são os painéis que você precisa:
  - *Recusas de gateway (5m)* `torii_sorafs_gateway_refusals_total` کو `profile`/`reason` کے escopo کے ساتھ استعمال کرتا ہے
    تاکہ SREs خراب pushes de política یا detecção de falhas de token کر سکیں۔
  - *Resultados de atualização de cache de alias* e *Alias Proof Age p90* `torii_sorafs_alias_cache_*` کو track کرتے ہیں
    تاکہ DNS cortado سے پہلے novas provas موجود ہونے کا ثبوت ملے۔
  - *Contagens de manifestos de registro de pinos* e *Contagem de alias ativos* backlog de registro de pinos estatísticos e total de aliases کو refletir کرتے ہیں
    تاکہ governança ہر liberação کو auditoria کر سکے۔
  - *Expiração do TLS do gateway (horas)* اس وقت destaque کرتا ہے جب gateway de publicação کا Expiração do certificado TLS کے قریب ہو
    (limiar de alerta 72 h)۔
  - *Resultados de SLA de replicação* e *Backlog de replicação* Telemetria `torii_sorafs_replication_*` پر نظر رکھتے ہیں
    تاکہ publicar کے بعد تمام réplicas GA bar پر ہوں۔
- variáveis ​​de modelo integradas (`profile`, `reason`) استعمال کریں تاکہ `docs.sora` perfil de publicação پر focus کیا جا سکے
  یا تمام gateways میں picos investigam کئے جا سکیں۔
- Painéis de painel de roteamento PagerDuty کو evidência کے طور پر استعمال کرتا ہے: alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, اور `DocsPortal/TLSExpiry` تب fire کرتے ہیں جب متعلقہ series
  limites ultrapassam کرے۔ runbook de alerta کو اسی صفحے سے link کریں تاکہ engenheiros de plantão repetição exata de consultas Prometheus کر سکیں۔

## Juntando tudo1. `npm run build` کے دوران conjunto de variáveis de ambiente de liberação/análise کریں اور etapa pós-construção کو
   `checksums.sha256`, `release.json`, e `link-report.json` emitem sinais de erro
2. visualizar o nome do host کے خلاف `npm run probe:portal` چلائیں اور `--expect-release` کو اسی tag سے fio کریں۔
   lista de verificação de publicação
3. `npm run check:links` چلائیں تاکہ entradas quebradas do mapa do site پر جلد falhar ہو اور gerar ہونے والی relatório JSON کو
   visualizar artefatos کے ساتھ arquivo کریں۔ Relatório mais recente da CI کو `artifacts/docs_portal/link-report.json` میں drop کرتا ہے
   Baixar registros de construção de governança سے pacote de evidências سیدھا download کر سکے۔
4. endpoint analítico کو اپنے coletor de preservação de privacidade (Plausível, ingestão OTEL auto-hospedada وغیرہ) کی طرف forward کریں
   اور ہر liberar کے لئے documento de taxas de amostragem کریں تاکہ contagens de painéis کو درست interpretar کریں۔
5. CI پہلے ہی visualizar/implantar fluxos de trabalho میں ان etapas کو wire کر چکا ہے
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`)، اس لئے simulações locais میں صرف cobertura de comportamento específico de segredos کرنا ہوتا ہے۔
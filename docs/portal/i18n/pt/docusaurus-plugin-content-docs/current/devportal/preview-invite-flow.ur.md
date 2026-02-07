---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو دعوتی فلو

## مقصد

روڈ میپ آئٹم **DOCS-SORA** ریویور آن بورڈنگ اور پبلک پریویو دعوتی پروگرام کو وہ آخری رکاوٹیں قرار دیتا ہے جن کے بعد پورٹل بیٹا سے باہر جا سکتا ہے۔ یہ صفحہ بیان کرتا ہے کہ ہر دعوتی ویو کیسے کھولی جائے, کون سے artefatos دعوتیں بھیجنے سے پہلے لازمی ہیں, اور فلو کی auditabilidade کیسے ثابت کی جائے۔ O que fazer com o cartão:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) ہر ریویور کی ہینڈلنگ کے لئے۔
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) checksum ضمانتوں کے لئے۔
- [`devportal/observability`](./observability.md) ٹیلی میٹری exportações e ganchos de alerta کے لئے۔

## ویو پلان

| ویو | سامعین | انٹری معیار | ایگزٹ معیار | Não |
| --- | --- | --- | --- | --- |
| **W0 - Mantenedores principais** | Mantenedores do Docs/SDK no primeiro dia مواد validar کرتے ہیں۔ | GitHub é `docs-portal-preview` آباد ہو, `npm run serve` checksum gate سبز ہو, Alertmanager 7 دن خاموش رہے۔ | تمام P0 docs ریویو, backlog ٹیگ شدہ, کوئی incidente de bloqueio نہ ہو۔ | Para validar کرنے کے لئے؛ دعوتی ای میل نہیں, صرف visualizar artefatos شیئر کریں۔ |
| **W1 - Parceiros** | SoraFS آپریٹرز, Torii integradores, e NDA کے تحت revisores de governança۔ | W0 ختم, قانونی شرائط منظور, teste de teste de proxy | پارٹنر sign-off جمع (emissão یا formulário assinado), ٹیلی میٹری میں <=10 revisores simultâneos، 14 dias کوئی regressão de segurança نہیں۔ | modelo de convite + solicitação de ingressos لازم۔ |
| **W2 - Comunidade** | کمیونٹی ویٹ لسٹ سے منتخب contribuidores۔ | W1 ختم, exercícios de incidentes ensaiados, FAQ público اپ ڈیٹ۔ | فیڈبیک ہضم، >=2 documentação libera pipeline de visualização سے بغیر rollback گزر چکی ہوں۔ | convites simultâneos محدود (<=25) اور ہفتہ وار بیچ۔ |

`status.md` é um rastreador de solicitação de visualização. سکے۔

## Lista de verificação de comprovação

Para obter o **دعوتیں شیڈول کرنے سے پہلے** مکمل کریں:

1. **Artefatos de CI disponíveis**
   - تازہ ترین `docs-portal-preview` + descritor `.github/workflows/docs-portal-preview.yml` کے ذریعے اپ لوڈ ہو۔
   - Pino SoraFS `docs/portal/docs/devportal/deploy-guide.md` میں نوٹ ہو (descritor de transição موجود ہو).
2. **Aplicação de soma de verificação**
   - `docs/portal/scripts/serve-verified-preview.mjs` `npm run serve` کے ذریعے invocar ہو۔
   - `scripts/preview_verify.sh` ہدایات macOS + Linux پر ٹیسٹ ہوں۔
3. **Linha de base de telemetria**
   - `dashboards/grafana/docs_portal.json` صحت مند Try it ٹریفک دکھائے اور `docs.preview.integrity` الرٹ سبز ہو۔
   - `docs/portal/docs/devportal/observability.md` کا تازہ apêndice Grafana لنکس کے ساتھ اپ ڈیٹ ہو۔
4. **Artefatos de governança**
   - problema do rastreador de convite تیار ہو (ہر ویو کے لئے ایک problema). 
   - modelo de registro do revisor کاپی ہو (دیکھیں [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - قانونی اور emissão de aprovações SRE کے ساتھ منسلک ہوں۔

دعوت بھیجنے سے پہلے rastreador de convite میں comprovação مکمل ہونے کا اندراج کریں۔

## فلو کے مراحل

1. **امیدوار منتخب کریں**
   - ویٹ لسٹ شیٹ یا پارٹنر کیو سے نکالیں۔
   - ہر امیدوار کے پاس مکمل modelo de solicitação ہونا یقینی بنائیں۔
2. **رسائی کی منظوری**
   - problema do rastreador de convites پر aprovador اسائن کریں۔
   - pré-requisitos چیک کریں (CLA/contrato, uso aceitável, resumo de segurança).
3. **دعوتیں ارسال کریں**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) کے espaços reservados (`<preview_tag>`, `<request_ticket>`, contatos) بھریں۔
   - descritor + hash de arquivo, experimente URL de teste, nos canais de suporte منسلک کریں۔
   - فائنل ای میل (یا Matrix/Slack transcript) issue میں محفوظ کریں۔
4. **Inscrição de integração**
   - rastreador de convite کو `invite_sent_at`, `expected_exit_at`, اور status (`pending`, `active`, `complete`, `revoked`) کے ساتھ اپ ڈیٹ کریں۔
   - auditabilidade کے لئے solicitação de admissão do revisor کو لنک کریں۔
5. **Telemetria مانیٹر کریں**
   - Alertas `docs.preview.session_active` e `TryItProxyErrors`
   - اگر ٹیلی میٹری linha de base سے ہٹے تو incidente کھولیں اور نتیجہ entrada de convite کے ساتھ نوٹ کریں۔
6. **فیڈبیک جمع کریں اور خارج ہوں**
   - فیڈبیک آنے پر یا `expected_exit_at` گزرنے پر دعوتیں بند کریں۔
   - اگلی coorte پر جانے سے پہلے ویو questão میں مختصر خلاصہ (descobertas, incidentes, próximas ações) اپ ڈیٹ کریں۔

## Evidências e relatórios

| Artefato | کہاں محفوظ کریں | اپ ڈیٹ cadência |
| --- | --- | --- |
| problema do rastreador de convites | GitHub پروجیکٹ `docs-portal-preview` | ہر دعوت کے بعد اپ ڈیٹ کریں۔ |
| exportação de lista de revisores | `docs/portal/docs/devportal/reviewer-onboarding.md` Registro vinculado | ہفتہ وار۔ |
| instantâneos de telemetria | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilização de pacote de telemetria کریں) | ہر ویو + incidentes کے بعد۔ |
| resumo de feedback | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (ہر ویو کیلئے فولڈر بنائیں) | ویو exit کے 5 دن کے اندر۔ |
| nota da reunião de governança | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | ہر DOCS-SORA sincronização de governança سے پہلے بھریں۔ |ہر بیچ کے بعد `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
چلائیں تاکہ مشین ریڈایبل resumo بنے۔ رینڈر شدہ JSON کو ویو emitir کے ساتھ منسلک کریں تاکہ revisores de governança پوری لاگ دوبارہ چلائے بغیر دعوتی A melhor opção para você

ہر ویو ختم ہونے پر evidência کی فہرست `status.md` کے ساتھ منسلک کریں تاکہ روڈ میپ انٹری جلدی اپ ڈیٹ ہو سکے۔

## Rollback e pause

جب درج ذیل میں سے کوئی ہو تو دعوتی فلو روک دیں (اور governança کو مطلع کریں):

- Experimente o incidente de proxy جس میں rollback کرنا پڑا (`npm run manage:tryit-proxy`).
- Fadiga de alerta: 7 dias úteis endpoints somente de visualização em >3 páginas de alerta.
- Lacuna de conformidade: دعوت بغیر termos assinados یا modelo de solicitação لاگ کئے بھیجی گئی۔
- Risco de integridade: `scripts/preview_verify.sh` سے incompatibilidade de soma de verificação پکڑا گیا۔

rastreador de convite میں remediação کی تصدیق کے بعد ہی دوبارہ شروع کریں۔
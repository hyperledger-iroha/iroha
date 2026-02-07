---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو ریویور آن بورڈنگ

## جائزہ

DOCS-SORA ڈویلپر پورٹل کے مرحلہ وار لانچ کو ٹریک کرتا ہے۔ compilações controladas por soma de verificação
(`npm run serve`) اور مضبوط Try it
کھلنے سے پہلے ویری فائیڈ ریویورز کی آن بورڈنگ۔ یہ گائیڈ بیان کرتا ہے کہ درخواستیں کیسے جمع کی جائیں,
اہلیت کیسے چیک کی جائے, رسائی کیسے دی جائے, اور شرکاء کو محفوظ طریقے سے O que você precisa saber
کوهورٹ پلاننگ, دعوتی cadência, اور ٹیلی میٹری exportações کے لئے
[visualizar fluxo de convite](./preview-invite-flow.md) دیکھیں؛ نیچے کے مراحل اس پر فوکس کرتے ہیں
کہ ریویور منتخب ہونے کے بعد کون سی کارروائیاں کرنی ہیں۔

- **اسکوپ:** وہ ریویورز جنہیں GA سے پہلے visualização de documentos (`docs-preview.sora`, compilações de páginas GitHub, e pacotes SoraFS) تک رسائی درکار ہے۔
- **آؤٹ آف اسکوپ:** Torii یا SoraFS آپریٹرز (kits de integração adicionais میں کور) e پروڈکشن پورٹل implantações (دیکھیں [`devportal/deploy-guide`](./deploy-guide.md))۔

## رولز اور پری ریکویزٹس

| Roda | عام اہداف | Artefactos de construção | Não |
| --- | --- | --- | --- |
| Mantenedor principal | نئی گائیڈز ویری فائی کرنا, testes de fumaça چلانا۔ | Identificador do GitHub, contato da matriz, CLA assinado | عموما پہلے سے `docs-preview` GitHub ٹیم میں ہوتا ہے؛ پھر بھی درخواست درج کریں تاکہ رسائی auditável رہے۔ |
| Revisor parceiro | پبلک ریلیز سے پہلے SDK snippets یا conteúdo de governança ویری فائی کرنا۔ | E-mail corporativo, POC legal, termos de visualização assinados۔ | ٹیلی میٹری + requisitos de manipulação de dados کی منظوری لازم ہے۔ |
| Voluntário comunitário | گائیڈز پر feedback de usabilidade | Identificador do GitHub, contato preferencial, fuso horário, CoC قبولیت۔ | کوہورٹس چھوٹے رکھیں؛ contrato de contribuição |

تمام ریویور اقسام کو لازمی ہے کہ:

1. visualizar artefatos کے لئے uso aceitável پالیسی تسلیم کریں۔
2. Apêndices de segurança/observabilidade پڑھیں
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. O serviço de snapshot کرنے سے پہلے `docs/portal/scripts/preview_verify.sh` چلانے پر رضامند ہوں۔

## Ingestão

1. درخواست دینے والے سے
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   فارم بھرنے کو کہیں (یا emitir میں copiar/colar کریں)۔ کم از کم یہ ریکارڈ کریں: شناخت, رابطے کا طریقہ،
   Identificador do GitHub, datas de revisão e documentos de segurança پڑھنے کی تصدیق۔
2. درخواست کو Rastreador `docs-preview` (emissão do GitHub یا ticket de governança) میں درج کریں اور aprovação do aprovador atribuído کریں۔
3. پری ریکویزٹس چیک کریں:
   - CLA / contrato de contribuidor فائل پر موجود ہو (یا referência de contrato de parceiro)۔
   - reconhecimento de uso aceitável درخواست میں محفوظ ہو۔
   - avaliação de risco مکمل ہو (مثال: revisores parceiros کو Jurídico نے aprovar کیا ہو)۔
4. Aprovador درخواست میں aprovação کرے اور problema de rastreamento کو کسی بھی entrada de gerenciamento de alterações سے لنک کرے
   (exemplo: `DOCS-SORA-Preview-####`)۔

## Provisionamento

1. **Artefatos شیئر کریں** — Fluxo de trabalho CI یا SoraFS pin سے تازہ ترین descritor de visualização + arquivo فراہم کریں
   (Artefato `docs-portal-preview`)۔ ریویورز کو یاد دلائیں کہ چلائیں:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Aplicação de checksum کے ساتھ serve کریں** — ریویورز کو checksum-gated کمانڈ کی طرف ریفر کریں:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   یہ `scripts/serve-verified-preview.mjs` کو reutilizar کرتا ہے تاکہ کوئی compilação não verificada غلطی سے لانچ نہ ہو۔

3. **GitHub رسائی دیں (اختیاری)** — اگر ریویورز کو ramificações não publicadas چاہییں تو انہیں revisão مدت کے لئے
   `docs-preview` GitHub ٹیم میں شامل کریں اور associação تبدیلی درخواست میں درج کریں۔

4. **Canais de suporte شیئر کریں** — contato de plantão (Matrix/Slack) e procedimento de incidente
   [`incident-runbooks`](./incident-runbooks.md) سے شیئر کریں۔

5. **Telemetria + feedback** — ریویورز کو یاد دلائیں کہ análises anônimas جمع کی جاتی ہے
   (دیکھیں [`observability`](./observability.md))۔ دعوت میں دیے گئے formulário de feedback یا modelo de problema فراہم کریں
   O evento é [`preview-feedback-log`](./preview-feedback-log) helper سے لاگ کریں تاکہ resumo da onda اپ ٹو ڈیٹ رہے۔

## ریویور چیک لسٹ

پریویو تک رسائی سے پہلے ریویورز کو یہ مکمل کرنا ہوگا:

1. verificação de artefatos baixados کریں (`preview_verify.sh`)۔
2. `npm run serve` (یا `serve:verified`) سے پورٹل لانچ کریں تاکہ checksum guard فعال ہو۔
3. A segurança e a observabilidade são importantes
4. Console OAuth/Try it کو login com código do dispositivo کے ذریعے ٹیسٹ کریں (اگر aplicável ہو) اور tokens de produção دوبارہ استعمال نہ کریں۔
5. rastreador acordado میں descobertas درج کریں (problema, documento compartilhado یا فارم) اور انہیں tag de lançamento de visualização سے tag کریں۔

## Mantenedor ذمہ داریاں اور offboarding| مرحلہ | Produtos |
| --- | --- |
| Início | یقینی بنائیں کہ lista de verificação de ingestão [`preview-feedback-log`](./preview-feedback-log) کے ذریعے `invite-sent` انٹری شامل کریں, اور اگر revisão ایک ہفتے سے زیادہ ہو تو sincronização de ponto médio شیڈول کریں۔ |
| Monitoramento | visualizar telemetria مانیٹر کریں (غیر معمولی Experimente ٹریفک، falhas de sonda) اور اگر کوئی مشکوک چیز ہو تو runbook de incidentes فالو کریں۔ descobertas آنے پر `feedback-submitted`/`issue-opened` eventos لاگ کریں تاکہ métricas de onda درست رہیں۔ |
| Desativação | عارضی GitHub یا SoraFS رسائی واپس لیں, `access-revoked` درج کریں, درخواست arquivo کریں (resumo de feedback + ações pendentes شامل کریں), اور registro do revisor اپ ڈیٹ کریں۔ ریویور سے مقامی builds صاف کرنے کو کہیں اور [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) سے بنا digest منسلک کریں۔ |

ریویورز کو ondas کے درمیان girar کرتے وقت بھی یہی عمل استعمال کریں۔ repo میں trilha de papel
(edição + modelos) برقرار رکھنے سے DOCS-SORA auditável رہتا ہے اور governança کو تصدیق میں مدد ملتی ہے
کہ visualizar controles documentados de acesso کے مطابق تھا۔

## Modelos de convite e rastreamento

- ہر divulgação کی شروعات
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  فائل سے کریں۔ یہ کم از کم قانونی زبان، soma de verificação de visualização ہدایات، اور اس توقع کو شامل کرتی ہے کہ
  ریویورز uso aceitável پالیسی تسلیم کریں۔
- template ایڈٹ کرتے وقت `<preview_tag>`, `<request_ticket>` اور canais de contato placeholders بدلیں۔
  mensagem final کی ایک کاپی ticket de admissão میں محفوظ کریں تاکہ ریویورز, aprovadores اور auditores
  بھیجے گئے الفاظ کا حوالہ دے سکیں۔
- دعوت بھیجنے کے بعد planilha de rastreamento یا issue کو `invite_sent_at` timestamp اور متوقع data final کے ساتھ
  اپ ڈیٹ کریں تاکہ [visualizar fluxo de convite](./preview-invite-flow.md) رپورٹ خودکار طور پر coorte اٹھا سکے۔
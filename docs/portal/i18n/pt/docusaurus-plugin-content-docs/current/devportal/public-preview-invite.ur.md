---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پبلک پریویو دعوتی پلے بک

## پروگرام کے مقاصد

یہ پلے بک وضاحت کرتی ہے کہ ریویور آن بورڈنگ ورک فلو فعال ہونے کے بعد پبلک پریویو کیسے اعلان اور چلایا جائے۔
یہ DOCS-SORA روڈمیپ کو دیانت دار رکھتی ہے کیونکہ ہر دعوت کے ساتھ قابل Artefatos تصدیق, سیکیورٹی رہنمائی,
Esse feedback é importante para você.

- **آڈیئنس:** کمیونٹی ممبرز, پارٹنرز اور mantenedores کی curados فہرست جنہوں نے visualização aceitável-uso پالیسی سائن کی ہے۔
- **سیلنگز:** tamanho de onda padrão <= 25 ریویورز, 14 dias de janela de acesso, 24h de resposta a incidentes۔

## لانچ گیٹ چیک لسٹ

O que você precisa saber sobre o que fazer:

1. تازہ ترین artefatos de visualização CI میں اپلوڈ ہوں (`docs-portal-preview`,
   manifesto de soma de verificação, descritor, pacote SoraFS)۔
2. `npm run --prefix docs/portal serve` (controlado por soma de verificação) اسی tag پر ٹیسٹ کیا گیا ہو۔
3. ریویور آن بورڈنگ ٹکٹس aprovar ہوں اور onda de convite سے لنک ہوں۔
4. سیکیورٹی, observabilidade, e incidente ڈاکس validar ہوں
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md))۔
5. feedback فارم یا modelo de problema تیار ہو (gravidade, etapas de reprodução, capturas de tela, e informações do ambiente کے فیلڈز شامل ہوں)۔
6. اعلان کی کاپی Docs/DevRel + Governança نے ریویو کی ہو۔

## دعوتی پیکیج

O que você precisa saber:

1. **Artefatos verificados** — Manifesto/plano SoraFS یا GitHub artefato کے لنکس دیں،
   ساتھ میں manifesto de soma de verificação e descritor بھی دیں۔ verificação کمانڈ واضح طور پر لکھیں تاکہ
   ریویورز site لانچ کرنے سے پہلے اسے چلا سکیں۔
2. **Instruções de serviço** — visualização controlada por soma de verificação کمانڈ شامل کریں:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Lembretes de segurança** — واضح کریں کہ tokens خود بخود expiram ہوتے ہیں, لنکس شیئر نہیں کیے جائیں،
   اور incidentes فوراً رپورٹ کیے جائیں۔
4. **Canal de feedback** — modelo/formulário de emissão لنک کریں اور expectativas de tempo de resposta واضح کریں۔
5. **Datas do programa** — datas de início/término, horário comercial, sincronizações, e janela de atualização فراہم کریں۔

نمونہ ای میل
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
میں دستیاب ہے اور یہ requisitos پوری کرتا ہے۔ بھیجنے سے پہلے espaços reservados (datas, URLs, contatos)
اپ ڈیٹ کریں۔

## پریویو host کو expor کریں

جب تک onboarding مکمل نہ ہو اور alterar ticket منظور نہ ہو تب تک visualizar host کو promover نہ کریں۔
اس سیکشن کے construir/publicar/verificar etapas de ponta a ponta کے لئے
[visualizar guia de exposição do host](./preview-host-exposure.md) دیکھیں۔

1. **Build پیکیجنگ:** carimbo de tag de liberação کریں اور artefatos determinísticos تیار کریں۔

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   script de pinos `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   O `portal.dns-cutover.json` e o `artifacts/sorafs/` podem ser usados ان فائلوں کو onda de convite
   کے ساتھ anexar کریں تاکہ ہر ریویور وہی verificação de bits کر سکے۔

2. **Visualizar alias de publicação کریں:** کمانڈ کو `--skip-submit` کے بغیر دوبارہ چلائیں
   (`TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` e prova de alias emitido pela governança فراہم کریں)۔
   اسکرپٹ `docs-preview.sora` پر manifest bind کرے گا اور pacote de evidências کے لئے
   `portal.manifest.submit.summary.json` e `portal.pin.report.json` padrão

3. **Investigação de implantação کریں:** convites بھیجنے سے پہلے alias resolver ہونا اور checksum کا tag سے match ہونا
   یقینی بنائیں۔

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) کو substituto کے طور پر útil رکھیں تاکہ
   اگر preview edge میں مسئلہ ہو تو ریویورز لوکل کاپی چلا سکیں۔

## کمیونیکیشن ٹائم لائن

| Jan | ایکشن | Proprietário |
| --- | --- | --- |
| D-3 | دعوتی کاپی finalizar کرنا، atualização de artefatos کرنا، verificação کا simulação | Documentos/DevRel |
| D-2 | Aprovação da governança + ticket de mudança | Documentos/DevRel + Governança |
| D-1 | modelo کے ذریعے دعوتیں بھیجیں، rastreador میں lista de destinatários اپ ڈیٹ کریں | Documentos/DevRel |
| D | chamada inicial / horário comercial, painéis de telemetria مانیٹر کریں | Documentos/DevRel + plantão |
| D+7 | resumo de feedback de ponto médio, problemas de bloqueio e triagem | Documentos/DevRel |
| D+14 | onda بند کریں, عارضی رسائی revogar کریں, `status.md` میں خلاصہ شائع کریں | Documentos/DevRel |

## Rastreamento de acesso e telemetria

1. ہر destinatário, carimbo de data e hora do convite, اور data de revogação کو visualizar registrador de feedback کے ساتھ ریکارڈ کریں
   (دیکھیں [`preview-feedback-log`](./preview-feedback-log)) تاکہ ہر wave ایک ہی trilha de evidência شیئر کرے:

   ```bash
   # artifacts/docs_portal_preview/feedback_log.json میں نیا invite event شامل کریں
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Eventos suportados ہیں `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, e `access-revoked`۔ log ڈیفالٹ طور پر
   `artifacts/docs_portal_preview/feedback_log.json` میں موجود ہے؛ اسے onda de convite ٹکٹ کے ساتھ
   formulários de consentimento سمیت anexar کریں۔ close-out نوٹ سے پہلے ajudante de resumo استعمال کریں تاکہ
   Roll-up auditável تیار ہو:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```resumo JSON ہر wave کے convites, کھلے destinatários, contagens de feedback, اور حالیہ ترین evento کے
   timestamp کو enumerar کرتا ہے۔ ajudante
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)
   پر مبنی ہے، اس لئے وہی fluxo de trabalho لوکل یا CI میں چل سکتا ہے۔ recapitulação
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   Modelo de resumo do modelo
2. painéis de telemetria کو wave میں استعمال ہونے والے `DOCS_RELEASE_TAG` کے ساتھ tag کریں تاکہ
   picos کو convidar coortes سے correlacionar کیا جا سکے۔
3. implantar o ambiente de visualização `npm run probe:portal -- --expect-release=<tag>` چلائیں تاکہ
   درست liberar metadados anunciar کرے۔
4. کسی بھی incidente کو modelo de runbook میں captura کریں اور اسے coorte سے link کریں۔

## Feedback e encerramento

1. feedback کو documento compartilhado یا quadro de problemas میں جمع کریں۔ itens کو `docs-preview/<wave>` سے tag کریں تاکہ
   proprietários de roteiro انہیں آسانی سے consulta کر سکیں۔
2. visualizar o registrador کی saída resumida سے relatório de onda بھریں, پھر coorte کو `status.md` میں resumir کریں
   (participantes, descobertas, correções planejadas) e DOCS-SORA marco بدلا ہو تو `roadmap.md` اپ ڈیٹ کریں۔
3. [`reviewer-onboarding`](./reviewer-onboarding.md) As etapas de desativação seguem کریں: acesso revogado کریں،
   arquivo de solicitações کریں، اور participantes کا شکریہ ادا کریں۔
4. اگلی wave کے لئے atualização de artefatos کریں, portões de soma de verificação دوبارہ چلائیں, اور modelo de convite کو نئی datas سے اپ ڈیٹ کریں۔

اس playbook کو مسلسل لاگو کرنے سے preview پروگرام auditável رہتا ہے اور Docs/DevRel کو دعوتیں
اسکیل کرنے کا repetível طریقہ ملتا ہے جیسے جیسے پورٹل GA کے قریب آتا ہے۔
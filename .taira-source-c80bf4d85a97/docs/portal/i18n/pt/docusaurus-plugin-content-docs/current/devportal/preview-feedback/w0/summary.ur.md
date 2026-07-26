---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w0-resumo
título: W0 کے وسط کا فيڈبیک خلاصہ
sidebar_label: W0 Nome (etiqueta)
descrição: mantenedores principais کے onda de visualização کے لئے وسطی چیک پوائنٹس, نتائج اور itens de ação.
---

| آئٹم | تفصیل |
| --- | --- |
| Para | W0 - mantenedores principais |
| خلاصہ تاریخ | 27/03/2025 |
| ریویو ونڈو | 25/03/2025 -> 08/04/2025 |
| شرکا | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidade-01 |
| آرٹیفیکٹ ٹیگ | `preview-2025-03-24` |

## نمایاں نکات

1. **Checksum ورک فلو** - تمام revisores نے تصدیق کی کہ `scripts/preview_verify.sh`
   مشترکہ descritor/arquivo جوڑی کے خلاف کامیاب رہا۔ کسی دستی substituir کی
   ضرورت نہیں ہوئی۔
2. **نیویگیشن فيڈبیک** - barra lateral کی ترتیب میں دو معمولی مسائل رپورٹ ہوئے
   (`docs-preview/w0 #1-#2`). دونوں Docs/DevRel کو دیے گئے اور لہر کو بلاک
   نہیں کرتے۔
3. **SoraFS runbook برابری** - sorafs-ops-01 em `sorafs/orchestrator-ops` اور
   `sorafs/multi-source-rollout` کے درمیان زیادہ واضح cross-links کی درخواست کی۔
   questão de acompanhamento W1 سے پہلے حل کرنا ہے۔
4. **ٹیلیمیٹری ریویو** - observabilidade-01 نے تصدیق کی کہ `docs.preview.integrity`,
   `TryItProxyErrors` اور Try-it proxy logs سب green رہے؛ کوئی alerta فائر نہیں ہوا۔

## ایکشن آئٹمز

| ID | وضاحت | Mal | اسٹیٹس |
| --- | --- | --- | --- |
| W0-A1 | entradas da barra lateral do devportal کو دوبارہ ترتیب دینا تاکہ revisores e documentos نمایاں ہوں (`preview-invite-*` کو ایک ساتھ کھیں). | Documentos-núcleo-01 | مکمل - barra lateral اب revisores docs کو مسلسل دکھاتا ہے (`docs/portal/sidebars.js`). |
| W0-A2 | `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout` são de alta qualidade e de cross-link. | Sorafs-ops-01 | مکمل - ہر runbook اب دوسرے کی طرف لنک کرتا ہے تاکہ rollout کے دوران دونوں گائیڈ نظر آئیں۔ |
| W0-A3 | rastreador de governança کے ساتھ instantâneos de telemetria + pacote de consulta شیئر کرنا۔ | Observabilidade-01 | Pacote - pacote `DOCS-SORA-Preview-W0` کے ساتھ منسلک ہے۔ |

## اختتامی خلاصہ (08/04/2025)

- پانچوں revisores نے تکمیل کی تصدیق کی, لوکل compilações صاف کیے, اور visualização ونڈو سے
  باہر نکل گئے؛ revogações de acesso `DOCS-SORA-Preview-W0` میں ریکارڈ ہیں۔
- لہر کے دوران کوئی incidentes یا alertas نہیں ہوئے؛ painéis de telemetria
  رہے۔ verde
- نیویگیشن + cross-link اقدامات (W0-A1/A2) نافذ ہو چکے ہیں اور اوپر کے docs میں
  دکھائی دیتے ہیں؛ rastreador de evidência de telemetria (W0-A3)
- pacote de evidências محفوظ کر دیا گیا: capturas de tela de telemetria, دعوت کی reconhecimentos,
  Qual é o problema do rastreador سے لنک ہیں۔

## اگلے اقدامات

- W1 کھولنے سے پہلے Itens de ação W0 نافذ کریں۔
- قانونی منظوری اور slot de teste de proxy حاصل کریں, پھر [fluxo de convite de visualização](../../preview-invite-flow.md) میں
  بیان کردہ comprovação de onda de parceiro اقدامات پر عمل کریں۔

_یہ خلاصہ [visualizar rastreador de convite](../../preview-invite-tracker.md) سے منسلک ہے تاکہ
DOCS-SORA roadmap قابلِ سراغ رہے۔_
---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w2-resumo
título: W2 فيڈبیک اور اسٹیٹس خلاصہ
sidebar_label: W2 خلاصہ
descrição: onda de visualização da comunidade (W2) کے لئے live digest۔
---

| آئٹم | تفصیل |
| --- | --- |
| Para | W2 - revisores da comunidade |
| دعوتی ونڈو | 15/06/2025 -> 29/06/2025 |
| آرٹیفیکٹ ٹیگ | `preview-2025-06-15` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W2` |
| شرکا | comm-vol-01...comm-vol-08 |

## نمایاں نکات

1. **Ferramentas de governança** - admissão da comunidade motivação/fuso horário فیلڈز کے ساتھ اپ ڈیٹ modelo de solicitação `docs/examples/docs_preview_request_template.md` میں موجود ہے۔
2. **Evidência de comprovação** - Experimente a alteração de proxy `OPS-TRYIT-188` 2025/06/09 کے saídas do descritor/checksum/sonda `artifacts/docs_preview/W2/` میں arquivo کیے گئے۔
3. **Onda de convites** - آٹھ revisores da comunidade کو 15/06/2025 کو مدعو کیا گیا، tabela de convites do rastreador de reconhecimentos میں لاگ ہوئے؛ سب نے navegação سے پہلے verificação de soma de verificação مکمل کیا۔
4. **Feedback** - `docs-preview/w2 #1` (texto da dica de ferramenta) اور `#2` (ordem da barra lateral de localização) 18/06/2025 کو فائل ہوئے اور 21/06/2025 تک حل ہو گئے (Docs-core-04/05)؛ لہر کے دوران کوئی incidentes نہیں ہوئے۔

## ایکشن آئٹمز

| ID | وضاحت | Mal | اسٹیٹس |
| --- | --- | --- | --- |
| W2-A1 | `docs-preview/w2 #1` (texto da dica de ferramenta) حل کرنا۔ | Documentos-núcleo-04 | ✅ مکمل (2025-06-21). |
| W2-A2 | `docs-preview/w2 #2` (barra lateral de localização) حل کرنا۔ | Documentos-núcleo-05 | ✅ مکمل (2025-06-21). |
| W2-A3 | arquivo de evidências de saída کرنا + roteiro/status اپ ڈیٹ کرنا۔ | Líder do Documentos/DevRel | ✅ مکمل (2025-06-29). |

## اختتامی خلاصہ (2025-06-29)

- تمام آٹھ revisores da comunidade نے تکمیل کی تصدیق کی اور acesso de visualização واپس لے لیا گیا؛ registro de convite do rastreador de reconhecimentos میں ریکارڈ ہوئے۔
- Instantâneos de telemetria آخری (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) verde رہے؛ logs اور Experimente transcrições de proxy `DOCS-SORA-Preview-W2` کے ساتھ منسلک ہیں۔
- pacote de evidências (descritor, log de soma de verificação, saída de sonda, relatório de link, capturas de tela Grafana, confirmações de convite) `artifacts/docs_preview/W2/preview-2025-06-15/` Arquivo میں ہوا۔
- rastreador کا Saída de registro do ponto de verificação W2
---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
título: W2 کمیونٹی ingestão پلان
sidebar_label: W2
descrição: کمیونٹی coorte de visualização کے لئے ingestão, aprovações, e lista de verificação de evidências۔
---

| آئٹم | تفصیل |
| --- | --- |
| Para | W2 - Revisores کمیونٹی |
| ہدف ونڈو | 3º trimestre de 2025 ہفتہ 1 (عارضی) |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-06-15` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W2` |

## مقاصد

1. Critérios de admissão e fluxo de trabalho de verificação
2. تجویز کردہ lista e adendo de uso aceitável کے لئے aprovação de governança حاصل کرنا۔
3. checksum سے verificar شدہ visualizar artefato اور pacote de telemetria کو نئی ونڈو کے لئے ریفریش کرنا۔
4. دعوت بھیجنے سے پہلے Experimente proxy ou painéis de controle e estágio کرنا۔

## ٹاسک بریک ڈاؤن

| ID | ٹاسک | Mal | مقررہ تاریخ | اسٹیٹس | Não |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Critérios de admissão de کمیونٹی (elegibilidade, slots máximos, requisitos de CoC) تیار کرنا اور governança کو گردش کرنا | Líder do Documentos/DevRel | 15/05/2025 | ✅ مکمل | admissão پالیسی `DOCS-SORA-Preview-W2` میں mesclar ہوئی اور 2025-05-20 کے conselho میٹنگ میں endossar ہوئی۔ |
| W2-P2 | modelo de solicitação کو کمیونٹی سوالات کے ساتھ اپ ڈیٹ کرنا (motivação, disponibilidade, necessidades de localização) | Documentos-núcleo-01 | 18/05/2025 | ✅ مکمل | `docs/examples/docs_preview_request_template.md` میں اب Comunidade سیکشن شامل ہے, ou ingestão فارم میں حوالہ ہے۔ |
| W2-P3 | entrada پلان کے لئے aprovação de governança حاصل کرنا (votação da reunião + ata registrada) | Ligação para governação | 22/05/2025 | ✅ مکمل | ووٹ 2025-05-20 کو متفقہ طور پر پاس ہوا؛ minutos + chamada `DOCS-SORA-Preview-W2` میں لنک ہیں۔ |
| W2-P4 | W2 ونڈو کے لئے Experimente teste de proxy + captura de telemetria شیڈول کرنا (`preview-2025-06-15`) | Documentos/DevRel + Operações | 05/06/2025 | ✅ مکمل | alterar ticket `OPS-TRYIT-188` منظور اور 2025-06-09 02:00-04:00 UTC میں execute ہوا؛ Capturas de tela Grafana ٹکٹ کے ساتھ arquivo ہیں۔ |
| W2-P5 | Não visualizar tag de artefato (`preview-2025-06-15`) construir/verificar Arquivo de descritor/soma de verificação/arquivo de logs de sonda | PortalTL | 07/06/2025 | ✅ مکمل | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 2025-06-10 کو چلایا گیا؛ saídas `artifacts/docs_preview/W2/preview-2025-06-15/` میں محفوظ ہیں۔ |
| W2-P6 | کمیونٹی lista de convites تیار کرنا (<=25 revisores, lotes organizados) informações de contato aprovadas pela governança کے ساتھ | Gerente de comunidade | 10/06/2025 | ✅ مکمل | پہلے coorte کے 8 revisores da comunidade منظور ہوئے؛ solicitar IDs `DOCS-SORA-Preview-REQ-C01...C08` rastreador میں لاگ ہیں۔ |

## Lista de verificação de evidências

- [x] registro de aprovação de governança (notas da reunião + link de votação) `DOCS-SORA-Preview-W2` کے ساتھ منسلک ہے۔
- [x] Modelo de solicitação atualizado `docs/examples/` کے تحت commit ہے۔
- [x] Descritor `preview-2025-06-15`, log de soma de verificação, saída da sonda, relatório de link, e Tente transcrição de proxy `artifacts/docs_preview/W2/` میں محفوظ ہیں۔
- [x] Capturas de tela Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) Janela de comprovação W2 کے لئے محفوظ ہیں۔
- [x] Tabela de lista de convites میں IDs de revisores, solicitar tickets, e envio de carimbos de data e hora de aprovação سے پہلے بھرے گئے (rastreador کے W2 سیکشن میں دیکھیں)۔

یہ پلان اپ ڈیٹ رکھیں؛ rastreador اسے ریفرنس کرتا ہے تاکہ DOCS-SORA roadmap واضح طور پر دیکھ سکے کہ Convites W2 سے پہلے کیا باقی ہے۔
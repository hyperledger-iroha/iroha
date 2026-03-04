---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-resumo
título: Сводка отзывов и закрытие W1
sidebar_label: Palavra W1
description: Выводы, действия и доказательства выхода для preview-волны партнеров/интеграторов Torii.
---

| Ponto | Detalhes |
| --- | --- |
| Volna | W1 - Parceiros e Integradores Torii |
| Programa de segurança | 12/04/2025 -> 26/04/2025 |
| Artefato | `preview-2025-04-12` |
| Trecker | `DOCS-SORA-Preview-W1` |
| Uчастники | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Momentos incríveis

1. **Soma de verificação do fluxo de trabalho** - Todos os revisores forneceram o descritor/arquivo через `scripts/preview_verify.sh`; логи сохранены рядом с подтверждениями приглашения.
2. **Телеметрия** - Дашборды `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` оставались зелеными на протяжении всей волны; páginas de alerta ou páginas de alerta não foram usadas.
3. **Feedback sobre documentos (`docs-preview/w1`)** - Verifique suas instruções:
   - `docs-preview/w1 #1`: уточнить навигационную формулировку в разделе Try it (закрыто).
   - `docs-preview/w1 #2`: tela de abertura Experimente (Experimente).
4. **Executar runbook** - O operador SoraFS pode ser usado para criar novos links cruzados com `orchestrator-ops` e `multi-source-rollout` Classificação W0.

## Pontos de destino

| ID | Descrição | Владелец | Status |
| --- | --- | --- | --- |
| W1-A1 | Обновить навигационную формулировку Experimente em `docs-preview/w1 #1`. | Documentos-núcleo-02 | ✅ Verdadeiro (18/04/2025). |
| W1-A2 | Обновить скриншот Experimente em `docs-preview/w1 #2`. | Documentos-núcleo-03 | ✅ Verdadeiro (19/04/2025). |
| W1-A3 | Свести выводы партнеров e телеметрию no roteiro/status. | Líder do Documentos/DevRel | ✅ Baixar (com. tracker + status.md). |

## Итоговое резюме (2025/04/26)

- Todos os revisores podem fornecer informações sobre o horário comercial final, definir artefatos locais e serviços públicos entrega.
- Телеметрия оставалась зеленой до выхода; snapshots finais enviados para `DOCS-SORA-Preview-W1`.
- Лог приглашений обновлен подтверждениями выхода; rastreador отметил W1 как 🈴 и добавил pontos de verificação.
- Bundle доказательств (descritor, log de soma de verificação, saída de sonda, transcrição de proxy Try it, capturas de tela de telemetria, resumo de feedback) arquivado em `artifacts/docs_preview/W1/`.

## Следующие шаги

- Подготовить план admissão comunitária W2 (aprovação de governança + modelo de solicitação de правки).
- Обновить preview artefact tag для волны W2 и перезапустить preflight скрипт после финализации дат.
- Перенести применимые выводы W1 в roadmap/status, чтобы community wave получила актуальные рекомендации.
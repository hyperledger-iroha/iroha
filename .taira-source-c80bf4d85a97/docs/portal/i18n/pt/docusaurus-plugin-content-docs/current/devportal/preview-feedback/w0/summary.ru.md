---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w0-resumo
título: Сводка отзывов на середине W0
sidebar_label: Abrir W0 (seção)
description: Контрольные точки середины, выводы e задачи для preview-волны mantenedores principais.
---

| Ponto | Detalhes |
| --- | --- |
| Volna | W0 - mantenedores principais |
| Dados Suíços | 27/03/2025 |
| Okno ревью | 25/03/2025 -> 08/04/2025 |
| Uчастники | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidade-01 |
| Artefato | `preview-2025-03-24` |

## Momentos incríveis

1. **Soma de verificação do fluxo de trabalho** - Seus revisores forneceram, что `scripts/preview_verify.sh`
   успешно прошел против общей пары descritor/arquivo. Substituição de Ручные não
   потребовались.
2. **Навигационный фидбек** - Зафиксированы две небольшие проблемы порядка в sidebar
   (`docs-preview/w0 #1-#2`). Ele foi baixado no Docs/DevRel e não foi bloqueado
   Volnu.
3. **Registre o runbook SoraFS** - sorafs-ops-01
   Eu tenho `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. Заведен
   questão de acompanhamento; Repita para W1.
4. **Ревью телеметрии** - observability-01 подтвердил, что `docs.preview.integrity`,
   `TryItProxyErrors` e логи прокси Try-it оставались зелеными; não há alertas
   срабатывали.

## Pontos de destino

| ID | Descrição | Владелец | Status |
| --- | --- | --- | --- |
| W0-A1 | Ao abrir os pontos da barra lateral do devportal, você encontrará documentos para revisores (`preview-invite-*` сгруппировать вместе). | Documentos-núcleo-01 | Завершено - barra lateral теперь показывает документы revisores подряд (`docs/portal/sidebars.js`). |
| W0-A2 | Insira seu código cruzado com `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Завершено - каждый runbook теперь ссылается на другой, чтобы операторы видели оба гайда во время rollout. |
| W0-A3 | Поделиться телеметрическими снимками + pacote de rastreamento com rastreador de governança. | Observabilidade-01 | Завершено - pacote fornecido para `DOCS-SORA-Preview-W0`. |

## Итоговое резюме (08/04/2025)

- Todos os revisores podem fornecer informações, procurar sites locais e вышли из
  pré-visualização; Os fatos foram fornecidos pelo fabricante em `DOCS-SORA-Preview-W0`.
- Инцидентов и алертов в ходе волны не было; телеметрические дашборды оставались
  зелеными весь período.
- Действия по навигации + кросс-ссылкам (W0-A1/A2) realizáveis e отражены в docs
  você; телеметрические доказательства (W0-A3) usa o rastreador.
- Архив доказательств сохранен: скриншоты телеметрии, подтверждения приглашений и
  Este é um problema com o rastreador de problemas.

## Следующие шаги

- Selecione os pontos de configuração W0 antes do início do W1.
- Получить юридическое одобрение и staging-слот для прокси, затем следовать шагам
  comprovação para todos os parceiros, exibida em [visualizar fluxo de convite](../../preview-invite-flow.md).

_Эта сводка связана из [rastreador de convite de pré-visualização](../../preview-invite-tracker.md), чтобы
сохранять трассируемость roteiro DOCS-SORA._
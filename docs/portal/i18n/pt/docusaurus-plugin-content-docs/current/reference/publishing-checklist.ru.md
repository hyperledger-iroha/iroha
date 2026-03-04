---
lang: pt
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Чек‑лист публикации portal
descrição: Шаги проверки перед обновлением портала документации Iroha.
---

Use esta verificação antes de usar o portal. Na garantia, isso
CI‑сборка, деплой GitHub Pages e ручные smoke‑тесты покрывают все разделы перед релизом
ou nosso roteiro.

## 1. Prova local

- `npm run sync-openapi -- --version=current --latest` (добавьте один или несколько
  флагов `--mirror=<label>`, если Torii OpenAPI меняется para замороженного snapshot'а).
- `npm run build` — убедитесь, что слоган “Construa em Iroha com confiança” mais adiante
  отображается em `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` — manifesto de exibição
  checksum’ов (добавьте `--descriptor`/`--archive` при тестировании скачанных CI‑артефактов).
- `npm run serve` — Executa o helper-режим com o checksum de verificação’ов, que é validado
  manifesto antes de `docusaurus serve`, isso não acontece
  snapshot não solicitado (também conhecido por `serve:verified` остаётся для явных вызовов).
- Просмотрите изменённые markdown-файлы через `npm run start` e dev-сервер com live-reload.

## 2. Solicitação pull-request’а

- Убедитесь, что job `docs-portal-build` executou успешно в
  `.github/workflows/check-docs.yml`.
- Verifique se isso foi feito por `ci/check_docs_portal.sh` (no log CI отображается hero‑проверка).
- Убедитесь, que visualização do fluxo de trabalho contém o manifesto (`build/checksums.sha256`) e isso
  скрипт проверки preview выполнился успешно (логи CI содержат вывод
  `scripts/preview_verify.sh`).
- Baixe o URL de visualização disponível nas páginas do GitHub na descrição PR.

## 3. Подписание по разделам

| Razdel | Владелец | Lista de verificação |
|--------|----------|----------|
| Glавная | DevRel | Hero‑копирайт отображается; início rápido de cartões ведут на валидные маршруты; CTA‑кнопки работают. |
| Norito | Norito WG | Selecione e execute o trabalho selecionado na CLI-флаги atual e no Norito-schema-доки. |
| SoraFS | Equipe de armazenamento | O início rápido é necessário para a conexão, através do qual o manifesto é iniciado, as instruções para a simulação de busca são comprovadas. |
| SDK-гайды | Lidos SDK | Rust/Python/JS-гайды são exemplos atuais e usados ​​em repositórios atuais. |
| Referência | Documentos/DevRel | O índice indica as especificações específicas do codec Norito compatível com o `norito.md`. |
| Pré-visualização do artefacto | Documentos/DevRel | Артефакт `docs-portal-preview` прикреплён к PR, smoke-proверки пройдены, ссылка опубликована для ревьюеров. |
| Segurança e experimente sandbox | Documentos/DevRel · Segurança | Faça login com código de dispositivo OAuth (`DOCS_OAUTH_*`), selecione `security-hardening.md`, verifique CSP/Trusted Types usando `npm run build` ou `npm run probe:portal`. |

Отметьте каждую строку в ходе review PR'а ou зафиксируйте follow-up-задачи, чтобы статус
оставался точным.

## 4. Notas de lançamento

- Selecione `https://docs.iroha.tech/` (ou URL de configuração da implementação do trabalho) nas notas de versão
  e aplicativos de status.
- Явно перечисляйте новые или изменённые разделы, чтобы downstream‑команды понимали,
  какие smoke‑testы им требуется перезапустить.
---
lang: pt
direction: ltr
source: docs/portal/docs/reference/README.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Индекс справочников
slug: /referência
---

Este foi o material fornecido para o Iroha. Esta página é estabelecida por mais de uma semana, através de jogos e tutoriais.

## Segunda temporada

- **Обзор кодека Norito** - `reference/norito-codec.md` напрямую ссылается на авторитетную спецификацию `norito.md`, пока таблица портала заполняется.
- **Torii OpenAPI** - `/reference/torii-openapi` отображает последнюю спецификацию REST Torii через Redoc. Altere o comando spec `npm run sync-openapi -- --version=current --latest` (use `--mirror=<label>` para copiar snapshot no histórico de download versão).
- **Configurações de configuração** - Verifique a configuração dos parâmetros do catálogo em `docs/source/references/configuration.md`. Se o portal não permitir a importação automática, você precisará deste Markdown para obter o valor desejado e переопределениями окружения.
- **Documentos de documentação** - Выпадающий список версий навбаре показывает замороженные snapshots, созданные с помощью `npm run docs:version -- <label>`, este é o método de avaliação mais recomendado.
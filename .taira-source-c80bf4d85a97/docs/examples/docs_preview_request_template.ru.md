---
lang: ru
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-11-10T20:01:03.610024+00:00"
translation_last_reviewed: 2026-01-01
---

# Запрос доступа к preview портала docs (шаблон)

Используйте этот шаблон при сборе сведений о ревьюере перед выдачей доступа к публичной
среде preview. Скопируйте markdown в тикет или форму запроса и замените плейсхолдеры.

```markdown
## Сводка запроса
- Заявитель: <полное имя / орг>
- GitHub handle: <username>
- Предпочитаемый контакт: <email/Matrix/Signal>
- Регион и часовой пояс: <UTC offset>
- Предлагаемые даты начала / окончания: <YYYY-MM-DD -> YYYY-MM-DD>
- Тип ревьюера: <Core maintainer | Partner | Community volunteer>

## Чек-лист соответствия
- [ ] Подписал политику приемлемого использования preview (link).
- [ ] Ознакомился с `docs/portal/docs/devportal/security-hardening.md`.
- [ ] Ознакомился с `docs/portal/docs/devportal/incident-runbooks.md`.
- [ ] Подтвердил сбор телеметрии и обезличенную аналитику (да/нет).
- [ ] Запрошен alias для SoraFS (да/нет). Имя alias: `<docs-preview-???>`

## Потребности доступа
- URL(ы) preview: <https://docs-preview.sora.link/...>
- Требуемые API scopes: <Torii read-only | Try it sandbox | none>
- Дополнительный контекст (SDK тесты, фокус ревью документации и т.д.):
  <детали здесь>

## Утверждение
- Ревьюер (maintainer): <имя + дата>
- Тикет governance / запрос на изменение: <ссылка>
```

---

## Вопросы для сообщества (W2+)
- Мотивация для доступа к preview (одно предложение):
- Основной фокус ревью (SDK, governance, Norito, SoraFS, другое):
- Еженедельная занятость и окно доступности (UTC):
- Нужна ли локализация или доступность (да/нет + детали):
- Подтвержден Кодекс поведения сообщества + addendum по acceptable-use preview (да/нет):

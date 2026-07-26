---
lang: ru
direction: ltr
source: docs/examples/docs_preview_invite_template.md
status: complete
translator: manual
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2026-01-01
---

# Приглашение на превью портала docs (шаблон)

Используйте этот шаблон при отправке инструкций доступа к превью для ревьюеров. Замените
плейсхолдеры (`<...>`) актуальными значениями, приложите артефакты descriptor +
archive, указанные в сообщении, и сохраните финальный текст в соответствующем intake-тикете.

```text
Тема: [DOCS-SORA] приглашение на превью портала docs <preview_tag> для <reviewer/org>

Здравствуйте <name>,

Спасибо, что согласились помочь с обзором портала docs перед GA. Вы допущены к волне
<wave_id>. Пожалуйста, выполните шаги ниже перед просмотром превью:

1. Скачайте проверенные артефакты из CI или SoraFS:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archive: <archive_url> (`sha256:<archive_sha256>`)
2. Запустите gate проверки checksum:

   ./docs/portal/scripts/preview_verify.sh      --descriptor <path-to-descriptor>      --archive <path-to-archive>      --build-dir <path-to-extracted-build>

3. Запустите превью с включенным enforcement checksum:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. Ознакомьтесь с заметками по acceptable-use, security и observability:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. Отправляйте feedback через <request_ticket> и помечайте каждое замечание тегом
   `<preview_tag>`.

Поддержка доступна в <contact_channel>. Инциденты или вопросы безопасности нужно
немедленно сообщать через <incident_channel>. Если нужны токены Torii API, запросите их
через тикет; никогда не переиспользуйте production credentials.

Доступ к превью истекает <end_date>, если не будет продлен письменно. Мы логируем
checksums и метаданные приглашения для governance; сообщите, когда закончите,
чтобы мы могли корректно снять доступ.

Спасибо еще раз за помощь в стабилизации портала!

- Команда DOCS-SORA
```

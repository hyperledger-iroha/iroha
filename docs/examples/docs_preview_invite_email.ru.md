---
lang: ru
direction: ltr
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-11-15T04:49:30.881970+00:00"
translation_last_reviewed: 2026-01-01
---

# Приглашение на превью портала docs (пример письма)

Используйте этот пример при подготовке исходящего сообщения. Он фиксирует точную
копию письма, отправленного комьюнити-ревьюерам волны W2 (`preview-2025-06-15`), чтобы
следующие волны могли повторить тон, инструкции по проверке и цепочку доказательств
без поиска по старым тикетам. Перед отправкой нового инвайта обновите ссылки на
артефакты, хеши, request IDs и даты.

```text
Тема: [DOCS-SORA] приглашение на превью портала docs preview-2025-06-15 для Horizon Wallet

Здравствуйте, Sam,

Спасибо, что снова подключили Horizon Wallet к комьюнити-превью W2. Волна W2
утверждена, так что вы можете начать обзор сразу после выполнения шагов ниже.
Пожалуйста, держите артефакты и токены доступа в тайне: каждый инвайт фиксируется
в DOCS-SORA-Preview-W2, а аудиторы сверят подтверждения.

1. Скачайте проверенные артефакты (те же bits, что мы отправили в SoraFS и CI):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. Проверьте bundle перед распаковкой:

   ./docs/portal/scripts/preview_verify.sh      --descriptor ~/Downloads/descriptor.json      --archive ~/Downloads/docs-portal-preview.tar.zst      --build-dir ~/sora-docs/preview-2025-06-15

3. Запустите превью с enforcement checksum:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. Перед тестированием изучите укрепленные runbooks:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. Отправляйте feedback через DOCS-SORA-Preview-REQ-C04 и помечайте каждое замечание
   `docs-preview/w2`. Если нужен структурированный intake, используйте форму:
   docs/examples/docs_preview_feedback_form.md.

Поддержка доступна в Matrix (`#docs-preview:matrix.org`), также есть office hours
2025-06-18 15:00 UTC. По вопросам безопасности или инцидентов сразу пейджите
alias on-call docs через ops@sora.org или +1-555-0109; не ждите office hours.

Доступ к превью для Horizon Wallet действует 2025-06-15 -> 2025-06-29. Сообщите,
когда завершите, чтобы мы отозвали временные ключи доступа и зафиксировали закрытие
в трекере.

Спасибо за помощь в подготовке портала к GA!

- Команда DOCS-SORA
```

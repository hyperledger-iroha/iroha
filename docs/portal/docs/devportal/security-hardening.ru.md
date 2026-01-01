---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 703ab06de69620a6930c5aae80a1fa310010a938b4db2b366003ea0ddd62ae8a
source_last_modified: "2025-12-19T22:35:27+00:00"
translation_last_reviewed: 2026-01-01
---

# Усиление безопасности и чеклист пен-теста

## Обзор

Пункт дорожной карты **DOCS-1b** требует OAuth device-code login, сильных политик
безопасности контента и повторяемых penetration tests прежде чем preview портал сможет
работать в сетях вне лаборатории. Это приложение описывает модель угроз, реализованные
в репозитории контроли и go-live чеклист, который должны выполнить gate reviews.

- **В рамках:** прокси Try it, встроенные панели Swagger/RapiDoc и кастомная
  консоль Try it, рендеримая `docs/portal/src/components/TryItConsole.jsx`.
- **Вне рамок:** сам Torii (покрывается Torii readiness reviews) и публикация SoraFS
  (покрывается DOCS-3/7).

## Модель угроз

| Актив | Риск | Митигация |
| --- | --- | --- |
| Torii bearer токены | Кража или повторное использование вне docs sandbox | device-code login (`DOCS_OAUTH_*`) выпускает короткоживущие токены, прокси редактирует headers, а консоль автоматически истекает кэшированные креды. |
| Try it прокси | Злоупотребление как открытым реле или обход лимитов Torii | `scripts/tryit-proxy*.mjs` enforced origin allowlists, rate limiting, health probes и явный `X-TryIt-Auth` forwarding; креды не сохраняются. |
| Runtime портала | Cross-site scripting или вредоносные embeds | `docusaurus.config.js` внедряет Content-Security-Policy, Trusted Types и Permissions-Policy headers; inline scripts ограничены runtime Docusaurus. |
| Данные наблюдаемости | Отсутствие телеметрии или подмена | `docs/portal/docs/devportal/observability.md` документирует probes/dashboards; `scripts/portal-probe.mjs` запускается в CI перед публикацией. |

Противники включают любопытных пользователей публичного preview, злоумышленников, тестирующих украденные ссылки,
и скомпрометированные браузеры, пытающиеся вытащить сохраненные креды. Все контроли должны работать
в обычных браузерах без доверенных сетей.

## Требуемые контроли

1. **OAuth device-code login**
   - Настройте `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` и связанные knobs в build окружении.
   - Карта Try it рендерит sign-in widget (`OAuthDeviceLogin.jsx`), который
     получает device code, опрашивает token endpoint и автоматически очищает токены
     после истечения. Ручные Bearer overrides остаются доступными для экстренного fallback.
   - Сборки теперь падают, если отсутствует OAuth конфигурация или если fallback TTLs
     выходят за окно 300-900 s, предписанное DOCS-1b; установите
     `DOCS_OAUTH_ALLOW_INSECURE=1` только для одноразовых локальных preview.
2. **Proxy guardrails**
   - `scripts/tryit-proxy.mjs` применяет allowed origins, rate limits, caps на размер запроса
     и upstream timeouts, одновременно помечая трафик `X-TryIt-Client` и редактируя токены
     в логах.
   - `scripts/tryit-proxy-probe.mjs` плюс `docs/portal/docs/devportal/observability.md`
     определяют liveness probe и правила dashboard; запускайте их перед каждым rollout.
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` теперь экспортирует детерминированные security headers:
     `Content-Security-Policy` (default-src self, строгие списки connect/img/script,
     требования Trusted Types), `Permissions-Policy`, и
     `Referrer-Policy: no-referrer`.
   - Список connect CSP whitelist'ит OAuth device-code и token endpoints
     (только HTTPS, если не задан `DOCS_SECURITY_ALLOW_INSECURE=1`), чтобы device login
     работал без ослабления sandbox для других origins.
   - Эти headers встроены прямо в сгенерированный HTML, поэтому статические хосты
     не требуют дополнительной конфигурации. Держите inline scripts ограниченными
     Docusaurus bootstrap.
4. **Runbooks, observability и rollback**
   - `docs/portal/docs/devportal/observability.md` описывает probes и dashboards, которые
     следят за login failures, proxy response codes и request budgets.
   - `docs/portal/docs/devportal/incident-runbooks.md` описывает путь эскалации
     при злоупотреблении sandbox; комбинируйте с
     `scripts/tryit-proxy-rollback.mjs` для безопасного переключения endpoints.

## Чеклист пен-теста и релиза

Выполните этот список для каждого продвижения preview (приложите результаты к release тикету):

1. **Проверить OAuth wiring**
   - Запустите `npm run start` локально с production `DOCS_OAUTH_*` exports.
   - Из чистого профиля браузера откройте Try it консоль и убедитесь, что device-code flow
     выдает токен, считает время жизни и очищает поле после истечения или sign-out.
2. **Пробить прокси**
   - `npm run tryit-proxy` против staging Torii, затем выполните
     `npm run probe:tryit-proxy` с настроенным sample path.
   - Проверьте логи на `authSource=override` и убедитесь, что rate limiting
     увеличивает счетчики при превышении окна.
3. **Подтвердить CSP/Trusted Types**
   - `npm run build` и откройте `build/index.html`. Убедитесь, что тег `<meta
     http-equiv="Content-Security-Policy">` соответствует ожидаемым директивам
     и что DevTools не показывает CSP violations при загрузке preview.
   - Используйте `npm run probe:portal` (или curl), чтобы получить развернутый HTML; probe
     теперь падает, если meta tags `Content-Security-Policy`, `Permissions-Policy` или
     `Referrer-Policy` отсутствуют или отличаются от значений в
     `docusaurus.config.js`, так что reviewers governance могут доверять
     exit code вместо ручной проверки curl output.
4. **Проверить observability**
   - Убедитесь, что dashboard Try it proxy зеленый (rate limits, error ratios,
     health probe metrics).
   - Запустите incident drill из `docs/portal/docs/devportal/incident-runbooks.md`,
     если хост изменился (новое развертывание Netlify/SoraFS).
5. **Документировать результаты**
   - Приложите screenshots/logs к release тикету.
   - Зафиксируйте каждую находку в шаблоне remediation report
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     чтобы owners, SLAs и evidence для retest было легко аудировать позже.
   - Сошлитесь на этот чеклист, чтобы пункт дорожной карты DOCS-1b оставался auditable.

Если любой шаг провален, остановите продвижение, заведите blocking issue и зафиксируйте план remediation в `status.md`.

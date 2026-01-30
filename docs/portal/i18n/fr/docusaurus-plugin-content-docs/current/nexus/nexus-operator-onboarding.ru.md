---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-operator-onboarding
title: Онбординг операторов data-space Sora Nexus
description: Зеркало `docs/source/sora_nexus_operator_onboarding.md`, отслеживающее end-to-end чеклист релиза для операторов Nexus.
---

:::note Канонический источник
Эта страница отражает `docs/source/sora_nexus_operator_onboarding.md`. Держите обе копии синхронизированными, пока локализованные издания не попадут в портал.
:::

# Onboarding операторов data-space Sora Nexus

Этот гайд описывает end-to-end поток, которому должны следовать операторы data-space Sora Nexus после объявления релиза. Он дополняет dual-track runbook (`docs/source/release_dual_track_runbook.md`) и заметку по выбору артефактов (`docs/source/release_artifact_selection.md`), описывая, как согласовать скачанные bundles/images, manifests и шаблоны конфигурации с глобальными ожиданиями по lanes до вывода узла в онлайн.

## Аудитория и предпосылки
- Вы одобрены программой Nexus и получили назначение data-space (lane index, data-space ID/alias и требования routing policy).
- У вас есть доступ к подписанным release artifacts от Release Engineering (tarballs, images, manifests, signatures, public keys).
- Вы сгенерировали или получили продакшн ключевой материал для роли validator/observer (Ed25519 идентичность узла; BLS консенсусный ключ + PoP для validators; плюс любые toggles конфиденциальных функций).
- Вы можете достучаться до существующих Sora Nexus peers, которые будут bootstrap-ить ваш узел.

## Шаг 1 - Подтвердить профиль релиза
1. Определите alias сети или chain ID, который вам выдали.
2. Запустите `scripts/select_release_profile.py --network <alias>` (или `--chain-id <id>`) в checkout этого репозитория. Helper использует `release/network_profiles.toml` и выводит профиль деплоя. Для Sora Nexus ответ должен быть `iroha3`. При любом другом значении остановитесь и свяжитесь с Release Engineering.
3. Зафиксируйте tag версии из анонса релиза (например `iroha3-v3.2.0`); он понадобится для загрузки artifacts и manifests.

## Шаг 2 - Получить и проверить артефакты
1. Скачайте bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) и его сопутствующие файлы (`.sha256`, опциональные `.sig/.pub`, `<profile>-<version>-manifest.json`, и `<profile>-<version>-image.json`, если вы деплоите контейнеры).
2. Проверьте целостность перед распаковкой:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Замените `openssl` на верификатор, одобренный организацией, если используете аппаратный KMS.
3. Просмотрите `PROFILE.toml` внутри tarball и JSON manifests, чтобы убедиться:
   - `profile = "iroha3"`
   - Поля `version`, `commit`, `built_at` совпадают с анонсом релиза.
   - OS/архитектура соответствуют цели деплоя.
4. Если используется container image, повторите проверку hash/signature для `<profile>-<version>-<os>-image.tar` и подтвердите image ID из `<profile>-<version>-image.json`.

## Шаг 3 - Подготовка конфигурации из шаблонов
1. Распакуйте bundle и скопируйте `config/` туда, где узел будет читать конфигурацию.
2. Относитесь к файлам под `config/` как к шаблонам:
   - Замените `public_key`/`private_key` на продакшн Ed25519 ключи. Удалите приватные ключи с диска, если узел берет их из HSM; обновите конфигурацию, чтобы указывать на HSM connector.
   - Настройте `trusted_peers`, `network.address`, и `torii.address` так, чтобы они отражали ваши доступные интерфейсы и назначенные bootstrap peers.
   - Обновите `client.toml` с operator-facing Torii endpoint (включая TLS конфигурацию при необходимости) и учетными данными для операционного tooling.
3. Сохраняйте chain ID из bundle, если только Governance явно не указала иначе - глобальная lane ожидает единый канонический chain identifier.
4. Планируйте запуск узла с флагом профиля Sora: `irohad --sora --config <path>`. Конфигурационный loader отклонит настройки SoraFS или multi-lane, если флаг отсутствует.

## Шаг 4 - Согласовать метаданные data-space и routing
1. Отредактируйте `config/config.toml`, чтобы раздел `[nexus]` соответствовал каталогу data-space, выданному Nexus Council:
   - `lane_count` должен равняться общему числу lanes, включенных в текущей эпохе.
   - Каждая запись в `[[nexus.lane_catalog]]` и `[[nexus.dataspace_catalog]]` должна содержать уникальный `index`/`id` и согласованные aliases. Не удаляйте существующие глобальные записи; добавьте делегированные aliases, если совет назначил дополнительные data-spaces.
   - Убедитесь, что каждая запись dataspace включает `fault_tolerance (f)`; комитеты lane-relay имеют размер `3f+1`.
2. Обновите `[[nexus.routing_policy.rules]]`, чтобы отразить выданную политику. Шаблон по умолчанию направляет governance инструкции на lane `1` и деплой контрактов на lane `2`; добавьте или измените правила так, чтобы трафик для вашего data-space шел в правильный lane и alias. Согласуйте с Release Engineering перед изменением порядка правил.
3. Проверьте пороги `[nexus.da]`, `[nexus.da.audit]`, и `[nexus.da.recovery]`. Ожидается, что операторы сохраняют значения, одобренные советом; изменяйте их только при ратификации обновленной политики.
4. Зафиксируйте финальную конфигурацию в операционном трекере. Dual-track runbook требует прикрепить эффективный `config.toml` (с редактированием секретов) к тикету онбординга.

## Шаг 5 - Предполетная валидация
1. Запустите встроенный валидатор конфигурации перед подключением к сети:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Команда выводит разрешенную конфигурацию и падает рано, если записи каталога/маршрутизации неконсистентны или genesis и config расходятся.
2. Если вы деплоите контейнеры, выполните ту же команду внутри образа после загрузки `docker load -i <profile>-<version>-<os>-image.tar` (не забудьте `--sora`).
3. Проверьте логи на предупреждения о placeholder идентификаторах lane/data-space. Если они есть, вернитесь к Шагу 4 - продакшн деплой не должен полагаться на placeholder IDs из шаблонов.
4. Выполните локальный smoke сценарий (например, отправьте `FindNetworkStatus` запрос через `iroha_cli`, проверьте что telemetry endpoints публикуют `nexus_lane_state_total`, и убедитесь, что streaming keys были ротированы или импортированы).

## Шаг 6 - Cutover и hand-off
1. Сохраните проверенный `manifest.json` и signature артефакты в релизном тикете, чтобы аудиторы могли воспроизвести проверки.
2. Уведомите Nexus Operations, что узел готов к вводу; включите:
   - Идентичность узла (peer ID, hostnames, Torii endpoint).
   - Эффективные значения каталога lane/data-space и routing policy.
   - Хэши проверенных бинарников/образов.
3. Скоординируйте финальный прием peers (gossip seeds и назначение lane) с `@nexus-core`. Не подключайтесь к сети, пока не получите одобрение; Sora Nexus требует детерминированной occupancy lanes и обновленного admissions manifest.
4. После ввода узла в строй обновите runbooks с вашими overrides и зафиксируйте tag релиза, чтобы следующая итерация стартовала с этой baseline.

## Справочный чеклист
- [ ] Профиль релиза подтвержден как `iroha3`.
- [ ] Хэши и подписи bundle/image проверены.
- [ ] Ключи, адреса peers и Torii endpoints обновлены до продакшн значений.
- [ ] Каталог lanes/dataspace и routing policy Nexus соответствуют назначению совета.
- [ ] Валидатор конфигурации (`irohad --sora --config ... --trace-config`) проходит без предупреждений.
- [ ] Manifests/signatures заархивированы в тикете онбординга и Ops уведомлен.

Для более широкого контекста о фазах миграции Nexus и ожиданиях по телеметрии см. [Nexus transition notes](./nexus-transition-notes).

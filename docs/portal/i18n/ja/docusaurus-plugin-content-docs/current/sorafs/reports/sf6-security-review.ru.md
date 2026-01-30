---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Отчет по безопасности SF-6
summary: Результаты и последующие действия по независимой оценке keyless signing, proof streaming и пайплайнов отправки manifests.
---

# Отчет по безопасности SF-6

**Окно оценки:** 2026-02-10 → 2026-02-18  
**Лиды проверки:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Область:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), proof streaming APIs, обработка manifests в Torii, интеграция Sigstore/OIDC, CI release hooks.  
**Артефакты:**  
- Исходники CLI и тесты (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii handlers manifest/proof (`crates/iroha_torii/src/sorafs/api.rs`)  
- Release automation (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Deterministic parity harness (`crates/sorafs_car/tests/sorafs_cli.rs`, [Отчет о паритете GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## Методология

1. **Threat modelling workshops** отразили возможности атакующих для рабочих станций разработчиков, CI систем и Torii узлов.  
2. **Code review** сфокусировался на поверхностях учетных данных (обмен токенами OIDC, keyless signing), валидации Norito manifests и back-pressure в proof streaming.  
3. **Dynamic testing** воспроизводило fixture manifests и симулировало сбои (token replay, manifest tampering, усеченные proof streams) с помощью parity harness и специальных fuzz drives.  
4. **Configuration inspection** проверило defaults `iroha_config`, обработку флагов CLI и release scripts, чтобы обеспечить детерминированные и аудируемые прогоны.  
5. **Process interview** подтвердило remediation flow, escalation paths и сбор audit evidence совместно с release owners Tooling WG.

## Сводка находок

| ID | Severity | Area | Finding | Resolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | High | Keyless signing | Default аудитория OIDC tokens была неявной в CI templates, что создавало риск cross-tenant replay. | Добавлено явное требование `--identity-token-audience` в release hooks и CI templates ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI теперь падает, если аудитория не указана. |
| SF6-SR-02 | Medium | Proof streaming | Back-pressure paths принимали неограниченные буферы подписчиков, что позволяло исчерпать память. | `sorafs_cli proof stream` ограничивает размеры каналов с детерминированным truncation, логирует Norito summaries и abort'ит поток; Torii mirror обновлен, чтобы ограничить response chunks (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medium | Отправка manifests | CLI принимал manifests без проверки встроенных chunk plans, когда `--plan` отсутствовал. | `sorafs_cli manifest submit` теперь пересчитывает и сравнивает CAR digests, если не указан `--expect-plan-digest`, отклоняя mismatches и выдавая remediation hints. Тесты покрывают успех/ошибки (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Low | Audit trail | В release checklist не было подписанного журнала утверждения security review. | Добавлен раздел [release process](../developer-releases.md), требующий приложить hashes memo review и URL тикета sign-off перед GA. |

Все high/medium находки были исправлены в окно ревью и подтверждены действующим parity harness. Критических скрытых проблем не осталось.

## Валидация контролей

- **Credential scope:** CI templates теперь требуют явных audience и issuer утверждений; CLI и release helper завершаются ошибкой, если `--identity-token-audience` не указан вместе с `--identity-token-provider`.  
- **Deterministic replay:** Обновленные тесты покрывают положительные/отрицательные потоки отправки manifests, гарантируя, что mismatched digests остаются недетерминированными ошибками и выявляются до обращения к сети.  
- **Proof streaming back-pressure:** Torii теперь стримит PoR/PoTR items через ограниченные каналы, а CLI хранит только усеченные samples latency + пять примеров отказов, предотвращая неограниченный рост подписчиков и сохраняя детерминированные summaries.  
- **Observability:** Счетчики proof streaming (`torii_sorafs_proof_stream_*`) и CLI summaries фиксируют причины abort, давая операторам audit breadcrumbs.  
- **Documentation:** Гайды для разработчиков ([developer index](../developer-index.md), [CLI reference](../developer-cli.md)) отмечают security-sensitive флаги и escalation workflows.

## Дополнения к release checklist

Release managers **обязаны** приложить следующие доказательства при продвижении GA кандидата:

1. Hash последнего memo security review (этот документ).  
2. Ссылка на тикет remediation (например, `governance/tickets/SF6-SR-2026.md`).  
3. Output `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` с явными аргументами audience/issuer.  
4. Логи parity harness (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Подтверждение, что release notes Torii включают telemetry counters bounded proof streaming.

Непредоставление артефактов выше блокирует GA sign-off.

**Reference hashes артефактов (sign-off 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Оставшиеся follow-ups

- **Threat model refresh:** Повторять эту ревизию ежеквартально или перед крупными добавлениями флагов CLI.  
- **Fuzzing coverage:** Транспортные кодировки proof streaming fuzz'ятся через `fuzz/proof_stream_transport`, охватывая payloads identity, gzip, deflate и zstd.  
- **Incident rehearsal:** Запланировать операторское упражнение, симулирующее компрометацию токена и rollback manifest, чтобы документация отражала отработанные процедуры.

## Approval

- Представитель Security Engineering Guild: @sec-eng (2026-02-20)  
- Представитель Tooling Working Group: @tooling-wg (2026-02-20)

Храните подписанные approvals вместе с release artefact bundle.

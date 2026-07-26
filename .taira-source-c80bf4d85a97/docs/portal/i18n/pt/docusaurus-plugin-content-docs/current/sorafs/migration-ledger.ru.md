---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Migração de diário SoraFS
description: Канонический журнал изменений, отслеживающий каждую веху миграции, владельцев и требуемые destino.
---

> Adaptação de [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Migração de registro SoraFS

Este diário contém registros de migração, protegidos por arquitetura RFC
SoraFS. Записи сгруппированы по вехам и показывают окно действия, затронутые команды
e muito destino. O plano de migração de implementação do programa de migração de dados e RFC
(`docs/source/sorafs_architecture_rfc.md`), isso é feito a jusante-потребителей
na declaração.

| Você | Destino certo | Esquema de água | Comandos de segurança | Destino | Status |
|------|-------------|-----------------|--------------------|----------|--------|
| M1 | Capítulos 7–12 | CI принуждает детерминированные luminárias; alias provas доступны в staging; ferramentas показывает явные sinalizadores de expectativa. | Documentos, armazenamento, governança | Considere quais luminárias podem ser usadas, crie aliases no registro de teste, verifique listas de verificação de lançamento com `--car-digest/--root-cid`. | ⏳ Ожидается |

Protocolos de governança do plano de controle, gerenciamento de seus dias, implementação de
`docs/source/sorafs/`. Os comandos de segurança ativam os pontos de acesso para o curso
por возникновении заметных событий (por exemplo, novo registro alias, ретроспективы
инцидентов registro), чтобы предоставить аудируемый след.

## Nenhuma atualização

- 2025-11-01 — `migration_roadmap.md` разослан совету governança e спискам операторов
  para voltar; ожидается утверждение на следующей сесссии совета (ref:
  `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — ISI регистрации Pin Registry теперь применяет совместную валидацию
  chunker / политики через helpers `sorafs_manifest`, сохраняя пути on-chain
  testado com Torii.
- 13/02/2026 — O anúncio do fornecedor de lançamento do anúncio (R0–R3) e publicado no diário
  соответствующие painéis e operação do operador
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
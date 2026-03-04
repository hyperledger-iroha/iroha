---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-charter
título: Хартия реестра chunker SoraFS
sidebar_label: Хартия реестра chunker
description: Хартия управления для подачи утверждения профилей chunker.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/chunker_registry_charter.md`. Se você tiver uma cópia sincronizada, a estrela do Sphinx não estará disponível na configuração.
:::

# Хартия управления реестром chunker SoraFS

> **Relatório:** 2025-10-29 Painel de Infraestrutura do Parlamento Sora (см.
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Любые поправки требуют
> формального голосования по governança; команды внедрения должны считать этот документ
> Normas, mas não há nenhuma nova compra.

Este cartão é um processo e uma função para restaurar o chunker SoraFS.
Para obter [Руководство по авторингу профилей chunker](./chunker-profile-authoring.md), описывая, как новые
профили предлагаются, рассматриваются, ратифицируются e в итоге выводятся из обращения.

##Oblado

Hartiya применяется к каждой записи в `sorafs_manifest::chunker_registry` и
para ferramentas simples, você pode usar o réестр (CLI manifesto, CLI de anúncio de provedor,
SDK). Sobre o nome de usuário e o identificador, prove
`chunker_registry::ensure_charter_compliance()`:

- Perfil de ID — положительные целые числа, монотонно возрастающие.
- Alça canônica `namespace.name@semver` **должен** быть первой записью
- Строки alias обрезаны, уникальны и не конфликтуют с каноническими handle других записей.

##Roli

- **Автор(ы)** – готовят предложение, regенерируют fixtures e собирают
  доказательства детерминизма.
- **Grupo de Trabalho de Ferramentas (TWG)** – валидирует предложение по опубликованным
  чеклистам и убеждается, что инварианты реестра соблюдены.
- **Conselho de Governança (GC)** – рассматривает отчет TWG, подписывает конверт предложения
  e утверждает сроки публикации/депрекации.
- **Equipe de armazenamento** – você pode realizar a restauração e publicação
  documentação atualizada.

## Ciclo livre

1. **Prедложения**
   - Автор запускает чеклист валидации из руководства по авторингу и создает
     JSON `ChunkerProfileProposalV1` em
     `docs/source/sorafs/proposals/`.
   - Abra sua CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Отправляет PR, содержащий luminárias, предложение, отчет о детерминизме и
     обновления реестра.

2. **Ferramentas de segurança (TWG)**
   - Повторяет чеклист валидации (fixtures, fuzz, pipeline manifest/PoR).
   - Запускает `cargo test -p sorafs_car --chunker-registry` e убеждается, что
     `ensure_charter_compliance()` fornece novas informações.
   - Verifique, este é o CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) отражает обновленные alias e handle.
   - Готовит краткий отчет с выводами и статусом passa/falha.

3. **Sobrevivência Social (GC)**
   - Рассматривает отчет TWG e метаданные предложения.
   - Подписывает resumo предложения (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     e добавляет подписи в конверт совета, который хранится рядом с fixtures.
   - Financiar o resultado do protocolo de governação.4. **Publicação**
   - Мержит PR, responsável:
     -`sorafs_manifest::chunker_registry_data`.
     - Documentação (`chunker_registry.md`, руководства по авторингу/соответствию).
     - Luminárias e opções de determinação.
   - Configure operadores e comandos SDK para um novo perfil e plano de implementação.

5. **Explicação / Compra**
   - Предложения, заменяющие существующий perfil, должны включать окно двойной публикации
     (грейс-периоды) e planejar atualização.
     в реестре и обновить registro de migração.

6. **Especificação de Escrita**
   - Удаление или hotfix требуют голосования совета с большинством.
   - TWG fornece documentação para obter informações sobre riscos e obter informações do diário.

## Otimização de ferramentas

- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` são fornecidos:
  - `--list-profiles` para restauração de inspeção.
  - `--promote-profile=<handle>` para geração de bloco canônico,
    use o perfil fornecido.
  - `--json-out=-` para controle de saída em stdout, verificando a configuração
    логи ревью.
- `ensure_charter_compliance()` é usado por um binário relevante
  (`manifest_chunk_store`, `provider_advert_stub`). CI тесты должны падать, если
  Novas informações estão disponíveis para você.

## Documentação

- Verifique se você está determinado em `docs/source/sorafs/reports/`.
- Os protocolos são baseados em configurações para o chunker instalado em
  `docs/source/sorafs/migration_ledger.md`.
- Verifique `roadmap.md` e `status.md` após a restauração da imagem.

## Ссылки

- Руководство по авторингу: [Руководство по авторингу профилей chunker](./chunker-profile-authoring.md)
- Solução de verificação: `docs/source/sorafs/chunker_conformance.md`
- Справочник реестра: [Реестр профилей chunker](./chunker-registry.md)
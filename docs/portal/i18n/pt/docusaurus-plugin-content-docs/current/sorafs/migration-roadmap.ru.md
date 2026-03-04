---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Дорожная карта миграции SoraFS"
---

> Adaptação de [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Дорожная карта миграции SoraFS (SF-1)

Este documento contém recomendações de operação para migração, segurança em
`docs/source/sorafs_architecture_rfc.md`. Em termos de entregas SF-1 em
готовые к исполнению вехи, критерии гейтов e чек-листы владельцев, чтобы команды
artefactos publicados na base SoraFS.

Дорожная карта намеренно детерминирована: каждая веха называет необходимые
artefactos, comandos e certificados, projetos downstream-пайплайны производили
идентичные выходы, uma governança сохраняла аудируемый след.

## Обзор вех

| Você | Okno | Основные цели | A entrega foi feita | Владельцы |
|------|------|---------------|-------------|-----------|
| **M1 - Aplicação Determinística** | Dias 7-12 | Para usar fixtures e usar alias proofs, você pode usar sinalizadores de expectativa. | Não verifique luminárias, manuseie-as com informações atualizadas, identifique-as no registro de alias. | Armazenamento, governança, SDKs |

O status está definido em `docs/source/sorafs/migration_ledger.md`. Qual é a sua situação
neste cartão de crédito, o livro razão, governança e engenharia de liberação
оставались синхронизированы.

## Потоки работ

### 2. Como determinar a fixação

| Shag | Você | Descrição | Proprietário(s) | Você |
|-----|------|----------|----------|-------|
| Repetições eléctricas | M0 | Еженедельные simulações, сравнивающие локальные chunk digests с `fixtures/sorafs_chunker`. Publicado em `docs/source/sorafs/reports/`. | Provedores de armazenamento | `determinism-<date>.md` é uma matriz aprovada/reprovada. |
| Принудить подписи | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` é usado para derivar ou manifestar. Dev substitui a renúncia de governança em PR. | GT Ferramentaria | Лог CI, ссылка на waiver ticket (если применимо). |
| Sinalizadores de expectativa | M1 | Пайплайны вызывают `sorafs_manifest_stub` com expectativas de saída para фиксации: | Documentos CI | Os scripts criados são selecionados para sinalizadores de expectativa (como o bloco de comandos não). |
| Fixação primeiro do registro | M2 | `sorafs pin propose` e `sorafs pin approve` оборачивают отправку manifesto; CLI para usar o `--require-registry`. | Operações de Governança | Log de auditoria da CLI do Registro, não há necessidade de conexão. |
| Paridade de observabilidade | M3 | Painéis Prometheus/Grafana предупреждают о расхождении inventário de blocos e manifestos de registro; alert'ы подключены к operações de plantão. | Observabilidade | Acesso ao painel, IDs de alertas, resultados do GameDay. |

#### Каноническая команда публикации

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Crie um resumo, divisão e CID de um arquivo de compilação, adicionado em uma pasta
livro razão de migração para arte.

### 3. Usando alias e comunicações| Shag | Você | Descrição | Proprietário(s) | Você |
|-----|------|----------|----------|-------|
| Provas de alias na preparação | M1 | Зарегистрировать reivindicações de alias na preparação do Pin Registry e прикрепить provas Merkle к manifestos (`--alias`). | Governança, Documentos | Pacote de provas рядом с manifest + комментарий ledger с именем alias. |
| Aplicação da prova | M2 | Os gateways oferecem manifestos sem cabeçalhos `Sora-Proof`; CI получает шаг `sorafs alias verify` para provas de извлечения. | Rede | Gateway de configuração de gateway + saída CI de acordo com a experiência. |

### 4. Comunicação e auditoria

- **Ledger Дисциплина:** каждое изменение состояния (desvio de fixação, envio de registro,
  ativação de alias) должно добавлять датированную заметку в
  `docs/source/sorafs/migration_ledger.md`.
- **Atas de governança:** заседания совета, утверждающие изменения registro de pinos ou
  alias политик, должны ссылаться на эту дорожную карту и razão.
- **Внешние коммуникации:** DevRel публикует обновления статуса на каждом этапе (блог +
  trecho de changelog), подчеркивая детерминированные гарантии и таймлайны alias.

## Riscos e riscos

| Acessar | Влияние | Mitigação |
|------------|---------|-----------|
| Доступность контракта Pin Registry | Implementação do primeiro pino do Блокирует M2. | Contrato de contrato para M2 com testes de repetição; поддерживать envelope substituto para отсутствия регрессий. |
| Peças de reposição | Tarefa para envelopes de manifesto e aprovações de registro. | Cerimônia de assinatura registrada em `docs/source/sorafs/signing_ceremony.md`; ротировать ключи с перекрытием e записью в ledger. |
| Cadence lança SDK | Os clientes também usam provas de alias para M3. | Синхронизировать окна релизов SDK com portas de marco; Crie listas de verificação de migração em modelos de lançamento. |

Riscos e mitigação de riscos em `docs/source/sorafs_architecture_rfc.md`
e должны кросс-референситься при изменениях.

## Verifique os critérios de criptografia que você precisa

| Você | Critérios |
|------|----------|
| M1 | - Trabalho noturno em luminárias зеленый семь дней подряд.  - Provas de alias de teste testadas no CI.  - Bandeiras de expectativa de governança ратифицирует политику. |

## Atualizações de atualização

1. Предлагайте изменения через PR, обновляя этот файл **и**
   `docs/source/sorafs/migration_ledger.md`.
2. Pesquisa de atas de governança e evidências de CI na descrição de PR.
3. Faça a mesclagem de armazenamento + lista de discussão DevRel com configuração e organização
   действиями операторов.

Este procedimento é garantido, pois a implementação SoraFS determina a determinação,
аудируемым и прозрачным между командами, участвующими в запуске Nexus.
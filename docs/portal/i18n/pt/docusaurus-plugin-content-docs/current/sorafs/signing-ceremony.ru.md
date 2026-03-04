---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: cerimônia de assinatura
título: Замена церемонии подписания
descrição: Как Парламент Sora утверждает и распространяет fixtures chunker SoraFS (SF-1b).
sidebar_label: Cerimônia de aprovação
---

> Roteiro: **SF-1b — luminárias утверждения Парламента Sora.**
> Fluxo de trabalho do Parlamento заменяет устаревшую офффлайн "церемонию подписания совета".

Ручной ритуал подписания fixtures chunker SoraFS завершен. Qual é o tempo de operação
проходят через **Парламент Sora** — DAO na nova versão, управляющую Nexus.
Você pode definir uma ligação XOR para obter mais informações, rotear no painel e
Use on-chain para утверждение, отклонение ou откат выпусков luminárias. Isso
Você precisa de processos e ferramentas para trabalhar.

## Обзор парламента

- **Гражданство** — O operador bloqueia o XOR, este estado está funcionando e
  получить право на жеребьевку.
- **Панели** — Ответственность распределена между вращающимися панелями
  (Infraestrutura, Moderação, Tesouraria, ...). Painel Infraestrutura revelado
  за утверждения luminárias SoraFS.
- **Жеребьевка и ротация** — Места в панелях перераспределяются с периодичностью,
  No entanto, o parlamento estadual não possui nenhum monopólio
  утверждения.

## Поток утверждения jogos

1. **Exibição de instruções**
   - Tooling WG загружает кандидатный pacote `manifest_blake3.json` e acessório diferencial
     no registro on-chain é `sorafs.fixtureProposal`.
   - Предложение фиксирует BLAKE3 digest, семантическую версию и заметки об изменениях.
2. **Revью e голосование**
   - Панель Infraestrutura получает назначение через очередь задач парламента.
   - Painel de controle usando artefatos CI, testes de paridade realizados e resultados on-chain
     взвешенными голосами.
3. **Finalização**
   - После достижения quorum runtime эмитит событие утверждения с каноническим digest
     manifesto e compromisso Merkle no dispositivo de carga útil.
   - Событие зеркалируется no registro SoraFS, чтобы клиенты могли получить последний
     manifesto, arquivo aberto.
4. **Prostração**
   - Auxiliares CLI (`cargo xtask sorafs-fetch-fixture`) подтягивают утвержденный manifesto
     usando Nexus RPC. Constantes JSON/TS/Go no repositório de sincronização
     запуском `export_vectors` e provеркой digest относительно on-chain записи.

## Organização do fluxo de trabalho

- Jogos de transferência:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Используйте парламентский fetch helper, чтобы скачать утвержденный envelope, проверить
  подписи и обновить локальные luminárias. Coloque `--signatures` em envelope, aberto
  parlamento; helper найдет сопутствующий manifesto, пересчитает BLAKE3 digest e применит
  perfil canônico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Execute `--manifest`, exceto o manifesto definido no URL do arquivo. Envelope sem envelope
отклоняются, если не задан `--allow-unsigned` para fumaça local.

- Para validar o manifesto do gateway de teste usado Torii no local
  cargas úteis:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```- Локальный CI больше не требует escalação `signer.json`.
  `ci/check_sorafs_fixtures.sh` fornece repositório de armazenamento com suporte on-chain
  compromisso e падает при расхождениях.

## Segurança da governança

- Конституция парламента определяет quorum, ротацию и эскалацию — конфигурация
  em sua caixa não há nada.
- Reversão de emergência обрабатывается через панель модерации парламента. Painel
  Infraestrutura pode reverter proposta ссылкой на предыдущий manifesto de resumo,
  e релиз заменяется утверждения.
- Registro de registro no registro SoraFS para reprodução forense.

## Perguntas frequentes

- **Qual é o `signer.json`?**  
  Sim, sim. Все авторство подписей хранится on-chain; `manifest_signatures.json`
  nos repositórios — este é o melhor dispositivo de desenvolvedor, que está disponível para download
  событием утверждения.

- **Nos ли еще локальные подписи Ed25519?**  
  Não. Утверждения парламента хранятся как artefatos on-chain. Локальные jogos
  é importante para a exibição, mas não é possível compará-lo com o resumo.

- **Como os comandos monitoram a operação?**  
  Verifique a solução `ParliamentFixtureApproved` ou feche o registro
  Nexus RPC, ele contém o manifesto de resumo e o painel de exibição.
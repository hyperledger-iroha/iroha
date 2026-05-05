---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-cli
título: Receitas de CLI da SoraFS
sidebar_label: Receitas CLI
descrição: Guia focado em tarefas da superfície consolidada de `sorafs_cli`.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/developer/cli.md`. Mantenha ambas as cópias sincronizadas.
:::

A superfície consolidada `sorafs_cli` (fornecida pela caixa `sorafs_car` com o recurso `cli` habilitado) expõe cada etapa necessária para preparar artefatos de SoraFS. Use este livro de receitas para ir direto aos fluxos de trabalho comuns; combine com o pipeline de manifesto e os runbooks do orquestrador para contexto operacional.

## Cargas úteis do Empacotar

Use `car pack` para produzir arquivos CAR deterministas e planos de chunk. O comando seleciona automaticamente o chunker SF-1, a menos que um identificador seja fornecido.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Alça do bloco padrão: `sorafs.sf1@1.0.0`.
- Entradas de diretório são percorridas em ordem lexicográfica para que os checksums fiquem estaveis entre plataformas.
- O resumo JSON inclui resumos do payload, metadados por chunk e o CID raiz reconhecida pelo registro e pelo orquestrador.

##Construir manifestos

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Opcoes `--pin-*` mapeiam diretamente para campos `PinPolicy` em `sorafs_manifest::ManifestBuilder`.
- Forneca `--chunk-plan` quando quiser que o CLI recalcule o digest SHA3 do chunk antes do envio; caso contrário, ele reutiliza o resumo embutido no resumo.
- O JSON espelha o payload Norito para diferenças simples durante as revisões.

##Assinar manifesta sem chaves de longa duração

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Aceita tokens inline, variáveis de ambiente ou fontes baseadas em arquivos.
- Adiciona metadados de procedência (`token_source`, `token_hash_hex`, resumo de chunk) sem persistir o JWT bruto, menos que `--include-token=true`.
- Funciona bem em CI: combine com OIDC do GitHub Actions definindo `--identity-token-provider=github-actions`.

## Enviar manifestos para o Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Faz decodificacao Norito para alias provas e verifica se apenas ao digest do manifesto antes de enviar via POST ao Torii.
- Recálculo do resumo SHA3 do pedaço do plano para evitar ataques de divergência.
- Os resumos de resposta capturam status HTTP, cabeçalhos e payloads do registro para auditorias posteriores.

## Verifique conteúdo do CAR e comprovantes

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstrua a árvore PoR e compare os resumos do payload com o resumo do manifesto.
- Captura de contagens e identificadores exigidos ao envio de provas de replicação para governança.

## Transmitir telemetria de provas

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Emite itens NDJSON para cada prova transmitida (desativo o replay com `--emit-events=false`).
- Agrega contagens de sucesso/falha, histogramas de latência e falhas amostradas no resumo JSON para que os dashboards possam plotar resultados sem ler logs.
- Sai com código não zero quando o gateway reporta falhas ou quando a verificação PoR local (via `--por-root-hex`) rejeita provas. Ajuste os limites com `--max-failures` e `--max-verification-failures` para execuções de ensaio.
- Apoie PoR hoje; PDP e PoTR reutilizam o mesmo envelope quando SF-13/SF-14 chegam.
- `--governance-evidence-dir` grave o resumo renderizado, metadados (timestamp, versão do CLI, URL do gateway, resumo do manifesto) e uma cópia do manifesto no diretorio fornecido para que pacotes de governança arquivem a evidência do proof-stream sem repetir a execução.

## Referências adicionais

- `docs/source/sorafs_cli.md` - documentação exaustiva de flags.
- `docs/source/sorafs_proof_streaming.md` - esquema de telemetria de provas e modelo de dashboard Grafana.
- `docs/source/sorafs/manifest_pipeline.md` - mergulho profundo em chunking, composição de manifesto e manejo de CAR.
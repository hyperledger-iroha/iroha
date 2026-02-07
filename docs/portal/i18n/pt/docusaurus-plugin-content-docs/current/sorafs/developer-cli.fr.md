---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-cli
título: Receitas CLI SoraFS
sidebar_label: Receitas CLI
descrição: Parcours orienté tâches de la surface consolidada `sorafs_cli`.
---

:::nota Fonte canônica
:::

A superfície consolidada `sorafs_cli` (fornecida pela caixa `sorafs_car` com o recurso `cli` ativado) expõe toda a etapa necessária para preparar os artefatos SoraFS. Use este livro de receitas para direcionar os fluxos de trabalho atuais; associe o pipeline de manifesto e os runbooks do orquestrador para o contexto operacional.

## Empacotar cargas úteis

Use `car pack` para produzir arquivos CAR determinados e planos de bloco. O comando seleciona automaticamente o pedaço SF-1, exceto se uma alça for fornecida.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Alça do chunker por padrão: `sorafs.sf1@1.0.0`.
- As entradas do repertório são distribuídas em ordem lexicográfica depois que as somas de verificação permanecem estáveis ​​entre as plataformas.
- O currículo JSON inclui os resumos da carga útil, os metadados por pedaço e o rastreamento CID reconhecido pelo registro e pelo orquestrador.

## Construa os manifestos

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- As opções `--pin-*` são mapeadas diretamente para os campos `PinPolicy` em `sorafs_manifest::ManifestBuilder`.
- Forneça `--chunk-plan` quando você desejar que a CLI recalcule o resumo SHA3 do bloco antes do envio; não é necessário reutilizar o resumo integrado do currículo.
- A saída JSON reflete a carga útil Norito para diferenças simples nas revistas.

## Assine os manifestos sem clés de longa duração

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Aceite tokens inline, variáveis de ambiente ou fontes baseadas em arquivos.
- Adicionado métodos de proveniência (`token_source`, `token_hash_hex`, resumo de pedaço) sem persistir o JWT bruto, exceto `--include-token=true`.
- Função bem no CI: combine com OIDC do GitHub Actions e definindo `--identity-token-provider=github-actions`.

## Insira os manifestos em Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Efetue a decodificação Norito das provas de alias e verifique quais são os correspondentes ao resumo do manifesto antes do POST vers Torii.
- Recalcule o resumo SHA3 do pedaço do plano para evitar ataques por incompatibilidade.
- Os currículos de resposta capturam o status HTTP, os cabeçalhos e as cargas úteis do registro para uma auditoria avançada.

## Verifique o conteúdo do CAR e as provas

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstrua a árvore PoR e compare os resumos da carga útil com o currículo do manifesto.
- Capture os cálculos e identificadores necessários para o envio de provas de replicação ao governo.

## Difusor da televisão de provas

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Insira os elementos NDJSON para cada fluxo de prova (desative o replay com `--emit-events=false`).
- Adicione os cálculos bem-sucedidos/checados, os histogramas de latência e os cheques atualizados no currículo JSON para que os painéis possam rastrear os resultados sem examinar os logs.
- Saia com um código não nulo quando o gateway sinalizar des échecs ou que a verificação do local PoR (via `--por-root-hex`) rejeite as provas. Ajuste seus arquivos com `--max-failures` e `--max-verification-failures` para repetições.
- Supporte PoR aujourd'hui; PDP e PoTR reutilizaram o mesmo envelope de um SF-13/SF-14 no local.
- `--governance-evidence-dir` escreve o currículo renderizado, os metadonnées (timestamp, versão do CLI, URL do gateway, resumo do manifesto) e uma cópia do manifesto no repertório fornecido para que os pacotes de governo possam arquivar a pré-prova do fluxo de prova sem reiniciar a execução.

## Referências complementares

- `docs/source/sorafs_cli.md` — documentação completa sobre flags.
- `docs/source/sorafs_proof_streaming.md` — esquema de prova de telemetria e modelo de painel Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — aprovado para o chunking, a composição do manifesto e o gerenciamento do CAR.
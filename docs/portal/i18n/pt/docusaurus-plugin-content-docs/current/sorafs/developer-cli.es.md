---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-cli
título: Recetario de CLI de SoraFS
sidebar_label: Recetario de CLI
descrição: Recorrido orientado a tarefas da superfície consolidada de `sorafs_cli`.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/developer/cli.md`. Mantenha ambas as cópias sincronizadas.
:::

A superfície consolidada de `sorafs_cli` (proporcionada pela caixa `sorafs_car` com o recurso `cli` habilitado) expõe cada passo necessário para preparar artefatos de SoraFS. Usa este recipiente para saltar diretamente para os rios comunes; combinado com o pipeline de manifesto e os runbooks do orquestrador para o contexto operacional.

## Cargas úteis do Empaquetar

Usa `car pack` para produzir arquivos CAR deterministas e planos de pedaços. O comando seleciona automaticamente o chunker SF-1 salvo que fornece um identificador.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Alça do chunker predeterminado: `sorafs.sf1@1.0.0`.
- As entradas do diretório são consultadas em ordem lexicográfica para que os checksums sejam mantidos estáveis ​​entre plataformas.
- O currículo JSON inclui resumos de carga útil, metadados por pedaço e a raiz CID reconhecida pelo registro e pelo sequestrador.

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

- As opções `--pin-*` são atribuídas diretamente aos campos `PinPolicy` e `sorafs_manifest::ManifestBuilder`.
- Usa `--chunk-plan` quando você deseja que a CLI recalcule o resumo SHA3 do pedaço antes do envio; pelo contrário, reutilize o resumo incrustado no currículo.
- A saída JSON reflete a carga útil Norito para diferenças simples durante as revisões.

## Firmar manifesta sin claves de longa duração

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Aceita tokens inline, variáveis de ambiente ou fontes baseadas em arquivos.
- Adicione metadados de procedência (`token_source`, `token_hash_hex`, resumo do pedaço) sem persistir o JWT em uma salva bruta que `--include-token=true`.
- Funciona bem no CI: combinado com OIDC do GitHub Actions configurando `--identity-token-provider=github-actions`.

## Enviar manifesta um Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Realize a decodificação Norito para alias de provas e verifique se coincide com o resumo do manifesto antes de POSTear para Torii.
- Recalcula o resumo SHA3 do pedaço do plano para prevenir ataques de desastre.
- Os resumos de resposta capturam o estado HTTP, cabeçalhos e cargas úteis do registro para auditorias posteriores.

## Verifique os conteúdos do CAR e as provas

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstrua a árvore PoR e compare os resumos da carga útil com o resumo do manifesto.
- Captura de conteúdo e identificadores necessários para o envio de provas de replicação para o governo.

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
```- Emite elementos NDJSON para cada prova transmitida (desativa o replay com `--emit-events=false`).
- Agrega conteúdos de sucesso/falha, histogramas de latência e falhas exibidos no currículo JSON para que os painéis possam gerar resultados sem ler logs.
- Venda com código distinto de zero quando o gateway reporta falhas ou a verificação PoR local (via `--por-root-hex`) rechaza as provas. Ajuste os umbrais com `--max-failures` e `--max-verification-failures` para execução de ensaio.
- Soporta PoR hoy; PDP e PoTR reutilizam o mesmo envelope quando o SF-13/SF-14 chega.
- `--governance-evidence-dir` escreve o currículo renderizado, metadados (timestamp, versão da CLI, URL do gateway, resumo do manifesto) e uma cópia do manifesto no diretório fornecido para que os pacotes de governo arquivem a evidência do fluxo de prova sem repetir a execução.

## Referências adicionais

- `docs/source/sorafs_cli.md` — documentação completa de sinalizadores.
- `docs/source/sorafs_proof_streaming.md` — esquema de telemetria de provas e planta de painel Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — aprofundamento em chunking, composição de manifesto e manejo de CAR.
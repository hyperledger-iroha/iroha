<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: developer-cli
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/cli.md`. Mantenha ambas as copias sincronizadas.
:::

A superficie consolidada `sorafs_cli` (fornecida pelo crate `sorafs_car` com o recurso `cli` habilitado) expoe cada etapa necessaria para preparar artefatos da SoraFS. Use este cookbook para ir direto a workflows comuns; combine com o pipeline de manifest e os runbooks do orquestrador para contexto operacional.

## Empacotar payloads

Use `car pack` para produzir arquivos CAR deterministas e planos de chunk. O comando seleciona automaticamente o chunker SF-1, a menos que um handle seja fornecido.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Handle de chunker padrao: `sorafs.sf1@1.0.0`.
- Entradas de diretorio sao percorridas em ordem lexicografica para que os checksums fiquem estaveis entre plataformas.
- O resumo JSON inclui digests do payload, metadados por chunk e o CID raiz reconhecido pelo registro e pelo orquestrador.

## Construir manifests

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
- Forneca `--chunk-plan` quando quiser que o CLI recalcule o digest SHA3 do chunk antes do envio; caso contrario ele reutiliza o digest embutido no resumo.
- A saida JSON espelha o payload Norito para diffs simples durante as revisoes.

## Assinar manifests sem chaves de longa duracao

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Aceita tokens inline, variaveis de ambiente ou fontes baseadas em arquivos.
- Adiciona metadados de procedencia (`token_source`, `token_hash_hex`, digest de chunk) sem persistir o JWT bruto, a menos que `--include-token=true`.
- Funciona bem em CI: combine com OIDC do GitHub Actions definindo `--identity-token-provider=github-actions`.

## Enviar manifests para o Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority bob@wonderland \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Faz decodificacao Norito para alias proofs e verifica se correspondem ao digest do manifest antes de enviar via POST ao Torii.
- Recalcula o digest SHA3 do chunk a partir do plano para evitar ataques de divergencia.
- Os resumos de resposta capturam status HTTP, headers e payloads do registro para auditoria posterior.

## Verificar conteudos de CAR e proofs

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstroi a arvore PoR e compara digests do payload com o resumo do manifest.
- Captura contagens e identificadores exigidos ao enviar proofs de replicacao para governanca.

## Transmitir telemetria de proofs

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Emite itens NDJSON para cada proof transmitido (desative o replay com `--emit-events=false`).
- Agrega contagens de sucesso/falha, histogramas de latencia e falhas amostradas no resumo JSON para que dashboards possam plotar resultados sem ler logs.
- Sai com codigo nao zero quando o gateway reporta falhas ou quando a verificacao PoR local (via `--por-root-hex`) rejeita proofs. Ajuste os limiares com `--max-failures` e `--max-verification-failures` para execucoes de ensaio.
- Suporta PoR hoje; PDP e PoTR reutilizam o mesmo envelope quando SF-13/SF-14 chegarem.
- `--governance-evidence-dir` grava o resumo renderizado, metadados (timestamp, versao do CLI, URL do gateway, digest do manifest) e uma copia do manifest no diretorio fornecido para que pacotes de governanca arquivem a evidencia do proof-stream sem repetir a execucao.

## Referencias adicionais

- `docs/source/sorafs_cli.md` - documentacao exaustiva de flags.
- `docs/source/sorafs_proof_streaming.md` - esquema de telemetria de proofs e template de dashboard Grafana.
- `docs/source/sorafs/manifest_pipeline.md` - mergulho profundo em chunking, composicao de manifest e manejo de CAR.

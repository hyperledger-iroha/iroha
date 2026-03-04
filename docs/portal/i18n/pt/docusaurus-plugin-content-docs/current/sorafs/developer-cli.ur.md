---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-cli
título: Livro de receitas CLI SoraFS
sidebar_label: livro de receitas CLI
descrição: Superfície `sorafs_cli` consolidada کا passo a passo focado em tarefas۔
---

:::nota مستند ماخذ
:::

Superfície `sorafs_cli` consolidada (caixa `sorafs_car` کے ذریعے recurso `cli` کے ساتھ فراہم ہوتا ہے) Artefatos SoraFS تیار کرنے کے لیے درکار ہر قدم expor کرتا ہے۔ O livro de receitas e os fluxos de trabalho são os mais importantes para você contexto operacional کے لیے اسے pipeline de manifesto اور runbooks do orquestrador کے ساتھ par کریں۔

## Cargas úteis do pacote

Arquivos CAR determinísticos e planos de blocos بنانے کے لیے `car pack` استعمال کریں۔ اگر handle فراہم نہ ہو تو command خودکار طور پر SF-1 chunker منتخب کرتا ہے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Alça do chunker padrão: `sorafs.sf1@1.0.0`.
- Ordem lexicográfica de entradas de diretório میں walk ہوتے ہیں تاکہ somas de verificação مختلف پلیٹ فارمز پر بھی stable رہیں۔
- Resumo JSON میں resumos de carga útil, فی metadados de pedaços, registro / orquestrador کے ذریعے پہچانا گیا root CID شامل ہوتا ہے۔

## Construir manifestos

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Opções `--pin-*` براہ راست `sorafs_manifest::ManifestBuilder` میں Campos `PinPolicy` سے mapa ہوتے ہیں۔
- `--chunk-plan` تب دیں جب آپ چاہتے ہوں کہ Envio CLI سے پہلے SHA3 chunk digest دوبارہ computar کرے؛ ورنہ وہ resumo میں incorporar شدہ digerir reutilizar کرتا ہے۔
- Saída JSON Carga útil Norito کی عکاسی کرتا ہے تاکہ comentários کے دوران diffs سیدھے ہوں۔

## Assinar manifestos sem chaves de longa duração

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Tokens embutidos, variáveis de ambiente e fontes baseadas em arquivo قبول کرتا ہے۔
- Metadados de proveniência (`token_source`, `token_hash_hex`, resumo de pedaços) Não
- CI میں بہتر کام کرتا ہے: GitHub Actions OIDC کے ساتھ `--identity-token-provider=github-actions` استعمال کریں۔

## Envie manifestos para Torii

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

- Provas de alias کے لیے Norito decodificação کرتا ہے اور Torii کو POST کرنے سے پہلے انہیں manifest digest سے match کرتا ہے۔
- Planejar سے pedaço SHA3 digest دوبارہ computar کرتا ہے تاکہ ataques de incompatibilidade روکے جا سکیں۔
- Resumos de respostas, auditoria, status HTTP, cabeçalhos e cargas úteis do registro.

## Verifique o conteúdo e as provas do CAR

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Árvore PoR دوبارہ بناتا ہے اور resumos de carga útil کو resumo do manifesto کے ساتھ comparar کرتا ہے۔
- Provas de replicação کو governança میں enviar کرتے وقت مطلوبہ contagens اور captura de identificadores کرتا ہے۔

## Telemetria à prova de fluxo

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- ہر prova transmitida کے لیے itens NDJSON emitem کرتا ہے (`--emit-events=false` سے repetição بند کریں)۔
- Contagens de sucesso/falha, histogramas de latência, falhas amostradas e resumo JSON میں agregado کرتا ہے تاکہ raspagem de logs de painéis کیے بغیر نتائج دکھا سکیں۔
- جب relatório de falhas de gateway کرے یا verificação PoR local (`--por-root-hex` کے ذریعے) provas rejeitadas کرے تو saída diferente de zero دیتا ہے۔ execuções de ensaio کے لیے `--max-failures` e `--max-verification-failures` سے ajuste de limites کریں۔
- آج PoR کو suporte کرتا ہے؛ PDP e PoTR SF-13/SF-14 são um envelope que pode ser reutilizado
- `--governance-evidence-dir` resumo renderizado, metadados (carimbo de data e hora, versão CLI, URL do gateway, resumo do manifesto), اور manifest کی ایک copiar فراہم کردہ diretório میں لکھتا ہے تاکہ pacotes de governança evidência de fluxo de prova کو executar دوبارہ کیے بغیر arquivo کر سکیں۔

## Referências adicionais

- `docs/source/sorafs_cli.md` — Sinalizadores de تمام کی جامع دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` — esquema de telemetria de prova e modelo de painel Grafana۔
- `docs/source/sorafs/manifest_pipeline.md` — chunking, composição do manifesto, e manuseio de CAR پر تفصیلی جائزہ۔
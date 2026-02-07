---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Processo de lançamento
resumo: porta de lançamento CLI/SDK چلائیں، مشترکہ política de versionamento لاگو کریں، اور notas de lançamento canônicas شائع کریں۔
---

# Processo de liberação

Binários SoraFS (`sorafs_cli`, `sorafs_fetch`, auxiliares) e caixas SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) ایک ساتھ navio ہوتے ہیں۔ Liberar
pipeline CLI e bibliotecas alinhadas رکھتا ہے، lint/cobertura de teste یقینی بناتا ہے،
Para os consumidores downstream, os artefatos são capturados. ہر tag candidata کے لیے
Lista de verificação da lista de verificação

## 0. Aprovação da revisão de segurança کی تصدیق

Portão de liberação técnica چلانے سے پہلے تازہ ترین captura de artefatos de revisão de segurança کریں:

- سب سے تازہ Memorando de revisão de segurança SF-6 ڈاؤن لوڈ کریں ([reports/sf6-security-review](./reports/sf6-security-review.md))
  Há um ticket de liberação de hash SHA256.
- Link do ticket de remediação (مثلاً `governance/tickets/SF6-SR-2026.md`) منسلک کریں اور
  Engenharia de Segurança e Grupo de Trabalho de Ferramentas کے aprovadores de aprovação نوٹ کریں۔
- تصدیق کریں کہ memorando کی lista de verificação de remediação بند ہے؛ liberação de itens não resolvidos کو bloquear کرتے ہیں۔
- Logs de chicote de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`) کو
  pacote de manifesto کے ساتھ upload کرنے کے لیے تیار رہیں۔
- یہ بھی تصدیق کریں کہ comando de assinatura میں `--identity-token-provider` کے ساتھ
  واضح `--identity-token-audience=<aud>` شامل ہو تاکہ Evidência de liberação de escopo Fulcio میں captura ہو۔

Governança کو اطلاع دیتے وقت اور lançamento, publicação کرتے وقت ان artefatos کو شامل کریں۔

## 1. Porta de liberação/teste چلائیں

`ci/check_sorafs_cli_release.sh` CLI auxiliar e caixas do SDK para formatação, Clippy e testes
چلاتا ہے، اور workspace-local target directory (`.target`) استعمال کرتا ہے تاکہ CI
contêineres میں conflitos de permissão سے بچا جا سکے۔

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

یہ script درج ذیل assertions کرتا ہے:

- `cargo fmt --all -- --check` (área de trabalho)
- `cargo clippy --locked --all-targets` `sorafs_car` کے لیے (recurso `cli` کے ساتھ)،
  `sorafs_manifest` ou `sorafs_chunker`
- `cargo test --locked --all-targets` انہی caixas کے لیے

اگر کوئی قدم falhar ہو تو marcação سے پہلے regressão درست کریں۔ Versão de compilações کو principal
کے ساتھ رہنا چاہیے؛ liberar ramificações میں corrige a seleção seletiva نہ کریں۔ Portão
یہ بھی چیک کرتا ہے کہ sinalizadores de assinatura sem chave (`--identity-token-issuer`, `--identity-token-audience`)
جہاں ضروری ہوں فراہم کیے گئے ہوں؛ argumentos ausentes, execute کو fail کر دیتے ہیں۔

## 2. Política de versionamento لاگو کریں

SoraFS CLI/SDK crates سب SemVer استعمال کرتے ہیں:

- `MAJOR`: versão 1.0 پہلی میں introdução ہوتا ہے۔ 1.0 سے پہلے `0.y` pequeno impacto
  **alterações importantes** کو ظاہر کرتا ہے, چاہے وہ superfície CLI میں ہوں یا Esquemas Norito میں۔
- `PATCH`: Correções de bugs, versões somente de documentação, atualizações de dependências e comportamento observável کو تبدیل نہیں کرتے۔

`sorafs_car`, `sorafs_manifest` e `sorafs_chunker` کو ہمیشہ ایک ہی versão پر رکھیں تاکہ
Consumidores downstream do SDK ایک string de versão alinhada پر depende کر سکیں۔ Aumento de versão کرتے وقت:1. ہر crate کے `Cargo.toml` میں `version =` campos اپڈیٹ کریں۔
2. `cargo update -p <crate>@<new-version>` کے ذریعے `Cargo.lock` regenerar کریں (versões explícitas do espaço de trabalho impõem کرتا ہے)۔
3. Porta de liberação دوبارہ چلائیں تاکہ artefatos obsoletos باقی نہ رہیں۔

## 3. Notas de versão

ہر release کو markdown changelog شائع کرنا چاہیے جو CLI, SDK اور mudanças que impactam a governança
کو destacar کرے۔ Modelo `docs/examples/sorafs_release_notes.md` کا استعمال کریں (liberação
diretório de artefatos میں کاپی کریں اور seções کو detalhes concretos سے بھر دیں)۔

Conteúdo mínimo:

- **Destaques**: consumidores CLI e SDK e manchetes de recursos۔
- **Etapas de atualização**: colisão de dependências de carga کرنے اور reexecução de equipamentos determinísticos کرنے کے Comandos TL;DR۔
- **Verificação**: hashes de saída de comando یا envelopes اور `ci/check_sorafs_cli_release.sh` کی revisão exata جو executar ہوئی۔

بھری ہوئی notas de lançamento کو tag کے ساتھ anexar کریں (corpo de lançamento do GitHub) اور انہیں deterministicamente
artefatos gerados

## 4. Solte os ganchos

`scripts/release_sorafs_cli.sh` چلائیں تاکہ pacote de assinatura اور resumo de verificação gerar ہو جو ہر liberação
کے ساتھ navio ہوتے ہیں۔ Wrapper ضرورت کے مطابق CLI build کرتا ہے, `sorafs_cli manifest sign` چلاتا ہے, اور فوراً
`manifest verify-signature` repetição کرتا ہے تاکہ marcação سے پہلے falhas سامنے آ جائیں۔ Exemplo:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Dicas:

- Liberar entradas (carga útil, planos, resumos, hash de token esperado) کو repo یا configuração de implantação میں rastrear کریں تاکہ script reproduzível رہے۔ `fixtures/sorafs_manifest/ci_sample/` e layout canônico do pacote de equipamentos
- Automação CI کو `.github/workflows/sorafs-cli-release.yml` پر base کریں؛ یہ portão de liberação چلاتا ہے، اوپر والا script چلاتا ہے، اور pacotes/assinaturas کو artefatos de fluxo de trabalho کے طور پر arquivo کرتا ہے۔ دوسرے Sistemas CI میں بھی وہی ordem de comando (portão de liberação -> assinar -> verificar) رکھیں تاکہ auditoria de logs gerados hashes کے ساتھ alinhar رہیں۔
- `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`, اور `manifest.verify.summary.json` کو اکٹھا رکھیں؛ یہی notificação de governança de pacotes میں consulte ہوتا ہے۔
- Para liberar luminárias canônicas اپڈیٹ کرے تو manifesto atualizado, plano de bloco, اور resumos کو `fixtures/sorafs_manifest/ci_sample/` میں کاپی کریں (اور `docs/examples/sorafs_ci_sample/manifest.template.json` اپڈیٹ کریں) marcação سے پہلے۔ Operadores downstream comprometeram fixtures پر dependem کرتے ہیں تاکہ liberar pacote reproduzir کر سکیں۔
- `sorafs_cli proof stream` verificação de canal limitado کا executar captura de log کریں اور pacote de liberação کے ساتھ anexar کریں تاکہ proteções de streaming de prova فعال رہنے کا ثبوت ملے۔
- Assinatura میں استعمال ہونے والا notas de versão `--identity-token-audience` exatas میں درج کریں؛ governança Política Fulcio کے خلاف audiência کو verificação cruzada کرتی ہے۔

جب release میں implementação de gateway بھی شامل ہو تو `scripts/sorafs_gateway_self_cert.sh` استعمال کریں۔ اسی pacote de manifesto کی طرف ponto کریں تاکہ artefato candidato a atestado سے correspondência ہو:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tag para publicar

Verificações پاس ہونے اور ganchos مکمل ہونے کے بعد:1. `sorafs_cli --version` e `sorafs_fetch --version` چلائیں تاکہ binários نئی relatório de versão کریں۔
2. Configuração de versão کو `sorafs_release.toml` com check-in (preferencial) یا کسی اور arquivo de configuração میں تیار کریں جو آپ کے repositório de implantação میں faixa ہو۔ Variáveis ​​de ambiente ad-hoc CLI کو `--config` (یا equivalente) کے ذریعے caminhos دیں تاکہ liberar entradas واضح اور reproduzível رہیں۔
3. Tag assinada (preferencial) ou tag anotada بنائیں:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Artefatos (pacotes CAR, manifestos, resumos de prova, notas de lançamento, resultados de atestado) کو registro do projeto میں upload کریں اور lista de verificação de governança (guia de implantação: [guia de implantação](./developer-deployment.md)) siga کریں۔ اگر release نے نئی fixtures بنائیں تو انہیں repositório de fixtures compartilhado یا armazenamento de objetos میں push کریں تاکہ pacote publicado de automação de auditoria کا controle de origem کے ساتھ diff کر سکے۔
5. Canal de governança کو etiqueta assinada, notas de lançamento, pacote de manifesto/hashes de assinatura, resumos `manifest.sign/verify` arquivados, e envelopes de atestado کے links کے ساتھ مطلع کریں۔ URL do trabalho de CI (arquivo de log) شامل کریں جس نے `ci/check_sorafs_cli_release.sh` ou `scripts/release_sorafs_cli.sh` چلایا۔ Ticket de governança اپڈیٹ کریں تاکہ aprovações de auditores کو artefatos سے rastreamento کر سکیں؛ جب `.github/workflows/sorafs-cli-release.yml` postagem de notificações de trabalho کرے تو resumos ad-hoc کے بجائے link de hashes gravados کریں۔

## 6. Acompanhamento pós-lançamento

- Versão نئی کی طرف اشارہ کرنے والی documentação (guias de início rápido, modelos de CI) اپڈیٹ کریں یا تصدیق کریں کہ کوئی تبدیلی درکار نہیں۔
- Logs de saída da porta de liberação کو auditores کے لیے arquivo کریں - انہیں artefatos assinados کے ساتھ محفوظ رکھیں۔

اس pipeline کی پیروی ہر ciclo de lançamento میں CLI, SDK crates اور garantia de governança کو lock-step میں رکھتی ہے۔
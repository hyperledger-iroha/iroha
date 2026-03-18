---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: cerimônia de assinatura
título: دستخطی تقریب کی جگہ نیا عمل
descrição: Sora پارلیمنٹ کس طرح SoraFS chunker fixtures کی منظوری اور تقسیم کرتی ہے (SF-1b).
sidebar_label: Nome da barra lateral
---

> Roteiro: **SF-1b — Aprovações de luminárias do Parlamento Sora.**
> پارلیمنٹ کا ورک فلو پرانی آف لائن "کونسل دستخطی تقریب" کی جگہ لیتا ہے۔

Acessórios chunker SoraFS اب تمام منظوری
**Sora Parliament** کے ذریعے ہوتی ہے، جو Nexus کو گورن کرنے والی DAO baseado em classificação ہے۔
پارلیمنٹ کے اراکین شہری بننے کے لیے XOR bond کرتے ہیں, پینلز کے درمیان گردش کرتے ہیں،
اور on-chain ووٹس کے ذریعے fixtures ریلیزز کو منظور، مسترد یا رول بیک کرتے ہیں۔
یہ گائیڈ عمل اور ferramentas de desenvolvedor کی وضاحت کرتی ہے۔

## پارلیمنٹ کا جائزہ

- **شہریت** — آپریٹرز مطلوبہ XOR bond کر کے شہری بنتے ہیں اور sortition کے اہل ہوتے ہیں۔
- **پینلز** — ذمہ داریاں گردش کرنے والے پینلز میں تقسیم ہیں (Infraestrutura,
  Moderação, Tesouraria, ...). Aprovações de acessórios do painel de infraestrutura SoraFS
- **Sortição e rotação** — پینل سیٹس پارلیمنٹ دستور میں متعین cadência پر دوبارہ
  قرعہ اندازی سے منتخب ہوتے ہیں تاکہ کوئی ایک گروہ منظوریوں پر اجارہ داری نہ رکھ سکے۔

## Fluxo de aprovação de luminárias

1. **Envio de proposta**
   - Conjunto de ferramentas WG `manifest_blake3.json` e diferencial de fixação e `sorafs.fixtureProposal`
     کے ذریعے registro on-chain میں اپلوڈ کرتا ہے۔
   - پروپوزل BLAKE3 digest, versão semântica اور تبدیلی نوٹس ریکارڈ کرتا ہے۔
2. **Revisão e votação**
   - Painel de infraestrutura پارلیمنٹ fila de tarefas کے ذریعے اسائنمنٹ وصول کرتا ہے۔
   - پینل ممبرز artefatos CI دیکھتے ہیں, testes de paridade چلاتے ہیں, اور votos ponderados na cadeia ڈالتے ہیں۔
3. **Finalização**
   - جب quorum پورا ہو جائے تو runtime ایک evento de aprovação جاری کرتا ہے جس میں canonical manifest digest digest
     A carga útil do fixture é o compromisso Merkle que você pode usar
   - یہ evento SoraFS registro میں espelho کیا جاتا ہے تاکہ کلائنٹس تازہ ترین Manifesto aprovado pelo Parlamento حاصل کر سکیں۔
4. **Distribuição**
   - Auxiliares CLI (`cargo xtask sorafs-fetch-fixture`) Nexus RPC سے منظور شدہ manifesto کھینچتے ہیں۔
     ریپو کے Constantes JSON/TS/Go `export_vectors` دوبارہ چلا کر اور digest کو ریکارڈ کے
     مقابل validar e sincronizar رہتے ہیں۔

## Fluxo de trabalho do desenvolvedor

- Jogos دوبارہ بنائیں:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Ajudante de busca do Parlamento استعمال کریں تاکہ منظور شدہ envelope ڈاؤن لوڈ ہو، verificação de assinaturas ہوں،
  اور مقامی atualização de luminárias ہوں۔ `--signatures` کو Parlamento کے شائع کردہ envelope پر پوائنٹ کریں؛
  helper متعلقہ manifest resolve کرتا ہے، BLAKE3 digest دوبارہ حساب کرتا ہے، اور canonical
  Perfil `sorafs.sf1@1.0.0` نافذ کرتا ہے۔

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

O manifesto é o URL do arquivo `--manifest`. غیر دستخط شدہ envelopes رد کر دیے جاتے ہیں
جب تک مقامی fumaça corre کے لیے `--allow-unsigned` سیٹ نہ ہو۔

- Gateway de teste کے ذریعے validação de manifesto کرنے کے لیے مقامی cargas úteis کے بجائے Torii کو ہدف بنائیں:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```- مقامی CI اب `signer.json` escalação کا تقاضا نہیں کرتا۔
  Repo `ci/check_sorafs_fixtures.sh` کی حالت کو تازہ ترین compromisso on-chain سے موازنہ کرتا ہے اور
  فرق ہونے پر falhar کر دیتا ہے۔

## Notas de governança

- پارلیمنٹ دستور quorum, rotação اور escalonamento کو governar کرتا ہے — configuração em nível de caixa درکار نہیں۔
- ہنگامی reversões پارلیمنٹ painel de moderação کے ذریعے سنبھالے جاتے ہیں۔ Painel de infraestrutura ایک reverter
  proposta فائل کرتا ہے جو پچھلے resumo do manifesto کو حوالہ دیتا ہے، اور منظوری کے بعد release بدل دی جاتی ہے۔
- تاریخی aprovações registro SoraFS میں repetição forense کے لیے دستیاب رہتے ہیں۔

## Perguntas frequentes

- **`signer.json` کہاں گیا؟**  
  اسے ہٹا دیا گیا ہے۔ تمام atribuição do signatário na cadeia موجود ہے؛ ریپو میں `manifest_signatures.json`
  صرف desenvolvedor fixture ہے جو آخری evento de aprovação سے میچ ہونا چاہیے۔

- **کیا اب بھی مقامی Ed25519 assinaturas درکار ہیں؟**  
  Não O Parlamento aprova artefatos na rede مقامی jogos
  reprodutibilidade کے لیے ہوتے ہیں مگر Parliament digest کے خلاف validar کیے جاتے ہیں۔

- **ٹیمیں aprovações کیسے مانیٹر کرتی ہیں؟**  
  Evento `ParliamentFixtureApproved` کو subscribe کریں یا Nexus RPC کے ذریعے registro کو consulta کریں
  تاکہ موجودہ manifest digest e painel roll call حاصل کیا جا سکے۔
---
lang: he
direction: rtl
source: docs/portal/docs/norito/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Inicio rapido do Norito
תיאור: Construa, valide e faca deploy de um contrato Kotodama com o tooling de release e red padrao de um unico peer.
Slug: /norito/Quickstart
---

Este passo a passo espelha o fluxo que esperamos que desenvolvedores sigam ao aprender Norito e Kotodama pela primeira vez: iniciar uma rede deterministica de um unico peer, compilar dry um-via depois, fazer dry um-via depois Torii com o CLI de referencia.

O contrato de exemplo grava um par chave/valor na conta do chamador para que voce possa verificar o efeito colateral imediatamente com `iroha_cli`.

## דרישות מוקדמות

- [Docker](https://docs.docker.com/engine/install/) עם Compose V2 הביליון (שימוש עבור דוגמה אישית ב-`defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) להורדת מערכות עזר בינואריות כדי להשמיע את התמונות הבאות.
- Binarios `koto_compile`, `ivm_run` ו-`iroha_cli`. קול חיבור לחלק מהקופה לעשות את סביבת העבודה como mostrado abaixo או baixar os artifacts de release correspondents:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Os binarios acima sao seguros para instalar Junto com o resto do workspace.
> Eles nunca fazem link com `serde`/`serde_json`; os codec Norito sao aplicados de ponta a ponta.

## 1. Inicie uma red dev de um unico peer

המאגר כולל את החבילה Docker צור גרדו פור `kagami swarm` (`defaults/docker-compose.single.yml`). Ele conecta a genesis padrao, a configuracao do cliente e os health probes para que Torii fique acessive em `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deixe o container rodando (em primeiro plano ou detached). Todas as chamadas de CLI posteriores apontam para esse peer via `defaults/client.toml`.

## 2. Escreva o contrato

Crie um diretorio de trabalho e salve o exemplo minimo de Kotodama:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Prefira manter os fontes Kotodama בשליטה דה versao. Exemplos hospedados no portal tambem estao disponiveis na [galeria de exemplos Norito](./examples/) שאל את זה על נקודת המוצא של ריקו.

## 3. Compile e faca dry-run com IVM

קומפילו או קונטרה לקוד byte IVM/Norito (`.to`) וביצוע-o localmente para confirmar que os syscalls do host funcionam antes de tocar a rede:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

O runner imprime o log `info("Hello from Kotodama")` e executa o syscall `SET_ACCOUNT_DETAIL` contra o host mockado. ראה או אופציונלי בינארי `ivm_tool` estimer disponivel, `ivm_tool inspect target/quickstart/hello.to` או כותרת ABI, OS תכונה ביטים ו-OS נקודות כניסה ליצוא.

## 4. Envie o bytecode באמצעות Torii

Com o nodo ainda rodando, envie o bytecode compilado para Torii usando o CLI. A identidade de desenvolvimento padrao e derivada da chave publica em `defaults/client.toml`, portanto o ID de conta e
```
ih58...
```

השתמש בכתובת URL של Torii, או מזהה שרשרת ו-Chave de assinatura:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```O CLI codifica a transacao com Norito, assina com a chave dev e envia ao peer em execucao. צפה ביומני מערכת ההפעלה לעשות Docker ל-Syscall `set_account_detail` או לפקח על ביצוע CLI ל-Hash da transacao מחויב.

## 5. Verifique a mudanca de estado

השתמש ב- mesmo perfil do CLI para buscar o פרטי חשבון que o contrato gravou:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Voce deve ver o payload JSON תומך ב-Norito:

```json
{
  "hello": "world"
}
```

Se o valor estiver ausente, confirme que o servico Docker compose ainda esta rodando e que o hash da transacao reportado por `iroha` chegou ao estado `Committed`.

## Proximos passos

- חקור [galeria de exemplos](./examples/) gerada automaticamente para ver
  como snippets Kotodama mais avancados se mapeiam para syscalls Norito.
- Leia o [guia de inicio do Norito](./getting-started) para uma explicacao
  עשה הכלים של מהדר/ראנר, יש לפרוס את המניפסטים ואת המטאדים IVM.
- Ao iterar nos seus contratos, השתמש ב-`npm run sync-norito-snippets` ללא סביבת עבודה
  regenerar snippets baixavis e manter os docs do portal e artifacts sincronizados com as fontes em `crates/ivm/docs/examples/`.
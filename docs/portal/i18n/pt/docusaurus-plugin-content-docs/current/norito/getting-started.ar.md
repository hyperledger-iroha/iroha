---
lang: pt
direction: ltr
source: docs/portal/docs/norito/getting-started.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بدء استخدام Norito

O código de bytecode Kotodama é o bytecode Norito. O problema é o Iroha.

## المتطلبات المسبقة

1. ثبّت سلسلة ادوات Rust (1.76 او احدث) واستنسخ هذا المستودع.
2. ابن او نزّل الثنائيات الداعمة:
   - `koto_compile` - Código Kotodama do bytecode IVM/Norito
   - `ivm_run` e `ivm_tool` - ادوات التشغيل المحلي والفحص
   - `iroha_cli` - Torii

   O Makefile está no arquivo `PATH`. يمكنك تنزيل artefatos جاهزة او بناؤها من المصدر. O conjunto de ferramentas do Toolchain está disponível no Makefile:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Verifique se o Iroha está funcionando corretamente. A configuração do URL do Torii pode alterar o URL do site para o `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Solução de problemas Kotodama

Use a palavra "olá mundo" em `examples/hello/hello.ko`. Para obter o bytecode Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Como fazer:

- `--abi 1` é definido como ABI 1 (não disponível).
- `--max-cycles 0` يطلب تنفيذا غير محدود؛ Você pode usar o preenchimento para obter o preenchimento desejado.

## 2. Solução de problemas Norito (Norito)

Use `ivm_tool` para usar no computador e no computador:

```sh
ivm_tool inspect target/examples/hello.to
```

Não há nenhuma alteração no ABI e no código postal. Isso é tudo que você precisa.

## 3. تشغيل العقد محليا

O bytecode de `ivm_run` é o código de byte que corresponde ao seguinte:

```sh
ivm_run target/examples/hello.to --args '{}'
```

O `hello` é definido como syscall `SET_ACCOUNT_DETAIL`. Certifique-se de que o produto esteja funcionando corretamente.

## 4. Nome do usuário `iroha_cli`

Você pode usar o CLI para usar o CLI. O valor da carga útil é o `.to` e a carga útil é Base64:

```sh
iroha_cli app contracts deploy \
  --authority ih58... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

O pacote de pacote do manifesto Norito + bytecode é Torii e o código de bytes. بعد التزام المعاملة يمكن استخدام hash الكود المعروض في الاستجابة لاسترجاع manifestos او سرد instâncias:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Solução de problemas Torii

Para criar bytecodes, você pode usar o bytecode para obter o valor do código `iroha_cli ledger transaction submit` ou عميل التطبيق). Você pode usar o sistema syscalls (`set_account_detail`, `transfer_asset`, `transfer_asset`).

## نصائح واستكشاف الاعطال

- Use `make examples-run` para obter informações e instruções de uso. Para obter mais informações sobre `KOTO`/`IVM`, você pode usar o `PATH`.
- Para obter `koto_compile` ABI, instale o `koto_compile` ABI v1 (`koto_compile --abi` بدون معاملات لعرض الدعم).
- O CLI é baseado em hexadecimal e Base64. Para obter mais informações, consulte `iroha_cli tools crypto keypair`.
- Para transferir cargas úteis Norito, use `ivm_tool disassemble` para obter o Kotodama.Não há nenhum problema no CI e no computador. Para definir o Kotodama e syscalls e Norito, execute:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`
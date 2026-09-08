# Kotodama for Visual Studio Code

Kotodama language support for `.ko` contracts and test modules. The client starts
`koto lsp` and uses the compiler's explicit `kotodama.project.json` source graph.
The extension also ships the canonical TextMate grammar for highlighting.

Install the repository's `koto` executable and put it on `PATH`, or set
`kotodama.serverPath` to its absolute path. Open the generated project folder:

```sh
iroha contract dev new counter
code counter
```

To build this extension from source:

```sh
npm ci --ignore-scripts
npm test
npm run package
code --install-extension kotodama.vsix
```

The extension starts one language server per workspace folder. Set
`kotodama.project` to the explicit project manifest (the default is
`kotodama.project.json`). `${workspaceFolder}` expands to that folder. A missing
manifest selects single-file analysis; imports are never inferred from disk.
Source overlays and source/manifest change notifications reach the compiler.
Configuration changes restart clients. Use **Kotodama: Restart Language Server**
after replacing the compiler executable.

`kotodama.zk` enables the compiler's ZK checks. The client requires a trusted
workspace before starting a configured executable, and passes arguments without
a shell. Formatting, diagnostics, and semantic editor operations use the installed
compiler; use the compiler and extension from the same checkout or release.

[Language documentation](https://docs.iroha.tech/blockchain/smart-contracts#first-project).
The grammar mirrors `specs/kotodama_grammar.md` and
`crates/kotodama_lang/grammar/v1.lex` and remains usable by GitHub Linguist.

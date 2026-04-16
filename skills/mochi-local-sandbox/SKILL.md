---
name: mochi-local-sandbox
description: "Bring up and use a Mochi-managed local Iroha sandbox plus its local Torii MCP endpoint. Use when Codex needs a reliable local devnet, needs to print or verify the `codex mcp add mochi-local --url ...` command, or should consume the generated `.env.local` and `.mochi/generated/*` bootstrap files for local-only development."
---

# Mochi Local Sandbox

Use Mochi as the local Iroha devnet launcher and MCP bridge.

## Quick Start

1. Start the sandbox from the target workspace:
   - `scripts/mochi_local_sandbox.sh up`
   - Use `MOCHI_WORKSPACE_ROOT=/path/to/app` when the current shell is not already in the app workspace.
   - Use `MOCHI_PROFILE=four-peer-bft` when the user wants the four-validator rehearsal instead of the default `single-peer`.
2. Confirm the sandbox is healthy:
   - `scripts/mochi_local_sandbox.sh status`
   - Healthy means `status: ready`, `ready: true`, and `mcp_ready: true`.
3. If Codex needs the local MCP endpoint, print the exact add command:
   - `scripts/mochi_local_sandbox.sh mcp-add-command`
4. Use the workspace bootstrap artifacts:
   - `.env.local`
   - `.mochi/generated/typescript/connect.ts`
   - `.mochi/generated/rust/connect.rs`
   - `.mochi/generated/kotlin/MochiConnect.kt`

## Working Rules

1. Prefer Mochi's curated local `iroha.*` MCP tools. Do not prefer raw `torii.*` tools when the local MCP surface is healthy.
2. Treat `.env.local`, `session.json`, and any generated `IROHA_PRIVATE_KEY` value as runtime-only local-dev material. Do not commit them or copy them into permanent docs.
3. Use `scripts/mochi_local_sandbox.sh env` when you need copy/paste shell exports for a local app.
4. Use `scripts/mochi_local_sandbox.sh reset` when the local chain must be wiped and regenerated.
5. If `up` fails, inspect:
   - `scripts/mochi_local_sandbox.sh status`
   - `<workspace>/.mochi/sandbox/<profile>/serve.log`
   - `<workspace>/.mochi/sandbox/<profile>/session.json` when it exists
6. If the user wants a custom non-preset profile, prefer the GUI or direct `mochi sandbox serve` flow instead of stretching the helper script beyond `single-peer` and `four-peer-bft`.

## Response Pattern

1. Bring the sandbox up if the user asked to test or use the local env and it is not already ready.
2. Report the exact local MCP add command when Codex or the user needs it.
3. Base app wiring advice on `.env.local` and `.mochi/generated/*`, not on ad-hoc handwritten env snippets.
4. Keep local sandbox guidance clearly separated from live-network guidance such as Taira.

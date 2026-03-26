const state = {
  session: { publicHex: "", privateHex: "" },
};

const sessionPublic = document.getElementById("session-public");
const sessionPrivate = document.getElementById("session-private");
const bindingInput = document.getElementById("binding-hash");
const amountInput = document.getElementById("send-amount");
const toriiInput = document.getElementById("torii-url");
const sponsorInput = document.getElementById("sponsor-id");
const chainInput = document.getElementById("chain-id");
const sessionAccountInput = document.getElementById("session-account");
const commandOutput = document.getElementById("command-output");
const payloadOutput = document.getElementById("payload-output");

function bufferToHex(buffer) {
  return Array.from(new Uint8Array(buffer))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function generateSessionKey() {
  try {
    if (window.crypto?.subtle?.generateKey) {
      const key = await window.crypto.subtle.generateKey(
        { name: "Ed25519", namedCurve: "Ed25519" },
        true,
        ["sign", "verify"],
      );
      const rawPublic = await window.crypto.subtle.exportKey("raw", key.publicKey);
      const rawPrivate = await window.crypto.subtle.exportKey("pkcs8", key.privateKey);
      state.session.publicHex = bufferToHex(rawPublic);
      state.session.privateHex = bufferToHex(rawPrivate);
    } else {
      throw new Error("Ed25519 generateKey unavailable");
    }
  } catch (error) {
    // Fallback: 32 random bytes for demo purposes.
    const fallback = new Uint8Array(32);
    window.crypto.getRandomValues(fallback);
    state.session.publicHex = bufferToHex(fallback);
    state.session.privateHex = bufferToHex(fallback);
    console.warn("Ed25519 WebCrypto unsupported, using random fallback", error);
  }

  sessionPublic.textContent = state.session.publicHex;
  sessionPrivate.textContent = state.session.privateHex;
}

function parseBinding(raw) {
  const trimmed = raw.trim();
  if (!trimmed) {
    return null;
  }
  if (trimmed.startsWith("{")) {
    try {
      return JSON.parse(trimmed);
    } catch (error) {
      console.warn("Could not parse binding JSON, returning raw", error);
      return trimmed;
    }
  }
  return trimmed;
}

function buildCommand() {
  const bindingRaw = bindingInput.value.trim();
  const binding = parseBinding(bindingRaw);
  if (!binding) {
    commandOutput.textContent = "Provide a keyed hash first.";
    payloadOutput.textContent = "";
    return;
  }

  const sponsorId =
    sponsorInput.value.trim() || "soraゴヂアネタアニャヴウザニキニョゾビバヤチョトテスシコダオョキュロッベイゴビチャヰショチャパヨツサツホキマイニキ";
  const torii = toriiInput.value.trim() || "http://localhost:8080";
  const chainId = chainInput.value.trim() || "dev-chain";
  const amount = amountInput.value.trim() || "10";
  const sessionAccount =
    sessionAccountInput.value.trim() ||
    "soraゴヂアニワグヹツムチョトヹヒョロポギフンルシハドクホャヒョヮブヒャギウゥトヒュボニョミガジヶャマアアミサヘ";

  const payload = {
    chain_id: chainId,
    authority: sponsorId,
    metadata: {
      walletless_session_pubkey_hex: state.session.publicHex,
      session_account_id: sessionAccount,
    },
    instructions: [
      {
        SendToTwitter: {
          binding_hash: binding,
          amount,
        },
      },
      {
        ClaimTwitterFollowReward: {
          binding_hash: binding,
        },
      },
    ],
  };

  const nodeCmd = [
    "node javascript/iroha_js/recipes/walletlessFollowGame.mjs",
    `--torii ${torii}`,
    `--chain-id ${chainId}`,
    `--sponsor ${sponsorId}`,
    `--binding '${bindingRaw.replace(/\s+/g, " ")}'`,
    `--amount ${amount}`,
    `--session-key ${state.session.privateHex}`,
    `--session-account ${sessionAccount}`,
  ].join(" \\\n  ");

  commandOutput.textContent = nodeCmd;
  payloadOutput.textContent = JSON.stringify(payload, null, 2);
}

document.getElementById("regen-session").addEventListener("click", generateSessionKey);
document.getElementById("build-flow").addEventListener("click", buildCommand);

generateSessionKey();

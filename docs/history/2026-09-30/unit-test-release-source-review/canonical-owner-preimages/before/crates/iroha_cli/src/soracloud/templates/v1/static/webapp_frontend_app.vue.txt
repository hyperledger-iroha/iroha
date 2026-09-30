<template>
  <main class="shell">
    <h1>__SERVICE_NAME__ Control Panel</h1>
    <p>Use an Ed25519 wallet to sign the challenge message and paste the signature.</p>
    <section>
      <h2>1) Request Challenge</h2>
      <form @submit.prevent="requestChallenge">
        <label>
          Public Key (32-byte hex)
          <input v-model="publicKey" placeholder="ed25519 public key hex" />
        </label>
        <button type="submit">Request Challenge</button>
      </form>
      <p v-if="challengeId">challenge id: {{ challengeId }}</p>
      <textarea
        v-if="challengeMessage"
        rows="6"
        readonly
        :value="challengeMessage"
      />
    </section>

    <section>
      <h2>2) Login</h2>
      <form @submit.prevent="login">
        <label>
          Signature (64-byte hex)
          <input v-model="signature" placeholder="ed25519 signature hex" />
        </label>
        <button type="submit">Login</button>
      </form>
    </section>

    <section>
      <h2>Session</h2>
      <button type="button" @click="loadMe">Refresh /api/auth/me</button>
      <button type="button" @click="logout">Logout</button>
      <p v-if="principal">principal: {{ principal }}</p>
      <p v-if="capabilities.length > 0">capabilities: {{ capabilities.join(", ") }}</p>
    </section>

    <p v-if="message">{{ message }}</p>
    <p v-if="error" class="error">{{ error }}</p>
  </main>
</template>

<script setup lang="ts">
import { ref } from "vue";

const publicKey = ref("");
const challengeId = ref("");
const challengeMessage = ref("");
const signature = ref("");
const principal = ref("");
const capabilities = ref<string[]>([]);
const message = ref("");
const error = ref("");

async function parseJson(response: Response) {
  const text = await response.text();
  if (!text) {
    return {};
  }
  return JSON.parse(text);
}

async function requestChallenge() {
  error.value = "";
  message.value = "";
  const response = await fetch("/api/auth/challenge", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ public_key: publicKey.value })
  });
  const payload = await parseJson(response);
  if (!response.ok) {
    error.value = payload.error ?? "challenge request failed";
    return;
  }
  challengeId.value = payload.challenge_id ?? "";
  challengeMessage.value = payload.message ?? "";
  message.value = "challenge issued; sign the message then submit login.";
}

async function login() {
  error.value = "";
  message.value = "";
  const response = await fetch("/api/auth/login", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      public_key: publicKey.value,
      challenge_id: challengeId.value,
      signature: signature.value
    })
  });
  const payload = await parseJson(response);
  if (!response.ok) {
    error.value = payload.error ?? "login failed";
    return;
  }
  principal.value = payload.principal ?? "";
  capabilities.value = payload.capabilities ?? [];
  message.value = "session established";
}

async function loadMe() {
  error.value = "";
  const response = await fetch("/api/auth/me");
  const payload = await parseJson(response);
  if (!response.ok) {
    error.value = payload.error ?? "session check failed";
    return;
  }
  principal.value = payload.principal ?? "";
  capabilities.value = payload.capabilities ?? [];
}

async function logout() {
  error.value = "";
  await fetch("/api/auth/logout", { method: "POST" });
  principal.value = "";
  capabilities.value = [];
  challengeId.value = "";
  signature.value = "";
  message.value = "session closed";
}
</script>

<style scoped>
.shell {
  font-family: "Avenir Next", "Segoe UI", sans-serif;
  max-width: 760px;
  margin: 3rem auto;
  padding: 0 1.25rem;
}

section {
  margin: 1.25rem 0;
  padding: 1rem;
  border: 1px solid #dde4ec;
  border-radius: 0.5rem;
}

form {
  display: grid;
  gap: 0.75rem;
  margin: 0.75rem 0;
}

input,
textarea {
  width: 100%;
  padding: 0.5rem;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}

button {
  margin-right: 0.75rem;
}

.error {
  color: #b42318;
}
</style>

<template>
  <main class="shell">
    <h1>__SERVICE_NAME__ PII Control Panel</h1>
    <p>Private routes require deterministic challenge login and capability authorization.</p>

    <section>
      <h2>Auth</h2>
      <form @submit.prevent="requestChallenge">
        <label>
          Public Key (32-byte hex)
          <input v-model="publicKey" placeholder="ed25519 public key hex" />
        </label>
        <button type="submit">Request Challenge</button>
      </form>
      <textarea
        v-if="challengeMessage"
        rows="6"
        readonly
        :value="challengeMessage"
      />
      <form @submit.prevent="login">
        <label>
          Signature (64-byte hex)
          <input v-model="signature" placeholder="ed25519 signature hex" />
        </label>
        <button type="submit">Login</button>
      </form>
      <button type="button" @click="loadMe">Refresh /pii/api/auth/me</button>
      <button type="button" @click="logout">Logout</button>
      <p v-if="principal">principal: {{ principal }}</p>
      <p v-if="capabilities.length > 0">capabilities: {{ capabilities.join(", ") }}</p>
    </section>

    <section>
      <h2>Consent</h2>
      <form @submit.prevent="grantConsent">
        <label>
          Subject ID
          <input v-model="subjectId" placeholder="subject-001" />
        </label>
        <label>
          Scope
          <input v-model="scope" placeholder="records.read" />
        </label>
        <button type="submit">Grant Consent</button>
      </form>
      <button type="button" @click="revokeConsent">Revoke Consent</button>
      <button type="button" @click="listConsentState">List Consent State</button>
    </section>

    <section>
      <h2>Retention / Deletion</h2>
      <button type="button" @click="runRetention">Run Retention Sweep</button>
      <button type="button" @click="requestDeletion">Request Subject Deletion</button>
      <button type="button" @click="listRetentionRuns">List Retention Runs</button>
    </section>

    <pre v-if="details">{{ details }}</pre>
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
const subjectId = ref("subject-001");
const scope = ref("records.read");
const message = ref("");
const error = ref("");
const details = ref("");

async function parseJson(response: Response) {
  const text = await response.text();
  if (!text) {
    return {};
  }
  return JSON.parse(text);
}

async function post(path: string, body: Record<string, string>) {
  error.value = "";
  const response = await fetch(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body)
  });
  const payload = await parseJson(response);
  if (!response.ok) {
    error.value = payload.error ?? "request failed";
    return null;
  }
  details.value = JSON.stringify(payload, null, 2);
  return payload;
}

async function get(path: string) {
  error.value = "";
  const response = await fetch(path);
  const payload = await parseJson(response);
  if (!response.ok) {
    error.value = payload.error ?? "request failed";
    return null;
  }
  details.value = JSON.stringify(payload, null, 2);
  return payload;
}

async function requestChallenge() {
  const payload = await post("/pii/api/auth/challenge", { public_key: publicKey.value });
  if (payload) {
    challengeId.value = payload.challenge_id ?? "";
    challengeMessage.value = payload.message ?? "";
    message.value = "challenge issued";
  }
}

async function login() {
  const payload = await post("/pii/api/auth/login", {
    public_key: publicKey.value,
    challenge_id: challengeId.value,
    signature: signature.value
  });
  if (payload) {
    principal.value = payload.principal ?? "";
    capabilities.value = payload.capabilities ?? [];
    message.value = "session established";
  }
}

async function loadMe() {
  const payload = await get("/pii/api/auth/me");
  if (payload) {
    principal.value = payload.principal ?? "";
    capabilities.value = payload.capabilities ?? [];
  }
}

async function logout() {
  await fetch("/pii/api/auth/logout", { method: "POST" });
  principal.value = "";
  capabilities.value = [];
  message.value = "session closed";
}

async function grantConsent() {
  const payload = await post("/pii/api/consent/grant", {
    subject_id: subjectId.value,
    scope: scope.value
  });
  if (payload) {
    message.value = `consent granted for ${payload.subject_id}`;
  }
}

async function revokeConsent() {
  const payload = await post("/pii/api/consent/revoke", {
    subject_id: subjectId.value,
    scope: scope.value
  });
  if (payload) {
    message.value = `consent revoked for ${payload.subject_id}`;
  }
}

async function runRetention() {
  const payload = await post("/pii/api/records/retention/sweep", {
    jurisdiction: "us",
    policy_version: "retention-v1"
  });
  if (payload) {
    message.value = `retention sweep planned=${payload.planned_actions}`;
  }
}

async function requestDeletion() {
  const payload = await post("/pii/api/records/delete", {
    subject_id: subjectId.value,
    reason: "subject request"
  });
  if (payload) {
    message.value = `deletion ticket ${payload.ticket_id}`;
  }
}

async function listConsentState() {
  const payload = await get("/pii/api/consent/state");
  if (payload) {
    message.value = "consent state refreshed";
  }
}

async function listRetentionRuns() {
  const payload = await get("/pii/api/retention/runs");
  if (payload) {
    message.value = "retention runs refreshed";
  }
}
</script>

<style scoped>
.shell {
  font-family: "Avenir Next", "Segoe UI", sans-serif;
  max-width: 860px;
  margin: 3rem auto;
  padding: 0 1.25rem;
}

section {
  margin: 1.5rem 0;
  padding: 1rem;
  border: 1px solid #dde4ec;
  border-radius: 0.5rem;
}

form {
  display: grid;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
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

pre {
  overflow: auto;
  padding: 0.75rem;
  border: 1px solid #dde4ec;
  border-radius: 0.5rem;
  background: #f7fafc;
}

.error {
  color: #b42318;
}
</style>

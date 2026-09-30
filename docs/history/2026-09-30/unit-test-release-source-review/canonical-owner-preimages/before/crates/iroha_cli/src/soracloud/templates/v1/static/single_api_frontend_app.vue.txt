<template>
  <main class="shell">
    <p class="eyebrow">Soracloud Single-API App</p>
    <h1>__APP_NAME__</h1>
    <p class="lede">
      Root-bound frontend published from <code>web/dist</code> with a deterministic API
      mounted under <code>/api</code>.
    </p>

    <section class="card">
      <div class="row">
        <span>Frontend mount</span>
        <code>/</code>
      </div>
      <div class="row">
        <span>API health route</span>
        <code>/api/healthz</code>
      </div>
      <button type="button" @click="checkHealth" :disabled="loading">
        {{ loading ? "Checking..." : "Check API Health" }}
      </button>
      <pre v-if="payload">{{ payload }}</pre>
      <p v-if="error" class="error">{{ error }}</p>
    </section>
  </main>
</template>

<script setup lang="ts">
import { ref } from "vue";

const loading = ref(false);
const payload = ref("");
const error = ref("");

async function checkHealth() {
  loading.value = true;
  error.value = "";
  try {
    const response = await fetch("/api/healthz");
    const body = await response.text();
    if (!response.ok) {
      throw new Error(body || `health check failed with status ${response.status}`);
    }
    payload.value = body;
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : String(caught);
  } finally {
    loading.value = false;
  }
}
</script>

<style scoped>
.shell {
  font-family: "Avenir Next", "Segoe UI", sans-serif;
  max-width: 780px;
  margin: 4rem auto;
  padding: 0 1.25rem 4rem;
  color: #16324f;
}

.eyebrow {
  margin: 0 0 0.75rem;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  font-size: 0.8rem;
  color: #567189;
}

.lede {
  max-width: 48rem;
  line-height: 1.6;
}

.card {
  margin-top: 2rem;
  padding: 1.25rem;
  border: 1px solid #dde4ec;
  border-radius: 0.9rem;
  background: linear-gradient(180deg, #ffffff 0%, #f7fafc 100%);
  box-shadow: 0 18px 40px rgba(22, 50, 79, 0.08);
}

.row {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  margin-bottom: 0.75rem;
}

button {
  margin-top: 0.5rem;
  padding: 0.7rem 1rem;
  border-radius: 999px;
  border: 0;
  background: #16324f;
  color: white;
  font: inherit;
  cursor: pointer;
}

button:disabled {
  opacity: 0.7;
  cursor: wait;
}

pre {
  margin: 1rem 0 0;
  padding: 1rem;
  border-radius: 0.75rem;
  background: #0f172a;
  color: #dbeafe;
  overflow-x: auto;
}

.error {
  margin-top: 1rem;
  color: #b42318;
}
</style>

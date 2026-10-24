<script setup>
import { ref } from "vue";
import MedicationPanel from "./components/MedicationPanel.vue";

// Önálló (PWA / mobil) Gyógyszer-ellenőrzés: nincs ügy, nincs adatbázis, nincs tárolás.
const error = ref("");
const left = ref(false);
const generation = ref(0);

function reopen() {
  left.value = false;
  error.value = "";
  generation.value += 1;
}
</script>

<template>
  <main class="mx-auto min-h-screen max-w-5xl px-4 py-5 sm:px-6" style="padding-bottom: max(1.25rem, env(safe-area-inset-bottom))">
    <div v-if="error" role="alert" class="mb-4 rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800">
      {{ error }}
    </div>

    <div v-if="left" class="grid gap-4 rounded-3xl border border-slate-200 bg-white p-6 text-center">
      <p class="text-lg font-semibold text-slate-900">A Gyógyszer-ellenőrzést nem nyitotta meg.</p>
      <p class="text-base text-slate-700">Bizonytalan gyógyszer esetén kérdezzen gyógyszerészt vagy orvost. Veszély esetén hívja a <strong>112</strong>-t.</p>
      <button class="justify-self-center rounded-full bg-red-600 px-6 py-3 text-base font-bold text-white hover:bg-red-700" @click="reopen">
        Megnyitás
      </button>
    </div>

    <MedicationPanel
      v-else
      :key="generation"
      @leave="left = true"
      @error="(message) => (error = message)"
    />
  </main>
</template>

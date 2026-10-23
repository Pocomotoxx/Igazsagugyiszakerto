<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  activateTab,
  browser,
  closeTab,
  hideAll,
  navigate,
  setBounds,
  showActive,
  tabAction,
} from "../browserStore.js";

const props = defineProps({
  caseId: { type: Number, default: null },
  // Üres panelnél ide kerülnek a javasolt indítóoldalak (pl. a keresési terv lépései).
  suggestions: { type: Array, default: () => [] },
});
const emit = defineEmits(["open-url", "archive", "error"]);

const viewport = ref(null);
const addressInput = ref("");
let observer = null;

const activeTab = computed(() => browser.tabs.find((tab) => tab.id === browser.activeId) || null);

watch(
  () => activeTab.value?.url,
  (url) => {
    if (url) addressInput.value = url;
  },
  { immediate: true },
);

function measure() {
  const el = viewport.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  setBounds({ x: rect.left, y: rect.top, width: rect.width, height: rect.height });
}

onMounted(async () => {
  await nextTick();
  measure();
  await showActive();
  observer = new ResizeObserver(measure);
  observer.observe(viewport.value);
  window.addEventListener("resize", measure);
  window.addEventListener("scroll", measure, true);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  window.removeEventListener("resize", measure);
  window.removeEventListener("scroll", measure, true);
  hideAll();
});

function normalizeAddress(raw) {
  const value = raw.trim();
  if (!value) return "";
  if (/^https?:\/\//i.test(value)) return value;
  if (/^[^\s]+\.[^\s]{2,}$/.test(value) && !value.includes(" ")) return `https://${value}`;
  return `https://www.google.com/search?q=${encodeURIComponent(value)}`;
}

async function submitAddress() {
  const url = normalizeAddress(addressInput.value);
  if (!url) return;
  try {
    if (activeTab.value) {
      await navigate({ caseId: props.caseId, tabId: activeTab.value.id, url });
    } else {
      emit("open-url", { url, title: "" });
    }
  } catch (error) {
    emit("error", `Navigálás sikertelen: ${String(error)}`);
  }
}

async function run(action) {
  if (!activeTab.value) return;
  try {
    await tabAction(activeTab.value.id, action);
  } catch (error) {
    emit("error", `Böngészőművelet sikertelen: ${String(error)}`);
  }
}
</script>

<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <div
        v-for="tab in browser.tabs"
        :key="tab.id"
        :class="[
          'flex max-w-[220px] items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-semibold transition',
          tab.id === browser.activeId
            ? 'border-slate-900 bg-slate-900 text-white'
            : 'border-slate-200 bg-white text-slate-700 hover:border-slate-300',
        ]"
      >
        <button class="min-w-0 flex-1 truncate text-left" @click="activateTab(tab.id)">
          <span v-if="tab.loading" class="mr-1">…</span>{{ tab.title }}
        </button>
        <button class="opacity-70 hover:opacity-100" title="Fül bezárása" @click="closeTab(tab.id)">×</button>
      </div>
    </div>

    <div class="flex items-center gap-2">
      <button class="rounded-full border border-slate-200 bg-white px-3 py-2 text-xs font-semibold disabled:opacity-40" :disabled="!activeTab" @click="run('back')">←</button>
      <button class="rounded-full border border-slate-200 bg-white px-3 py-2 text-xs font-semibold disabled:opacity-40" :disabled="!activeTab" @click="run('forward')">→</button>
      <button class="rounded-full border border-slate-200 bg-white px-3 py-2 text-xs font-semibold disabled:opacity-40" :disabled="!activeTab" @click="run('reload')">⟳</button>
      <input
        v-model="addressInput"
        @keydown.enter="submitAddress"
        placeholder="Cím vagy keresőkifejezés"
        class="min-w-0 flex-1 rounded-full border border-slate-200 bg-white px-4 py-2 text-sm outline-none"
      />
      <button
        class="rounded-full border border-sky-200 bg-sky-50 px-4 py-2 text-xs font-semibold text-sky-700 transition hover:bg-sky-100 disabled:opacity-40"
        :disabled="!activeTab"
        @click="emit('archive', activeTab)"
      >
        Archiválás
      </button>
    </div>

    <div ref="viewport" class="relative h-[70vh] min-h-[420px] overflow-hidden rounded-3xl border border-slate-200 bg-white">
      <div v-if="!activeTab" class="flex h-full flex-col items-center justify-center gap-3 p-6 text-center text-sm text-slate-500">
        <div>Nincs megnyitott fül. Nyiss meg egy találatot a „Böngészőben” gombbal, vagy a keresési terv lépéseit.</div>
        <div v-if="suggestions.length" class="flex flex-wrap justify-center gap-2">
          <button
            v-for="item in suggestions"
            :key="item.url"
            class="rounded-full border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:border-slate-400"
            @click="emit('open-url', item)"
          >
            {{ item.title }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

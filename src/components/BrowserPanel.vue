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
const addressEl = ref(null);
const focused = ref(false);
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
  window.addEventListener("keydown", onKey);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  window.removeEventListener("resize", measure);
  window.removeEventListener("scroll", measure, true);
  window.removeEventListener("keydown", onKey);
  hideAll();
});

const hueOf = (text) => {
  let hash = 0;
  for (const ch of text || "") hash = (hash * 31 + ch.charCodeAt(0)) % 360;
  return hash;
};
const initialOf = (tab) => (tab.title || tab.url || "?").replace(/^https?:\/\/(www\.)?/, "").charAt(0).toUpperCase();
const hostOf = (url) => {
  try {
    return new URL(url).hostname;
  } catch {
    return "";
  }
};
const isSecure = computed(() => (activeTab.value?.url || "").startsWith("https://"));
const displayAddress = computed(() => {
  if (focused.value || !activeTab.value) return addressInput.value;
  try {
    const u = new URL(activeTab.value.url);
    return u.hostname.replace(/^www\./, "") + (u.pathname + u.search === "/" ? "" : u.pathname + u.search);
  } catch {
    return addressInput.value;
  }
});

function newTab() {
  emit("open-url", { url: "https://www.google.com/", title: "Új lap" });
}

function onKey(event) {
  if (!(event.ctrlKey || event.metaKey)) return;
  const key = event.key.toLowerCase();
  if (key === "l") {
    event.preventDefault();
    addressEl.value?.focus();
    addressEl.value?.select();
  } else if (key === "t") {
    event.preventDefault();
    newTab();
  } else if (key === "w" && activeTab.value) {
    event.preventDefault();
    closeTab(activeTab.value.id);
  } else if (key === "r" && activeTab.value) {
    event.preventDefault();
    run("reload");
  }
}

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
  <div class="overflow-hidden rounded-2xl border border-slate-300 bg-slate-100 shadow-lg">
    <!-- Fülsáv -->
    <div class="flex items-end gap-1 bg-slate-200 px-2 pt-2">
      <div
        v-for="tab in browser.tabs"
        :key="tab.id"
        @click="activateTab(tab.id)"
        @auxclick.middle.prevent="closeTab(tab.id)"
        :class="[
          'group relative flex h-9 min-w-[110px] max-w-[220px] flex-1 cursor-default items-center gap-2 rounded-t-xl px-3 text-xs transition',
          tab.id === browser.activeId
            ? 'bg-white text-slate-900 shadow-[0_-1px_0_rgba(0,0,0,0.06)]'
            : 'text-slate-600 hover:bg-slate-100/80',
        ]"
        :title="tab.title"
      >
        <span
          v-if="tab.loading"
          class="h-4 w-4 shrink-0 animate-spin rounded-full border-2 border-slate-300 border-t-sky-500"
        ></span>
        <span
          v-else
          class="flex h-4 w-4 shrink-0 items-center justify-center rounded-full text-[9px] font-bold text-white"
          :style="{ background: `hsl(${hueOf(hostOf(tab.url))} 55% 45%)` }"
        >{{ initialOf(tab) }}</span>
        <span class="min-w-0 flex-1 truncate font-medium">{{ tab.title }}</span>
        <button
          class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-slate-500 hover:bg-slate-300/70"
          title="Lap bezárása (Ctrl+W)"
          @click.stop="closeTab(tab.id)"
        >×</button>
      </div>
      <button
        class="mb-1 flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-lg leading-none text-slate-600 hover:bg-slate-300/70"
        title="Új lap (Ctrl+T)"
        @click="newTab"
      >+</button>
    </div>

    <!-- Eszköztár -->
    <div class="relative flex items-center gap-1 border-b border-slate-200 bg-white px-2 py-1.5">
      <button class="flex h-8 w-8 items-center justify-center rounded-full text-slate-600 hover:bg-slate-100 disabled:opacity-30" :disabled="!activeTab" title="Vissza" @click="run('back')">←</button>
      <button class="flex h-8 w-8 items-center justify-center rounded-full text-slate-600 hover:bg-slate-100 disabled:opacity-30" :disabled="!activeTab" title="Előre" @click="run('forward')">→</button>
      <button class="flex h-8 w-8 items-center justify-center rounded-full text-slate-600 hover:bg-slate-100 disabled:opacity-30" :disabled="!activeTab" title="Újratöltés (Ctrl+R)" @click="run('reload')">⟳</button>

      <div
        :class="[
          'mx-1 flex h-9 min-w-0 flex-1 items-center gap-2 rounded-full px-4 transition',
          focused ? 'bg-white ring-2 ring-sky-500' : 'bg-slate-100 hover:bg-slate-200/70',
        ]"
      >
        <span :class="isSecure ? 'text-emerald-600' : 'text-slate-400'" class="text-xs" :title="isSecure ? 'Biztonságos kapcsolat (HTTPS)' : 'Nem titkosított vagy nincs oldal'">{{ isSecure ? "🔒" : "ⓘ" }}</span>
        <input
          ref="addressEl"
          :value="displayAddress"
          @input="addressInput = $event.target.value"
          @focus="focused = true; $event.target.select()"
          @blur="focused = false"
          @keydown.enter="submitAddress(); $event.target.blur()"
          @keydown.esc="addressInput = activeTab?.url || ''; $event.target.blur()"
          placeholder="Keresés vagy webcím megadása (Ctrl+L)"
          class="min-w-0 flex-1 bg-transparent text-sm text-slate-900 outline-none"
        />
      </div>

      <button
        class="flex h-8 items-center gap-1.5 rounded-full bg-sky-50 px-3 text-xs font-semibold text-sky-700 transition hover:bg-sky-100 disabled:opacity-30"
        :disabled="!activeTab"
        title="Az oldal archiválása az aktív ügyhöz"
        @click="emit('archive', activeTab)"
      >⬇ Archiválás</button>

      <!-- Betöltési csík -->
      <div v-if="activeTab?.loading" class="absolute inset-x-0 bottom-0 h-0.5 overflow-hidden bg-sky-100">
        <div class="h-full w-1/3 animate-[irbr-load_1.1s_ease-in-out_infinite] bg-sky-500"></div>
      </div>
    </div>

    <!-- Tartalom -->
    <div ref="viewport" class="relative h-[72vh] min-h-[440px] bg-white">
      <div v-if="!activeTab" class="flex h-full flex-col items-center justify-center gap-4 p-6 text-center">
        <div class="text-lg font-semibold text-slate-700">IRBR böngésző</div>
        <div class="max-w-md text-sm text-slate-500">Nyiss meg egy találatot a „Böngészőben" gombbal, írj címet a sávba, vagy indíts a keresési terv lépéseivel.</div>
        <div v-if="suggestions.length" class="flex max-w-2xl flex-wrap justify-center gap-2">
          <button
            v-for="item in suggestions"
            :key="item.url"
            class="rounded-full border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:border-slate-400 hover:bg-slate-50"
            @click="emit('open-url', item)"
          >{{ item.title }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
@keyframes irbr-load {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(300%); }
}
</style>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  retryTab,
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
  platforms: { type: Array, default: () => [] },
  enabledIds: { type: Array, default: () => [] },
  initialQuery: { type: String, default: "" },
});
const emit = defineEmits(["open-url", "archive", "error", "search", "toggle-platform"]);

const viewport = ref(null);
const addressInput = ref("");
const searchInput = ref(props.initialQuery);
const platformsOpen = ref(false);
const isEnabled = (id) => props.enabledIds.length === 0 || props.enabledIds.includes(id);
const enabledCount = computed(() => props.platforms.filter((p) => isEnabled(p.id)).length);
function submitSearch() {
  const query = searchInput.value.trim();
  if (query) emit("search", query);
}
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

let lastRect = "";
function measure() {
  const el = viewport.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const key = [rect.left, rect.top, rect.width, rect.height].map((v) => Math.round(v)).join(",");
  if (key === lastRect) return;
  lastRect = key;
  setBounds({ x: rect.left, y: rect.top, width: rect.width, height: rect.height });
}
let poll = null;

onMounted(async () => {
  await nextTick();
  measure();
  await showActive();
  observer = new ResizeObserver(measure);
  observer.observe(viewport.value);
  window.addEventListener("resize", measure);
  window.addEventListener("scroll", measure, true);
  window.addEventListener("keydown", onKey);
  // a bannerek, a fülsor és az értesítések eltolhatják a nézetet; a natív webview követi
  poll = setInterval(measure, 250);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  window.removeEventListener("resize", measure);
  window.removeEventListener("scroll", measure, true);
  window.removeEventListener("keydown", onKey);
  clearInterval(poll);
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
    <!-- Keresés minden platformon -->
    <div class="relative flex flex-wrap items-center gap-2 border-b border-slate-200 bg-slate-50 px-3 py-2">
      <input
        v-model="searchInput"
        @keydown.enter="submitSearch"
        placeholder="Keresés minden platformon (pl. takarítás eger órabér)"
        class="min-w-[240px] flex-1 rounded-full border border-slate-200 bg-white px-4 py-2 text-sm outline-none focus:border-slate-400"
      />
      <button @click="submitSearch" :disabled="!searchInput.trim()" class="rounded-full bg-sky-600 px-5 py-2 text-sm font-semibold text-white transition hover:bg-sky-500 disabled:opacity-50">
        Keresés
      </button>
      <button @click="platformsOpen = !platformsOpen" class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:border-slate-300">
        Platformok ({{ enabledCount }}/{{ platforms.length }}) ▾
      </button>
      <div v-if="platformsOpen" class="absolute right-3 top-full z-20 mt-1 grid max-h-80 w-80 gap-1 overflow-auto rounded-2xl border border-slate-200 bg-white p-3 shadow-xl">
        <label v-for="platform in platforms" :key="platform.id" class="flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1 text-xs hover:bg-slate-50">
          <input type="checkbox" class="accent-slate-900" :checked="isEnabled(platform.id)" @change="emit('toggle-platform', platform.id)" />
          <span class="font-semibold text-slate-800">{{ platform.label }}</span>
          <span class="ml-auto text-[10px] uppercase tracking-wide text-slate-400">{{ platform.kind }}</span>
        </label>
      </div>
    </div>

    <!-- Fülsáv -->
    <div class="flex items-end gap-1 overflow-x-auto bg-slate-200 px-2 pt-2">
      <div
        v-for="tab in browser.tabs"
        :key="tab.id"
        @click="activateTab(tab.id)"
        @auxclick.middle.prevent="closeTab(tab.id)"
        :class="[
          'group relative flex h-9 min-w-[120px] max-w-[220px] shrink-0 flex-1 cursor-default items-center gap-2 rounded-t-xl px-3 text-xs transition',
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
        :disabled="!activeTab || activeTab.kind === 'medication'"
        :title="activeTab?.kind === 'medication' ? 'A gyógyszer-ellenőrzés oldalai nem archiválhatók (egészségügyi adat).' : 'Az oldal archiválása az aktív ügyhöz'"
        @click="emit('archive', activeTab)"
      >⬇ Archiválás</button>

      <!-- Betöltési csík -->
      <div v-if="activeTab?.loading" class="absolute inset-x-0 bottom-0 h-0.5 overflow-hidden bg-sky-100">
        <div class="h-full w-1/3 animate-[irbr-load_1.1s_ease-in-out_infinite] bg-sky-500"></div>
      </div>
    </div>

    <div
      v-if="activeTab && (activeTab.error || activeTab.slow)"
      class="flex flex-wrap items-center gap-3 border-b border-amber-200 bg-amber-50 px-4 py-2 text-xs text-amber-900"
    >
      <span v-if="activeTab.error">A fül nem tölthető be: {{ activeTab.error }}</span>
      <span v-else>Az oldal betöltése lassú vagy elakadt. Ha üres marad, próbáld újra, vagy nyisd meg külső böngészőben.</span>
      <button class="rounded-full border border-amber-300 bg-white px-3 py-1 font-semibold" @click="retryTab(activeTab.id)">Újra</button>
      <button class="rounded-full border border-amber-300 bg-white px-3 py-1 font-semibold" @click="openUrl(activeTab.url)">Külső böngészőben</button>
    </div>

    <!-- Gyógyszer-ellenőrzés: a figyelmeztetés a fülek mellett végig látszik -->
    <div
      v-if="activeTab && activeTab.kind === 'medication'"
      role="alert"
      class="border-b-2 border-red-700 bg-red-600 px-4 py-2 text-sm font-bold text-white"
    >
      ⚠ FIGYELEM: ez csak internetes keresés, NEM helyettesíti a szakszerű gyógyszerazonosítást. Ismeretlen gyógyszert ne vegyen be és ne adjon be senkinek; kérdezzen gyógyszerészt vagy orvost. Veszély esetén: 112.
    </div>

    <!-- Tartalom -->
    <div ref="viewport" class="relative h-[72vh] min-h-[440px] bg-white">
      <div v-if="!activeTab" class="flex h-full flex-col items-center justify-center gap-4 p-6 text-center">
        <div class="text-lg font-semibold text-slate-700">IRBR böngésző</div>
        <div class="max-w-md text-sm text-slate-500">Írj be egy keresést fent: minden platformhoz (Google, Facebook, Magyar Közlöny, Nemzeti Jogszabálytár, bíróságok stb.) külön fül nyílik a találataival.</div>
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

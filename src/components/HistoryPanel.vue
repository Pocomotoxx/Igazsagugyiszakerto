<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

const props = defineProps({
  caseId: { type: Number, default: null },
  searchRuns: { type: Array, default: () => [] },
});
const emit = defineEmits(["error"]);

const PAGE_SIZE = 20;
const blankFilters = () => ({ text: "", tier: 0, status: "", decision: "", domain: "", archive_kind: "", date_from: "", date_to: "" });

const filters = ref(blankFilters());
const page = ref(0);
const data = ref({ total: 0, items: [], timeline: [] });
const loading = ref(false);
const inspection = ref(null);
const exportFormat = ref("csv");
const exportedPath = ref("");
const compareA = ref(null);
const compareB = ref(null);
const delta = ref(null);
const healthRows = ref([]);

const pageCount = computed(() => Math.max(1, Math.ceil(data.value.total / PAGE_SIZE)));
const maxDay = computed(() => Math.max(1, ...data.value.timeline.map((point) => point.count)));
const providerIds = computed(() => {
  const ids = new Map();
  healthRows.value.forEach((row) => (row.providers || []).forEach((p) => ids.set(p.id, p.label)));
  return Array.from(ids.entries());
});

async function call(command, args) {
  try {
    return await invoke(command, args);
  } catch (error) {
    emit("error", `${command}: ${String(error)}`);
    return null;
  }
}

async function load() {
  if (!props.caseId) return;
  loading.value = true;
  const result = await call("search_history", {
    caseId: props.caseId,
    filters: { ...filters.value, tier: Number(filters.value.tier) || 0 },
    page: page.value,
    pageSize: PAGE_SIZE,
  });
  if (result) data.value = result;
  loading.value = false;
}

async function loadHealth() {
  if (!props.caseId) return;
  healthRows.value = (await call("get_source_health", { caseId: props.caseId, limit: 12 })) || [];
}

function applyFilters() {
  page.value = 0;
  load();
}

function resetFilters() {
  filters.value = blankFilters();
  applyFilters();
}

async function open(item) {
  inspection.value = await call("inspect_source", { sourceId: item.id });
}

async function exportNow() {
  exportedPath.value = "";
  const path = await call("export_sources", {
    caseId: props.caseId,
    format: exportFormat.value,
    filters: { ...filters.value, tier: Number(filters.value.tier) || 0 },
  });
  if (path) exportedPath.value = path;
}

async function reveal() {
  try {
    await revealItemInDir(exportedPath.value);
  } catch (error) {
    emit("error", `A mappa megnyitása sikertelen: ${String(error)}`);
  }
}

async function compare() {
  delta.value = null;
  if (!compareA.value || !compareB.value || compareA.value === compareB.value) {
    emit("error", "Válassz két különböző keresési futamot az összehasonlításhoz.");
    return;
  }
  delta.value = await call("compare_search_runs", { runA: compareA.value, runB: compareB.value });
}

const fmt = (value) => value || "—";
const freshnessClass = (value) =>
  value === "friss"
    ? "border-emerald-200 bg-emerald-50 text-emerald-700"
    : value === "elavult"
      ? "border-amber-200 bg-amber-50 text-amber-700"
      : "border-slate-200 bg-slate-50 text-slate-500";
const integrityClass = (state) =>
  state === "ok"
    ? "border-emerald-300 bg-emerald-50 text-emerald-800"
    : state === "eltér"
      ? "border-rose-300 bg-rose-50 text-rose-800"
      : "border-amber-300 bg-amber-50 text-amber-800";
const stateClass = (state) =>
  state === "ok"
    ? "bg-emerald-100 text-emerald-800"
    : state === "hiba"
      ? "bg-rose-100 text-rose-800"
      : "bg-slate-100 text-slate-500";

function cell(row, id) {
  return (row.providers || []).find((p) => p.id === id);
}

onMounted(() => {
  load();
  loadHealth();
});
watch(() => props.caseId, () => {
  inspection.value = null;
  load();
  loadHealth();
});
watch(page, load);
</script>

<template>
  <div v-if="!caseId" class="text-sm text-slate-500">Válassz aktív ügyet az előzmények megtekintéséhez.</div>
  <div v-else class="grid gap-6">
    <div>
      <h2 class="text-2xl font-semibold text-slate-950">Előzmények és export</h2>
      <p class="mt-1 text-sm text-slate-600">
        Az ügy archivált forrásai szűrhetően, idővonallal. Egy rekordra kattintva látszik az eredet, az időpontok, az integritás és a kapcsolódó anyagok.
      </p>
    </div>

    <!-- Szűrők -->
    <div class="grid gap-3 rounded-3xl border border-slate-200 bg-white/85 p-4 md:grid-cols-4">
      <input v-model="filters.text" @keydown.enter="applyFilters" placeholder="Szöveg (ékezet- és kisbetű-független)" class="rounded-2xl border border-slate-200 px-3 py-2 text-sm outline-none md:col-span-2" />
      <input v-model="filters.domain" @keydown.enter="applyFilters" placeholder="Domain (pl. ksh.hu)" class="rounded-2xl border border-slate-200 px-3 py-2 text-sm outline-none" />
      <select v-model="filters.tier" class="rounded-2xl border border-slate-200 px-3 py-2 text-sm outline-none">
        <option :value="0">Minden tier</option>
        <option :value="1">Tier 1</option>
        <option :value="2">Tier 1–2</option>
        <option :value="3">Tier 1–3</option>
      </select>
      <select v-model="filters.status" class="rounded-2xl border border-slate-200 px-3 py-2 text-sm outline-none">
        <option value="">Minden státusz</option>
        <option value="nyers">Nyers</option>
        <option value="ellenorzes_alatt">Ellenőrzés alatt</option>
        <option value="validalt">Validált</option>
      </select>
      <select v-model="filters.decision" class="rounded-2xl border border-slate-200 px-3 py-2 text-sm outline-none">
        <option value="">Minden döntés</option>
        <option value="felhasznalhato">Felhasználható</option>
        <option value="hatteranyag">Háttéranyag</option>
        <option value="kizarando">Kizárandó</option>
      </select>
      <select v-model="filters.archive_kind" class="rounded-2xl border border-slate-200 px-3 py-2 text-sm outline-none">
        <option value="">Minden típus</option>
        <option value="live">élő</option>
        <option value="wayback_snapshot">Wayback snapshot</option>
        <option value="wayback_index">Wayback index</option>
      </select>
      <input v-model="filters.date_from" type="date" class="rounded-2xl border border-slate-200 px-3 py-2 text-sm outline-none" title="Begyűjtés kezdete" />
      <input v-model="filters.date_to" type="date" class="rounded-2xl border border-slate-200 px-3 py-2 text-sm outline-none" title="Begyűjtés vége" />
      <div class="flex gap-2 md:col-span-4">
        <button @click="applyFilters" class="rounded-full bg-slate-900 px-4 py-2 text-xs font-semibold text-white hover:bg-slate-700">Szűrés</button>
        <button @click="resetFilters" class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700">Törlés</button>
        <div class="ml-auto flex items-center gap-2">
          <select v-model="exportFormat" class="rounded-full border border-slate-200 bg-white px-3 py-2 text-xs font-semibold outline-none">
            <option value="csv">CSV</option>
            <option value="json">JSON</option>
            <option value="html">Nyomtatható HTML</option>
            <option value="stix">STIX 2.1</option>
          </select>
          <button @click="exportNow" class="rounded-full border border-sky-200 bg-sky-50 px-4 py-2 text-xs font-semibold text-sky-700 hover:bg-sky-100">
            Exportálás ({{ data.total }})
          </button>
        </div>
      </div>
      <div v-if="exportedPath" class="rounded-2xl border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-800 md:col-span-4">
        Mentve: <span class="break-all font-mono">{{ exportedPath }}</span>
        <button @click="reveal" class="ml-2 font-semibold underline">Mappa megnyitása</button>
      </div>
    </div>

    <!-- Idővonal -->
    <div v-if="data.timeline.length" class="rounded-3xl border border-slate-200 bg-white/85 p-4">
      <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Begyűjtés idővonala (nap szerint)</div>
      <div class="mt-3 flex h-20 items-end gap-1 overflow-x-auto">
        <div
          v-for="point in data.timeline"
          :key="point.date"
          class="flex min-w-[14px] flex-1 flex-col items-center justify-end"
          :title="`${point.date}: ${point.count} forrás`"
        >
          <div class="w-full rounded-t bg-sky-500" :style="{ height: `${Math.max(6, (point.count / maxDay) * 100)}%` }"></div>
        </div>
      </div>
      <div class="mt-1 flex justify-between text-[10px] text-slate-500">
        <span>{{ data.timeline[0].date }}</span>
        <span>{{ data.timeline[data.timeline.length - 1].date }}</span>
      </div>
    </div>

    <div class="grid gap-6 xl:grid-cols-[1fr_1fr]">
      <!-- Lista -->
      <div class="grid content-start gap-3">
        <div class="text-xs text-slate-500">{{ loading ? "Betöltés…" : `${data.total} találat` }}</div>
        <button
          v-for="item in data.items"
          :key="item.id"
          @click="open(item)"
          :class="[
            'rounded-2xl border bg-white p-4 text-left transition hover:border-slate-400',
            inspection?.record.id === item.id ? 'border-slate-900' : 'border-slate-200',
          ]"
        >
          <div class="flex flex-wrap items-center gap-2 text-[11px] font-semibold">
            <span class="rounded-full border border-slate-200 bg-slate-50 px-2 py-0.5">Tier {{ item.tier }}</span>
            <span :class="freshnessClass(item.freshness)" class="rounded-full border px-2 py-0.5">{{ item.freshness || "ismeretlen" }}</span>
            <span class="text-slate-500">{{ item.collected_at }}</span>
          </div>
          <div class="mt-2 text-sm font-semibold text-slate-900">{{ item.title }}</div>
          <div class="mt-1 break-all text-xs text-slate-500">{{ item.url }}</div>
        </button>
        <div v-if="!loading && !data.items.length" class="rounded-2xl border border-dashed border-slate-300 p-6 text-center text-sm text-slate-500">
          Nincs a szűrőnek megfelelő archivált forrás.
        </div>
        <div class="flex items-center justify-between text-xs">
          <button :disabled="page === 0" @click="page--" class="rounded-full border border-slate-200 bg-white px-3 py-1.5 font-semibold disabled:opacity-40">← Előző</button>
          <span class="text-slate-500">{{ page + 1 }} / {{ pageCount }}</span>
          <button :disabled="page + 1 >= pageCount" @click="page++" class="rounded-full border border-slate-200 bg-white px-3 py-1.5 font-semibold disabled:opacity-40">Következő →</button>
        </div>
      </div>

      <!-- Rekordvizsgáló -->
      <div class="rounded-3xl border border-slate-200 bg-white/90 p-5">
        <div v-if="!inspection" class="text-sm text-slate-500">Válassz egy forrást a listából a vizsgálathoz.</div>
        <div v-else class="grid gap-4">
          <div>
            <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Rekordvizsgáló</div>
            <h3 class="mt-1 text-lg font-semibold text-slate-900">{{ inspection.record.title }}</h3>
            <div class="mt-1 break-all text-xs text-slate-500">{{ inspection.record.url }}</div>
          </div>

          <div :class="integrityClass(inspection.integrity.state)" class="rounded-2xl border px-3 py-2 text-xs">
            <div class="font-semibold">Integritás: {{ inspection.integrity.state }}</div>
            <div class="mt-1">{{ inspection.integrity.detail }}</div>
            <div v-if="inspection.integrity.stored_hash" class="mt-1 break-all font-mono text-[10px]">SHA-256: {{ inspection.integrity.stored_hash }}</div>
          </div>

          <dl class="grid grid-cols-[9rem_1fr] gap-x-3 gap-y-1 text-xs">
            <dt class="text-slate-500">Közzététel</dt><dd>{{ fmt(inspection.record.published_at) }}</dd>
            <dt class="text-slate-500">Módosítás</dt><dd>{{ fmt(inspection.record.modified_at) }}</dd>
            <dt class="text-slate-500">Megfigyelés (snapshot)</dt><dd>{{ fmt(inspection.record.observed_at) }}</dd>
            <dt class="text-slate-500">Begyűjtés</dt><dd>{{ fmt(inspection.record.collected_at) }}</dd>
            <dt class="text-slate-500">Időforrás</dt><dd>{{ fmt(inspection.record.time_source) }}</dd>
            <dt class="text-slate-500">Frissesség</dt><dd>{{ fmt(inspection.record.freshness) }}</dd>
            <dt class="text-slate-500">Státusz / döntés</dt><dd>{{ inspection.record.source_status }} / {{ inspection.record.usability_decision }}</dd>
          </dl>
          <p class="text-[11px] leading-5 text-slate-500">
            A közzétételi idő a forrás saját jelzése. Ha nincs megadva, az üres marad, és a begyűjtés ideje nem helyettesíti.
          </p>

          <div v-if="inspection.related.length">
            <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Kapcsolódó rekordok</div>
            <ul class="mt-2 grid gap-1 text-xs">
              <li v-for="rel in inspection.related.slice(0, 12)" :key="rel.id" class="rounded-xl bg-slate-50 px-3 py-2">
                <span class="font-semibold">{{ rel.title }}</span>
                <span class="ml-2 text-violet-700">{{ rel.reason }}</span>
              </li>
            </ul>
          </div>

          <div v-if="inspection.audit.length">
            <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Auditesemények</div>
            <ul class="mt-2 grid gap-1 text-xs text-slate-600">
              <li v-for="(entry, index) in inspection.audit" :key="index">
                <span class="font-mono text-[10px] text-slate-400">{{ entry.timestamp }}</span> · {{ entry.action_type }}
              </li>
            </ul>
          </div>

          <details v-if="inspection.content_excerpt">
            <summary class="cursor-pointer text-xs font-semibold text-slate-600">Tartalom részlete</summary>
            <pre class="mt-2 max-h-56 overflow-auto whitespace-pre-wrap rounded-xl bg-slate-50 p-3 text-[11px] leading-5">{{ inspection.content_excerpt }}</pre>
          </details>
        </div>
      </div>
    </div>

    <!-- Futamok összehasonlítása -->
    <div class="rounded-3xl border border-slate-200 bg-white/85 p-5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Keresési futamok összehasonlítása</div>
      <div class="mt-3 flex flex-wrap items-center gap-2 text-xs">
        <select v-model="compareA" class="rounded-full border border-slate-200 bg-white px-3 py-2 outline-none">
          <option :value="null" disabled>Régebbi futam</option>
          <option v-for="run in searchRuns" :key="run.id" :value="run.id">#{{ run.id }} · {{ run.created_at }}</option>
        </select>
        <span>→</span>
        <select v-model="compareB" class="rounded-full border border-slate-200 bg-white px-3 py-2 outline-none">
          <option :value="null" disabled>Újabb futam</option>
          <option v-for="run in searchRuns" :key="run.id" :value="run.id">#{{ run.id }} · {{ run.created_at }}</option>
        </select>
        <button @click="compare" class="rounded-full bg-slate-900 px-4 py-2 font-semibold text-white hover:bg-slate-700">Összehasonlít</button>
      </div>
      <div v-if="delta" class="mt-4 grid gap-4 md:grid-cols-2">
        <div>
          <div class="text-xs font-semibold text-emerald-700">Új találatok ({{ delta.new_total }})</div>
          <ul class="mt-2 grid gap-1 text-xs">
            <li v-for="item in delta.new_items" :key="item.url" class="break-all rounded-xl bg-emerald-50 px-3 py-1.5">{{ item.title }}<span class="block text-[10px] text-slate-500">{{ item.url }}</span></li>
          </ul>
        </div>
        <div>
          <div class="text-xs font-semibold text-rose-700">Kiesett találatok ({{ delta.dropped_total }})</div>
          <ul class="mt-2 grid gap-1 text-xs">
            <li v-for="item in delta.dropped_items" :key="item.url" class="break-all rounded-xl bg-rose-50 px-3 py-1.5">{{ item.title }}<span class="block text-[10px] text-slate-500">{{ item.url }}</span></li>
          </ul>
        </div>
        <div class="text-xs text-slate-600 md:col-span-2">
          Változatlan: {{ delta.kept_count }}.
          <span v-for="change in delta.tier_changes" :key="change.url" class="ml-2">
            {{ change.title }}: Tier {{ change.from }} → {{ change.to }}
          </span>
        </div>
      </div>
      <p v-if="!searchRuns.length" class="mt-3 text-xs text-slate-500">Még nincs rögzített keresési futam ebben az ügyben.</p>
    </div>

    <!-- Forrás-állapot mátrix -->
    <div class="rounded-3xl border border-slate-200 bg-white/85 p-5">
      <div class="flex items-center justify-between">
        <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Forrás-állapot mátrix (utolsó futamok)</div>
        <button @click="loadHealth" class="rounded-full border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold">Frissítés</button>
      </div>
      <div v-if="healthRows.length && providerIds.length" class="mt-3 overflow-x-auto">
        <table class="w-full text-left text-xs">
          <thead>
            <tr class="text-slate-500">
              <th class="py-1 pr-3 font-semibold">Futam</th>
              <th v-for="[id, label] in providerIds" :key="id" class="py-1 pr-3 font-semibold">{{ label }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in healthRows" :key="row.run_id" class="border-t border-slate-100">
              <td class="py-1.5 pr-3 font-mono text-[11px]">#{{ row.run_id }} <span class="text-slate-400">{{ row.created_at }}</span></td>
              <td v-for="[id] in providerIds" :key="id" class="py-1.5 pr-3">
                <span v-if="cell(row, id)" :class="stateClass(cell(row, id).state)" class="rounded-full px-2 py-0.5 font-semibold" :title="cell(row, id).error">
                  {{ cell(row, id).state }}{{ cell(row, id).state === "ok" ? ` · ${cell(row, id).count}` : "" }}
                </span>
                <span v-else class="text-slate-300">—</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else class="mt-3 text-xs text-slate-500">Az állapotadatok az új keresési futamokkal kezdenek gyűlni.</p>
    </div>
  </div>
</template>

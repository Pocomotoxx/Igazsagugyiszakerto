<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import HelpTooltip from "./components/HelpTooltip.vue";
import SourceReviewPanel from "./components/SourceReviewPanel.vue";
import BrowserPanel from "./components/BrowserPanel.vue";
import HistoryPanel from "./components/HistoryPanel.vue";
import { captureTab, closeSearchTabs, openTab as openBrowserTab } from "./browserStore.js";

const allTabs = [
  { id: "demo", label: "Bemutató", advanced: true },
  { id: "cases", label: "Ügyek" },
  { id: "browser", label: "Böngésző" },
  { id: "archive", label: "Archivum" },
  { id: "history", label: "Előzmények", advanced: true },
  { id: "audit", label: "Audit", advanced: true },
  { id: "report", label: "Riport" },
];

const UI_MODE_KEY = "irbr-ui-mode";
function loadUiMode() {
  try {
    return localStorage.getItem(UI_MODE_KEY) === "advanced" ? "advanced" : "simple";
  } catch {
    return "simple";
  }
}
const uiMode = ref(loadUiMode());
const advanced = computed(() => uiMode.value === "advanced");
const tabs = computed(() => allTabs.filter((tab) => advanced.value || !tab.advanced));

function setUiMode(mode) {
  uiMode.value = mode;
  try {
    localStorage.setItem(UI_MODE_KEY, mode);
  } catch {
    // a mód csak kényelmi beállítás, mentés nélkül is működik
  }
  if (mode === "simple" && allTabs.find((tab) => tab.id === currentTab.value)?.advanced) {
    currentTab.value = "cases";
  }
}

const engines = [
  { id: "google", label: "Google", note: "Széles recall, gyors első futam." },
  { id: "bing", label: "Bing", note: "Eltérő indexelés, más találati sorrend." },
  { id: "duckduckgo", label: "DuckDuckGo", note: "Más rangsorolás, jó kontrollkör." },
];

const searchModes = [
  { id: "native", label: "Natív böngésző alapú", note: "Kereső- és forrásnyitási terv; ha be van állítva keresési API (Tavily, CSE, Brave), valódi webes találatokkal kiegészítve." },
  { id: "hybrid", label: "AI + natív böngésző", note: "A natív futamot (terv + valódi találatok) AI keresési javaslatokkal egészíti ki." },
  { id: "ai_only", label: "Csak AI API", note: "A futam kizárólag AI-alapú keresési és ellenőrzési javaslatokat ad, keresési API-k nélkül." },
];

const aiProviders = [
  { id: "openai", label: "OpenAI" },
  { id: "openrouter", label: "OpenRouter" },
  { id: "gemini", label: "Google Gemini" },
];

const nativeRunModes = [
  { id: "broad", label: "Széles webes futam", note: "Minimális szűrés, valódi Google/Bing recall." },
  { id: "filtered", label: "Szűrt szakértői futam", note: "Forráscsomagokkal és szakértői szűkítéssel." },
  { id: "combined", label: "Kombinált futam", note: "Széles és szűrt futam együtt." },
];

const tierOptions = [
  { value: 0, label: "Minden tier" },
  { value: 1, label: "Csak Tier 1 - Hivatalos" },
  { value: 2, label: "Tier 1-2 - Hivatalos és tudományos" },
  { value: 3, label: "Tier 1-3 - Piaci / keresőig" },
  { value: 4, label: "Tier 1-4 - Kontrollal együtt" },
];

const presets = [
  { id: "haztartas", label: "Háztartási munka", must: "háztartási munka", phrase: "óradíj" },
  { id: "gondozas", label: "Gondozás", must: "házi gondozás", phrase: "óradíj" },
  { id: "takaritas", label: "Takarítás", must: "takarítás", phrase: "óradíj" },
  { id: "bebi", label: "Bébiszitter", must: "bébiszitter", phrase: "óradíj" },
  { id: "mezogazdasag", label: "Mezőgazdasági gépi munka", must: "mezőgazdasági gépi munka", phrase: "órabér" },
];

const cases = ref([]);
const currentCase = ref(null);
const currentTab = ref("cases");
const sources = ref([]);
const auditLogs = ref([]);
const searchRuns = ref([]);
const searchResults = ref([]);
const selectedRunIds = ref([]);
const activeSearchRunId = ref(null);
const archiveRunFilter = ref("all");
const archiveStatusFilter = ref("all");
const archiveDecisionFilter = ref("all");
const archivePriorityFilter = ref("all");
const archiveOriginFilter = ref("all");
const expandedResultKeys = ref({});
const expandedRunIds = ref({});
const searchPlanExpanded = ref(false);
const runNoteDrafts = ref({});
const sourceReviewDrafts = ref({});
const caseOpinionDraft = ref({
  preliminary_opinion: "",
  final_opinion: "",
  report_status: "tervezet",
  finalizable: "nem",
  open_questions: "",
  closed_points: "",
});
const selectedEngines = ref(["google", "bing"]);
const selectedTierFilter = ref(0);
const isSearching = ref(false);
const isSavingReport = ref(false);
const busyArchiveId = ref(null);
const reportPath = ref("");
const reportSaved = ref(false);
const lastSearchSummary = ref("");
const appError = ref("");
const searchWarning = ref("");
const searchDiagnostics = ref(null);
const runDelta = ref(null);

const PLATFORMS_KEY = "irbr-platforms";
const platformList = ref([]);
const platformLinks = ref([]);
function loadEnabledPlatforms() {
  try {
    const raw = JSON.parse(localStorage.getItem(PLATFORMS_KEY) || "[]");
    return Array.isArray(raw) ? raw.filter((id) => typeof id === "string") : [];
  } catch {
    return [];
  }
}
// üres lista = minden platform bekapcsolva
const enabledPlatforms = ref(loadEnabledPlatforms());
const isPlatformEnabled = (id) => enabledPlatforms.value.length === 0 || enabledPlatforms.value.includes(id);
const activePlatformCount = computed(() => platformList.value.filter((p) => isPlatformEnabled(p.id)).length);
const platformQuery = (id) => platformLinks.value.find((link) => link.id === id)?.query || "";

function togglePlatform(id) {
  const all = platformList.value.map((p) => p.id);
  let current = enabledPlatforms.value.length === 0 ? [...all] : [...enabledPlatforms.value];
  current = current.includes(id) ? current.filter((item) => item !== id) : [...current, id];
  if (current.length === 0) return; // legalább egy platform maradjon bekapcsolva
  enabledPlatforms.value = current.length === all.length ? [] : current;
  try {
    localStorage.setItem(PLATFORMS_KEY, JSON.stringify(enabledPlatforms.value));
  } catch {
    // a beállítás csak kényelmi, mentés nélkül is működik
  }
}

let platformLinksTimer = null;
async function refreshPlatformLinks() {
  const query = broadQuery.value.trim();
  if (!query) {
    platformLinks.value = [];
    return;
  }
  try {
    platformLinks.value = await api("platform_links", { query, enabledPlatforms: enabledPlatforms.value });
  } catch {
    platformLinks.value = [];
  }
}
const searchHealth = ref([]);
const expandedStories = ref({});

const stateRank = { hiba: 3, ok: 2, kihagyva: 1, kikapcsolva: 0 };

function mergeHealth(outputs) {
  const merged = new Map();
  outputs.flatMap((output) => output.health || []).forEach((entry) => {
    const current = merged.get(entry.id);
    if (!current) {
      merged.set(entry.id, { ...entry });
      return;
    }
    current.count += entry.count;
    current.ms += entry.ms;
    if ((stateRank[entry.state] ?? 0) > (stateRank[current.state] ?? 0)) current.state = entry.state;
    if (entry.error && !current.error) current.error = entry.error;
  });
  return Array.from(merged.values());
}

const realHitCount = computed(
  () => searchResults.value.filter((item) => item.evidence_level === "RETRIEVED").length,
);
const cardCount = computed(() => searchResults.value.length - realHitCount.value);
const activeHealth = computed(() => searchHealth.value.filter((e) => e.state === "ok" || e.state === "hiba"));
const inactiveHealthCount = computed(() => searchHealth.value.length - activeHealth.value.length);
const ddgEntry = computed(() => searchHealth.value.find((e) => e.id === "browser" || e.id === "ddg"));

const resultTab = ref("all");
const EXTRA_TAB_LABELS = { web: "Webes találatok", ai: "AI javaslatok", egyeb: "További források" };
const platformKey = (item) => item.platform || "egyeb";

const resultTabs = computed(() => {
  const counts = new Map();
  searchResults.value.forEach((item) => {
    const key = platformKey(item);
    const entry = counts.get(key) || { total: 0, real: 0 };
    entry.total += 1;
    if (item.evidence_level === "RETRIEVED") entry.real += 1;
    counts.set(key, entry);
  });
  const order = ["web", ...platformList.value.map((p) => p.id), "egyeb", "ai"];
  const labelOf = (id) => platformList.value.find((p) => p.id === id)?.label || EXTRA_TAB_LABELS[id] || id;
  const tabs = order
    .filter((id, index) => counts.has(id) && order.indexOf(id) === index)
    .map((id) => ({ id, label: labelOf(id), ...counts.get(id) }));
  const real = searchResults.value.filter((item) => item.evidence_level === "RETRIEVED").length;
  return [{ id: "all", label: "Összes", total: searchResults.value.length, real }, ...tabs];
});

const visibleResults = computed(() => {
  const seen = new Set();
  const inTab = searchResults.value.filter(
    (item) => resultTab.value === "all" || platformKey(item) === resultTab.value,
  );
  return inTab.filter((result) => {
    if (!result.story_id || result.story_size < 2) return true;
    if (expandedStories.value[result.story_id]) return true;
    if (seen.has(result.story_id)) return false;
    seen.add(result.story_id);
    return true;
  });
});

function hiddenInStory(result) {
  if (!result.story_id || result.story_size < 2) return 0;
  return expandedStories.value[result.story_id] ? 0 : result.story_size - 1;
}

function toggleStory(result) {
  expandedStories.value = {
    ...expandedStories.value,
    [result.story_id]: !expandedStories.value[result.story_id],
  };
}

const freshnessClass = (value) =>
  value === "friss"
    ? "border-emerald-200 bg-emerald-50 text-emerald-700"
    : value === "elavult"
      ? "border-amber-200 bg-amber-50 text-amber-700"
      : "border-slate-200 bg-slate-50 text-slate-500";

const healthClass = (state) =>
  state === "ok"
    ? "border-emerald-200 bg-emerald-50 text-emerald-700"
    : state === "hiba"
      ? "border-rose-200 bg-rose-50 text-rose-700"
      : "border-slate-200 bg-slate-50 text-slate-500";

const formatDay = (iso) => (iso ? iso.slice(0, 10) : "");

const newCaseTitle = ref("");
const newCaseDesc = ref("");
const newCaseDeadline = ref("");

const searchTerms = ref({
  mustInclude: "",
  exactPhrase: "",
  anyOf: "",
  exclude: "",
  site: "",
  region: "",
  yearFrom: "",
  yearTo: "",
});
const rawQuery = ref("");
const sourcePacks = ref({
  official: true,
  science: true,
  market: true,
  archive: true,
  reddit: false,
  facebook_groups: true,
  shared: false,
  community_general: false,
  control: false,
});
const researchLens = ref("balanced");
const searchMode = ref("native");
const nativeRunMode = ref("combined");
const settingsSaved = ref(false);
const cseSettings = ref({
  api_key: "",
  api_key_set: false,
  cx: "",
});
const braveSettings = ref({
  api_key: "",
  api_key_set: false,
});
const tavilySettings = ref({
  api_key: "",
  api_key_set: false,
});
const aiSettings = ref({
  ai_provider: "openai",
  api_key: "",
  api_key_set: false,
  ai_model: "gpt-4o-mini",
  system_prompt:
    "Te egy igazsagugyi szakerto munkajat tamogato kutatasi asszisztens vagy. Adj rovid, gyakorlatias keresesi javaslatokat, ellenorzesi szempontokat, keresokifejezes-variantokat es lehetoseg szerint relevans forrashelyeket. JSON formatumban valaszolj.",
});

async function api(command, args = {}) {
  return invoke(command, args);
}

const aiConfigured = computed(
  () => aiSettings.value.api_key.trim().length > 0 || aiSettings.value.api_key_set,
);

const cseConfigured = computed(
  () =>
    cseSettings.value.cx.trim().length > 0 &&
    (cseSettings.value.api_key.trim().length > 0 || cseSettings.value.api_key_set),
);

const braveConfigured = computed(
  () => braveSettings.value.api_key.trim().length > 0 || braveSettings.value.api_key_set,
);

const tavilyConfigured = computed(
  () => tavilySettings.value.api_key.trim().length > 0 || tavilySettings.value.api_key_set,
);

const sourcePackMeta = {
  official: {
    label: "Hivatalos",
    domains: ["site:njt.hu", "site:magyarkozlony.hu", "site:ksh.hu", "site:kozbeszerzes.hu"],
    info: "njt.hu, magyarkozlony.hu, ksh.hu, kozbeszerzes.hu",
    note: "Jogszabályok, közlönyök, statisztikák és közbeszerzési értesítők.",
  },
  science: {
    label: "Tudományos",
    domains: ["site:scholar.google.com", "site:ncbi.nlm.nih.gov"],
    info: "scholar.google.com, ncbi.nlm.nih.gov",
    note: "Tudományos publikációk és módszertani háttéranyagok.",
  },
  market: {
    label: "Piaci",
    domains: ["site:profession.hu", "site:allasok.hu", "site:jooble.org"],
    info: "profession.hu, allasok.hu, jooble.org",
    note: "Munkaerőpiaci és piaci kontrollforrások.",
  },
  archive: {
    label: "Webarchív",
    domains: ["site:archive.org"],
    info: "archive.org",
    note: "Internet Archive és archivált dokumentumváltozatok.",
  },
  reddit: {
    label: "Reddit kontroll",
    domains: ["site:reddit.com"],
    info: "reddit.com",
    note: "Közösségi kontroll és gyakorlati tapasztalatok.",
  },
  facebook_groups: {
    label: "Facebook csoportok",
    domains: ["site:facebook.com/groups", "site:facebook.com"],
    info: "facebook.com/groups, facebook.com",
    note: "Nyilvánosan indexelt Facebook csoportok és oldalak.",
  },
  shared: {
    label: "Megosztott",
    domains: ["site:drive.google.com", "site:docs.google.com"],
    info: "drive.google.com, docs.google.com",
    note: "Megosztott dokumentumok, táblázatok és prezentációk.",
  },
  community_general: {
    label: "Általános közösségi kontroll",
    domains: ["site:facebook.com", "site:jofogas.hu"],
    info: "facebook.com, jofogas.hu",
    note: "Vegyes közösségi és piactéri kontrollréteg.",
  },
  control: {
    label: "Vegyes kontroll",
    domains: ["site:jofogas.hu", "site:facebook.com"],
    info: "jofogas.hu, facebook.com",
    note: "Gyors ellenőrző kontrollforrások.",
  },
};

const sourcePackQueries = Object.fromEntries(
  Object.entries(sourcePackMeta).map(([key, value]) => [key, value.domains]),
);

const selectedSourcePackKeys = computed(() =>
  Object.entries(sourcePacks.value)
    .filter(([, enabled]) => enabled)
    .map(([key]) => key),
);

const selectedSourcePackSummary = computed(() => {
  if (selectedSourcePackKeys.value.length === 0) {
    return "Nincs forráscsomag kiválasztva, ezért a rendszer széles webes keresést használ.";
  }

  return selectedSourcePackKeys.value
    .map((key) => {
      const meta = sourcePackMeta[key];
      return `${meta?.label || key}: ${meta?.info || ""}`;
    })
    .join(" | ");
});

async function loadAppSettings() {
  const settings = await api("get_app_settings");
  searchMode.value = settings.search_mode || "native";
  aiSettings.value = {
    ai_provider: settings.ai_provider || "openai",
    api_key: "",
    api_key_set: Boolean(settings.api_key_set),
    ai_model: settings.ai_model || "gpt-4o-mini",
    system_prompt: settings.system_prompt || aiSettings.value.system_prompt,
  };
  cseSettings.value = {
    api_key: "",
    api_key_set: Boolean(settings.google_cse_key_set),
    cx: settings.google_cse_cx || "",
  };
  braveSettings.value = {
    api_key: "",
    api_key_set: Boolean(settings.brave_key_set),
  };
  tavilySettings.value = {
    api_key: "",
    api_key_set: Boolean(settings.tavily_key_set),
  };
}

async function saveAppSettings() {
  try {
    await api("save_app_settings", {
      searchMode: searchMode.value,
      aiProvider: aiSettings.value.ai_provider,
      apiKey: aiSettings.value.api_key,
      aiModel: aiSettings.value.ai_model,
      systemPrompt: aiSettings.value.system_prompt,
      googleCseKey: cseSettings.value.api_key,
      googleCseCx: cseSettings.value.cx,
      braveKey: braveSettings.value.api_key,
      tavilyKey: tavilySettings.value.api_key,
    });
    if (aiSettings.value.api_key.trim()) {
      aiSettings.value.api_key_set = true;
      aiSettings.value.api_key = "";
    }
    if (cseSettings.value.api_key.trim()) {
      cseSettings.value.api_key_set = true;
      cseSettings.value.api_key = "";
    }
    if (braveSettings.value.api_key.trim()) {
      braveSettings.value.api_key_set = true;
      braveSettings.value.api_key = "";
    }
    if (tavilySettings.value.api_key.trim()) {
      tavilySettings.value.api_key_set = true;
      tavilySettings.value.api_key = "";
    }
    settingsSaved.value = true;
    appError.value = "";
    setTimeout(() => {
      settingsSaved.value = false;
    }, 2500);
  } catch (error) {
    appError.value = `Beállítás mentési hiba: ${String(error)}`;
  }
}

function buildYearClause(yearFrom, yearTo) {
  const start = Number.parseInt(yearFrom, 10);
  const end = Number.parseInt(yearTo, 10);

  if (Number.isNaN(start) && Number.isNaN(end)) {
    return "";
  }

  const currentYear = new Date().getFullYear();
  const effectiveStart = Number.isNaN(start) ? null : start;
  const effectiveEnd = Number.isNaN(end) ? null : end;

  if (effectiveStart !== null && effectiveEnd !== null && effectiveEnd >= effectiveStart) {
    if (effectiveEnd - effectiveStart <= 6) {
      return `(${Array.from({ length: effectiveEnd - effectiveStart + 1 }, (_, index) => effectiveStart + index).join(" OR ")})`;
    }
    return `${effectiveStart}..${effectiveEnd}`;
  }

  if (effectiveStart !== null) {
    if (currentYear - effectiveStart <= 6 && currentYear >= effectiveStart) {
      return `(${Array.from({ length: currentYear - effectiveStart + 1 }, (_, index) => effectiveStart + index).join(" OR ")})`;
    }
    return `after:${effectiveStart}`;
  }

  return `before:${effectiveEnd}`;
}

function buildExternalSearchUrl(engine, query) {
  const encoded = encodeURIComponent(query);
  if (engine === "bing") return `https://www.bing.com/search?q=${encoded}`;
  if (engine === "duckduckgo") return `https://duckduckgo.com/?q=${encoded}`;
  return `https://www.google.com/search?q=${encoded}`;
}

function buildFallbackSearchResults(query) {
  return selectedEngines.value.map((engine) => ({
    title: `${engines.find((item) => item.id === engine)?.label || engine} keresés indítása`,
    url: buildExternalSearchUrl(engine, query),
    original_url: "",
    archived_url: "",
    archive_kind: "live",
    snapshot_timestamp: "",
    snippet: "Védőháló találat: közvetlen keresőnyitás ugyanarra a lekérdezésre, ha a generált terv valamiért nem adott vissza kártyát.",
    tier: 3,
    source_type: "Keresőmotor",
    relevance: 100,
    query_used: query,
    action_label: "Megnyitás",
  }));
}

onMounted(async () => {
  try {
    await api("init_db");
    try {
      platformList.value = await api("list_platforms");
    } catch {
      platformList.value = [];
    }
    await loadAppSettings();
    await loadCases();
  } catch (error) {
    appError.value = `Inicializációs hiba: ${String(error)}`;
  }
});

watch(currentCase, async (value) => {
  if (!value) {
    sources.value = [];
    auditLogs.value = [];
    searchRuns.value = [];
    caseOpinionDraft.value = {
      preliminary_opinion: "",
      final_opinion: "",
      report_status: "tervezet",
      finalizable: "nem",
      open_questions: "",
      closed_points: "",
    };
    return;
  }
  await Promise.all([loadSources(), loadAuditLogs(), loadSearchRuns()]);
  caseOpinionDraft.value = {
    preliminary_opinion: value.preliminary_opinion || "",
    final_opinion: value.final_opinion || "",
    report_status: value.report_status || "tervezet",
    finalizable: value.finalizable || "nem",
    open_questions: value.open_questions || "",
    closed_points: value.closed_points || "",
  };
});

const broadQuery = computed(() => {
  if (rawQuery.value.trim()) {
    return rawQuery.value.trim();
  }

  const parts = [];
  if (searchTerms.value.mustInclude.trim()) {
    parts.push(searchTerms.value.mustInclude.trim());
  }
  if (searchTerms.value.exactPhrase.trim()) {
    parts.push(`"${searchTerms.value.exactPhrase.trim()}"`);
  }
  if (searchTerms.value.anyOf.trim()) {
    parts.push(`(${searchTerms.value.anyOf.trim().split(/\s+/).join(" OR ")})`);
  }
  if (searchTerms.value.exclude.trim()) {
    parts.push(
      searchTerms.value.exclude
        .trim()
        .split(/\s+/)
        .map((term) => `-${term}`)
        .join(" "),
    );
  }
  if (searchTerms.value.site.trim()) {
    parts.push(`site:${searchTerms.value.site.trim()}`);
  }
  if (searchTerms.value.region.trim()) {
    parts.push(searchTerms.value.region.trim());
  }
  const yearClause = buildYearClause(searchTerms.value.yearFrom.trim(), searchTerms.value.yearTo.trim());
  if (yearClause) {
    parts.push(yearClause);
  }

  return parts.join(" ").trim();
});

const constructedQuery = computed(() => broadQuery.value);

watch([broadQuery, enabledPlatforms], () => {
  clearTimeout(platformLinksTimer);
  platformLinksTimer = setTimeout(refreshPlatformLinks, 300);
});

const sourcePackLabels = {
  official: "Hivatalos források",
  science: "Tudományos források",
  market: "Piaci források",
  archive: "Webarchív",
  reddit: "Reddit kontroll",
  facebook_groups: "Facebook csoportok",
  shared: "Megosztott dokumentumok",
  community_general: "Általános közösségi",
  control: "Kontrollforrások",
};

const lensClause = computed(() => {
  if (researchLens.value === "pricing") {
    return "(árlista OR óradíj OR bérkalkuláció OR díjtáblázat)";
  }
  if (researchLens.value === "legal") {
    return "(jogszabály OR módszertan OR iránymutatás OR szakvélemény)";
  }
  if (researchLens.value === "operations") {
    return "(költség OR normatíva OR üzemeltetés OR hatékonyság)";
  }
  return "";
});

function buildScopedBaseQuery() {
  const base = broadQuery.value;
  if (!base) return "";
  return lensClause.value ? `${base} ${lensClause.value}` : base;
}

const filteredQueries = computed(() => {
  const base = buildScopedBaseQuery();
  if (!base) return [];

  const queries = [];

  if (searchTerms.value.site.trim()) {
    queries.push({
      label: "Cél domain",
      query: base,
      info: `Kézi domainkorlát: ${searchTerms.value.site.trim()}`,
      note: "A keresés a kézzel megadott cél domainre van korlátozva.",
    });
  } else if (selectedSourcePackKeys.value.length > 0) {
    selectedSourcePackKeys.value.forEach((key) => {
      const terms = sourcePackQueries[key] || [];
      const meta = sourcePackMeta[key] || {};
      if (terms.length === 0) return;
      queries.push({
        label: meta.label || sourcePackLabels[key] || key,
        query: `${base} (${terms.join(" OR ")})`,
        info: meta.info || terms.join(", "),
        note: meta.note || "Csak a kiválasztott forráscsomag oldalain futó keresés.",
      });
    });
  } else {
    queries.push({
      label: "Széles webes futam",
      query: base,
      info: "Nincs forráscsomag-korlát",
      note: "Széles webes keresés forráscsomag-szűkítés nélkül.",
    });
  }

  const seen = new Set();
  return queries.filter(({ query }) => {
    if (seen.has(query) || query === base) return false;
    seen.add(query);
    return true;
  });
});

const GOOGLE_WORD_LIMIT = 32;

function countQueryWords(query) {
  return query.split(/\s+/).filter(Boolean).length;
}

const queryWordWarnings = computed(() => {
  const warnings = [];
  if (broadQuery.value && countQueryWords(broadQuery.value) > GOOGLE_WORD_LIMIT) {
    warnings.push(`A széles alaplekérdezés ${countQueryWords(broadQuery.value)} szóból áll.`);
  }
  filteredQueries.value.forEach(({ label, query }) => {
    const count = countQueryWords(query);
    if (count > GOOGLE_WORD_LIMIT) {
      warnings.push(`A(z) "${label}" szűrt lekérdezés ${count} szóból áll.`);
    }
  });
  return warnings;
});

const searchPlan = computed(() => {
  if (filteredQueries.value.length === 0) return [];

  const plans = [];
  filteredQueries.value.forEach(({ label, query, info, note }) => {
    plans.push({
      title: `${plans.length + 1}. Forráskorlátozott futam: ${label}`,
      note: `${note || "Csak a kiválasztott forráscsomag oldalain futó keresés."} Keresett oldalak: ${info || "nincs külön megadva"}.`,
      query,
    });
  });

  return plans;
});

const browserPlanSuggestions = computed(() =>
  platformLinks.value.map((link) => ({ title: link.label, url: link.url })),
);

const archiveInsights = computed(() => {
  const tierCounts = { 1: 0, 2: 0, 3: 0, 4: 0 };
  const domainCounts = new Map();

  sources.value.forEach((source) => {
    tierCounts[source.tier] = (tierCounts[source.tier] || 0) + 1;
    try {
      const host = new URL(source.url).hostname.replace(/^www\./, "");
      domainCounts.set(host, (domainCounts.get(host) || 0) + 1);
    } catch {
      domainCounts.set("ismeretlen", (domainCounts.get("ismeretlen") || 0) + 1);
    }
  });

  const duplicatedDomains = Array.from(domainCounts.entries())
    .filter(([, count]) => count > 1)
    .sort((left, right) => right[1] - left[1]);

  const missingTiers = [1, 2, 3].filter((tier) => !tierCounts[tier]);

  return {
    tierCounts,
    duplicatedDomains,
    missingTiers,
  };
});

const selectedRunComparison = computed(() => {
  if (selectedRunIds.value.length < 2) return [];
  return selectedRunIds.value
    .map((id) => searchRuns.value.find((run) => run.id === id))
    .filter(Boolean);
});

const archiveGroups = computed(() => {
  const grouped = new Map();

  sources.value.forEach((source) => {
    const key = source.search_run_id ?? "unlinked";
    if (!grouped.has(key)) {
      grouped.set(key, []);
    }
    grouped.get(key).push(source);
  });

  return Array.from(grouped.entries())
    .map(([key, items]) => ({
      key,
      label: key === "unlinked" ? "Kézi vagy nem kapcsolt mentések" : getRunLabel(key),
      items,
    }))
    .sort((left, right) => right.items.length - left.items.length);
});

const filteredSources = computed(() => {
  let result = sources.value;

  if (archiveRunFilter.value === "unlinked") {
    result = result.filter((source) => !source.search_run_id);
  } else if (archiveRunFilter.value !== "all") {
    result = result.filter((source) => String(source.search_run_id) === archiveRunFilter.value);
  }

  if (archiveStatusFilter.value !== "all") {
    result = result.filter((source) => source.source_status === archiveStatusFilter.value);
  }

  if (archiveDecisionFilter.value !== "all") {
    result = result.filter((source) => source.usability_decision === archiveDecisionFilter.value);
  }

  if (archivePriorityFilter.value !== "all") {
    result = result.filter((source) => String(source.priority_level) === archivePriorityFilter.value);
  }

  if (archiveOriginFilter.value !== "all") {
    result = result.filter((source) => getSourceOrigin(source) === archiveOriginFilter.value);
  }

  return result;
});

const sourceStatusSummary = computed(() => ({
  nyers: sources.value.filter((source) => source.source_status === "nyers").length,
  ellenorzes_alatt: sources.value.filter((source) => source.source_status === "ellenorzes_alatt").length,
  validalt: sources.value.filter((source) => source.source_status === "validalt").length,
}));

const validatedSources = computed(() =>
  sources.value
    .filter((source) => source.source_status === "validalt")
    .sort((left, right) => {
      const leftReviewed = left.reviewed_at || "";
      const rightReviewed = right.reviewed_at || "";
      return rightReviewed.localeCompare(leftReviewed);
    }),
);

function getRunLabel(searchRunId) {
  if (!searchRunId) return "Kézi vagy nem kapcsolt mentés";
  const run = searchRuns.value.find((item) => item.id === searchRunId);
  return run ? `${run.created_at} | ${run.summary}` : `Keresési futam #${searchRunId}`;
}

function ensureSourceDraft(source) {
  if (!sourceReviewDrafts.value[source.id]) {
    sourceReviewDrafts.value[source.id] = {
      source_status: source.source_status || "nyers",
      priority_level: source.priority_level ?? 2,
      usability_decision: source.usability_decision || "hatteranyag",
      proof_summary: source.proof_summary || "",
      reviewer_name: source.reviewer_name || "",
      reviewed_at: source.reviewed_at || "",
      relevance_note: source.relevance_note || "",
      probative_value: source.probative_value || "",
      missing_points: source.missing_points || "",
    };
  }
  return sourceReviewDrafts.value[source.id];
}

function formatSourceStatus(status) {
  if (status === "ellenorzes_alatt") return "Ellenőrzés alatt";
  if (status === "validalt") return "Validált";
  return "Nyers";
}

function formatPriority(priority) {
  if (priority === 1) return "P1";
  if (priority === 2) return "P2";
  return "P3";
}

function formatUsabilityDecision(decision) {
  if (decision === "felhasznalhato") return "Felhasználható";
  if (decision === "kizarando") return "Kizárandó";
  return "Háttéranyag";
}

function formatArchiveKind(kind) {
  if (kind === "wayback_snapshot") return "Wayback snapshot";
  if (kind === "wayback_index") return "Wayback index";
  return "Élő forrás";
}

function getSourceOrigin(source) {
  const url = `${source.original_url || ""} ${source.url || ""}`.toLowerCase();

  if (url.includes("reddit.com")) return "reddit";
  if (url.includes("facebook.com")) return "facebook";

  if (
    source.archive_kind === "wayback_snapshot" ||
    source.archive_kind === "wayback_index" ||
    url.includes("archive.org")
  ) {
    return "webarchive";
  }

  if (
    url.includes("njt.hu") ||
    url.includes("magyarkozlony.hu") ||
    url.includes("ksh.hu") ||
    url.includes("kozbeszerzes.hu")
  ) {
    return "official";
  }

  return "other";
}

function formatSourceOrigin(origin) {
  if (origin === "reddit") return "Reddit";
  if (origin === "facebook") return "Facebook";
  if (origin === "webarchive") return "Webarchív";
  if (origin === "official") return "Hivatalos forrás";
  return "Egyéb";
}

function getOriginCaution(origin) {
  if (origin === "reddit" || origin === "facebook") {
    return "Közösségi kontrollforrás: önálló elsődleges bizonyítékként nem, csak ellenőrzési vagy háttéranyagként kezelendő.";
  }

  if (origin === "webarchive") {
    return "Archivált forrás: az időállapot és az eredeti közzétételi környezet külön ellenőrzendő.";
  }

  return "";
}

function isWaybackResult(result) {
  return result.archive_kind === "wayback_snapshot" || result.archive_kind === "wayback_index";
}

function formatReportStatus(status) {
  if (status === "ellenorzes_alatt") return "Ellenőrzés alatt";
  if (status === "veglegesitett") return "Véglegesített";
  return "Tervezet";
}

function formatFinalizable(value) {
  return value === "igen" ? "Igen" : "Nem";
}

function getSearchResultKey(result) {
  return `${result.url}-${result.query_used}`;
}

function isResultExpanded(result) {
  return Boolean(expandedResultKeys.value[getSearchResultKey(result)]);
}

function toggleResultExpanded(result) {
  const key = getSearchResultKey(result);
  expandedResultKeys.value = {
    ...expandedResultKeys.value,
    [key]: !expandedResultKeys.value[key],
  };
}

function isRunExpanded(runId) {
  return Boolean(expandedRunIds.value[runId]);
}

function toggleRunExpanded(runId) {
  expandedRunIds.value = {
    ...expandedRunIds.value,
    [runId]: !expandedRunIds.value[runId],
  };
}

function applyQuickReview(source, patch) {
  const draft = ensureSourceDraft(source);
  Object.assign(draft, patch);
  saveSourceReview(source);
}

function buildQualityChecks() {
  const issues = [];

  if (!currentCase.value) {
    issues.push("Nincs kiválasztott ügy.");
    return issues;
  }

  if (sources.value.length === 0) {
    issues.push("Nincs archivált forrás az ügyhöz.");
  }

  const missingProof = highlightedSources.value.filter((source) => !source.proof_summary?.trim());
  if (missingProof.length > 0) {
    issues.push(`Hiányzó bizonyítási kivonat ${missingProof.length} felhasználható forrásnál.`);
  }

  const unreviewedValidated = validatedSources.value.filter(
    (source) => !source.reviewer_name?.trim() || !source.reviewed_at?.trim(),
  );
  if (unreviewedValidated.length > 0) {
    issues.push(`Hiányos validálási metaadat ${unreviewedValidated.length} validált forrásnál.`);
  }

  if (caseOpinionDraft.value.finalizable === "igen" && !caseOpinionDraft.value.final_opinion?.trim()) {
    issues.push("Az ügy véglegesíthetőnek van jelölve, de nincs végleges szakértői konklúzió.");
  }

  if (caseOpinionDraft.value.report_status === "veglegesitett" && caseOpinionDraft.value.open_questions?.trim()) {
    issues.push("A riport véglegesített állapotú, de maradtak nyitott kérdések.");
  }

  return issues;
}

const qualityChecks = computed(() => buildQualityChecks());

const highlightedSources = computed(() =>
  sources.value
    .filter((source) => source.usability_decision === "felhasznalhato")
    .sort((left, right) => {
      if (left.priority_level !== right.priority_level) {
        return left.priority_level - right.priority_level;
      }
      return right.id - left.id;
    }),
);

const excludedSources = computed(() =>
  sources.value
    .filter((source) => source.usability_decision === "kizarando")
    .sort((left, right) => {
      if (left.priority_level !== right.priority_level) {
        return left.priority_level - right.priority_level;
      }
      return right.id - left.id;
    }),
);

const weakSources = computed(() =>
  sources.value
    .filter((source) => {
      const text = `${source.probative_value || ""} ${source.relevance_note || ""}`.toLowerCase();
      return (
        source.usability_decision !== "kizarando" &&
        (text.includes("gyenge") ||
          text.includes("korlátozott") ||
          text.includes("nem elegendő") ||
          text.includes("ellenőrizendő"))
      );
    })
    .sort((left, right) => {
      if (left.priority_level !== right.priority_level) {
        return left.priority_level - right.priority_level;
      }
      return right.id - left.id;
    }),
);

const caseSummary = computed(() => {
  const findings = highlightedSources.value
    .filter((source) => (source.proof_summary || source.relevance_note || "").trim())
    .slice(0, 5)
    .map((source) => ({
      title: source.title,
      text: source.proof_summary?.trim() || source.relevance_note?.trim() || "Nincs rövid összefoglaló.",
      priority: formatPriority(source.priority_level),
    }));

  const counterFindings = excludedSources.value
    .filter((source) => (source.proof_summary || source.probative_value || source.relevance_note || "").trim())
    .slice(0, 5)
    .map((source) => ({
      title: source.title,
      text:
        source.probative_value?.trim() ||
        source.proof_summary?.trim() ||
        source.relevance_note?.trim() ||
        "Nincs rövid indokolás.",
      priority: formatPriority(source.priority_level),
    }));

  const gaps = Array.from(
    new Set(
      sources.value
        .map((source) => source.missing_points?.trim())
        .filter(Boolean)
        .flatMap((item) => item.split(/[.;\n]/))
        .map((item) => item.trim())
        .filter((item) => item.length > 4),
    ),
  ).slice(0, 8);

  return {
    findings,
    counterFindings,
    gaps,
  };
});

const executiveSummary = computed(() => {
  const conclusion =
    caseOpinionDraft.value.final_opinion?.trim() ||
    caseOpinionDraft.value.preliminary_opinion?.trim() ||
    "Még nincs rögzített összefoglaló álláspont.";

  return [
    `Riport státusz: ${formatReportStatus(caseOpinionDraft.value.report_status)}.`,
    `Végleges állásfoglalásra alkalmas: ${formatFinalizable(caseOpinionDraft.value.finalizable)}.`,
    `Felhasználható források: ${highlightedSources.value.length}, validált források: ${validatedSources.value.length}.`,
    `Nyitott kérdések: ${caseSummary.value.gaps.length || 0}, kizárandó források: ${excludedSources.value.length}.`,
    `Aktuális konklúzió: ${conclusion}`,
  ];
});

const attachmentIndex = computed(() =>
  archiveGroups.value.map((group, index) => ({
    label: `M${index + 1}`,
    title: group.label,
    count: group.items.length,
    runNote:
      group.key === "unlinked"
        ? ""
        : searchRuns.value.find((run) => String(run.id) === String(group.key))?.note || "",
  })),
);

const evidenceMatrix = computed(() =>
  highlightedSources.value.slice(0, 8).map((source, index) => ({
    code: `E${index + 1}`,
    title: source.title,
    claim: source.proof_summary?.trim() || source.relevance_note?.trim() || "Nincs rövid állítás.",
    strength: source.probative_value?.trim() || "Nincs külön jelzett bizonyító erő.",
    status: formatSourceStatus(source.source_status),
    decision: formatUsabilityDecision(source.usability_decision),
  })),
);

const originSections = ["official", "webarchive", "reddit", "facebook", "other"];

function buildOriginGroups(items) {
  const grouped = new Map(originSections.map((origin) => [origin, []]));

  items.forEach((source) => {
    const origin = getSourceOrigin(source);
    if (!grouped.has(origin)) {
      grouped.set(origin, []);
    }
    grouped.get(origin).push(source);
  });

  return originSections
    .map((origin) => ({
      origin,
      label: formatSourceOrigin(origin),
      items: grouped.get(origin) || [],
    }))
    .filter((group) => group.items.length > 0);
}

const highlightedSourcesByOrigin = computed(() => buildOriginGroups(highlightedSources.value));
const allSourcesByOrigin = computed(() => buildOriginGroups(sources.value));
const originSummary = computed(() =>
  buildOriginGroups(sources.value).map((group) => `${group.label}: ${group.items.length}`),
);

const reportText = computed(() => {
  if (!currentCase.value) return "";

  const now = new Date().toLocaleDateString("hu-HU");
  const lines = [
    "# Igazságügyi Szakértői Keresési és Forráselemzési Összefoglaló",
    "",
    "## 1. Azonosító adatok",
    `- Ügy megnevezése: ${currentCase.value.title}`,
    `- Riport dátuma: ${now}`,
    `- Határidő: ${currentCase.value.deadline || "nincs megadva"}`,
    `- Ügyállapot: ${currentCase.value.status}`,
    `- Riport státusza: ${formatReportStatus(caseOpinionDraft.value.report_status)}`,
    `- Végleges állásfoglalásra alkalmas: ${formatFinalizable(caseOpinionDraft.value.finalizable)}`,
    "",
    "## 2. Vezetői kivonat",
  ];

  executiveSummary.value.forEach((item) => {
    lines.push(`- ${item}`);
  });

  lines.push("");
  lines.push("## 3. Kutatási fókusz és ügykeret");
  lines.push(
    currentCase.value.description || "A felhasználó nem adott meg részletes ügyösszefoglalót.",
  );
  lines.push("");
  lines.push("## 4. Utolsó keresési futam");
  lines.push(lastSearchSummary.value || "Még nem történt keresési futam ebben a munkamenetben.");
  lines.push("");
  lines.push("## 5. Javasolt keresési és ellenőrzési sorrend");

  if (searchPlan.value.length === 0) {
    lines.push("- Nincs összeállított keresési terv.");
  } else {
    searchPlan.value.forEach((plan) => {
      lines.push(`### ${plan.title}`);
      lines.push(`- Munkacél: ${plan.note}`);
      lines.push(`- Lekérdezés: \`${plan.query}\``);
      lines.push("");
    });
  }

  lines.push("## 6. Ügyszintű döntéstámogatás");
  if (caseSummary.value.findings.length === 0) {
    lines.push("- Még nincs elég feldolgozott forrás a megállapítások összegzéséhez.");
  } else {
    lines.push("### 6.1. Fő megállapítások");
    caseSummary.value.findings.forEach((item) => {
      lines.push(`- ${item.priority} | ${item.title}: ${item.text}`);
    });
  }
  if (caseSummary.value.counterFindings.length > 0) {
    lines.push("");
    lines.push("### 6.2. Ellen-megállapítások vagy kizárási okok");
    caseSummary.value.counterFindings.forEach((item) => {
      lines.push(`- ${item.priority} | ${item.title}: ${item.text}`);
    });
  }
  if (caseSummary.value.gaps.length > 0) {
    lines.push("");
    lines.push("### 6.3. Mi hiányzik még az állásfoglaláshoz?");
    caseSummary.value.gaps.forEach((item) => {
      lines.push(`- ${item}`);
    });
  }
  if (weakSources.value.length > 0) {
    lines.push("");
    lines.push("### 6.4. Gyenge vagy óvatosan kezelendő források");
    weakSources.value.slice(0, 6).forEach((source) => {
      lines.push(
        `- ${formatPriority(source.priority_level)} | ${source.title}: ${
          source.probative_value?.trim() || source.relevance_note?.trim() || "Gyenge vagy korlátozott bizonyító erő."
        }`,
      );
    });
  }

  lines.push("");
  lines.push("## 7. Szakértői konklúzió");
  lines.push(`- Riport státusz: ${formatReportStatus(caseOpinionDraft.value.report_status)}`);
  lines.push(`- Végleges állásfoglalásra alkalmas: ${formatFinalizable(caseOpinionDraft.value.finalizable)}`);
  lines.push("### 7.1. Előzetes álláspont");
  lines.push(
    caseOpinionDraft.value.preliminary_opinion?.trim() ||
      "Még nincs rögzített előzetes szakértői álláspont.",
  );
  lines.push("");
  lines.push("### 7.2. Végleges álláspont");
  lines.push(
    caseOpinionDraft.value.final_opinion?.trim() ||
      "Még nincs rögzített végleges szakértői konklúzió.",
  );
  lines.push("");
  lines.push("### 7.3. Lezárt pontok");
  lines.push(
    caseOpinionDraft.value.closed_points?.trim() ||
      "Még nincs külön rögzített lezárt pontlista.",
  );
  lines.push("");
  lines.push("### 7.4. Nyitott kérdések");
  lines.push(
    caseOpinionDraft.value.open_questions?.trim() ||
      "Még nincs külön rögzített nyitott kérdéslista.",
  );
  lines.push("");

  lines.push("## 8. Kiemelt források");
  if (highlightedSources.value.length === 0) {
    lines.push("- Még nincs felhasználhatónak minősített forrás.");
  } else {
    highlightedSourcesByOrigin.value.forEach((group, groupIndex) => {
      lines.push(`### 8.${groupIndex + 1}. ${group.label} források`);
      lines.push(`- Darabszám: ${group.items.length}`);
      if (getOriginCaution(group.origin)) {
        lines.push(`- Megjegyzés: ${getOriginCaution(group.origin)}`);
      }
      lines.push("");

      group.items.forEach((source, sourceIndex) => {
        lines.push(`#### 8.${groupIndex + 1}.${sourceIndex + 1}. ${source.title}`);
        lines.push(`- Eredet: ${group.label}`);
        lines.push(`- Döntés: ${formatUsabilityDecision(source.usability_decision)}`);
        lines.push(`- Prioritás: ${formatPriority(source.priority_level)}`);
        lines.push(`- Státusz: ${formatSourceStatus(source.source_status)}`);
        lines.push(`- Futam: ${getRunLabel(source.search_run_id)}`);
        lines.push(`- URL: ${source.url}`);
        if (source.original_url) {
          lines.push(`- Eredeti URL: ${source.original_url}`);
        }
        if (source.archived_url) {
          lines.push(`- Archivált URL: ${source.archived_url}`);
        }
        if (source.snapshot_timestamp) {
          lines.push(`- Snapshot: ${source.snapshot_timestamp}`);
        }
        lines.push(`- Forrástípus: ${formatArchiveKind(source.archive_kind)}`);
        if (getOriginCaution(group.origin)) {
          lines.push(`- Óvatossági megjegyzés: ${getOriginCaution(group.origin)}`);
        }
        if (source.reviewer_name?.trim()) {
          lines.push(`- Ellenőrző: ${source.reviewer_name.trim()}`);
        }
        if (source.reviewed_at?.trim()) {
          lines.push(`- Ellenőrzés ideje: ${source.reviewed_at.trim()}`);
        }
        lines.push(`- Bizonyítási kivonat: ${source.proof_summary?.trim() || "Nincs kitöltve."}`);
        lines.push("");
      });
    });
  }

  lines.push("## 9. Bizonyítási mátrix");
  if (evidenceMatrix.value.length === 0) {
    lines.push("- Még nincs felhasználható forrás a bizonyítási mátrixhoz.");
  } else {
    evidenceMatrix.value.forEach((item) => {
      lines.push(`### ${item.code}. ${item.title}`);
      lines.push(`- Alátámasztott állítás: ${item.claim}`);
      lines.push(`- Bizonyító erő: ${item.strength}`);
      lines.push(`- Státusz: ${item.status}`);
      lines.push(`- Döntés: ${item.decision}`);
      lines.push("");
    });
  }

  lines.push("## 10. Mellékletjegyzék");
  if (attachmentIndex.value.length === 0) {
    lines.push("- Még nincs mellékletbe rendezhető forráscsoport.");
  } else {
    attachmentIndex.value.forEach((item) => {
      lines.push(`- ${item.label} | ${item.title} | források száma: ${item.count}`);
      if (item.runNote?.trim()) {
        lines.push(`  Megjegyzés: ${item.runNote.trim()}`);
      }
    });
  }

  lines.push("");
  lines.push("## 11. Kutatási futamokhoz kapcsolt források");
  if (archiveGroups.value.length === 0) {
    lines.push("- Nincs archivált forrás.");
  } else {
    archiveGroups.value.forEach((group, groupIndex) => {
      lines.push(`### 11.${groupIndex + 1}. ${group.label}`);
      lines.push(`- Források száma: ${group.items.length}`);
      const groupRun =
        group.key === "unlinked"
          ? null
          : searchRuns.value.find((run) => String(run.id) === String(group.key));
      if (groupRun?.note?.trim()) {
        lines.push(`- Szakértői megjegyzés: ${groupRun.note.trim()}`);
      } else {
        lines.push("- Szakértői megjegyzés helye: ................................");
      }
      lines.push("");

      group.items.forEach((source, sourceIndex) => {
        lines.push(`#### ${groupIndex + 1}.${sourceIndex + 1}. ${source.title}`);
        lines.push(`- Tier: ${source.tier}`);
        lines.push(`- Státusz: ${formatSourceStatus(source.source_status)}`);
        lines.push(`- Prioritás: ${formatPriority(source.priority_level)}`);
        lines.push(`- Felhasználási döntés: ${formatUsabilityDecision(source.usability_decision)}`);
        lines.push(`- URL: ${source.url}`);
        if (source.original_url) {
          lines.push(`- Eredeti URL: ${source.original_url}`);
        }
        if (source.archived_url) {
          lines.push(`- Archivált URL: ${source.archived_url}`);
        }
        if (source.snapshot_timestamp) {
          lines.push(`- Snapshot: ${source.snapshot_timestamp}`);
        }
        lines.push(`- Forrástípus: ${formatArchiveKind(source.archive_kind)}`);
        lines.push(`- Időbélyeg: ${source.timestamp}`);
        if (source.reviewer_name?.trim()) {
          lines.push(`- Ellenőrző: ${source.reviewer_name.trim()}`);
        }
        if (source.reviewed_at?.trim()) {
          lines.push(`- Ellenőrzés ideje: ${source.reviewed_at.trim()}`);
        }
        lines.push(`- Hash: \`${source.hash}\``);
        lines.push(`- Lokális fájl: \`${source.file_path}\``);
        if (source.proof_summary?.trim()) {
          lines.push(`- Bizonyítási kivonat: ${source.proof_summary.trim()}`);
        }
        if (source.relevance_note?.trim()) {
          lines.push(`- Relevancia: ${source.relevance_note.trim()}`);
        }
        if (source.probative_value?.trim()) {
          lines.push(`- Bizonyító erő: ${source.probative_value.trim()}`);
        }
        if (source.missing_points?.trim()) {
          lines.push(`- Hiányosságok: ${source.missing_points.trim()}`);
        }
        lines.push("");
      });
    });
  }

  lines.push("## 12. Forráseredet szerinti bontás");
  if (allSourcesByOrigin.value.length === 0) {
    lines.push("- Nincs archivált forrás az eredet szerinti bontáshoz.");
  } else {
    allSourcesByOrigin.value.forEach((group, groupIndex) => {
      lines.push(`### 12.${groupIndex + 1}. ${group.label}`);
      lines.push(`- Források száma: ${group.items.length}`);
      if (getOriginCaution(group.origin)) {
        lines.push(`- Megjegyzés: ${getOriginCaution(group.origin)}`);
      }
      group.items.slice(0, 8).forEach((source) => {
        lines.push(
          `- ${formatPriority(source.priority_level)} | ${formatUsabilityDecision(source.usability_decision)} | ${source.title} | ${source.url}`,
        );
      });
      if (group.items.length > 8) {
        lines.push(`- ... további ${group.items.length - 8} forrás ugyanebben az eredeti kategóriában.`);
      }
      lines.push("");
    });
  }

  lines.push("## 13. Archivum gyorsértékelés");
  if (sources.value.length === 0) {
    lines.push("- Nincs elegendő adat az archivum értékeléséhez.");
  } else {
    lines.push(
      `- Tier eloszlás: T1=${archiveInsights.value.tierCounts[1]}, T2=${archiveInsights.value.tierCounts[2]}, T3=${archiveInsights.value.tierCounts[3]}, T4=${archiveInsights.value.tierCounts[4]}.`,
    );
    lines.push(
      `- Forrás státuszok: nyers=${sourceStatusSummary.value.nyers}, ellenőrzés alatt=${sourceStatusSummary.value.ellenorzes_alatt}, validált=${sourceStatusSummary.value.validalt}.`,
    );
    lines.push(`- Validált források száma: ${validatedSources.value.length}.`);
    lines.push(`- Kizárandó források száma: ${excludedSources.value.length}.`);
    lines.push(`- Gyenge bizonyító erejűként jelölt források száma: ${weakSources.value.length}.`);
    if (originSummary.value.length > 0) {
      lines.push(`- Eredet szerinti megoszlás: ${originSummary.value.join(", ")}.`);
    }
    if (archiveInsights.value.missingTiers.length > 0) {
      lines.push(`- Hiányzó fő tier-ek: ${archiveInsights.value.missingTiers.join(", ")}.`);
    } else {
      lines.push("- A fő tier-ek mind reprezentálva vannak.");
    }
    if (archiveInsights.value.duplicatedDomains.length > 0) {
      lines.push(
        `- Ismétlődő domainek: ${archiveInsights.value.duplicatedDomains
          .map(([domain, count]) => `${domain} (${count})`)
          .join(", ")}.`,
      );
    }
  }

  lines.push("## 14. Audit kivonat");
  if (auditLogs.value.length === 0) {
    lines.push("- Nincs audit esemény.");
  } else {
    auditLogs.value.slice(0, 12).forEach((log) => {
      lines.push(`- ${log.timestamp} | ${log.action_type} | ${log.details}`);
    });
  }

  lines.push("");
  lines.push("## 15. Keresési futam napló");
  if (searchRuns.value.length === 0) {
    lines.push("- Még nincs rögzített keresési futam.");
  } else {
    searchRuns.value.slice(0, 8).forEach((run) => {
      lines.push(
        `- ${run.created_at} | ${run.engine} | ${run.summary} | találatok: ${run.result_count}`,
      );
      if (run.note?.trim()) {
        lines.push(`  Megjegyzés: ${run.note.trim()}`);
      }
    });
  }

  lines.push("");
  lines.push("## 16. Szakértői záradék");
  lines.push(
    "Ez az összefoglaló keresési futamokra, archivált forrásokra és szakértői jelölésekre épül. A végleges állásfoglalás kizárólag külön szakértői ellenőrzés és felelősségvállalás mellett rögzíthető.",
  );

  return lines.join("\n");
});

async function loadCases() {
  const loadedCases = await api("get_cases");
  cases.value = loadedCases;
  if (currentCase.value) {
    currentCase.value = loadedCases.find((item) => item.id === currentCase.value.id) ?? currentCase.value;
  }
  if (!currentCase.value && loadedCases.length > 0) {
    currentCase.value = loadedCases[0];
    currentTab.value = "browser";
  }
}

async function loadSources() {
  if (!currentCase.value) return;
  sources.value = await api("get_sources", { caseId: currentCase.value.id });
  sourceReviewDrafts.value = Object.fromEntries(
    sources.value.map((source) => [
      source.id,
      {
        source_status: source.source_status || "nyers",
        priority_level: source.priority_level ?? 2,
        usability_decision: source.usability_decision || "hatteranyag",
        proof_summary: source.proof_summary || "",
        reviewer_name: source.reviewer_name || "",
        reviewed_at: source.reviewed_at || "",
        relevance_note: source.relevance_note || "",
        probative_value: source.probative_value || "",
        missing_points: source.missing_points || "",
      },
    ]),
  );
}

async function loadAuditLogs() {
  if (!currentCase.value) return;
  auditLogs.value = await api("get_audit_logs", { caseId: currentCase.value.id });
}

async function loadSearchRuns() {
  if (!currentCase.value) return;
  searchRuns.value = await api("get_search_runs", { caseId: currentCase.value.id });
  runNoteDrafts.value = Object.fromEntries(searchRuns.value.map((run) => [run.id, run.note || ""]));
}

async function handleCreateCase() {
  if (!newCaseTitle.value.trim()) {
    appError.value = "Az ügy megnevezése kötelező.";
    return;
  }

  try {
    const newId = await api("create_case", {
      title: newCaseTitle.value.trim(),
      description: newCaseDesc.value.trim(),
      deadline: newCaseDeadline.value,
    });
    newCaseTitle.value = "";
    newCaseDesc.value = "";
    newCaseDeadline.value = "";
    await loadCases();
    currentCase.value = cases.value.find((item) => item.id === newId) ?? cases.value[0] ?? null;
    currentTab.value = "browser";
  } catch (error) {
    appError.value = `Ügy létrehozási hiba: ${String(error)}`;
  }
}

async function handleDeleteCase(id) {
  if (!confirm("Biztosan törlöd az ügyet és a kapcsolódó archivumot?")) return;

  try {
    await api("delete_case", { id });
    if (currentCase.value?.id === id) {
      currentCase.value = null;
    }
    await loadCases();
  } catch (error) {
    appError.value = `Törlési hiba: ${String(error)}`;
  }
}

function applyPreset(preset) {
  searchTerms.value.mustInclude = preset.must;
  searchTerms.value.exactPhrase = preset.phrase;
}

function clearSearchForm() {
  rawQuery.value = "";
  searchTerms.value = {
    mustInclude: "",
    exactPhrase: "",
    anyOf: "",
    exclude: "",
    site: "",
    region: "",
    yearFrom: "",
    yearTo: "",
  };
  sourcePacks.value = {
    official: true,
    science: true,
    market: true,
    archive: true,
    reddit: false,
    facebook_groups: true,
    shared: false,
    community_general: false,
    control: false,
  };
  researchLens.value = "balanced";
  searchResults.value = [];
  lastSearchSummary.value = "";
  searchWarning.value = "";
  searchDiagnostics.value = null;
  searchRuns.value = currentCase.value ? searchRuns.value : [];
  selectedRunIds.value = [];
  activeSearchRunId.value = null;
  archiveRunFilter.value = "all";
  archiveStatusFilter.value = "all";
  archiveDecisionFilter.value = "all";
  archivePriorityFilter.value = "all";
  archiveOriginFilter.value = "all";
}

function parseRunSummary(summary) {
  const enginesMatch = summary.match(/Motorok:\s(.+?)\.\sTalálatok:/);
  const resultMatch = summary.match(/Találatok:\s(\d+)/);
  const planMatch = summary.match(/Keresési terv-lépések:\s(\d+)/);

  return {
    engines: enginesMatch ? enginesMatch[1].split(",").map((item) => item.trim()).filter(Boolean) : ["google"],
    resultCount: resultMatch ? Number.parseInt(resultMatch[1], 10) : 0,
    planCount: planMatch ? Number.parseInt(planMatch[1], 10) : 0,
  };
}

function applySearchRun(run) {
  rawQuery.value = run.base_query;
  const parsed = parseRunSummary(run.summary);
  selectedEngines.value = parsed.engines.length ? parsed.engines : ["google"];
  selectedTierFilter.value = run.tier_filter;
  lastSearchSummary.value = `Korábbi futam betöltve: ${run.summary}`;
  activeSearchRunId.value = run.id;
}

async function rerunSearchRun(run) {
  applySearchRun(run);
  await runSearch();
}

async function saveRunNote(run) {
  try {
    await api("update_search_run_note", {
      id: run.id,
      note: runNoteDrafts.value[run.id] || "",
    });
    await Promise.all([loadSearchRuns(), loadAuditLogs()]);
  } catch (error) {
    appError.value = `Futam-megjegyzés mentési hiba: ${String(error)}`;
  }
}

async function saveCaseOpinions() {
  if (!currentCase.value) return;

  if (
    caseOpinionDraft.value.report_status === "veglegesitett" &&
    (!caseOpinionDraft.value.final_opinion?.trim() || caseOpinionDraft.value.finalizable !== "igen")
  ) {
    appError.value = "Véglegesített riporthoz végleges konklúzió és 'igen' véglegesíthetőség szükséges.";
    return;
  }

  try {
    await api("update_case_opinions", {
      id: currentCase.value.id,
      preliminaryOpinion: caseOpinionDraft.value.preliminary_opinion || "",
      finalOpinion: caseOpinionDraft.value.final_opinion || "",
      reportStatus: caseOpinionDraft.value.report_status || "tervezet",
      finalizable: caseOpinionDraft.value.finalizable || "nem",
      openQuestions: caseOpinionDraft.value.open_questions || "",
      closedPoints: caseOpinionDraft.value.closed_points || "",
    });
    await Promise.all([loadCases(), loadAuditLogs()]);
  } catch (error) {
    appError.value = `Ügy-álláspont mentési hiba: ${String(error)}`;
  }
}

async function saveSourceReview(source) {
  try {
    const draft = ensureSourceDraft(source);

    if (draft.usability_decision === "felhasznalhato" && !draft.proof_summary?.trim()) {
      appError.value = "Felhasználható forráshoz kötelező rövid bizonyítási kivonatot megadni.";
      return;
    }

    if (draft.source_status === "validalt" && (!draft.reviewer_name?.trim() || !draft.reviewed_at?.trim())) {
      appError.value = "Validált forráshoz ellenőrző személy és ellenőrzési időpont szükséges.";
      return;
    }

    await api("update_source_review", {
      id: source.id,
      sourceStatus: draft.source_status || "nyers",
      priorityLevel: Number(draft.priority_level) || 2,
      usabilityDecision: draft.usability_decision || "hatteranyag",
      proofSummary: draft.proof_summary || "",
      reviewerName: draft.reviewer_name || "",
      reviewedAt: draft.reviewed_at || "",
      relevanceNote: draft.relevance_note || "",
      probativeValue: draft.probative_value || "",
      missingPoints: draft.missing_points || "",
    });
    await Promise.all([loadSources(), loadAuditLogs()]);
  } catch (error) {
    appError.value = `Forrásértékelés mentési hiba: ${String(error)}`;
  }
}

function toggleRunComparison(runId) {
  if (selectedRunIds.value.includes(runId)) {
    selectedRunIds.value = selectedRunIds.value.filter((id) => id !== runId);
    return;
  }
  if (selectedRunIds.value.length >= 2) {
    selectedRunIds.value = [selectedRunIds.value[1], runId];
    return;
  }
  selectedRunIds.value = [...selectedRunIds.value, runId];
}

function toggleEngine(engineId) {
  if (selectedEngines.value.includes(engineId)) {
    if (selectedEngines.value.length > 1) {
      selectedEngines.value = selectedEngines.value.filter((item) => item !== engineId);
    }
    return;
  }
  selectedEngines.value = [...selectedEngines.value, engineId];
}

function buildQuerySet() {
  // Minden kérés minden bekapcsolt platformon lefut; a platformonkénti igazítást a backend végzi.
  const base = broadQuery.value.trim();
  return base ? [base] : [];
}

function buildQuerySetLegacy() {
  const broad = broadQuery.value.trim();
  const filtered = filteredQueries.value.map((item) => item.query);
  const hasPackScope = !searchTerms.value.site.trim() && selectedSourcePackKeys.value.length > 0;

  if (!broad) return [];

  if (searchMode.value === "ai_only") {
    return [broad];
  }

  if (nativeRunMode.value === "broad") {
    return hasPackScope ? (filtered.length > 0 ? filtered : [broad]) : [broad];
  }

  if (nativeRunMode.value === "filtered") {
    return filtered.length > 0 ? filtered : [broad];
  }

  return hasPackScope ? (filtered.length > 0 ? filtered : [broad]) : [broad, ...filtered];
}

async function runSearch() {
  if (!currentCase.value) {
    appError.value = "Keresés előtt hozz létre vagy válassz ki egy ügyet.";
    currentTab.value = "cases";
    return;
  }

  const querySet = buildQuerySet();
  if (querySet.length === 0) return;
  const primaryQuery = querySet[0];

  if (searchMode.value === "ai_only" && !aiConfigured.value) {
    appError.value = "AI módhoz előbb ments el egy érvényes API kulcsot a keresési beállításoknál.";
    return;
  }

  isSearching.value = true;
  appError.value = "";
  searchWarning.value = "";
  searchDiagnostics.value = null;
  runDelta.value = null;
  resultTab.value = "all";
  expandedStories.value = {};
  expandedResultKeys.value = {};

  try {
    const calls = [];

    if (searchMode.value !== "ai_only") {
      calls.push(
        api("execute_search", {
          caseId: currentCase.value.id,
          searchQuery: primaryQuery,
          tierFilter: selectedTierFilter.value,
          engine: "google",
          searchMode: "native",
          includeCse: true,
          selectedSourcePacks: [],
          enabledPlatforms: enabledPlatforms.value,
        }),
      );
    }

    if (searchMode.value !== "native") {
      calls.push(
        api("execute_search", {
          caseId: currentCase.value.id,
          searchQuery: primaryQuery,
          tierFilter: selectedTierFilter.value,
          engine: "google",
          searchMode: "ai_only",
          includeCse: false,
          selectedSourcePacks: [],
          enabledPlatforms: enabledPlatforms.value,
        }),
      );
    }

    const settledOutputs = await Promise.allSettled(calls);
    const outputs = settledOutputs
      .filter((item) => item.status === "fulfilled")
      .map((item) => item.value);
    const failedOutputs = settledOutputs
      .filter((item) => item.status === "rejected")
      .map((item) => String(item.reason));

    if (outputs.length === 0) {
      throw new Error(failedOutputs[0] || "A keresési futamok nem adtak vissza feldolgozható eredményt.");
    }

    const merged = outputs.flatMap((output) => output.results);
    const aiErrors = outputs.map((output) => output.ai_error).filter(Boolean);
    const cseErrors = outputs.map((output) => output.cse_error).filter(Boolean);
    const braveErrors = outputs.map((output) => output.brave_error).filter(Boolean);
    const tavilyErrors = outputs.map((output) => output.tavily_error).filter(Boolean);
    const generatedCount = outputs.reduce((sum, output) => sum + (output.generated_count || 0), 0);
    const filteredOutCount = outputs.reduce((sum, output) => sum + (output.filtered_out_count || 0), 0);

    const unique = new Map();
    merged.forEach((result) => {
      const previous = unique.get(result.url);
      if (!previous || previous.relevance < result.relevance) {
        unique.set(result.url, result);
      }
    });

    // A valódi találatok előre kerülnek, utánuk a keresőkártyák; mindkét csoporton belül relevancia szerint.
    const isReal = (item) => (item.evidence_level === "RETRIEVED" ? 1 : 0);
    searchResults.value = Array.from(unique.values()).sort(
      (left, right) => isReal(right) - isReal(left) || right.relevance - left.relevance,
    );
    searchHealth.value = mergeHealth(outputs);

    // Azonos történetek összevonása a teljes (több lekérdezésből összeállt) listán.
    const retrievedIdx = searchResults.value
      .map((item, index) => (item.evidence_level === "RETRIEVED" ? index : -1))
      .filter((index) => index >= 0);
    if (retrievedIdx.length > 1) {
      try {
        const ids = await api("cluster_stories", {
          texts: retrievedIdx.map((i) => `${searchResults.value[i].title} ${searchResults.value[i].snippet}`),
        });
        const sizes = new Map();
        ids.forEach((id) => sizes.set(id, (sizes.get(id) || 0) + 1));
        retrievedIdx.forEach((resultIndex, position) => {
          searchResults.value[resultIndex].story_id = ids[position] + 1;
          searchResults.value[resultIndex].story_size = sizes.get(ids[position]);
        });
      } catch {
        // az összevonás csak kényelmi funkció; hiba esetén a találatok önállóak maradnak
      }
    }
    searchDiagnostics.value = {
      generated: generatedCount,
      filteredOut: filteredOutCount,
      kept: searchResults.value.length,
    };

    const warningParts = [];
    if (aiErrors.length > 0) {
      warningParts.push(`Az AI kiegészítés nem futott le, a natív találatok viszont megmaradtak. Hiba: ${aiErrors[0]}`);
    }
    if (cseErrors.length > 0) {
      warningParts.push(`A Google CSE valódi találatlekérés nem futott le, a keresési terv kártyái megmaradtak. Hiba: ${cseErrors[0]}`);
    }
    if (braveErrors.length > 0) {
      warningParts.push(`A Brave Search valódi találatlekérés nem futott le, a keresési terv kártyái megmaradtak. Hiba: ${braveErrors[0]}`);
    }
    if (tavilyErrors.length > 0) {
      warningParts.push(`A Tavily valódi találatlekérés nem futott le, a keresési terv kártyái megmaradtak. Hiba: ${tavilyErrors[0]}`);
    }
    if (failedOutputs.length > 0) {
      warningParts.push(`A keresési részfutamok közül ${failedOutputs.length} hibával leállt, de a sikeres találatok megmaradtak. Első hiba: ${failedOutputs[0]}`);
    }
    searchWarning.value = warningParts.join(" | ");

    if (searchResults.value.length === 0) {
      searchResults.value = buildFallbackSearchResults(primaryQuery);
      appError.value =
        filteredOutCount > 0
          ? `A futam ${generatedCount} kártyát generált, de a Tier-szűrő mindet elrejtette. Közvetlen keresőnyitási védőháló jelent meg – érdemes a Tier-szűrőt lazítani.`
          : "A generált keresési terv nem adott vissza találatkártyát, ezért közvetlen keresőnyitási védőháló jelent meg.";
    }
    lastSearchSummary.value =
      `Mód: ${searchModes.find((item) => item.id === searchMode.value)?.label || searchMode.value}. ` +
      `${searchMode.value === "native" ? `Natív futam: ${nativeRunModes.find((item) => item.id === nativeRunMode.value)?.label || nativeRunMode.value}. ` : ""}` +
      `Elsődleges lekérdezés: ${primaryQuery}. Platformok: ${activePlatformCount.value}. ` +
      `Találatok: ${searchResults.value.length}. Keresési terv-lépések: ${platformLinks.value.length}.`;
    activeSearchRunId.value = await api("record_search_run", {
      caseId: currentCase.value.id,
      summary: lastSearchSummary.value,
      baseQuery: querySet.join(" || "),
      engine: `${searchMode.value} | ${enabledPlatforms.value.length ? enabledPlatforms.value.join(", ") : "minden platform"} | ${aiSettings.value.ai_provider}`,
      tierFilter: selectedTierFilter.value,
      planCount: platformLinks.value.length,
      resultCount: searchResults.value.length,
      results: searchResults.value.map((item) => ({
        url: item.url,
        title: item.title,
        tier: item.tier,
        source_type: item.source_type || "",
        query_used: item.query_used || "",
        published_at: item.published_at || "",
        evidence_level: item.evidence_level || "",
        story_id: item.story_id || 0,
      })),
      health: searchHealth.value,
    });
    try {
      runDelta.value = await api("previous_run_delta", {
        caseId: currentCase.value.id,
        runId: activeSearchRunId.value,
      });
    } catch {
      runDelta.value = null;
    }
    await Promise.all([loadAuditLogs(), loadSearchRuns()]);
  } catch (error) {
    appError.value = `Keresési hiba: ${String(error)}`;
  } finally {
    isSearching.value = false;
  }
}

async function resetTierFilterAndRerun() {
  selectedTierFilter.value = 0;
  await runSearch();
}

async function openSearchResult(result) {
  try {
    await openUrl(result.url);
  } catch (error) {
    appError.value = `A hivatkozás megnyitása sikertelen: ${String(error)}`;
  }
}

async function openInBrowserPanel({ url, title }) {
  if (!currentCase.value) {
    appError.value = "A böngésző használatához válassz aktív ügyet.";
    return;
  }

  try {
    await openBrowserTab({ caseId: currentCase.value.id, url, title });
    currentTab.value = "browser";
  } catch (error) {
    appError.value = `A böngészőfül megnyitása sikertelen: ${String(error)}`;
  }
}

async function openResultInViewer(result) {
  await openInBrowserPanel({ url: result.url, title: result.title });
}

async function openSearchPlanInBrowser() {
  for (const item of browserPlanSuggestions.value) {
    await openInBrowserPanel(item);
  }
}

const notice = ref("");

async function runBrowserSearch(query) {
  if (!currentCase.value) {
    appError.value = "A kereséshez válassz vagy hozz létre egy ügyet.";
    currentTab.value = "cases";
    return;
  }
  appError.value = "";
  notice.value = "";
  rawQuery.value = query;
  try {
    const links = await api("browser_tab_links", { query, enabledPlatforms: enabledPlatforms.value });
    if (links.length === 0) {
      appError.value = "Nincs bekapcsolt platform. Kapcsolj be legalább egyet a Platformok menüben.";
      return;
    }
    await closeSearchTabs();
    activeSearchRunId.value = await api("record_search_run", {
      caseId: currentCase.value.id,
      summary: `Böngészős keresés: ${query}. Platformok: ${links.length}. Találatok: ${links.length}. Keresési terv-lépések: ${links.length}.`,
      baseQuery: query,
      engine: `böngésző | ${enabledPlatforms.value.length ? enabledPlatforms.value.join(", ") : "minden platform"}`,
      tierFilter: 0,
      planCount: links.length,
      resultCount: links.length,
      results: links.map((link) => ({
        url: link.url,
        title: link.label,
        tier: 3,
        source_type: "Böngészőfül",
        query_used: query,
        published_at: "",
        evidence_level: "CONSTRUCTED",
        story_id: 0,
      })),
      health: [],
    });
    for (let index = 0; index < links.length; index += 1) {
      await openBrowserTab({
        caseId: currentCase.value.id,
        url: links[index].url,
        title: links[index].label,
        kind: "search",
        lazy: index > 0,
        activate: index === 0,
      });
    }
    await Promise.all([loadAuditLogs(), loadSearchRuns()]);
  } catch (error) {
    appError.value = `A böngészős keresés nem indítható: ${String(error)}`;
  }
}

async function archiveBrowserTab(tab) {
  if (!tab || !currentCase.value) return;
  notice.value = "";
  try {
    const page = await captureTab(tab.id);
    const url = page.url || tab.url;
    const title = page.title || tab.title || url;
    const content = [
      `Cím: ${title}`,
      `URL: ${url}`,
      `Fül: ${tab.kind === "search" ? tab.title : "kézzel megnyitott oldal"}`,
      `Lekérdezés: ${rawQuery.value || ""}`,
      "Forrás: beépített böngésző (az oldal megjelenített szövege, ahogy a felhasználó látta)",
      page.published ? `Közzétéve (oldal jelzése): ${page.published}` : "",
      "",
      "--- Megjelenített oldaltartalom ---",
      page.text || "(az oldalról nem olvasható ki szöveg)",
    ].join("\n");
    await api("archive_source", {
      caseId: currentCase.value.id,
      searchRunId: activeSearchRunId.value,
      title,
      url,
      tier: 3,
      publishedAt: page.published || null,
      modifiedAt: page.modified || null,
      timeSource: page.published || page.modified ? "meta" : null,
      content,
    });
    await Promise.all([loadSources(), loadAuditLogs()]);
    notice.value = `Archiválva: ${title}`;
  } catch (error) {
    appError.value = `Az oldal archiválása sikertelen: ${String(error)}`;
  }
}

function isSearchEngineUrl(url) {
  try {
    const host = new URL(url).hostname.replace(/^www\./, "");
    return (
      host === "google.com" ||
      host.endsWith(".google.com") ||
      host === "bing.com" ||
      host === "duckduckgo.com"
    );
  } catch {
    return false;
  }
}

async function openQueryInGoogle(query) {
  if (!query?.trim()) return;

  try {
    await openUrl(buildExternalSearchUrl("google", query.trim()));
  } catch (error) {
    appError.value = `A Google keresés megnyitása sikertelen: ${String(error)}`;
  }
}

async function copyQueryToClipboard(query) {
  if (!query?.trim()) return;

  try {
    await navigator.clipboard.writeText(query.trim());
    appError.value = "";
  } catch (error) {
    appError.value = `A lekérdezés másolása sikertelen: ${String(error)}`;
  }
}

async function archiveResult(result) {
  return archiveResultVariant(result, "current");
}

async function archiveResultVariant(result, variant = "current") {
  if (!currentCase.value) return;

  if (!activeSearchRunId.value) {
    appError.value = "Archiválás előtt futtass le vagy tölts be egy keresési futamot.";
    return;
  }

  const targetUrl =
    variant === "original" && result.original_url?.trim() ? result.original_url.trim() : result.url;
  const archiveLabel =
    variant === "original" && result.original_url?.trim()
      ? `${result.title} [élő forrás]`
      : result.title;
  const archiveKind =
    variant === "original" && result.original_url?.trim() ? "live" : result.archive_kind;

  busyArchiveId.value = `${result.url}|${variant}`;
  try {
    let fetchedSection = "";
    let pageTimes = { published_at: result.published_at || "", modified_at: "", time_source: result.published_at ? "provider" : "" };
    if (!isSearchEngineUrl(targetUrl)) {
      try {
        const page = await api("fetch_page_text", { url: targetUrl });
        if (page.published_at || page.modified_at) {
          pageTimes = {
            published_at: page.published_at || pageTimes.published_at,
            modified_at: page.modified_at || "",
            time_source: page.published_at ? page.time_source : pageTimes.time_source || page.time_source,
          };
        }
        if (page.text?.trim()) {
          fetchedSection = [
            "",
            "--- Letöltött oldaltartalom (automatikus kivonat) ---",
            `Oldal címe: ${page.title || "nincs cím"}`,
            `HTTP státusz: ${page.status}`,
            `Végleges URL: ${page.final_url}`,
            page.truncated ? "FIGYELEM: a letöltés a méretkorlát miatt csonka." : "",
            page.published_at ? `Közzétéve (oldal jelzése): ${page.published_at}` : "",
            page.modified_at ? `Módosítva (oldal jelzése): ${page.modified_at}` : "",
            "",
            page.text,
          ].join("\n");
        }
      } catch {
        fetchedSection = [
          "",
          "--- Letöltött oldaltartalom ---",
          "Az oldal automatikus letöltése nem sikerült, az archívum a találatkártya kivonatát tartalmazza.",
        ].join("\n");
      }
    }

    await api("archive_source", {
      caseId: currentCase.value.id,
      searchRunId: activeSearchRunId.value,
      title: archiveLabel,
      url: targetUrl,
      tier: result.tier,
      publishedAt: pageTimes.published_at || null,
      modifiedAt: pageTimes.modified_at || null,
      timeSource: pageTimes.time_source || null,
      content: [
        `Cím: ${archiveLabel}`,
        `URL: ${targetUrl}`,
        `Eredeti URL: ${result.original_url || targetUrl}`,
        `Archivált URL: ${result.archived_url || ""}`,
        `Snapshot: ${result.snapshot_timestamp || ""}`,
        `Archiválási nézet: ${archiveKind}`,
        `Tier: ${result.tier}`,
        `Forrástípus: ${result.source_type}`,
        `Lekérdezés: ${result.query_used}`,
        "",
        result.snippet,
        fetchedSection,
      ].join("\n"),
    });
    await Promise.all([loadSources(), loadAuditLogs()]);
  } catch (error) {
    appError.value = `Archiválási hiba: ${String(error)}`;
  } finally {
    busyArchiveId.value = null;
  }
}

async function removeSource(sourceId) {
  if (!confirm("Biztosan törlöd ezt az archivált forrást?")) return;

  try {
    await api("delete_source", { id: sourceId });
    await Promise.all([loadSources(), loadAuditLogs()]);
  } catch (error) {
    appError.value = `Forrástörlési hiba: ${String(error)}`;
  }
}

async function openArchivedFile(source) {
  try {
    await revealItemInDir(source.file_path);
  } catch (error) {
    appError.value = `A mentett fájl helyének megnyitása sikertelen: ${String(error)}`;
  }
}

async function saveReport() {
  if (!currentCase.value) return;

  const qualityIssues = buildQualityChecks();
  if (qualityIssues.length > 0) {
    appError.value = `Riport export blokkolva: ${qualityIssues.join(" | ")}`;
    return;
  }

  isSavingReport.value = true;
  reportSaved.value = false;
  try {
    reportPath.value = await api("save_report", {
      caseId: currentCase.value.id,
      reportContent: reportText.value,
    });
    reportSaved.value = true;
    await loadAuditLogs();
  } catch (error) {
    appError.value = `Riportmentési hiba: ${String(error)}`;
  } finally {
    isSavingReport.value = false;
  }
}

function tierBadgeClass(tier) {
  if (tier === 1) return "bg-emerald-100 text-emerald-800 border-emerald-200";
  if (tier === 2) return "bg-sky-100 text-sky-800 border-sky-200";
  if (tier === 3) return "bg-amber-100 text-amber-900 border-amber-200";
  return "bg-rose-100 text-rose-900 border-rose-200";
}
</script>

<template>
  <div class="min-h-screen bg-[radial-gradient(circle_at_top_left,_#eff6ff,_#e2e8f0_55%,_#cbd5e1)] text-slate-900">
    <div class="mx-auto flex min-h-screen max-w-7xl flex-col px-4 py-6 sm:px-6 lg:px-8">
      <header class="glass mb-6 rounded-[28px] px-6 py-5">
        <div class="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
          <div class="max-w-3xl">
            <div class="mb-3 inline-flex items-center gap-3 rounded-full border border-sky-200 bg-white/80 px-4 py-2 text-xs font-semibold uppercase tracking-[0.2em] text-sky-700">
              IRBR
              <span class="h-1.5 w-1.5 rounded-full bg-sky-500"></span>
              Tauri-only kereséstámogató
            </div>
            <h1 :class="advanced ? 'text-3xl sm:text-4xl' : 'text-2xl'" class="max-w-3xl font-semibold tracking-tight text-slate-950">
              Igazságügyi szakértői keresésekhez épített kutatási munkaasztal.
            </h1>
            <p v-if="advanced" class="mt-3 max-w-2xl text-sm leading-6 text-slate-600">
              A rendszer nem színlel élő találatlistát. Keresési futamokat, forráspiramist, archiválható nyomvonalat és exportálható összefoglalót ad.
            </p>
            <div v-if="advanced" class="mt-3">
              <HelpTooltip text="A kérdőjelek fölé húzva az egeret rövid magyarázat jelenik meg az adott funkcióról vagy blokkról." />
            </div>
          </div>

          <div v-if="advanced" class="grid gap-3 sm:grid-cols-3 lg:min-w-[420px]">
            <div class="rounded-2xl border border-white/70 bg-white/75 px-4 py-3 shadow-sm">
              <div class="flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-slate-500">Aktív ügyek <HelpTooltip text="Ennyi ügy van jelenleg a rendszerben. Az ügyek külön keresési és riport munkafolyamot kapnak." /></div>
              <div class="mt-2 text-2xl font-semibold text-slate-900">{{ cases.length }}</div>
            </div>
            <div class="rounded-2xl border border-white/70 bg-white/75 px-4 py-3 shadow-sm">
              <div class="flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-slate-500">Archivált források <HelpTooltip text="Minden kézzel mentett vagy archivált forrás ide számít. Ezek később értékelhetők és riportba tehetők." /></div>
              <div class="mt-2 text-2xl font-semibold text-slate-900">{{ sources.length }}</div>
            </div>
            <div class="rounded-2xl border border-white/70 bg-white/75 px-4 py-3 shadow-sm">
              <div class="flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-slate-500">Audit események <HelpTooltip text="Az audit napló követi a fontosabb keresési, archiválási és riport műveleteket." /></div>
              <div class="mt-2 text-2xl font-semibold text-slate-900">{{ auditLogs.length }}</div>
            </div>
          </div>
        </div>
      </header>

      <div v-if="appError" class="mb-4 rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800">
        {{ appError }}
      </div>

      <div v-if="notice" class="mb-4 rounded-2xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
        {{ notice }}
      </div>

      <div v-if="searchWarning" class="mb-4 rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
        {{ searchWarning }}
      </div>

      <div class="mb-6 flex flex-wrap items-center gap-3">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          @click="currentTab = tab.id"
          :class="[
            'rounded-full border px-4 py-2 text-sm font-semibold transition',
            currentTab === tab.id
              ? 'border-slate-900 bg-slate-900 text-white'
              : 'border-white/70 bg-white/75 text-slate-700 hover:border-slate-300 hover:bg-white',
          ]"
        >
          {{ tab.label }}
        </button>

        <div class="ml-auto flex items-center rounded-full border border-white/70 bg-white/80 p-1" title="Egyszerű: csak a napi munka. Haladó: minden beállítás és az audit nézet.">
          <button
            v-for="mode in [{ id: 'simple', label: 'Egyszerű' }, { id: 'advanced', label: 'Haladó' }]"
            :key="mode.id"
            @click="setUiMode(mode.id)"
            :class="[
              'rounded-full px-3 py-1.5 text-xs font-semibold transition',
              uiMode === mode.id ? 'bg-slate-900 text-white' : 'text-slate-600 hover:bg-slate-100',
            ]"
          >{{ mode.label }}</button>
        </div>

        <div class="flex min-w-[260px] items-center gap-2 rounded-full border border-white/70 bg-white/80 px-4 py-2">
          <span class="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Aktív ügy</span>
          <select
            v-model="currentCase"
            class="min-w-0 flex-1 bg-transparent text-sm font-semibold text-slate-900 outline-none"
          >
            <option :value="null" disabled>Válassz ügyet</option>
            <option v-for="item in cases" :key="item.id" :value="item">
              {{ item.title }}
            </option>
          </select>
        </div>
      </div>

      <main class="flex-1">
        <section v-if="currentTab === 'demo'" class="grid gap-6 lg:grid-cols-[1fr_1fr]">
          <div class="glass rounded-[28px] p-6">
            <div class="flex items-center gap-3">
              <h2 class="text-2xl font-semibold text-slate-950">Szoftver bemutató</h2>
              <HelpTooltip text="Ez a rövid bemutató végigvezeti a felhasználót az alkalmazás fő részein és javasolt használati sorrendjén." />
            </div>
            <p class="mt-2 text-sm leading-6 text-slate-600">
              Az IRBR célja, hogy igazságügyi szakértői kutatási munkát támogasson keresési futamokkal, forrásarchiválással, auditnyomvonallal és riportkészítéssel.
            </p>

            <div class="mt-6 grid gap-4">
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm">
                <div class="text-sm font-semibold text-slate-900">1. Ügy létrehozása</div>
                <p class="mt-2 text-sm leading-6 text-slate-600">Először hozz létre egy ügyet. Minden keresés, archivált forrás és riport egy konkrét ügyhöz kapcsolódik.</p>
              </article>
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm">
                <div class="text-sm font-semibold text-slate-900">2. Keresési futam indítása</div>
                <p class="mt-2 text-sm leading-6 text-slate-600">Add meg a keresési feltételeket, válassz futtatási módot és keresőmotort, majd indíts keresési futamot.</p>
              </article>
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm">
                <div class="text-sm font-semibold text-slate-900">3. Találatok archiválása</div>
                <p class="mt-2 text-sm leading-6 text-slate-600">A megnyitási tervből archiváld a releváns találatokat. Ezek kerülnek át az archivumba további értékelésre.</p>
              </article>
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm">
                <div class="text-sm font-semibold text-slate-900">4. Forrásértékelés és riport</div>
                <p class="mt-2 text-sm leading-6 text-slate-600">Az archivált forrásokat státusz, döntés és prioritás szerint értékeld, majd készíts riportot az ügyhöz.</p>
              </article>
            </div>
          </div>

          <div class="glass rounded-[28px] p-6">
            <div class="flex items-center gap-3">
              <h2 class="text-2xl font-semibold text-slate-950">Fő menük jelentése</h2>
              <HelpTooltip text="Ez a blokk röviden elmagyarázza, mire való a felső menüsor minden fő része." />
            </div>
            <div class="mt-6 grid gap-4">
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm"><div class="text-sm font-semibold text-slate-900">Bemutató</div><p class="mt-2 text-sm text-slate-600">Gyors áttekintés első használathoz.</p></article>
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm"><div class="text-sm font-semibold text-slate-900">Ügyek</div><p class="mt-2 text-sm text-slate-600">Ügyek létrehozása, kiválasztása és állapotkövetése.</p></article>
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm"><div class="text-sm font-semibold text-slate-900">Keresési Futam</div><p class="mt-2 text-sm text-slate-600">Lekérdezések összeállítása és megnyitási/keresési terv generálása.</p></article>
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm"><div class="text-sm font-semibold text-slate-900">Archivum</div><p class="mt-2 text-sm text-slate-600">Archivált források ellenőrzése, minősítése és súlyozása.</p></article>
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm"><div class="text-sm font-semibold text-slate-900">Audit</div><p class="mt-2 text-sm text-slate-600">Műveleti napló és visszakövethetőség.</p></article>
              <article class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm"><div class="text-sm font-semibold text-slate-900">Riport</div><p class="mt-2 text-sm text-slate-600">Szakértői összefoglaló és exportált anyag készítése.</p></article>
            </div>
          </div>
        </section>

        <section v-else-if="currentTab === 'cases'" class="grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
          <div class="glass rounded-[28px] p-6">
            <div class="mb-5 flex items-end justify-between gap-4">
              <div>
                <div class="flex items-center gap-3">
                  <h2 class="text-2xl font-semibold text-slate-950">Ügylista</h2>
                  <HelpTooltip text="Itt kezeled az összes ügyet. Egy ügyhöz tartoznak a keresési futamok, az archivált források és a riportok." />
                </div>
                <p class="mt-1 text-sm text-slate-600">Keresési munkafolyamat ügyalapon szervezve.</p>
              </div>
            </div>

            <div class="grid gap-4">
              <article
                v-for="item in cases"
                :key="item.id"
                class="rounded-3xl border border-slate-200 bg-white/90 p-5 shadow-sm"
              >
                <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                  <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-2">
                      <h3 class="text-lg font-semibold text-slate-900">{{ item.title }}</h3>
                      <span class="rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.18em] text-emerald-700">
                        {{ item.status }}
                      </span>
                    </div>
                    <p class="mt-2 text-sm leading-6 text-slate-600">{{ item.description || "Nincs részletes leírás." }}</p>
                    <div class="mt-3 flex flex-wrap gap-4 text-xs text-slate-500">
                      <span>Határidő: {{ item.deadline || "nincs" }}</span>
                      <span>Létrehozva: {{ item.created_at }}</span>
                      <span>Riport: {{ formatReportStatus(item.report_status) }}</span>
                      <span>Véglegesíthető: {{ formatFinalizable(item.finalizable) }}</span>
                    </div>
                    <div class="mt-3 flex flex-wrap gap-2">
                      <span class="rounded-full border border-slate-200 bg-slate-50 px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-700">
                        {{ formatReportStatus(item.report_status) }}
                      </span>
                      <span
                        :class="item.finalizable === 'igen' ? 'border-emerald-200 bg-emerald-50 text-emerald-700' : 'border-amber-200 bg-amber-50 text-amber-800'"
                        class="rounded-full border px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.18em]"
                      >
                        {{ item.finalizable === "igen" ? "Véglegesíthető" : "Nyitott" }}
                      </span>
                    </div>
                  </div>

                  <div class="flex shrink-0 gap-2">
                    <button
                      @click="currentCase = item; currentTab = 'browser'"
                      class="rounded-full bg-slate-900 px-4 py-2 text-xs font-semibold text-white transition hover:bg-slate-700"
                    >
                      Megnyitás
                    </button>
                    <button
                      @click="handleDeleteCase(item.id)"
                      class="rounded-full border border-rose-200 bg-rose-50 px-4 py-2 text-xs font-semibold text-rose-700 transition hover:bg-rose-100"
                    >
                      Törlés
                    </button>
                  </div>
                </div>
              </article>

              <div v-if="cases.length === 0" class="rounded-3xl border border-dashed border-slate-300 bg-white/70 p-8 text-center text-sm text-slate-500">
                Még nincs ügy. Hozz létre egyet a jobb oldali panelen.
              </div>
            </div>
          </div>

          <aside class="glass rounded-[28px] p-6">
            <h2 class="text-2xl font-semibold text-slate-950">Új ügy</h2>
            <p class="mt-1 text-sm text-slate-600">Egy ügyhöz több keresési kör és több archivált forrás tartozhat.</p>

            <div class="mt-5 grid gap-4">
              <label class="grid gap-1.5 text-sm">
                <span class="font-semibold text-slate-700">Megnevezés</span>
                <input v-model="newCaseTitle" class="rounded-2xl border border-slate-200 bg-white px-4 py-3 outline-none transition focus:border-slate-400" />
              </label>
              <label class="grid gap-1.5 text-sm">
                <span class="font-semibold text-slate-700">Leírás</span>
                <textarea v-model="newCaseDesc" rows="5" class="rounded-2xl border border-slate-200 bg-white px-4 py-3 outline-none transition focus:border-slate-400"></textarea>
              </label>
              <label class="grid gap-1.5 text-sm">
                <span class="font-semibold text-slate-700">Határidő</span>
                <input v-model="newCaseDeadline" type="date" class="rounded-2xl border border-slate-200 bg-white px-4 py-3 outline-none transition focus:border-slate-400" />
              </label>
              <button
                @click="handleCreateCase"
                class="rounded-2xl bg-sky-600 px-4 py-3 text-sm font-semibold text-white shadow-lg shadow-sky-600/20 transition hover:bg-sky-500"
              >
                Ügy létrehozása
              </button>
            </div>
          </aside>
        </section>

        <section v-else-if="currentTab === 'browser'">
          <BrowserPanel
            :case-id="currentCase?.id ?? null"
            :platforms="platformList"
            :enabled-ids="enabledPlatforms"
            :initial-query="rawQuery"
            @search="runBrowserSearch"
            @toggle-platform="togglePlatform"
            @open-url="openInBrowserPanel"
            @archive="archiveBrowserTab"
            @error="(message) => (appError = message)"
          />
        </section>

        <section v-else-if="currentTab === 'history'" class="glass rounded-[28px] p-6">
          <HistoryPanel
            :case-id="currentCase?.id ?? null"
            :search-runs="searchRuns"
            @error="(message) => (appError = message)"
          />
        </section>

        <section v-else-if="currentTab === 'archive'" class="glass rounded-[28px] p-6">
          <div class="flex items-start justify-between gap-4">
            <div>
              <div class="flex items-center gap-3">
                <h2 class="text-2xl font-semibold text-slate-950">Archivum</h2>
                <HelpTooltip text="Az archivumban vannak a mentett források. Itt lehet őket státuszba tenni, ellenőrizni, minősíteni és riportba emelni." />
              </div>
              <p class="mt-1 text-sm text-slate-600">A kézzel jóváhagyott és mentett forráskivonatok listája.</p>
            </div>
          </div>

          <div class="mt-6 grid gap-4 lg:grid-cols-3">
            <div class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm">
              <div class="text-[11px] uppercase tracking-[0.18em] text-slate-500">Tier eloszlás</div>
              <div class="mt-3 grid gap-2 text-sm text-slate-700">
                <div>T1: {{ archiveInsights.tierCounts[1] }}</div>
                <div>T2: {{ archiveInsights.tierCounts[2] }}</div>
                <div>T3: {{ archiveInsights.tierCounts[3] }}</div>
                <div>T4: {{ archiveInsights.tierCounts[4] }}</div>
              </div>
            </div>
            <div class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm">
              <div class="text-[11px] uppercase tracking-[0.18em] text-slate-500">Hiányzó fő tier</div>
              <div class="mt-3 text-sm text-slate-700">
                {{ archiveInsights.missingTiers.length ? archiveInsights.missingTiers.join(", ") : "Nincs hiányzó fő tier." }}
              </div>
            </div>
            <div class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm">
              <div class="text-[11px] uppercase tracking-[0.18em] text-slate-500">Ismétlődő domainek</div>
              <div class="mt-3 text-sm text-slate-700">
                <template v-if="archiveInsights.duplicatedDomains.length">
                  <div v-for="[domain, count] in archiveInsights.duplicatedDomains.slice(0, 4)" :key="domain">
                    {{ domain }} ({{ count }})
                  </div>
                </template>
                <template v-else>Nincs feltűnő duplikáció.</template>
              </div>
            </div>
          </div>

          <div class="mt-6 flex flex-wrap items-center gap-3">
            <label class="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Futam szűrő</label>
            <select v-model="archiveRunFilter" class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 outline-none">
              <option value="all">Minden forrás</option>
              <option value="unlinked">Nem kapcsolt mentések</option>
              <option v-for="run in searchRuns" :key="run.id" :value="String(run.id)">
                {{ run.created_at }} | {{ run.result_count }} találat
              </option>
            </select>
            <label class="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Státusz</label>
            <select v-model="archiveStatusFilter" class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 outline-none">
              <option value="all">Minden státusz</option>
              <option value="nyers">Nyers</option>
              <option value="ellenorzes_alatt">Ellenőrzés alatt</option>
              <option value="validalt">Validált</option>
            </select>
            <label class="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Döntés</label>
            <select v-model="archiveDecisionFilter" class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 outline-none">
              <option value="all">Minden döntés</option>
              <option value="felhasznalhato">Felhasználható</option>
              <option value="hatteranyag">Háttéranyag</option>
              <option value="kizarando">Kizárandó</option>
            </select>
            <label class="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Prioritás</label>
            <select v-model="archivePriorityFilter" class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 outline-none">
              <option value="all">Minden prioritás</option>
              <option value="1">P1</option>
              <option value="2">P2</option>
              <option value="3">P3</option>
            </select>
            <label class="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Eredet</label>
            <select v-model="archiveOriginFilter" class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 outline-none">
              <option value="all">Minden eredet</option>
              <option value="official">Hivatalos forrás</option>
              <option value="webarchive">Webarchív</option>
              <option value="reddit">Reddit</option>
              <option value="facebook">Facebook</option>
              <option value="other">Egyéb</option>
            </select>
            <div class="rounded-full border border-slate-200 bg-white px-3 py-2 text-xs text-slate-600">
              Szűrt elemek: {{ filteredSources.length }}
            </div>
            <div class="rounded-full border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
              Nyers: {{ sourceStatusSummary.nyers }} | Ellenőrzés: {{ sourceStatusSummary.ellenorzes_alatt }} | Validált: {{ sourceStatusSummary.validalt }}
            </div>
          </div>

          <div class="mt-4 rounded-3xl border border-sky-200 bg-sky-50/80 p-4 text-sm text-sky-900">
            <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-sky-700">Gyors súgó</div>
            <p class="mt-2 leading-6">
              Ajánlott sorrend: `nyers` -> `ellenőrzés alatt` -> `validált`, közben állíts döntést és prioritást. P1 csak ténylegesen felhasználható, ellenőrzött forráshoz legyen.
            </p>
          </div>

          <div
            :class="qualityChecks.length === 0 ? 'border-emerald-200 bg-emerald-50/80 text-emerald-900' : 'border-rose-200 bg-rose-50/80 text-rose-900'"
            class="mt-4 rounded-3xl border p-4 text-sm"
          >
            <div class="text-[11px] font-semibold uppercase tracking-[0.18em]">
              Minőségellenőrzés
            </div>
            <div v-if="qualityChecks.length === 0" class="mt-2">
              Nincs ismert blokkoló konzisztenciahiba. A riport exportálható.
            </div>
            <div v-else class="mt-2 grid gap-2">
              <div
                v-for="issue in qualityChecks"
                :key="issue"
                class="rounded-2xl border border-current/20 bg-white/70 px-3 py-2"
              >
                {{ issue }}
              </div>
            </div>
          </div>

          <div v-if="validatedSources.length > 0" class="mt-6 rounded-3xl border border-emerald-200 bg-emerald-50/80 p-5">
            <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
              <div>
                <h3 class="text-lg font-semibold text-emerald-950">Validált források összesítője</h3>
                <p class="mt-1 text-sm text-emerald-800">A legutóbb ellenőrzött, validált tételek külön kiemelve.</p>
              </div>
              <div class="rounded-full border border-emerald-300 bg-white px-4 py-2 text-xs font-semibold text-emerald-900">
                Összesen: {{ validatedSources.length }}
              </div>
            </div>
            <div class="mt-4 grid gap-3">
              <article
                v-for="source in validatedSources.slice(0, 5)"
                :key="`validated-${source.id}`"
                class="rounded-2xl border border-emerald-200 bg-white/90 p-4"
              >
                <div class="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                  <div>
                    <div class="text-xs font-semibold uppercase tracking-[0.18em] text-emerald-700">
                      {{ formatPriority(source.priority_level) }} | {{ formatUsabilityDecision(source.usability_decision) }}
                    </div>
                    <h4 class="mt-2 text-base font-semibold text-slate-900">{{ source.title }}</h4>
                    <p class="mt-1 text-xs text-slate-600">
                      Ellenőrző: {{ source.reviewer_name || "nincs megadva" }} | Időpont: {{ source.reviewed_at || "nincs rögzítve" }}
                    </p>
                    <p class="mt-2 text-sm text-slate-700">{{ source.proof_summary || "Nincs bizonyítási kivonat." }}</p>
                  </div>
                  <div class="text-xs text-slate-500">{{ getRunLabel(source.search_run_id) }}</div>
                </div>
              </article>
            </div>
          </div>

          <div v-if="sources.length > 0" class="mt-6 grid gap-4 lg:grid-cols-3">
            <article class="rounded-3xl border border-sky-200 bg-sky-50/80 p-5">
              <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-sky-700">Fő megállapítások</div>
              <div v-if="caseSummary.findings.length > 0" class="mt-3 grid gap-3">
                <div
                  v-for="item in caseSummary.findings"
                  :key="`finding-${item.title}`"
                  class="rounded-2xl border border-sky-200 bg-white/90 p-3"
                >
                  <div class="text-xs font-semibold text-sky-800">{{ item.priority }} | {{ item.title }}</div>
                  <p class="mt-2 text-sm leading-6 text-slate-700">{{ item.text }}</p>
                </div>
              </div>
              <p v-else class="mt-3 text-sm text-slate-600">Még nincs elég összerendezett bizonyítási kivonat.</p>
            </article>

            <article class="rounded-3xl border border-rose-200 bg-rose-50/80 p-5">
              <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-rose-700">Ellen-megállapítások</div>
              <div v-if="caseSummary.counterFindings.length > 0" class="mt-3 grid gap-3">
                <div
                  v-for="item in caseSummary.counterFindings"
                  :key="`counter-${item.title}`"
                  class="rounded-2xl border border-rose-200 bg-white/90 p-3"
                >
                  <div class="text-xs font-semibold text-rose-800">{{ item.priority }} | {{ item.title }}</div>
                  <p class="mt-2 text-sm leading-6 text-slate-700">{{ item.text }}</p>
                </div>
              </div>
              <p v-else class="mt-3 text-sm text-slate-600">Nincs külön kiemelt kizárási vagy ellenérv-lista.</p>
            </article>

            <article class="rounded-3xl border border-amber-200 bg-amber-50/80 p-5">
              <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-amber-700">Mi hiányzik még?</div>
              <div v-if="caseSummary.gaps.length > 0" class="mt-3 grid gap-2">
                <div
                  v-for="item in caseSummary.gaps"
                  :key="`gap-${item}`"
                  class="rounded-2xl border border-amber-200 bg-white/90 px-3 py-2 text-sm text-slate-700"
                >
                  {{ item }}
                </div>
              </div>
              <p v-else class="mt-3 text-sm text-slate-600">Nincs külön jelzett hiányzó bizonyítási pont.</p>
            </article>
          </div>

          <div class="mt-6 rounded-3xl border border-slate-200 bg-white/90 p-5 shadow-sm">
            <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
              <div>
                <h3 class="text-lg font-semibold text-slate-900">Strukturált szakértői álláspont</h3>
                <p class="mt-1 text-sm text-slate-600">Az ügy végső összegzését külön kezeli az alkalmazás, elkülönítve az előzetes és a végleges következtetést.</p>
              </div>
              <button
                @click="saveCaseOpinions"
                class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 transition hover:border-slate-300"
              >
                Álláspont mentése
              </button>
            </div>

            <div class="mt-4 grid gap-4">
              <div class="grid gap-4 lg:grid-cols-2">
                <label class="grid gap-1.5 text-sm text-slate-700">
                  <span class="font-semibold">Riport státusza</span>
                  <select
                    v-model="caseOpinionDraft.report_status"
                    class="rounded-3xl border border-slate-200 bg-slate-50 px-4 py-3 outline-none"
                  >
                    <option value="tervezet">Tervezet</option>
                    <option value="ellenorzes_alatt">Ellenőrzés alatt</option>
                    <option value="veglegesitett">Véglegesített</option>
                  </select>
                </label>
                <label class="grid gap-1.5 text-sm text-slate-700">
                  <span class="font-semibold">Alkalmas végleges állásfoglalásra?</span>
                  <select
                    v-model="caseOpinionDraft.finalizable"
                    class="rounded-3xl border border-slate-200 bg-slate-50 px-4 py-3 outline-none"
                  >
                    <option value="nem">Nem</option>
                    <option value="igen">Igen</option>
                  </select>
                </label>
              </div>
              <label class="grid gap-1.5 text-sm text-slate-700">
                <span class="font-semibold">Előzetes szakértői álláspont</span>
                <textarea
                  v-model="caseOpinionDraft.preliminary_opinion"
                  rows="4"
                  class="rounded-3xl border border-slate-200 bg-slate-50 px-4 py-3 outline-none"
                  placeholder="Milyen előzetes következtetés látszik jelenleg a validált és felhasználható források alapján?"
                ></textarea>
              </label>
              <label class="grid gap-1.5 text-sm text-slate-700">
                <span class="font-semibold">Végleges szakértői konklúzió</span>
                <textarea
                  v-model="caseOpinionDraft.final_opinion"
                  rows="5"
                  class="rounded-3xl border border-slate-200 bg-slate-50 px-4 py-3 outline-none"
                  placeholder="A végleges, vállalható álláspont, amely a bizonyítottság, hiányok és kizárt források mérlegelése után rögzíthető."
                ></textarea>
              </label>
              <div class="grid gap-4 lg:grid-cols-2">
                <label class="grid gap-1.5 text-sm text-slate-700">
                  <span class="font-semibold">Lezárt pontok</span>
                  <textarea
                    v-model="caseOpinionDraft.closed_points"
                    rows="4"
                    class="rounded-3xl border border-slate-200 bg-slate-50 px-4 py-3 outline-none"
                    placeholder="Mely kérdések tekinthetők lezártnak, alátámasztottnak vagy kellően bizonyítottnak?"
                  ></textarea>
                </label>
                <label class="grid gap-1.5 text-sm text-slate-700">
                  <span class="font-semibold">Nyitott kérdések</span>
                  <textarea
                    v-model="caseOpinionDraft.open_questions"
                    rows="4"
                    class="rounded-3xl border border-slate-200 bg-slate-50 px-4 py-3 outline-none"
                    placeholder="Mely pontok hiányoznak még a végleges szakértői állásfoglaláshoz?"
                  ></textarea>
                </label>
              </div>
            </div>
          </div>

          <div v-if="archiveRunFilter === 'all' && archiveStatusFilter === 'all'" class="mt-6 grid gap-4">
            <article
              v-for="group in archiveGroups"
              :key="group.key"
              class="rounded-3xl border border-slate-200 bg-white/85 p-5 shadow-sm"
            >
              <div class="mb-4 flex items-center justify-between gap-4">
                <div>
                  <h3 class="text-lg font-semibold text-slate-900">{{ group.label }}</h3>
                  <p class="mt-1 text-xs text-slate-500">{{ group.items.length }} archivált forrás ebben a csoportban.</p>
                </div>
              </div>

              <div class="grid gap-4">
                <article
                  v-for="source in group.items"
                  :key="source.id"
                  class="rounded-3xl border border-slate-200 bg-white p-5"
                >
                  <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                    <div class="min-w-0">
                      <div class="flex flex-wrap items-center gap-2">
                        <span :class="tierBadgeClass(source.tier)" class="rounded-full border px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.18em]">
                          Tier {{ source.tier }}
                        </span>
                        <span class="rounded-full border border-amber-200 bg-amber-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-amber-800">
                          {{ formatSourceStatus(source.source_status) }}
                        </span>
                        <span class="rounded-full border border-slate-200 bg-slate-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-slate-600">
                          {{ formatPriority(source.priority_level) }}
                        </span>
                        <span class="rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-emerald-800">
                          {{ formatUsabilityDecision(source.usability_decision) }}
                        </span>
                        <span class="rounded-full border border-sky-200 bg-sky-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-sky-800">
                          {{ formatArchiveKind(source.archive_kind) }}
                        </span>
                        <span class="rounded-full border border-violet-200 bg-violet-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-violet-800">
                          {{ formatSourceOrigin(getSourceOrigin(source)) }}
                        </span>
                        <span class="text-xs text-slate-500">{{ source.timestamp }}</span>
                      </div>
                      <h4 class="mt-3 text-lg font-semibold text-slate-900">{{ source.title }}</h4>
                      <p class="mt-2 break-all text-xs text-sky-700">{{ source.url }}</p>
                      <p v-if="source.original_url" class="mt-2 break-all text-xs text-slate-500">Eredeti URL: {{ source.original_url }}</p>
                      <p v-if="source.archived_url" class="mt-1 break-all text-xs text-slate-500">Archivált URL: {{ source.archived_url }}</p>
                      <p v-if="source.snapshot_timestamp" class="mt-1 text-xs text-slate-500">Snapshot: {{ source.snapshot_timestamp }}</p>
                      <p class="mt-3 text-xs leading-6 text-slate-500">Hash: <span class="font-mono">{{ source.hash }}</span></p>
                    </div>

                    <div class="flex shrink-0 flex-wrap gap-2">
                      <button
                        @click="openArchivedFile(source)"
                        class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 transition hover:border-slate-300"
                      >
                        Fájl helye
                      </button>
                      <button
                        @click="removeSource(source.id)"
                        class="rounded-full border border-rose-200 bg-rose-50 px-4 py-2 text-xs font-semibold text-rose-700 transition hover:bg-rose-100"
                      >
                        Törlés
                      </button>
                    </div>
                  </div>

                  <pre class="mt-4 max-h-56 overflow-auto rounded-2xl bg-slate-950 p-4 text-xs leading-6 text-slate-200">{{ source.content }}</pre>

                  <SourceReviewPanel
                    :source="source"
                    :draft="ensureSourceDraft(source)"
                    @quick-review="(patch) => applyQuickReview(source, patch)"
                    @save-review="saveSourceReview(source)"
                  />
                </article>
              </div>
            </article>
          </div>

          <div v-else class="mt-6 grid gap-4">
            <article
              v-for="source in filteredSources"
              :key="source.id"
              class="rounded-3xl border border-slate-200 bg-white/90 p-5 shadow-sm"
            >
              <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                <div class="min-w-0">
                  <div class="flex flex-wrap items-center gap-2">
                    <span :class="tierBadgeClass(source.tier)" class="rounded-full border px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.18em]">
                      Tier {{ source.tier }}
                    </span>
                    <span class="rounded-full border border-amber-200 bg-amber-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-amber-800">
                      {{ formatSourceStatus(source.source_status) }}
                    </span>
                    <span class="rounded-full border border-slate-200 bg-slate-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-slate-600">
                      {{ formatPriority(source.priority_level) }}
                    </span>
                    <span class="rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-emerald-800">
                      {{ formatUsabilityDecision(source.usability_decision) }}
                    </span>
                    <span class="rounded-full border border-sky-200 bg-sky-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-sky-800">
                      {{ formatArchiveKind(source.archive_kind) }}
                    </span>
                    <span class="rounded-full border border-violet-200 bg-violet-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-violet-800">
                      {{ formatSourceOrigin(getSourceOrigin(source)) }}
                    </span>
                    <span class="text-xs text-slate-500">{{ source.timestamp }}</span>
                    <span class="rounded-full border border-slate-200 bg-slate-50 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-slate-600">
                      {{ getRunLabel(source.search_run_id) }}
                    </span>
                  </div>
                  <h3 class="mt-3 text-lg font-semibold text-slate-900">{{ source.title }}</h3>
                  <p class="mt-2 break-all text-xs text-sky-700">{{ source.url }}</p>
                  <p v-if="source.original_url" class="mt-2 break-all text-xs text-slate-500">Eredeti URL: {{ source.original_url }}</p>
                  <p v-if="source.archived_url" class="mt-1 break-all text-xs text-slate-500">Archivált URL: {{ source.archived_url }}</p>
                  <p v-if="source.snapshot_timestamp" class="mt-1 text-xs text-slate-500">Snapshot: {{ source.snapshot_timestamp }}</p>
                  <p class="mt-3 text-xs leading-6 text-slate-500">Hash: <span class="font-mono">{{ source.hash }}</span></p>
                </div>

                <div class="flex shrink-0 flex-wrap gap-2">
                  <button
                    @click="openArchivedFile(source)"
                    class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 transition hover:border-slate-300"
                  >
                    Fájl helye
                  </button>
                  <button
                    @click="removeSource(source.id)"
                    class="rounded-full border border-rose-200 bg-rose-50 px-4 py-2 text-xs font-semibold text-rose-700 transition hover:bg-rose-100"
                  >
                    Törlés
                  </button>
                </div>
              </div>

              <pre class="mt-4 max-h-56 overflow-auto rounded-2xl bg-slate-950 p-4 text-xs leading-6 text-slate-200">{{ source.content }}</pre>

              <SourceReviewPanel
                :source="source"
                :draft="ensureSourceDraft(source)"
                @quick-review="(patch) => applyQuickReview(source, patch)"
                @save-review="saveSourceReview(source)"
              />
            </article>

            <div v-if="filteredSources.length === 0" class="rounded-3xl border border-dashed border-slate-300 bg-white/70 p-10 text-center text-sm text-slate-500">
              Nincs forrás ebben a kiválasztott szűrőben.
            </div>
          </div>

          <div v-if="sources.length === 0" class="mt-6 rounded-3xl border border-dashed border-slate-300 bg-white/70 p-10 text-center text-sm text-slate-500">
            Még nincs archivált forrás.
          </div>
        </section>

        <section v-else-if="currentTab === 'audit'" class="glass rounded-[28px] p-6">
          <div class="flex items-start justify-between gap-4">
            <div>
              <div class="flex items-center gap-3">
                <h2 class="text-2xl font-semibold text-slate-950">Audit nyomvonal</h2>
                <HelpTooltip text="Az audit nyomvonal minden fontosabb műveletet időrendben naplóz. Ez a visszakövethetőséget segíti." />
              </div>
              <p class="mt-1 text-sm text-slate-600">Minden fontos művelet külön naplóbejegyzést kap.</p>
            </div>
            <button
              @click="loadAuditLogs"
              class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-600 transition hover:border-slate-300"
            >
              Frissítés
            </button>
          </div>

          <div class="mt-6 grid gap-3">
            <article
              v-for="log in auditLogs"
              :key="log.id"
              class="rounded-3xl border border-slate-200 bg-white/90 p-4 shadow-sm"
            >
              <div class="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                <div>
                  <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">{{ log.action_type }}</div>
                  <div class="mt-1 text-sm font-semibold text-slate-900">{{ log.query }}</div>
                  <p class="mt-2 text-sm leading-6 text-slate-600">{{ log.details }}</p>
                </div>
                <div class="text-xs text-slate-500">{{ log.timestamp }}</div>
              </div>
            </article>

            <div v-if="auditLogs.length === 0" class="rounded-3xl border border-dashed border-slate-300 bg-white/70 p-10 text-center text-sm text-slate-500">
              Még nincs audit esemény.
            </div>
          </div>
        </section>

        <section v-else-if="currentTab === 'report'" class="grid gap-6 lg:grid-cols-[0.8fr_1.2fr]">
          <aside class="glass rounded-[28px] p-6">
            <div class="flex items-center gap-3">
              <h2 class="text-2xl font-semibold text-slate-950">Riport export</h2>
              <HelpTooltip text="Itt készül a szakértői összefoglaló. A riport az ügyadatokból, az archivált forrásokból és az audit kivonatból épül fel." />
            </div>
            <p class="mt-1 text-sm leading-6 text-slate-600">
              A riport a jelenlegi ügyből, az archivált forrásokból és az audit kivonatból épül fel.
            </p>

            <div class="mt-6 rounded-3xl border border-slate-200 bg-white/80 p-4 text-sm text-slate-600">
              <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Mit tartalmaz</div>
              <ul class="mt-3 grid gap-2">
                <li>Ügyazonosító összefoglaló</li>
                <li>Utolsó keresési futam metaadatai</li>
                <li>Archivált forrásjegyzék hash-ekkel</li>
                <li>Audit kivonat</li>
              </ul>
            </div>

            <button
              @click="saveReport"
              :disabled="isSavingReport || !currentCase"
              class="mt-6 w-full rounded-2xl bg-slate-900 px-4 py-3 text-sm font-semibold text-white transition hover:bg-slate-700 disabled:opacity-60"
            >
              {{ isSavingReport ? "Mentés..." : "Markdown riport mentése" }}
            </button>

            <div v-if="reportSaved" class="mt-4 rounded-2xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
              Riport mentve: <span class="break-all font-mono text-xs">{{ reportPath }}</span>
            </div>
          </aside>

          <div class="glass rounded-[28px] p-6">
            <div class="flex items-start justify-between gap-4">
              <div>
                <div class="flex items-center gap-3">
                  <h2 class="text-2xl font-semibold text-slate-950">Riport előnézet</h2>
                  <HelpTooltip text="Ez az élő előnézet mutatja, milyen anyag fog ténylegesen mentődni exportáláskor." />
                </div>
                <p class="mt-1 text-sm text-slate-600">Szerkesztés helyett most fix, auditálható összefoglaló generálódik.</p>
              </div>
            </div>

            <textarea
              :value="reportText"
              readonly
              rows="30"
              class="mt-5 w-full resize-y rounded-3xl border border-slate-200 bg-white p-4 font-mono text-xs leading-6 text-slate-800 outline-none"
            ></textarea>
          </div>
        </section>
      </main>
    </div>
  </div>
</template>

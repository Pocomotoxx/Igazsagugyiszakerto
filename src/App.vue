<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
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

const cases = ref([]);
const currentCase = ref(null);
const currentTab = ref("cases");
const sources = ref([]);
const auditLogs = ref([]);
const searchRuns = ref([]);
const activeSearchRunId = ref(null);
const archiveRunFilter = ref("all");
const archiveStatusFilter = ref("all");
const archiveDecisionFilter = ref("all");
const archivePriorityFilter = ref("all");
const archiveOriginFilter = ref("all");
const sourceReviewDrafts = ref({});
const caseOpinionDraft = ref({
  preliminary_opinion: "",
  final_opinion: "",
  report_status: "tervezet",
  finalizable: "nem",
  open_questions: "",
  closed_points: "",
});
const isSavingReport = ref(false);
const reportPath = ref("");
const reportSaved = ref(false);
const appError = ref("");
const searchWarning = ref("");

const PLATFORMS_KEY = "irbr-platforms";
const platformList = ref([]);
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

const newCaseTitle = ref("");
const newCaseDesc = ref("");
const newCaseDeadline = ref("");

const rawQuery = ref("");

async function api(command, args = {}) {
  return invoke(command, args);
}

onMounted(async () => {
  try {
    await api("init_db");
    try {
      platformList.value = await api("list_platforms");
    } catch {
      platformList.value = [];
    }
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

function formatReportStatus(status) {
  if (status === "ellenorzes_alatt") return "Ellenőrzés alatt";
  if (status === "veglegesitett") return "Véglegesített";
  return "Tervezet";
}

function formatFinalizable(value) {
  return value === "igen" ? "Igen" : "Nem";
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
  lines.push("## 4. Keresési futamok");
  if (searchRuns.value.length === 0) {
    lines.push("Még nem történt keresési futam ebben az ügyben.");
  } else {
    searchRuns.value.slice(0, 10).forEach((run) => {
      lines.push(`- ${run.created_at} | \`${run.base_query}\` | ${run.summary}`);
      if (run.note?.trim()) lines.push(`  - Szakértői megjegyzés: ${run.note.trim()}`);
    });
  }
  lines.push("");
  lines.push("## 5. Keresési és ellenőrzési sorrend");
  lines.push(
    "- A kérés minden bekapcsolt platformon (Google, Google Hírek, Facebook, Magyar Közlöny, Nemzeti Jogszabálytár, KSH, bíróságok stb.) külön böngészőfülön futott, a platform szintaxisára igazított lekérdezéssel.",
  );
  lines.push("");

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

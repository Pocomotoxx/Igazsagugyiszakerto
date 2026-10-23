import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const LABEL_PREFIX = "browser-";
const pending = new Map();

export const browser = reactive({
  caseId: null,
  tabs: [],
  activeId: null,
  bounds: { x: 0, y: 0, width: 800, height: 600 },
  mounted: false,
});

let counter = 0;
let listening = false;

function labelOf(tab) {
  return `${LABEL_PREFIX}${tab.id}`;
}

async function ensureListener() {
  if (listening) return;
  listening = true;
  await listen("browser-page-text", (event) => {
    const { label, payload } = event.payload;
    const tab = browser.tabs.find((item) => labelOf(item) === label);
    const resolver = tab && pending.get(tab.id);
    if (!resolver) return;
    pending.delete(tab.id);
    try {
      resolver(JSON.parse(payload));
    } catch {
      resolver({ title: tab.title, url: tab.url, text: "", published: "", modified: "" });
    }
  });
  await listen("browser-title", (event) => {
    const { label, title } = event.payload;
    const tab = browser.tabs.find((item) => labelOf(item) === label);
    // a keresési fül a platform nevét viseli, a többi az oldal címét
    if (tab && tab.kind !== "search" && title?.trim()) {
      tab.title = title.trim();
      tab.titled = true;
    }
  });
  await listen("browser-nav", (event) => {
    const { label, url, loading } = event.payload;
    const tab = browser.tabs.find((item) => labelOf(item) === label);
    if (!tab) return;
    tab.url = url;
    tab.loading = loading;
    if (!tab.titled) tab.title = shortTitle(url, tab.initialTitle);
  });
}

function shortTitle(url, fallback) {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return fallback || url;
  }
}

export function setBounds(bounds) {
  browser.bounds = bounds;
  const tab = browser.tabs.find((item) => item.id === browser.activeId);
  if (!browser.mounted || !tab || !tab.created) return Promise.resolve();
  return invoke("browser_set_bounds", { tabId: tab.id, ...bounds }).catch(() => {});
}

// A fül webview-ja csak az első megjelenítéskor jön létre (lusta betöltés): egy keresés
// tucatnyi fület nyithat anélkül, hogy tucatnyi oldal töltődne egyszerre.
async function ensureCreated(tab) {
  if (tab.created) return;
  tab.created = true;
  tab.loading = true;
  await invoke("browser_open_tab", {
    caseId: browser.caseId,
    tabId: tab.id,
    url: tab.url,
    ...browser.bounds,
  });
}

export async function openTab({ caseId, url, title, kind = "page", lazy = false, activate = true }) {
  await ensureListener();
  counter += 1;
  browser.caseId = caseId;
  const tab = {
    id: `t${Date.now().toString(36)}${counter}`,
    url,
    title: title || shortTitle(url),
    initialTitle: title || "",
    loading: !lazy,
    kind,
    created: false,
    titled: Boolean(title) && kind === "search",
  };
  browser.tabs.push(tab);
  if (activate) await activateTab(tab.id);
  return tab;
}

export async function activateTab(tabId) {
  const tab = browser.tabs.find((item) => item.id === tabId);
  if (!tab) return;
  const previous = browser.activeId;
  if (previous && previous !== tabId) {
    await invoke("browser_set_visible", { tabId: previous, visible: false }).catch(() => {});
  }
  browser.activeId = tabId;
  await ensureCreated(tab);
  await invoke("browser_set_bounds", { tabId, ...browser.bounds }).catch(() => {});
  await invoke("browser_set_visible", { tabId, visible: browser.mounted }).catch(() => {});
}

/** Bezárja a korábbi keresés füleit (a kézzel nyitott oldalak megmaradnak). */
export async function closeSearchTabs() {
  const ids = browser.tabs.filter((tab) => tab.kind === "search").map((tab) => tab.id);
  for (const id of ids) await closeTab(id);
}

/** Az aktív oldal megjelenített szövege és időadatai (archiváláshoz), a webview-ból kiolvasva. */
export function captureTab(tabId) {
  const tab = browser.tabs.find((item) => item.id === tabId);
  if (!tab || !tab.created) return Promise.reject(new Error("Az oldal még nincs betöltve."));
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(tabId);
      reject(new Error("Az oldal szövegének kiolvasása időtúllépés miatt nem sikerült."));
    }, 8000);
    pending.set(tabId, (data) => {
      clearTimeout(timer);
      resolve(data);
    });
    invoke("browser_capture_text", { tabId }).catch((error) => {
      clearTimeout(timer);
      pending.delete(tabId);
      reject(error);
    });
  });
}

export async function closeTab(tabId) {
  const index = browser.tabs.findIndex((tab) => tab.id === tabId);
  if (index === -1) return;
  if (browser.tabs[index].created) await invoke("browser_close_tab", { tabId }).catch(() => {});
  browser.tabs.splice(index, 1);
  if (browser.activeId === tabId) {
    const next = browser.tabs[Math.min(index, browser.tabs.length - 1)];
    browser.activeId = null;
    if (next) await activateTab(next.id);
  }
}

export function navigate({ caseId, tabId, url }) {
  return invoke("browser_navigate", { caseId, tabId, url });
}

export function tabAction(tabId, action) {
  return invoke("browser_action", { tabId, action });
}

export async function showActive() {
  browser.mounted = true;
  const tab = browser.tabs.find((item) => item.id === browser.activeId);
  if (!tab) return;
  await ensureCreated(tab);
  await invoke("browser_set_bounds", { tabId: tab.id, ...browser.bounds }).catch(() => {});
  await invoke("browser_set_visible", { tabId: tab.id, visible: true }).catch(() => {});
}

export async function hideAll() {
  browser.mounted = false;
  await invoke("browser_hide_all").catch(() => {});
}

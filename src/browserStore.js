import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const LABEL_PREFIX = "browser-";

export const browser = reactive({
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
  await listen("browser-nav", (event) => {
    const { label, url, loading } = event.payload;
    const tab = browser.tabs.find((item) => labelOf(item) === label);
    if (!tab) return;
    tab.url = url;
    tab.loading = loading;
    if (!tab.title || tab.title === tab.initialTitle) {
      tab.title = shortTitle(url, tab.initialTitle);
    }
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
  if (!browser.mounted || !browser.activeId) return Promise.resolve();
  return invoke("browser_set_bounds", { tabId: browser.activeId, ...bounds }).catch(() => {});
}

export async function openTab({ caseId, url, title }) {
  await ensureListener();
  counter += 1;
  const tab = {
    id: `t${Date.now().toString(36)}${counter}`,
    url,
    title: title || shortTitle(url),
    initialTitle: title || "",
    loading: true,
  };
  const previous = browser.activeId;
  await invoke("browser_open_tab", {
    caseId,
    tabId: tab.id,
    url,
    ...browser.bounds,
  });
  if (previous) await invoke("browser_set_visible", { tabId: previous, visible: false }).catch(() => {});
  browser.tabs.push(tab);
  browser.activeId = tab.id;
  if (!browser.mounted) await invoke("browser_set_visible", { tabId: tab.id, visible: false }).catch(() => {});
  return tab;
}

export async function activateTab(tabId) {
  if (browser.activeId === tabId) return;
  const previous = browser.activeId;
  browser.activeId = tabId;
  if (previous) await invoke("browser_set_visible", { tabId: previous, visible: false }).catch(() => {});
  await invoke("browser_set_bounds", { tabId, ...browser.bounds }).catch(() => {});
  if (browser.mounted) await invoke("browser_set_visible", { tabId, visible: true }).catch(() => {});
}

export async function closeTab(tabId) {
  const index = browser.tabs.findIndex((tab) => tab.id === tabId);
  if (index === -1) return;
  await invoke("browser_close_tab", { tabId }).catch(() => {});
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
  if (!browser.activeId) return;
  await invoke("browser_set_bounds", { tabId: browser.activeId, ...browser.bounds }).catch(() => {});
  await invoke("browser_set_visible", { tabId: browser.activeId, visible: true }).catch(() => {});
}

export async function hideAll() {
  browser.mounted = false;
  await invoke("browser_hide_all").catch(() => {});
}

// Az App.vue <script setup> blokkját Node alatt futtatja Vue-reaktivitás nélküli, egyszerű
// helyettesítőkkel; a Tauri-hívások (invoke) rögzítettek és szkriptelhetők. Regressziós próbákhoz.
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { beginOperation, CaseChangedError } from "../src/caseGuard.js";

const APP = fileURLToPath(new URL("../src/App.vue", import.meta.url));

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export function loadApp(overrides = {}) {
  const src = fs.readFileSync(APP, "utf8");
  let script = src.slice(src.indexOf("<script setup>") + 14, src.indexOf("</script>"));
  script = script.replace(/^import .*$/gm, "");
  const names = [...script.matchAll(/^(?:async function |function |const |let )(\w+)/gm)].map((m) => m[1]);
  const body = `${script}\nreturn { ${[...new Set(names)].join(", ")} };`;
  const log = [];
  const ref = (value) => ({ value });
  const computed = (fn) => ({ get value() { return fn(); } });
  const invoke = async (command, args) => {
    log.push([command, args]);
    return overrides.invoke ? overrides.invoke(command, args) : null;
  };
  const fn = new Function(
    "ref", "computed", "watch", "onMounted", "invoke", "revealItemInDir", "confirm",
    "captureTab", "closeSearchTabs", "closeTabsNotOf", "openBrowserTab", "beginOperation", "CaseChangedError",
    "HelpTooltip", "SourceReviewPanel", "BrowserPanel", "HistoryPanel", "localStorage",
    body,
  );
  const app = fn(
    ref, computed, () => {}, () => {}, invoke, async () => {}, overrides.confirm || (() => true),
    overrides.captureTab || (async () => ({})),
    overrides.closeSearchTabs || (async () => {}),
    overrides.closeTabsNotOf || (async () => { log.push(["closeTabsNotOf"]); }),
    async (options) => { log.push(["openTab", options]); },
    beginOperation, CaseChangedError,
    null, null, null, null, { getItem: () => null, setItem: () => {} },
  );
  return { app, log };
}

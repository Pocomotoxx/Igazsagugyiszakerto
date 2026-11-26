// Az audit (2026-11-25) 1. és 2. kiadást akadályozó hibájának regressziós próbái.
import assert from "node:assert/strict";
import test from "node:test";
import { loadApp, sleep } from "./harness.mjs";

const caseA = { id: 1, title: "A ügy" };
const caseB = { id: 2, title: "B ügy" };

test("keresés közbeni ügyváltás: sem futam, sem fül nem jön létre másik ügyben", async () => {
  const { app, log } = loadApp({
    invoke: async (command) => {
      if (command === "browser_tab_links") {
        await sleep(30);
        return [{ label: "Google", url: "https://g/1" }, { label: "Facebook", url: "https://g/2" }];
      }
      return command === "record_search_run" ? 11 : null;
    },
  });
  app.currentCase.value = caseA;
  const search = app.runBrowserSearch("takarítás eger órabér");
  await sleep(5);
  app.currentCase.value = caseB; // ügyváltás a válasz megérkezése előtt
  await search;

  assert.equal(log.some(([c]) => c === "record_search_run"), false, "nem mentődhet futam");
  assert.equal(log.some(([c]) => c === "openTab"), false, "nem nyílhat fül");
  assert.match(app.searchWarning.value, /ügyet váltottál/);
});

test("keresés ügyváltás nélkül: a futam és a fülek ugyanahhoz az ügyhöz és futamhoz kötődnek", async () => {
  const { app, log } = loadApp({
    invoke: async (command) => {
      if (command === "browser_tab_links") return [{ label: "Google", url: "https://g/1" }, { label: "KSH", url: "https://g/2" }];
      return command === "record_search_run" ? 11 : null;
    },
  });
  app.currentCase.value = caseA;
  await app.runBrowserSearch("takarítás");
  const record = log.find(([c]) => c === "record_search_run")[1];
  assert.equal(record.caseId, 1);
  const tabs = log.filter(([c]) => c === "openTab").map(([, o]) => o);
  assert.equal(tabs.length, 2);
  assert.ok(tabs.every((t) => t.caseId === 1 && t.runId === 11));
});

test("másik ügy füle nem archiválható az aktív ügyhöz (A futama + B azonosítója nem keveredhet)", async () => {
  const { app, log } = loadApp({
    captureTab: async () => ({ title: "A cikke", url: "https://a.hu", text: "A ügy tartalma", kind: "teljes_oldal" }),
  });
  app.currentCase.value = caseB; // közben B az aktív
  const tabOfA = { id: "t1", title: "Google", url: "https://a.hu", kind: "search", caseId: 1, runId: 11 };
  await app.archiveBrowserTab(tabOfA, false);
  assert.equal(log.some(([c]) => c === "archive_source"), false);
  assert.match(app.appError.value, /másik ügyhöz/);
});

test("archiválás közbeni ügyváltás: semmi nem mentődik", async () => {
  const { app, log } = loadApp({
    captureTab: async () => { await sleep(30); return { title: "cím", url: "https://a.hu", text: "szöveg", kind: "teljes_oldal" }; },
  });
  app.currentCase.value = caseA;
  const tab = { id: "t1", title: "Google", url: "https://a.hu", kind: "search", caseId: 1, runId: 11 };
  const run = app.archiveBrowserTab(tab, false);
  await sleep(5);
  app.currentCase.value = caseB;
  await run;
  assert.equal(log.some(([c]) => c === "archive_source"), false);
});

test("archiválás a fül saját ügyével és futamával, kijelölt szövegként", async () => {
  const { app, log } = loadApp({
    captureTab: async (id, selectionOnly) => ({
      title: "Díjjegyzék", url: "https://x.hu/dij", text: "a kijelölt rész", kind: selectionOnly ? "kijelolt_szoveg" : "teljes_oldal", truncated: false,
    }),
  });
  app.currentCase.value = caseA;
  await app.archiveBrowserTab({ id: "t1", title: "Google", url: "https://x.hu/dij", kind: "search", caseId: 1, runId: 11 }, true);
  const call = log.find(([c]) => c === "archive_source")[1];
  assert.deepEqual(
    { caseId: call.caseId, searchRunId: call.searchRunId, captureKind: call.captureKind, captureTruncated: call.captureTruncated },
    { caseId: 1, searchRunId: 11, captureKind: "kijelolt_szoveg", captureTruncated: false },
  );
  assert.match(call.content, /KIJELÖLT SZÖVEGRÉSZLET/);
});

test("kijelölés nélkül a „csak a kijelölt szöveg” mód nem ment semmit", async () => {
  const { app, log } = loadApp({
    captureTab: async () => ({ title: "x", url: "https://x.hu", text: "", kind: "kijelolt_szoveg", empty_selection: true }),
  });
  app.currentCase.value = caseA;
  await app.archiveBrowserTab({ id: "t1", title: "G", url: "https://x.hu", kind: "search", caseId: 1, runId: null }, true);
  assert.equal(log.some(([c]) => c === "archive_source"), false);
  assert.match(app.appError.value, /Nincs kijelölt szöveg/);
});

test("késői betöltési válasz nem írja felül a másik ügy forrásait", async () => {
  const { app } = loadApp({
    invoke: async (command, args) => {
      if (command === "get_sources") {
        await sleep(args.caseId === 1 ? 40 : 1);
        return [{ id: args.caseId * 100, title: `forrás ${args.caseId}`, usability_decision: "hatteranyag" }];
      }
      return null;
    },
  });
  app.currentCase.value = caseA;
  const slow = app.loadSources();
  app.currentCase.value = caseB;
  await app.loadSources();
  await slow;
  assert.equal(app.sources.value[0].title, "forrás 2");
});

test("az audit 2. hibája: a nem véglegesíthető állapot nem jelenhet meg véglegesítettként, és nem adható ki", async () => {
  const { app, log } = loadApp({ confirm: () => true });
  app.currentCase.value = { ...caseA, description: "x", deadline: "", report_status: "tervezet", finalizable: "nem", final_opinion: "", open_questions: "", closed_points: "", preliminary_opinion: "" };
  app.sources.value = [{ id: 1, title: "Nyers háttérforrás", url: "https://x.hu", tier: 3, usability_decision: "hatteranyag", source_status: "nyers", priority_level: 2, proof_summary: "", reviewer_name: "", reviewed_at: "" }];
  app.caseOpinionDraft.value = { preliminary_opinion: "", final_opinion: "", report_status: "veglegesitett", finalizable: "nem", open_questions: "", closed_points: "" };
  app.approverName.value = "Dr. Minta";

  // a riportszöveg nem állíthatja véglegesítettnek
  assert.match(app.reportStatusLabel.value, /^Tervezet/);
  assert.equal(app.reportText.value.includes("Riport státusza: Véglegesített"), false);
  // a végleges kiadás tiltott (mentetlen módosítás is akadály: a mentett állapot még tervezet)
  assert.ok(app.finalReleaseBlockers.value.length >= 2);
  await app.approveFinalReport();
  assert.equal(log.some(([c]) => c === "approve_final_report"), false);

  // a tervezet viszont exportálható (a háttérrendszer jelöli)
  await app.saveReport();
  assert.equal(log.some(([c]) => c === "save_report"), true);
});

test("érvényes, mentett véglegesítés: a végleges kiadás elindulhat", async () => {
  const { app, log } = loadApp({ invoke: async (c) => (c === "approve_final_report" ? { version: 1, approved_by: "Dr. Minta" } : []) });
  const saved = { ...caseA, description: "x", deadline: "", report_status: "veglegesitett", finalizable: "igen", final_opinion: "A munka értéke 480 000 Ft.", open_questions: "", closed_points: "", preliminary_opinion: "" };
  app.currentCase.value = saved;
  app.sources.value = [{ id: 1, title: "Számla", url: "https://x.hu", tier: 1, usability_decision: "felhasznalhato", source_status: "validalt", priority_level: 1, proof_summary: "igazolja", reviewer_name: "Dr. Minta", reviewed_at: "2026-01-02" }];
  app.caseOpinionDraft.value = { preliminary_opinion: "", final_opinion: saved.final_opinion, report_status: "veglegesitett", finalizable: "igen", open_questions: "", closed_points: "" };
  app.approverName.value = "Dr. Minta";
  assert.deepEqual(app.finalReleaseBlockers.value, []);
  await app.approveFinalReport();
  const call = log.find(([c]) => c === "approve_final_report");
  assert.ok(call);
  assert.equal(call[1].caseId, 1);
  assert.equal(call[1].approverName, "Dr. Minta");
});

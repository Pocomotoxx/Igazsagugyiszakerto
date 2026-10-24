import assert from "node:assert/strict";
import test from "node:test";
import { buildLinks, LENS_MANUAL_URL, LENS_UPLOAD_URL, MAX_NAME_CHARS, sanitizeName } from "./links.js";

test("sanitizeName: vezérlőkarakterek, idézőjelek, zárójelek és operátorok kiszűrése", () => {
  assert.equal(sanitizeName("  Algopyrin \n 500 mg  "), "Algopyrin 500 mg");
  assert.equal(sanitizeName('"Ibuprofen" (400)'), "Ibuprofen 400");
  assert.equal(sanitizeName("Aspirin site:evil.com"), "Aspirin");
  assert.equal(sanitizeName("a"), null);
  assert.equal(sanitizeName("   "), null);
  assert.equal(sanitizeName("site:x.hu"), null);
  assert.equal(sanitizeName(null), null);
  assert.equal(Array.from(sanitizeName("x".repeat(500))).length, MAX_NAME_CHARS);
});

test("sanitizeName: ékezetes nevek és a nagy kisbetű-különbség megmarad", () => {
  assert.equal(sanitizeName("Cataflam Dolo 25 mg"), "Cataflam Dolo 25 mg");
  assert.equal(sanitizeName("Nurofen Forte – 400 mg"), "Nurofen Forte – 400 mg");
});

test("buildLinks: betegtájékoztató, OGYÉI és képes keresés Google-címmel", () => {
  const links = buildLinks("Algopyrin 500 mg");
  const byId = Object.fromEntries(links.map((l) => [l.id, l.url]));
  assert.ok(byId.med_betegtajekoztato.includes(encodeURIComponent("betegtájékoztató")));
  assert.ok(byId.med_ogyei.includes(encodeURIComponent("site:ogyei.gov.hu")));
  assert.ok(byId.med_kepek.includes("tbm=isch"));
  assert.ok(links.every((l) => l.url.startsWith("https://www.google.com/search?")));
});

test("buildLinks: a név nem módosíthatja a lekérdezést", () => {
  const [first] = buildLinks('Aspirin" site:evil.com');
  assert.ok(!first.url.includes("evil.com"));
});

test("buildLinks: üres névre érthető hibaüzenet", () => {
  assert.throws(() => buildLinks("  "), /gyógyszer nevét/);
});

test("a Lens-címek https-ek", () => {
  assert.ok(LENS_UPLOAD_URL.startsWith("https://lens.google.com/"));
  assert.ok(LENS_MANUAL_URL.startsWith("https://lens.google.com/"));
});

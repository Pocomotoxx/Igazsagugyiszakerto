import assert from "node:assert/strict";
import test from "node:test";
import { beginOperation, CaseChangedError } from "./caseGuard.js";

test("ugyanaz az ügy: az őr átengedi", () => {
  let current = 1;
  const op = beginOperation(() => current);
  assert.equal(op.caseId, 1);
  assert.equal(op.same(), true);
  op.assertSame();
});

test("ügyváltás után az őr megszakítja a műveletet (az audit 1. hibája)", async () => {
  let current = 1;
  const op = beginOperation(() => current);
  await new Promise((resolve) => setTimeout(resolve, 5));
  current = 2; // ügyváltás a válasz megérkezése előtt
  assert.equal(op.same(), false);
  assert.throws(() => op.assertSame(), CaseChangedError);
});

test("visszaváltás A→B→A után sem folytatódhat a korábbi művelet tévesen: azonos azonosítóra újra igaz", () => {
  let current = 1;
  const op = beginOperation(() => current);
  current = 2;
  current = 1;
  // ugyanahhoz az ügyhöz való visszatérés megengedett (a művelet a saját ügyéhez tartozik)
  assert.equal(op.same(), true);
});

test("ügy nélkül indított művelet soha nem folytatódhat", () => {
  let current = null;
  const op = beginOperation(() => current);
  current = 1;
  assert.equal(op.same(), false);
  assert.throws(() => op.assertSame(), CaseChangedError);
});

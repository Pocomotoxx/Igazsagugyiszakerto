// Ügyhatár-őr: egy hosszabb művelet (keresés, archiválás) az indulásakor rögzíti az ügyet, és minden
// várakozás után ellenőrzi, hogy az aktív ügy ugyanaz maradt-e. Ügyváltás után a késői válasz nem
// módosíthatja a másik ügy állapotát, és semmi nem mentődik idegen ügyhöz.

export class CaseChangedError extends Error {
  constructor() {
    super("Az aktív ügy közben megváltozott, ezért a művelet megszakadt (hogy ne kerüljön adat másik ügybe).");
    this.name = "CaseChangedError";
  }
}

/** `getCaseId`: az aktív ügy azonosítóját adó függvény (null, ha nincs ügy). */
export function beginOperation(getCaseId) {
  const caseId = getCaseId();
  return {
    caseId,
    same() {
      return caseId !== null && caseId !== undefined && getCaseId() === caseId;
    },
    assertSame() {
      if (!this.same()) throw new CaseChangedError();
    },
  };
}

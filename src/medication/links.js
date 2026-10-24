// Gyógyszer-ellenőrzés: platformfüggetlen keresőlink-képzés. Ez a modul a böngészőben (PWA),
// az asztali Tauri-alkalmazásban és később a mobil (Android/iOS) héjakban is ugyanúgy fut.
// NEM gyógyszerazonosítás: csak keresőoldalak címeit állítja elő; semmit nem értelmez és nem tárol.

export const MAX_NAME_CHARS = 100;
const MIN_NAME_CHARS = 2;

/** A Google Lens fotó-feltöltő végpontja (multipart, `encoded_image` mező). */
export const LENS_UPLOAD_URL = "https://lens.google.com/v3/upload?hl=hu";
/** A Google Lens nyitóoldala (kézi feltöltéshez; mindig működő tartalék). */
export const LENS_MANUAL_URL = "https://lens.google.com/";

/**
 * Megtisztított gyógyszernév: vezérlőkarakterek, idézőjelek és zárójelek nélkül, összevont
 * szóközökkel, legfeljebb 100 karakter, keresőoperátorok (site:, inurl:, filetype:) nélkül.
 * Túl rövid vagy üres bemenetre null.
 */
export function sanitizeName(input) {
  const cleaned = String(input ?? "")
    // eslint-disable-next-line no-control-regex
    .replace(/[\u0000-\u001f\u007f-\u009f"“”'()<>\\]/g, " ")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
  const name = Array.from(cleaned).slice(0, MAX_NAME_CHARS).join("").trim();
  if (Array.from(name).length < MIN_NAME_CHARS) return null;
  const safe = name
    .split(" ")
    .filter((word) => !/^(site|inurl|filetype|intitle|intext|related|cache):/i.test(word))
    .join(" ");
  return Array.from(safe).length < MIN_NAME_CHARS ? null : safe;
}

const google = (query, extra = "") =>
  `https://www.google.com/search?q=${encodeURIComponent(query)}&hl=hu&gl=hu${extra}`;

/**
 * A névre induló keresések: betegtájékoztató, az OGYÉI hivatalos oldalai és képes találatok
 * (a doboz és a tabletta kinézetének összevetéséhez). Hibás névre Error-t dob magyar üzenettel.
 */
export function buildLinks(rawName) {
  const name = sanitizeName(rawName);
  if (!name) {
    throw new Error("Írd be a gyógyszer nevét (legalább 2 karakter), keresőoperátorok és idézőjelek nélkül.");
  }
  return [
    { id: "med_betegtajekoztato", label: "Betegtájékoztató", url: google(`"${name}" gyógyszer betegtájékoztató`) },
    { id: "med_ogyei", label: "OGYÉI (hivatalos)", url: google(`${name} site:ogyei.gov.hu`) },
    { id: "med_kepek", label: "Képek (doboz, tabletta)", url: google(`${name} gyógyszer doboz tabletta`, "&tbm=isch") },
  ];
}

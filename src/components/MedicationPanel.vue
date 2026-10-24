<script setup>
import { onBeforeUnmount, onMounted, ref } from "vue";
import DisclaimerDialog from "./DisclaimerDialog.vue";
import { buildLinks, LENS_MANUAL_URL, LENS_UPLOAD_URL } from "../medication/links.js";

const emit = defineEmits(["open-lens", "open-manual-lens", "open-links", "leave", "error"]);

const LENS_IMAGE_KEY = "irbr-lens-image";
// Az asztali (Tauri) héjban a találatok a beépített böngészőpanel füleiben nyílnak; böngészőben
// (PWA) és később Android/iOS alatt a rendszer böngészője kapja őket. A logika ugyanaz.
const desktopShell = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const webLinks = ref([]);
const MAX_SIDE = 1600;

// Minden művelet indítása előtt megjelenik a figyelmeztetés; a megerősítés után fut le a művelet.
const dialog = ref({ open: false, action: "", run: null });
const medName = ref("");
const imageData = ref("");
const cameraOn = ref(false);
const video = ref(null);
const fileInput = ref(null);
const captureInput = ref(null);
// Érintőképernyős eszközön a natív kamera (capture) a legmegbízhatóbb; a webkamera az asztali gépeké.
const touchDevice = typeof navigator !== "undefined" && navigator.maxTouchPoints > 0;
let stream = null;

function askThen(action, run) {
  dialog.value = { open: true, action, run };
}

function confirmDialog() {
  const run = dialog.value.run;
  dialog.value = { open: false, action: "", run: null };
  if (run) run();
}

function cancelDialog() {
  const wasIntro = dialog.value.action === "A Gyógyszer-ellenőrzés megnyitása";
  dialog.value = { open: false, action: "", run: null };
  if (wasIntro) emit("leave");
}

onMounted(() => {
  askThen("A Gyógyszer-ellenőrzés megnyitása", () => {});
});

function stopCamera() {
  if (stream) {
    stream.getTracks().forEach((track) => track.stop());
    stream = null;
  }
  cameraOn.value = false;
}

onBeforeUnmount(() => {
  stopCamera();
  imageData.value = "";
});

async function startCamera() {
  try {
    if (!navigator.mediaDevices?.getUserMedia) throw new Error("nincs kamera-támogatás");
    stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: "environment" }, audio: false });
    cameraOn.value = true;
    await new Promise((resolve) => setTimeout(resolve, 0));
    if (video.value) {
      video.value.srcObject = stream;
      await video.value.play();
    }
  } catch (error) {
    stopCamera();
    emit("error", `A kamera nem indítható (${String(error)}). Használd a „Kép kiválasztása" gombot.`);
  }
}

function toJpegDataUrl(source, width, height) {
  const scale = Math.min(1, MAX_SIDE / Math.max(width, height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(width * scale));
  canvas.height = Math.max(1, Math.round(height * scale));
  canvas.getContext("2d").drawImage(source, 0, 0, canvas.width, canvas.height);
  return canvas.toDataURL("image/jpeg", 0.85);
}

function capture() {
  const el = video.value;
  if (!el || !el.videoWidth) return;
  imageData.value = toJpegDataUrl(el, el.videoWidth, el.videoHeight);
  stopCamera();
}

function onFileChosen(event) {
  const file = event.target.files?.[0];
  event.target.value = "";
  if (!file) return;
  if (!file.type.startsWith("image/")) {
    emit("error", "Csak képfájl választható.");
    return;
  }
  const url = URL.createObjectURL(file);
  const img = new Image();
  img.onload = () => {
    imageData.value = toJpegDataUrl(img, img.naturalWidth, img.naturalHeight);
    URL.revokeObjectURL(url);
  };
  img.onerror = () => {
    URL.revokeObjectURL(url);
    emit("error", "A kép nem olvasható.");
  };
  img.src = url;
}

function discardImage() {
  imageData.value = "";
}

function dataUrlToFile(dataUrl) {
  const comma = dataUrl.indexOf(",");
  const type = (dataUrl.slice(0, comma).match(/data:([^;]+)/) || [])[1] || "image/jpeg";
  const binary = atob(dataUrl.slice(comma + 1));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return new File([bytes], "gyogyszer.jpg", { type });
}

/** Böngészőben/PWA-ban: a fotót űrlapként küldi a Google Lens-nek új lapon (a megerősítő kattintásból, szinkron). */
function submitToLensInBrowser(dataUrl) {
  const form = document.createElement("form");
  form.method = "POST";
  form.action = LENS_UPLOAD_URL;
  form.enctype = "multipart/form-data";
  form.target = "_blank";
  form.rel = "noopener noreferrer";
  form.style.display = "none";
  const input = document.createElement("input");
  input.type = "file";
  input.name = "encoded_image";
  const transfer = new DataTransfer();
  transfer.items.add(dataUrlToFile(dataUrl));
  input.files = transfer.files;
  form.appendChild(input);
  document.body.appendChild(form);
  form.submit();
  form.remove();
}

function sendToLens() {
  if (!desktopShell) {
    try {
      submitToLensInBrowser(imageData.value);
    } catch {
      emit("error", "A fotó nem adható át a Google Lens-nek. Használd a „Google Lens megnyitása (kézi feltöltés)” gombot.");
      return;
    }
    imageData.value = "";
    return;
  }
  try {
    localStorage.setItem(LENS_IMAGE_KEY, imageData.value);
  } catch {
    emit("error", "A fotó nem adható át (a kép túl nagy vagy a tároló nem elérhető). Próbálj kisebb képet.");
    return;
  }
  imageData.value = ""; // a fotó a feltöltő oldal olvasása után sem marad a gépen
  emit("open-lens");
}

function openManualLens() {
  if (desktopShell) {
    emit("open-manual-lens");
  } else {
    window.open(LENS_MANUAL_URL, "_blank", "noopener,noreferrer");
  }
}

function searchByName() {
  try {
    const links = buildLinks(medName.value);
    if (desktopShell) {
      emit("open-links", links);
    } else {
      // böngészőben egy kattintás egy lapot nyithat (a felugró-blokkoló miatt): a találatok linkként jelennek meg
      webLinks.value = links;
    }
  } catch (error) {
    emit("error", error.message);
  }
}
</script>

<template>
  <div class="grid gap-5">
    <DisclaimerDialog :open="dialog.open" :action="dialog.action" @confirm="confirmDialog" @cancel="cancelDialog" />

    <!-- Folyamatosan látható figyelmeztetés -->
    <div role="alert" class="rounded-3xl border-2 border-red-700 bg-red-600 px-6 py-4 text-white shadow-lg">
      <div class="text-xl font-extrabold">⚠ Ez NEM gyógyszerazonosítás</div>
      <p class="mt-1 text-base font-semibold leading-7">
        Az eszköz csak internetes keresést (Google / Google Lens) indít, és nem helyettesíti a szakszerű gyógyszerazonosítást.
        Ismeretlen gyógyszert ne vegyen be és ne adjon be senkinek; kérdezzen gyógyszerészt vagy orvost.
        Mérgezés vagy rosszullét gyanúja esetén hívja a <span class="underline">112</span>-t.
      </p>
    </div>

    <div class="glass rounded-[28px] p-6">
      <h2 class="text-2xl font-semibold text-slate-950">Gyógyszer-ellenőrzés</h2>
      <p class="mt-1 text-sm text-slate-600">Fotózd le az ismeretlen gyógyszerdobozt, vagy írd be a gyógyszer nevét, és a program a Google-ben keres rá.</p>

      <div class="mt-6 grid gap-6 lg:grid-cols-2">
        <!-- Fotó -->
        <section class="grid content-start gap-3 rounded-3xl border border-slate-200 bg-white/90 p-5">
          <h3 class="text-lg font-semibold text-slate-900">1. Fotó Google Lens-szel</h3>
          <p class="text-sm text-slate-600">Készíts képet a dobozról (a nevet és a hatóanyagot tartalmazó oldalról), vagy válassz meglévő képet. Személyes adat ne legyen a képen.</p>

          <div v-if="cameraOn" class="grid gap-2">
            <video ref="video" class="w-full rounded-2xl bg-black" playsinline muted></video>
            <div class="flex gap-2">
              <button class="rounded-full bg-sky-600 px-5 py-3 text-base font-semibold text-white hover:bg-sky-500" @click="capture">📷 Fénykép készítése</button>
              <button class="rounded-full border border-slate-300 bg-white px-5 py-3 text-base font-semibold text-slate-700" @click="stopCamera">Kamera leállítása</button>
            </div>
          </div>

          <div v-else-if="imageData" class="grid gap-3">
            <img :src="imageData" alt="A kiválasztott fotó előnézete" class="max-h-72 w-full rounded-2xl border border-slate-200 object-contain" />
            <div class="flex flex-wrap gap-2">
              <button class="rounded-full bg-red-600 px-5 py-3 text-base font-bold text-white hover:bg-red-700" @click="askThen('A fotó küldése a Google Lens-nek', sendToLens)">
                Keresés Google Lens-szel
              </button>
              <button class="rounded-full border border-slate-300 bg-white px-5 py-3 text-base font-semibold text-slate-700" @click="discardImage">Kép elvetése</button>
            </div>
          </div>

          <div v-else class="flex flex-wrap gap-2">
            <button
              v-if="touchDevice"
              class="rounded-full bg-sky-600 px-5 py-3 text-base font-semibold text-white hover:bg-sky-500"
              @click="askThen('Fénykép készítése a telefon kamerájával', () => captureInput?.click())"
            >📷 Fénykép készítése</button>
            <button
              v-else
              class="rounded-full bg-sky-600 px-5 py-3 text-base font-semibold text-white hover:bg-sky-500"
              @click="askThen('A webkamera bekapcsolása', startCamera)"
            >📷 Webkamera indítása</button>
            <button class="rounded-full border border-slate-300 bg-white px-5 py-3 text-base font-semibold text-slate-700" @click="askThen('Kép kiválasztása fájlból', () => fileInput?.click())">Kép kiválasztása</button>
          </div>
          <input ref="fileInput" type="file" accept="image/*" class="hidden" @change="onFileChosen" />
          <input ref="captureInput" type="file" accept="image/*" capture="environment" class="hidden" @change="onFileChosen" />

          <button
            class="justify-self-start text-sm font-semibold text-slate-600 underline-offset-2 hover:underline"
            @click="askThen('A Google Lens megnyitása kézi feltöltéshez', openManualLens)"
          >A Google Lens megnyitása (kézi feltöltés)</button>
        </section>

        <!-- Név -->
        <section class="grid content-start gap-3 rounded-3xl border border-slate-200 bg-white/90 p-5">
          <h3 class="text-lg font-semibold text-slate-900">2. Keresés a gyógyszer nevére</h3>
          <p class="text-sm text-slate-600">Add meg a gyógyszer nevét úgy, ahogy a dobozon áll (pl. „Algopyrin 500 mg"). Három keresés nyílik: betegtájékoztató, az OGYÉI hivatalos oldalai és képek a doboz összevetéséhez.</p>
          <input
            v-model="medName"
            maxlength="100"
            placeholder="A gyógyszer neve"
            class="rounded-2xl border border-slate-300 bg-white px-4 py-3 text-lg outline-none focus:border-slate-500"
            @keydown.enter="medName.trim() && askThen('Keresés a gyógyszer nevére', searchByName)"
          />
          <button
            :disabled="!medName.trim()"
            class="justify-self-start rounded-full bg-red-600 px-6 py-3 text-base font-bold text-white hover:bg-red-700 disabled:opacity-50"
            @click="askThen('Keresés a gyógyszer nevére', searchByName)"
          >Keresés</button>
          <div v-if="webLinks.length" class="grid gap-2">
            <div class="text-sm font-semibold text-slate-700">Nyisd meg a keresések egyikét (új lapon):</div>
            <a
              v-for="link in webLinks"
              :key="link.id"
              :href="link.url"
              target="_blank"
              rel="noopener noreferrer"
              class="rounded-2xl border border-red-200 bg-red-50 px-4 py-3 text-base font-semibold text-red-800 hover:bg-red-100"
            >{{ link.label }} →</a>
          </div>
        </section>
      </div>

      <div class="mt-6 grid gap-3 rounded-3xl border border-amber-300 bg-amber-50 p-5 text-sm leading-6 text-amber-950">
        <div class="text-base font-bold">Mit tegyen, ha bizonytalan?</div>
        <ul class="list-disc space-y-1 pl-6">
          <li>Kérdezze meg a <strong>gyógyszerészt</strong> a legközelebbi gyógyszertárban (a doboz mutatása elég).</li>
          <li>Hívja a <strong>háziorvost</strong> vagy a kezelőorvost, ügyeleti időben az orvosi ügyeletet.</li>
          <li>Mérgezés, tévedésből bevett gyógyszer vagy rosszullét esetén <strong>azonnal hívja a 112-t</strong>.</li>
          <li>A program nem tárolja a fotót és a beírt nevet; a keresés a Google-n keresztül történik.</li>
        </ul>
      </div>
    </div>
  </div>
</template>

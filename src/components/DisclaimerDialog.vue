<script setup>
import { nextTick, ref, watch } from "vue";

const props = defineProps({
  open: { type: Boolean, default: false },
  // Mit indít a felhasználó (pl. "Fotó küldése a Google Lens-nek").
  action: { type: String, default: "" },
});
const emit = defineEmits(["confirm", "cancel"]);

const cancelButton = ref(null);

watch(
  () => props.open,
  async (open) => {
    if (open) {
      await nextTick();
      cancelButton.value?.focus();
    }
  },
);
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/70 p-4"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="med-disclaimer-title"
    @keydown.esc="emit('cancel')"
  >
    <div class="w-full max-w-xl overflow-hidden rounded-3xl bg-white shadow-2xl">
      <div class="bg-red-600 px-6 py-4 text-white">
        <div id="med-disclaimer-title" class="text-xl font-extrabold">⚠ FIGYELEM – ez nem gyógyszerazonosítás</div>
      </div>
      <div class="grid gap-3 px-6 py-5 text-base leading-7 text-slate-900">
        <p v-if="action" class="rounded-xl bg-slate-100 px-4 py-2 text-sm font-semibold text-slate-700">Indítandó művelet: {{ action }}</p>
        <p class="font-bold">
          Ez az eszköz csak internetes keresést (Google / Google Lens) indít. NEM helyettesíti a szakszerű gyógyszerazonosítást, és nem orvosi vagy gyógyszerészi tanács.
        </p>
        <ul class="list-disc space-y-1 pl-6">
          <li>A keresés tévedhet: a hasonló nevű vagy kinézetű gyógyszerek összetéveszthetők.</li>
          <li class="font-bold">Ismeretlen gyógyszert NE vegyen be, és NE adjon be senkinek.</li>
          <li>Az azonosításhoz kérdezzen <strong>gyógyszerészt</strong>, <strong>háziorvost</strong> vagy a kezelőorvost.</li>
          <li>Mérgezés vagy rosszullét gyanúja esetén <strong>azonnal hívja a 112-es segélyhívót</strong>.</li>
          <li>A fotó vagy a beírt név a Google-hoz kerül. Ne szerepeljen a képen személyes adat (név, TAJ-szám, címke).</li>
        </ul>
      </div>
      <div class="flex flex-wrap justify-end gap-3 border-t border-slate-200 bg-slate-50 px-6 py-4">
        <button
          ref="cancelButton"
          class="rounded-full border border-slate-300 bg-white px-5 py-2.5 text-sm font-semibold text-slate-700 hover:bg-slate-100"
          @click="emit('cancel')"
        >Mégsem</button>
        <button
          class="rounded-full bg-red-600 px-6 py-2.5 text-sm font-bold text-white hover:bg-red-700"
          @click="emit('confirm')"
        >Megértettem, folytatom</button>
      </div>
    </div>
  </div>
</template>

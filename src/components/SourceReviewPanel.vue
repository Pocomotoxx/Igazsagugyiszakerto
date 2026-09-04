<script setup>
defineProps({
  draft: {
    type: Object,
    required: true,
  },
  source: {
    type: Object,
    required: true,
  },
});

defineEmits(["quick-review", "save-review"]);
</script>

<template>
  <div class="mt-4 grid gap-3 rounded-2xl border border-slate-200 bg-slate-50 p-4">
    <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Szakértői forrásértékelés</div>
    <div class="flex flex-wrap gap-2">
      <button
        @click="$emit('quick-review', { source_status: 'ellenorzes_alatt', priority_level: 1 })"
        class="rounded-full border border-amber-200 bg-amber-50 px-3 py-1.5 text-[11px] font-semibold text-amber-800 transition hover:bg-amber-100"
      >
        Gyors: ellenőrzés alatt + P1
      </button>
      <button
        @click="$emit('quick-review', { source_status: 'validalt', usability_decision: 'felhasznalhato' })"
        class="rounded-full border border-emerald-200 bg-emerald-50 px-3 py-1.5 text-[11px] font-semibold text-emerald-800 transition hover:bg-emerald-100"
      >
        Gyors: validált + felhasználható
      </button>
      <button
        @click="$emit('quick-review', { usability_decision: 'kizarando', priority_level: 3 })"
        class="rounded-full border border-rose-200 bg-rose-50 px-3 py-1.5 text-[11px] font-semibold text-rose-800 transition hover:bg-rose-100"
      >
        Gyors: kizárandó
      </button>
    </div>
    <div class="grid gap-3 md:grid-cols-3">
      <label class="grid gap-1.5 text-xs text-slate-700">
        <span class="font-semibold">Státusz</span>
        <select
          v-model="draft.source_status"
          class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
        >
          <option value="nyers">Nyers</option>
          <option value="ellenorzes_alatt">Ellenőrzés alatt</option>
          <option value="validalt">Validált</option>
        </select>
      </label>
      <label class="grid gap-1.5 text-xs text-slate-700">
        <span class="font-semibold">Prioritás</span>
        <select
          v-model.number="draft.priority_level"
          class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
        >
          <option :value="1">P1 - azonnali</option>
          <option :value="2">P2 - fontos</option>
          <option :value="3">P3 - háttér</option>
        </select>
      </label>
      <label class="grid gap-1.5 text-xs text-slate-700">
        <span class="font-semibold">Felhasználási döntés</span>
        <select
          v-model="draft.usability_decision"
          class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
        >
          <option value="felhasznalhato">Felhasználható</option>
          <option value="hatteranyag">Háttéranyag</option>
          <option value="kizarando">Kizárandó</option>
        </select>
      </label>
    </div>
    <label class="grid gap-1.5 text-xs text-slate-700">
      <span class="font-semibold">Mit bizonyít ez a forrás?</span>
      <textarea
        v-model="draft.proof_summary"
        rows="2"
        class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
        placeholder="Rövid bizonyítási kivonat: milyen tényt, összeget, szakmai állítást vagy kontrollt támaszt alá."
      ></textarea>
    </label>
    <div class="grid gap-3 md:grid-cols-2">
      <label class="grid gap-1.5 text-xs text-slate-700">
        <span class="font-semibold">Ellenőrző személy</span>
        <input
          v-model="draft.reviewer_name"
          type="text"
          class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
          placeholder="Szakértő vagy ellenőrző neve"
        />
      </label>
      <label class="grid gap-1.5 text-xs text-slate-700">
        <span class="font-semibold">Ellenőrzés ideje</span>
        <input
          v-model="draft.reviewed_at"
          type="text"
          class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
          placeholder="YYYY-MM-DD HH:MM:SS vagy hagyd üresen automatikus kitöltéshez"
        />
      </label>
    </div>
    <label class="grid gap-1.5 text-xs text-slate-700">
      <span class="font-semibold">Miért releváns?</span>
      <textarea
        v-model="draft.relevance_note"
        rows="2"
        class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
        placeholder="Röviden: miért fontos ez a forrás az ügy szempontjából."
      ></textarea>
    </label>
    <label class="grid gap-1.5 text-xs text-slate-700">
      <span class="font-semibold">Bizonyító erő</span>
      <textarea
        v-model="draft.probative_value"
        rows="2"
        class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
        placeholder="Mennyire erős vagy gyenge bizonyító erejű, és miért."
      ></textarea>
    </label>
    <label class="grid gap-1.5 text-xs text-slate-700">
      <span class="font-semibold">Mi hiányzik még?</span>
      <textarea
        v-model="draft.missing_points"
        rows="2"
        class="rounded-2xl border border-slate-200 bg-white px-3 py-2 outline-none"
        placeholder="Hiányzó adat, ellenőrizendő pont, szükséges kiegészítés."
      ></textarea>
    </label>
    <div class="flex justify-end">
      <button
        @click="$emit('save-review')"
        class="rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 transition hover:border-slate-300"
      >
        Forrásértékelés mentése
      </button>
    </div>
  </div>
</template>

<script setup>
defineProps({
  text: {
    type: String,
    required: true,
  },
});
</script>

<template>
  <span class="group relative inline-flex items-center align-middle">
    <span class="flex h-5 w-5 cursor-help items-center justify-center rounded-full border border-sky-200 bg-sky-50 text-[11px] font-bold text-sky-700">
      ?
    </span>
    <span class="pointer-events-none absolute left-1/2 top-full z-20 mt-2 hidden w-64 -translate-x-1/2 rounded-2xl border border-slate-200 bg-slate-950 px-3 py-2 text-[11px] leading-5 text-slate-100 shadow-2xl group-hover:block">
      {{ text }}
    </span>
  </span>
</template>

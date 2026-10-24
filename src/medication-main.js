import { createApp } from "vue";
import "./index.css";
import MedicationApp from "./MedicationApp.vue";

createApp(MedicationApp).mount("#app");

// PWA: offline alkalmazáshéj (csak https vagy localhost alatt, és nem a Tauri-héjban)
if ("serviceWorker" in navigator && !("__TAURI_INTERNALS__" in window)) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("/sw.js").catch(() => {
      // a szolgáltatás nélkül is működik az alkalmazás, csak offline nem érhető el
    });
  });
}

// A főablak a kiválasztott fotót átmenetileg a localStorage-ba teszi; ez az oldal azonnal
// kiolvassa és törli, majd űrlapként elküldi a Google Lens-nek. A kép nem marad a gépen.
(function () {
  var KEY = "irbr-lens-image";
  var msg = document.getElementById("msg");
  var data = null;
  try {
    data = localStorage.getItem(KEY);
    localStorage.removeItem(KEY);
  } catch (e) {
    data = null;
  }
  if (!data) {
    msg.textContent = "Nincs átadható fotó. Zárd be ezt a fület, és válassz képet a Gyógyszer-ellenőrzés oldalon.";
    return;
  }
  try {
    var comma = data.indexOf(",");
    var header = data.slice(0, comma);
    var type = (header.match(/data:([^;]+)/) || [])[1] || "image/jpeg";
    var binary = atob(data.slice(comma + 1));
    var bytes = new Uint8Array(binary.length);
    for (var i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
    var file = new File([bytes], "gyogyszer.jpg", { type: type });
    var transfer = new DataTransfer();
    transfer.items.add(file);
    document.getElementById("file").files = transfer.files;
    document.getElementById("f").submit();
  } catch (error) {
    msg.textContent = "A fotó átadása nem sikerült. Nyisd meg kézzel a Google Lens-t a Gyógyszer-ellenőrzés oldalon.";
  }
})();

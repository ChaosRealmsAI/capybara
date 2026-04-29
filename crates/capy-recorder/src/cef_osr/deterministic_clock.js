(() => {
  if (window.__nf_export_clock_installed) return;
  window.__nf_export_clock_installed = true;
  let now = 0;
  let rafId = 1;
  let rafQueue = [];
  const callbacks = new Map();
  Object.defineProperty(window, "__nf_external_now", {
configurable: true,
get() { return now; },
set(v) { now = Number(v) || 0; }
  });
  window.requestAnimationFrame = function(cb) {
const id = rafId++;
callbacks.set(id, cb);
rafQueue.push(id);
return id;
  };
  window.cancelAnimationFrame = function(id) {
callbacks.delete(id);
  };
  Date.now = function() { return Math.round(now); };
  try {
Object.defineProperty(window.performance, "now", {
  configurable: true,
  value: function() { return now; }
});
  } catch (_e) {}
  window.__nf_flush_raf = function(rounds) {
const count = Math.max(1, Number(rounds) || 1);
for (let pass = 0; pass < count; pass += 1) {
  const q = rafQueue;
  rafQueue = [];
  for (const id of q) {
    const cb = callbacks.get(id);
    callbacks.delete(id);
    if (typeof cb === "function") cb(now);
  }
}
  };
  window.__nf_sync_document_animations = function(t) {
try {
  const animations = document.getAnimations ? document.getAnimations({ subtree: true }) : [];
  for (const animation of animations) {
    try {
      animation.pause();
      animation.currentTime = Number(t) || 0;
    } catch (_e) {}
  }
} catch (_e) {}
  };
  window.__nf_export_prepare_frame = async function(t) {
const target = Number(t) || 0;
window.__nf_external_now = target;
window.__nf_sync_document_animations(target);
if (typeof window.__nf_seek_export === "function") {
  window.__nf_seek_export(target);
} else if (window.__nf && typeof window.__nf.seek === "function") {
  await window.__nf.seek(target);
} else {
  throw new Error("Timeline export seek bridge missing");
}
window.__nf_flush_raf(3);
window.__nf_sync_document_animations(target);
if (typeof window.__nf_wait_media_export === "function") {
  await window.__nf_wait_media_export(target);
}
window.__nf_flush_raf(1);
const raw = typeof window.__nf_read_seek_export === "function"
  ? window.__nf_read_seek_export()
  : JSON.stringify({ t: target, frameReady: true, seq: 1 });
const payload = JSON.parse(String(raw || "null"));
return payload || { t: target, frameReady: true, seq: 1 };
  };
})();

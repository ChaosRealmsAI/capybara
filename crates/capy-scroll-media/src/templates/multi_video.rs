pub fn multi_video_story_css() -> &'static str {
    r#":root{color-scheme:dark}*{box-sizing:border-box}.capy-story-page{margin:0;background:#080706;color:#f5efe5;font-family:ui-sans-serif,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}.capy-story-shell{min-height:calc(var(--chapter-count,3)*150vh);background:radial-gradient(circle at 72% 28%,rgba(176,130,74,.22),transparent 32%),linear-gradient(180deg,#0d0b08 0%,#17120d 54%,#090806 100%)}.capy-story-stage{position:sticky;top:0;min-height:100svh;overflow:hidden;isolation:isolate}.capy-story-video{position:absolute;inset:0;width:100%;height:100%;object-fit:cover;background:#080706;filter:saturate(1.05) contrast(1.04)}.capy-story-vignette{position:absolute;inset:0;z-index:2;background:linear-gradient(90deg,rgba(8,7,6,.88),rgba(8,7,6,.34) 44%,rgba(8,7,6,.16)),linear-gradient(180deg,rgba(8,7,6,.42),transparent 36%,rgba(8,7,6,.76));pointer-events:none}.capy-story-header{position:absolute;z-index:5;inset:0 0 auto;display:flex;align-items:center;justify-content:space-between;padding:28px clamp(22px,5vw,72px);font-size:12px;font-weight:800;letter-spacing:.13em;text-transform:uppercase;color:rgba(245,239,229,.88)}.capy-story-brand{display:flex;align-items:center;gap:12px}.capy-story-mark{display:grid;place-items:center;width:36px;height:36px;border:1px solid rgba(245,239,229,.46);border-radius:50%;font-family:Georgia,serif;letter-spacing:0}.capy-story-progress{opacity:.7}.capy-story-copy{position:absolute;z-index:4;left:clamp(22px,6vw,86px);bottom:clamp(42px,9vh,96px);width:min(580px,calc(100vw - 44px))}.capy-story-eyebrow{margin:0 0 16px;color:#c79b5d;font-size:12px;font-weight:800;letter-spacing:.16em;text-transform:uppercase}.capy-story-title{margin:0;font-family:Georgia,"Times New Roman",serif;font-weight:500;line-height:.96;font-size:clamp(56px,9vw,132px)}.capy-story-summary{max-width:520px;margin:22px 0 0;color:rgba(245,239,229,.76);font-size:clamp(17px,1.8vw,23px);line-height:1.56}.capy-story-chapters{position:absolute;z-index:6;right:clamp(20px,5vw,76px);top:50%;width:min(460px,38vw);transform:translateY(-50%)}.capy-story-chapter{position:absolute;inset:0;opacity:0;transform:translate3d(0,24px,0);filter:blur(3px);transition:opacity .42s ease,transform .5s cubic-bezier(.2,.85,.2,1),filter .42s ease;pointer-events:none}.capy-story-chapter.is-active{position:relative;opacity:1;transform:translate3d(0,0,0);filter:blur(0)}.capy-story-chapter.is-before{opacity:.08;transform:translate3d(0,-18px,0)}.capy-story-number{color:#c79b5d;font-family:Georgia,serif;font-size:clamp(54px,7vw,96px);line-height:.8}.capy-story-chapter h2{margin:0;font-family:Georgia,"Times New Roman",serif;font-weight:500;line-height:.98;font-size:clamp(40px,5vw,76px)}.capy-story-kicker{margin:18px 0 0;color:#d6ad76;font-size:12px;font-weight:800;letter-spacing:.15em;text-transform:uppercase}.capy-story-body{margin:16px 0 0;color:rgba(245,239,229,.75);font-size:clamp(16px,1.35vw,20px);line-height:1.62}.capy-story-rail{position:absolute;z-index:7;left:clamp(22px,5vw,72px);right:clamp(22px,5vw,72px);bottom:24px;height:1px;background:rgba(245,239,229,.18)}.capy-story-rail-fill{width:calc(var(--story-progress,0)*100%);height:100%;background:#c79b5d}.capy-story-error{min-height:100vh;display:grid;place-items:center;padding:24px;color:#f5efe5;background:#120f0b}@media(max-width:860px){.capy-story-shell{min-height:calc(var(--chapter-count,3)*130vh)}.capy-story-video{object-fit:cover}.capy-story-vignette{background:linear-gradient(180deg,rgba(8,7,6,.2),rgba(8,7,6,.32) 35%,rgba(8,7,6,.9))}.capy-story-header{padding:20px}.capy-story-progress{display:none}.capy-story-copy{left:20px;right:20px;bottom:56px;width:auto}.capy-story-title{font-size:clamp(48px,15vw,76px)}.capy-story-summary{font-size:17px}.capy-story-chapters{left:20px;right:20px;top:auto;bottom:122px;width:auto;transform:none}.capy-story-chapter{filter:none}.capy-story-chapter h2{font-size:clamp(34px,11vw,58px)}.capy-story-number{font-size:48px}.capy-story-body{font-size:16px}.capy-story-rail{left:20px;right:20px;bottom:18px}}@media(prefers-reduced-motion:reduce){.capy-story-chapter{transition:none}}"#
}

pub fn multi_video_story_js() -> &'static str {
    r#"(() => {
  const FPS_FALLBACK = 24;

  function clamp(value, min = 0, max = 1) {
    return Math.max(min, Math.min(max, value));
  }

  function rangesToArray(ranges) {
    return Array.from({ length: ranges.length }, (_, index) => [
      Number(ranges.start(index).toFixed(3)),
      Number(ranges.end(index).toFixed(3))
    ]);
  }

  function resolveUrl(base, path) {
    return new URL(path, base).toString();
  }

  function text(value) {
    return value == null ? "" : String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;");
  }

  function chapterHtml(chapter, index) {
    const number = String(index + 1).padStart(2, "0");
    return `<article class="capy-story-chapter" data-chapter="${chapter.id}">
      <div class="capy-story-number">${number}</div>
      <h2>${text(chapter.title)}</h2>
      <p class="capy-story-kicker">${text(chapter.kicker)}</p>
      <p class="capy-story-body">${text(chapter.body)}</p>
    </article>`;
  }

  async function init(root) {
    const manifestUrl = root.dataset.manifest;
    if (!manifestUrl) throw new Error("data-manifest is required");
    const manifestResponse = await fetch(manifestUrl);
    if (!manifestResponse.ok) throw new Error(`manifest request failed: ${manifestResponse.status}`);
    const manifest = await manifestResponse.json();
    const base = new URL(manifestUrl, location.href);
    const chapters = manifest.chapters || [];
    if (!chapters.length) throw new Error("manifest chapters are empty");

    root.style.setProperty("--chapter-count", String(chapters.length));
    root.classList.add("capy-story-shell");
    root.innerHTML = `<div class="capy-story-stage">
      <video class="capy-story-video" muted playsinline preload="auto" disablepictureinpicture></video>
      <div class="capy-story-vignette"></div>
      <header class="capy-story-header">
        <div class="capy-story-brand"><span class="capy-story-mark">CS</span><span>${text(manifest.eyebrow || "Scroll Story")}</span></div>
        <div class="capy-story-progress">00 / ${String(chapters.length).padStart(2, "0")}</div>
      </header>
      <div class="capy-story-copy">
        <p class="capy-story-eyebrow">${text(manifest.eyebrow)}</p>
        <h1 class="capy-story-title">${text(manifest.title)}</h1>
        <p class="capy-story-summary">${text(manifest.summary)}</p>
      </div>
      <div class="capy-story-chapters">${chapters.map(chapterHtml).join("")}</div>
      <div class="capy-story-rail"><div class="capy-story-rail-fill"></div></div>
    </div>`;

    const video = root.querySelector(".capy-story-video");
    const progressLabel = root.querySelector(".capy-story-progress");
    const chapterEls = Array.from(root.querySelectorAll(".capy-story-chapter"));
    const samples = [];
    let activeIndex = -1;
    let playhead = 0;
    let targetPlayhead = 0;
    let localProgress = 0;
    let raf = 0;
    let lastFrame = 0;
    let loadPromise = null;

    function clipFor(chapter) {
      const role = root.dataset.clip || "hq";
      if (role === "default") return chapter.default_clip;
      if (role === "fallback") return chapter.fallback_clip;
      if (role === "hq") return chapter.hq_clip;
      return role;
    }

    function state(extra = {}) {
      const active = chapters[activeIndex] || chapters[0];
      const quality = video.getVideoPlaybackQuality ? video.getVideoPlaybackQuality() : null;
      return {
        ready: true,
        title: manifest.title,
        chapterCount: chapters.length,
        activeIndex,
        activeClip: active ? active.id : null,
        playhead: Number(playhead.toFixed(4)),
        targetPlayhead: Number(targetPlayhead.toFixed(4)),
        localProgress: Number(localProgress.toFixed(4)),
        currentTime: Number(video.currentTime.toFixed(3)),
        duration: active ? Number(active.source.duration) : 0,
        videoSrc: video.dataset.src || "",
        seekable: rangesToArray(video.seekable),
        droppedVideoFrames: quality ? quality.droppedVideoFrames : null,
        totalVideoFrames: quality ? quality.totalVideoFrames : null,
        samples: samples.slice(-12),
        ...extra
      };
    }

    function publish(extra = {}) {
      window.__capyMultiVideoStory = state(extra);
      root.dispatchEvent(new CustomEvent("capy-multi-video-story:state", {
        detail: window.__capyMultiVideoStory,
        bubbles: true
      }));
    }

    function storyPlayhead() {
      const rect = root.getBoundingClientRect();
      const max = root.offsetHeight - innerHeight;
      return max > 0 ? clamp(-rect.top / max) : 0;
    }

    function setActive(index) {
      const nextIndex = Math.max(0, Math.min(chapters.length - 1, index));
      if (nextIndex === activeIndex && video.dataset.src) return;
      activeIndex = nextIndex;
      const chapter = chapters[activeIndex];
      chapterEls.forEach((el, itemIndex) => {
        el.classList.toggle("is-active", itemIndex === activeIndex);
        el.classList.toggle("is-before", itemIndex < activeIndex);
      });
      progressLabel.textContent = `${String(activeIndex + 1).padStart(2, "0")} / ${String(chapters.length).padStart(2, "0")}`;
      const src = resolveUrl(base, clipFor(chapter));
      video.poster = resolveUrl(base, chapter.poster);
      if (video.dataset.src === src) return;
      video.dataset.src = src;
      video.replaceChildren(Object.assign(document.createElement("source"), {
        src,
        type: "video/mp4"
      }));
      video.preload = "auto";
      lastFrame = 0;
      loadPromise = new Promise((resolve, reject) => {
        const done = () => {
          cleanup();
          video.pause();
          resolve(video);
        };
        const fail = () => {
          cleanup();
          reject(video.error || new Error("video-load-failed"));
        };
        const cleanup = () => {
          video.removeEventListener("loadedmetadata", done);
          video.removeEventListener("error", fail);
          loadPromise = null;
        };
        video.addEventListener("loadedmetadata", done);
        video.addEventListener("error", fail);
        video.load();
      });
      const pendingLoad = loadPromise;
      pendingLoad
        .then(() => {
          if (chapters[activeIndex] === chapter) seekActive(chapter, localProgress);
        })
        .catch((error) => publish({ error: String(error) }));
    }

    function seekActive(chapter, progress) {
      if (video.readyState < HTMLMediaElement.HAVE_METADATA) return;
      const fps = Number(chapter.source.fps || FPS_FALLBACK);
      const frames = Number(chapter.source.frame_count || Math.round(Number(chapter.source.duration || 0) * fps));
      const frame = Math.max(1, Math.min(frames, Math.round(progress * (frames - 1) + 1)));
      if (frame === lastFrame) return;
      const targetTime = clamp((frame - 1) / fps, 0, Number(chapter.source.duration || video.duration || 0));
      lastFrame = frame;
      const startedAt = performance.now();
      const record = (paintedMediaTime = video.currentTime) => {
        samples.push({
          clip: chapter.id,
          frame,
          targetTime: Number(targetTime.toFixed(3)),
          actualTime: Number(video.currentTime.toFixed(3)),
          paintedMediaTime: Number(paintedMediaTime.toFixed(3)),
          latencyMs: Number((performance.now() - startedAt).toFixed(1)),
          seekable: rangesToArray(video.seekable)
        });
        if (samples.length > 30) samples.shift();
        publish();
      };
      video.addEventListener("seeked", () => {
        if ("requestVideoFrameCallback" in video) {
          video.requestVideoFrameCallback((_, metadata) => record(metadata.mediaTime));
        } else {
          requestAnimationFrame(() => record());
        }
      }, { once: true });
      video.currentTime = targetTime;
    }

    function render(value) {
      playhead = clamp(value);
      root.style.setProperty("--story-progress", playhead.toFixed(4));
      const scaled = playhead * chapters.length;
      const index = playhead >= 1 ? chapters.length - 1 : Math.floor(scaled);
      localProgress = playhead >= 1 ? 1 : scaled - index;
      setActive(index);
      seekActive(chapters[activeIndex], localProgress);
      publish();
    }

    function tick() {
      const distance = targetPlayhead - playhead;
      if (Math.abs(distance) < 0.0008) {
        render(targetPlayhead);
        raf = 0;
        return;
      }
      render(playhead + distance * 0.24);
      raf = requestAnimationFrame(tick);
    }

    function setPlayhead(value, immediate = false) {
      targetPlayhead = clamp(value);
      if (immediate) {
        if (raf) cancelAnimationFrame(raf);
        raf = 0;
        render(targetPlayhead);
        return;
      }
      if (!raf) raf = requestAnimationFrame(tick);
    }

    window.addEventListener("scroll", () => setPlayhead(storyPlayhead()), { passive: true });
    window.addEventListener("resize", () => setPlayhead(storyPlayhead(), true));
    setActive(0);
    if (loadPromise) await loadPromise.catch(() => {});
    render(0);
    document.documentElement.dataset.capyMultiVideoStoryReady = "true";
  }

  window.CapyMultiVideoStory = { init };
  document.addEventListener("DOMContentLoaded", () => {
    document.querySelectorAll("[data-capy-multi-video-story]").forEach((root) => {
      init(root).catch((error) => {
        root.innerHTML = `<div class="capy-story-error">${String(error)}</div>`;
        root.dataset.capyError = String(error);
      });
    });
  });
})();"#
}

pub fn multi_video_story_html() -> &'static str {
    r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Capybara Multi-Video Scroll Story</title>
    <meta name="description" content="A multi-video scroll story package generated by Capybara." />
    <link rel="stylesheet" href="./runtime/multi-video-story.css" />
  </head>
  <body class="capy-story-page">
    <main data-capy-multi-video-story data-manifest="./manifest.json" data-clip="hq"></main>
    <script src="./runtime/multi-video-story.js"></script>
  </body>
</html>"#
}

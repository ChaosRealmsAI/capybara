pub fn runtime_css() -> &'static str {
    r#"*{box-sizing:border-box}[data-capy-scroll-video]{display:block;min-height:520vh}.capy-scroll-stage{position:sticky;top:0;width:100%;height:100vh;overflow:hidden;background:#fff}.capy-scroll-stage video{display:block;width:100%;height:100%;object-fit:contain;background:#fff}.capy-scroll-story[data-capy-fill="cover"] video{object-fit:cover}"#
}

pub fn runtime_js() -> &'static str {
    r#"(() => {
  const FPS_FALLBACK = 24;

  function clamp(value, min, max) {
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

  async function init(root) {
    const manifestUrl = root.dataset.manifest;
    if (!manifestUrl) throw new Error("data-manifest is required");
    const manifestResponse = await fetch(manifestUrl);
    if (!manifestResponse.ok) throw new Error(`manifest request failed: ${manifestResponse.status}`);
    const manifest = await manifestResponse.json();
    const base = new URL(manifestUrl, location.href);
    const story = root.closest(".capy-scroll-story") || root.parentElement || root;
    const stage = document.createElement("div");
    const video = document.createElement("video");
    const samples = [];
    let playhead = 0;
    let targetPlayhead = 0;
    let raf = 0;
    let lastFrame = 0;
    let loadPromise = null;

    stage.className = "capy-scroll-stage";
    video.muted = true;
    video.playsInline = true;
    video.preload = "none";
    video.poster = resolveUrl(base, manifest.poster);
    video.disablePictureInPicture = true;
    stage.append(video);
    root.replaceChildren(stage);
    root.__capyScrollVideo = { manifest, video, samples, load, setPlayhead };

    function state(extra = {}) {
      return {
        playhead,
        targetPlayhead,
        frame: Number(root.dataset.frame || "1"),
        currentTime: Number(video.currentTime.toFixed(3)),
        seekable: rangesToArray(video.seekable),
        samples: samples.slice(-12),
        ...extra
      };
    }

    function dispatch(name, extra) {
      root.dispatchEvent(new CustomEvent(name, { detail: state(extra), bubbles: true }));
    }

    function clipFor(value) {
      switch (value) {
        case "fallback":
          return manifest.fallback_clip;
        case "hq":
          return manifest.hq_clip;
        case "default":
        case "":
        case undefined:
        case null:
          return manifest.default_clip;
        default:
          return value;
      }
    }

    function load(src = root.dataset.clip || "default") {
      const clip = clipFor(src);
      const next = resolveUrl(base, clip);
      if (video.dataset.src === next && video.readyState >= HTMLMediaElement.HAVE_METADATA) {
        return Promise.resolve(video);
      }
      if (video.dataset.src === next && loadPromise) return loadPromise;
      const source = document.createElement("source");
      source.src = next;
      source.type = "video/mp4";
      video.replaceChildren(source);
      video.dataset.src = next;
      video.preload = "auto";
      loadPromise = new Promise((resolve, reject) => {
        const done = () => {
          cleanup();
          video.pause();
          dispatch("capy-scroll-video:loaded", { src: clip });
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
      return loadPromise;
    }

    function storyPlayhead() {
      const rect = story.getBoundingClientRect();
      const max = story.offsetHeight - innerHeight;
      return max > 0 ? clamp(-rect.top / max, 0, 1) : 0;
    }

    function record(frame, targetTime, startedAt) {
      const finish = (paintedMediaTime = video.currentTime) => {
        samples.push({
          frame,
          targetTime: Number(targetTime.toFixed(3)),
          actualTime: Number(video.currentTime.toFixed(3)),
          paintedMediaTime: Number(paintedMediaTime.toFixed(3)),
          latencyMs: Number((performance.now() - startedAt).toFixed(1)),
          seekable: rangesToArray(video.seekable)
        });
        if (samples.length > 30) samples.shift();
        dispatch("capy-scroll-video:seek", { sample: samples[samples.length - 1] });
      };
      if ("requestVideoFrameCallback" in video) {
        video.requestVideoFrameCallback((_, metadata) => finish(metadata.mediaTime));
      } else {
        requestAnimationFrame(() => finish());
      }
    }

    function render(value) {
      playhead = clamp(value, 0, 1);
      const fps = Number(manifest.fps || FPS_FALLBACK);
      const frames = Number(manifest.frame_count || Math.round(Number(manifest.duration || 0) * fps));
      const frame = Math.max(1, Math.min(frames, Math.round(playhead * (frames - 1) + 1)));
      root.dataset.playhead = playhead.toFixed(4);
      root.dataset.frame = String(frame);
      if (video.readyState < HTMLMediaElement.HAVE_METADATA) {
        if (frame > 1) load().catch((error) => dispatch("capy-scroll-video:error", { error: String(error) }));
        return;
      }
      if (frame === lastFrame) return;
      const targetTime = clamp((frame - 1) / fps, 0, Number(manifest.duration || video.duration || 0));
      lastFrame = frame;
      const startedAt = performance.now();
      video.addEventListener("seeked", () => record(frame, targetTime, startedAt), { once: true });
      video.currentTime = targetTime;
      dispatch("capy-scroll-video:state", {});
    }

    function tick() {
      const distance = targetPlayhead - playhead;
      if (Math.abs(distance) < 0.0008) {
        render(targetPlayhead);
        raf = 0;
        return;
      }
      render(playhead + distance * 0.26);
      raf = requestAnimationFrame(tick);
    }

    function setPlayhead(value, immediate = false) {
      targetPlayhead = clamp(value, 0, 1);
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
    if ("IntersectionObserver" in window) {
      const observer = new IntersectionObserver((entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          observer.disconnect();
          load().catch((error) => dispatch("capy-scroll-video:error", { error: String(error) }));
        }
      }, { threshold: 0.08 });
      observer.observe(story);
    }
    setTimeout(() => load().catch((error) => dispatch("capy-scroll-video:error", { error: String(error) })), 900);
    render(0);
  }

  window.CapyScrollVideo = { init };
  document.addEventListener("DOMContentLoaded", () => {
    document.querySelectorAll("[data-capy-scroll-video]").forEach((root) => {
      init(root).catch((error) => {
        root.dataset.capyError = String(error);
        root.dispatchEvent(new CustomEvent("capy-scroll-video:error", {
          detail: { error: String(error) },
          bubbles: true
        }));
      });
    });
  });
})();"#
}

pub fn demo_html() -> &'static str {
    r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Capy Scroll Media Demo</title>
    <link rel="icon" href="data:," />
    <link rel="stylesheet" href="runtime/scroll-video.css" />
    <style>
      body{margin:0;background:#fff;color:#18181b;font-family:-apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif}
      .hero,.outro{min-height:80vh;display:grid;align-items:center;width:min(1080px,calc(100% - 40px));margin:0 auto}
      h1{font-size:clamp(42px,7vw,88px);line-height:1;margin:0}
      p{font-size:20px;line-height:1.6;color:#52525b;max-width:680px}
    </style>
  </head>
  <body>
    <section class="hero">
      <div>
        <h1>Capy Scroll Media</h1>
        <p>Scroll down. The video below is driven by a normalized playhead and all-keyframe MP4.</p>
      </div>
    </section>
    <section class="capy-scroll-story">
      <div data-capy-scroll-video data-manifest="manifest.json"></div>
    </section>
    <section class="outro">
      <p>Use this package from any static HTML page that can serve MP4 byte ranges.</p>
    </section>
    <script src="runtime/scroll-video.js"></script>
  </body>
</html>"#
}

pub fn scroll_hq_html() -> &'static str {
    r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Capy Scroll Media HQ</title>
    <link rel="icon" href="data:," />
    <link rel="stylesheet" href="runtime/scroll-video.css" />
    <style>
      html,body{margin:0;background:#fff;overscroll-behavior:none}.capy-scroll-story{margin:0;padding:0}
    </style>
  </head>
  <body>
    <main class="capy-scroll-story"><div data-capy-scroll-video data-manifest="manifest.json" data-clip="hq"></div></main>
    <script src="runtime/scroll-video.js"></script>
  </body>
</html>"#
}

pub fn raw_quality_html() -> &'static str {
    r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Raw Quality Check</title>
    <link rel="icon" href="data:," />
    <style>
      html,body{margin:0;min-height:100%;background:#fff}body{overscroll-behavior:none}.scrub{height:520vh}.stage{position:sticky;top:0;width:100%;height:100vh;overflow:hidden;background:#fff}video{display:block;width:100%;height:100%;object-fit:contain;background:#fff}
    </style>
  </head>
  <body data-playhead="0" data-frame="1">
    <main class="scrub" data-scrub>
      <div class="stage">
        <video data-video muted playsinline preload="auto" disablepictureinpicture></video>
      </div>
    </main>
    <script>
      const video=document.querySelector("[data-video]");const scrub=document.querySelector("[data-scrub]");let manifest={fps:24,frame_count:1,duration:1,hq_clip:""};let playhead=0;let targetPlayhead=0;let raf=0;let lastFrame=0;const seekSamples=[];window.__rawQuality={seekSamples,overlays:false};function clamp(v,min,max){return Math.max(min,Math.min(max,v))}function progress(){const rect=scrub.getBoundingClientRect();const max=scrub.offsetHeight-innerHeight;return max>0?clamp(-rect.top/max,0,1):0}function render(value){playhead=clamp(value,0,1);const fps=Number(manifest.fps||24);const frames=Number(manifest.frame_count||Math.round(Number(manifest.duration||0)*fps));const frame=Math.max(1,Math.min(frames,Math.round(playhead*(frames-1)+1)));document.body.dataset.playhead=playhead.toFixed(4);document.body.dataset.frame=String(frame);if(video.readyState<HTMLMediaElement.HAVE_METADATA||frame===lastFrame)return;lastFrame=frame;const target=clamp((frame-1)/fps,0,Number(manifest.duration||video.duration||0));const started=performance.now();video.addEventListener("seeked",()=>{const done=(mediaTime=video.currentTime)=>{seekSamples.push({frame,targetTime:Number(target.toFixed(3)),actualTime:Number(video.currentTime.toFixed(3)),paintedMediaTime:Number(mediaTime.toFixed(3)),latencyMs:Number((performance.now()-started).toFixed(1))});if(seekSamples.length>24)seekSamples.shift()};if("requestVideoFrameCallback"in video){video.requestVideoFrameCallback((_,metadata)=>done(metadata.mediaTime))}else{requestAnimationFrame(()=>done())}},{once:true});video.currentTime=target}function tick(){const distance=targetPlayhead-playhead;if(Math.abs(distance)<0.0008){render(targetPlayhead);raf=0;return}render(playhead+distance*0.28);raf=requestAnimationFrame(tick)}function setTarget(value){targetPlayhead=clamp(value,0,1);if(!raf)raf=requestAnimationFrame(tick)}video.addEventListener("loadedmetadata",()=>{video.pause();render(0)});addEventListener("scroll",()=>setTarget(progress()),{passive:true});addEventListener("resize",()=>{targetPlayhead=progress();render(targetPlayhead)});fetch("manifest.json").then((res)=>res.json()).then((data)=>{manifest=data;video.poster=data.poster;const source=document.createElement("source");source.src=data.hq_clip;source.type="video/mp4";video.append(source);video.load();window.__rawQuality.manifest=data;render(0)});
    </script>
  </body>
</html>"#
}

(() => {
  const story = document.querySelector(".assembly-story");
  const hero = document.querySelector(".hero");
  const layers = [...document.querySelectorAll(".watch-layer")];
  const chapters = [...document.querySelectorAll(".chapter")];

  if (!story || !hero || layers.length === 0 || chapters.length === 0) {
    return;
  }

  const clamp = (value, min = 0, max = 1) => Math.min(max, Math.max(min, value));
  const mix = (from, to, progress) => from + (to - from) * progress;
  const smoothstep = (progress) => progress * progress * (3 - 2 * progress);

  const layerMotion = [
    { fromX: -54, fromY: 16, toX: 0, toY: 0, fromScale: 1.04, toScale: 1, fromRotate: -13, toRotate: 0 },
    { fromX: 56, fromY: -18, toX: 0, toY: 0, fromScale: 0.9, toScale: 1, fromRotate: 18, toRotate: 0 },
    { fromX: -44, fromY: 49, toX: 0, toY: 0, fromScale: 0.82, toScale: 1, fromRotate: -22, toRotate: 0 },
    { fromX: 44, fromY: 43, toX: 0, toY: 0, fromScale: 0.82, toScale: 1, fromRotate: 12, toRotate: 0 },
    { fromX: 0, fromY: -58, toX: 0, toY: 0, fromScale: 1.16, toScale: 1, fromRotate: 9, toRotate: 0 },
    { fromX: 0, fromY: 62, toX: 0, toY: 0, fromScale: 0.86, toScale: 1, fromRotate: -34, toRotate: 0 },
  ];

  let targetProgress = 0;
  let currentProgress = 0;
  const targetLayerProgress = Array(layers.length).fill(0);
  const currentLayerProgress = Array(layers.length).fill(0);
  let activeChapter = -1;
  let rafId = 0;
  let reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const layerState = [];

  layers.forEach((layer, index) => {
    layer.style.setProperty("--layer-index", String(index));
  });

  function progressForLayer(progress) {
    if (reducedMotion) {
      return 1;
    }
    return smoothstep(clamp(progress));
  }

  function setLayerTransform(layer, motion, progress) {
    const layerProgress = progressForLayer(progress);
    const x = mix(motion.fromX, motion.toX, layerProgress);
    const y = mix(motion.fromY, motion.toY, layerProgress);
    const scale = mix(motion.fromScale, motion.toScale, layerProgress);
    const rotate = mix(motion.fromRotate, motion.toRotate, layerProgress);
    const opacity = reducedMotion ? 1 : clamp(0.2 + layerProgress * 0.8);

    layer.style.opacity = opacity.toFixed(3);
    layer.style.transform = `translate(-50%, -50%) translate3d(${x.toFixed(3)}%, ${y.toFixed(3)}%, 0) scale(${scale.toFixed(4)}) rotate(${rotate.toFixed(3)}deg)`;

    layerState[Number(layer.dataset.layer)] = {
      x: Number(x.toFixed(3)),
      y: Number(y.toFixed(3)),
      scale: Number(scale.toFixed(4)),
      rotate: Number(rotate.toFixed(3)),
      opacity: Number(opacity.toFixed(3)),
      progress: Number(layerProgress.toFixed(4)),
    };
  }

  function setActiveChapter(index) {
    if (index === activeChapter) {
      return;
    }
    activeChapter = index;
    chapters.forEach((chapter, chapterIndex) => {
      chapter.classList.toggle("is-active", chapterIndex === index);
      chapter.classList.toggle("is-before", chapterIndex < index);
    });
  }

  function measureTargetProgress() {
    const rect = story.getBoundingClientRect();
    const scrollable = Math.max(1, rect.height - window.innerHeight);
    targetProgress = clamp(-rect.top / scrollable);

    layers.forEach((_, index) => {
      const chapter = chapters[index];
      const chapterRect = chapter.getBoundingClientRect();
      const startLine = window.innerHeight * (window.innerWidth < 900 ? 0.78 : 0.82);
      const travel = window.innerHeight * (window.innerWidth < 900 ? 0.72 : 0.62);
      targetLayerProgress[index] = clamp((startLine - chapterRect.top) / travel);
    });

    const heroRect = hero.getBoundingClientRect();
    const heroProgress = clamp(-heroRect.top / Math.max(1, heroRect.height));
    document.documentElement.style.setProperty("--hero-parallax", `${(heroProgress * 52).toFixed(2)}px`);

    if (rect.top > window.innerHeight * 0.2) {
      setActiveChapter(0);
      return;
    }

    const activeLine = window.innerWidth < 900 ? window.innerHeight * 0.66 : window.innerHeight * 0.52;
    let nearest = 0;
    let nearestDistance = Number.POSITIVE_INFINITY;
    chapters.forEach((chapter, index) => {
      const chapterRect = chapter.getBoundingClientRect();
      const chapterCenter = chapterRect.top + chapterRect.height * 0.5;
      const distance = Math.abs(chapterCenter - activeLine);
      if (distance < nearestDistance) {
        nearest = index;
        nearestDistance = distance;
      }
    });
    setActiveChapter(nearest);
  }

  function publishState() {
    window.__watchmakerState = {
      ready: document.documentElement.dataset.watchmakerReady === "true",
      activeChapter,
      targetProgress: Number(targetProgress.toFixed(4)),
      currentProgress: Number(currentProgress.toFixed(4)),
      layerCount: layers.length,
      chapterCount: chapters.length,
      layers: layerState,
    };
  }

  function render() {
    const ease = reducedMotion ? 1 : 0.16;
    currentProgress += (targetProgress - currentProgress) * ease;
    let maxLayerDiff = 0;
    currentLayerProgress.forEach((progress, index) => {
      currentLayerProgress[index] = progress + (targetLayerProgress[index] - progress) * ease;
      if (Math.abs(currentLayerProgress[index] - targetLayerProgress[index]) < 0.0008) {
        currentLayerProgress[index] = targetLayerProgress[index];
      }
      maxLayerDiff = Math.max(maxLayerDiff, Math.abs(currentLayerProgress[index] - targetLayerProgress[index]));
    });
    if (Math.abs(currentProgress - targetProgress) < 0.0008) {
      currentProgress = targetProgress;
    }

    document.documentElement.style.setProperty("--assembly", currentProgress.toFixed(4));
    layers.forEach((layer, index) => {
      setLayerTransform(layer, layerMotion[index], currentLayerProgress[index]);
    });
    publishState();

    if (Math.abs(currentProgress - targetProgress) > 0.0008 || maxLayerDiff > 0.0008) {
      rafId = window.requestAnimationFrame(render);
    } else {
      rafId = 0;
    }
  }

  function scheduleRender() {
    measureTargetProgress();
    if (!rafId) {
      rafId = window.requestAnimationFrame(render);
    }
  }

  function markReady() {
    const waits = layers.map((img) => {
      if (img.complete && img.naturalWidth > 0) {
        return Promise.resolve();
      }
      if (img.decode) {
        return img.decode().catch(() => {});
      }
      return new Promise((resolve) => img.addEventListener("load", resolve, { once: true }));
    });

    Promise.all(waits).then(() => {
      document.documentElement.dataset.watchmakerReady = "true";
      scheduleRender();
    });
  }

  window.addEventListener("scroll", scheduleRender, { passive: true });
  window.addEventListener("resize", scheduleRender);
  window.matchMedia("(prefers-reduced-motion: reduce)").addEventListener("change", (event) => {
    reducedMotion = event.matches;
    scheduleRender();
  });

  measureTargetProgress();
  render();
  markReady();
})();

const heroSvg = `
<svg width="720" height="720" viewBox="0 0 720 720" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <radialGradient id="cup" cx="32%" cy="18%" r="82%">
      <stop stop-color="#fff7ed"/>
      <stop offset=".48" stop-color="#e7d2b4"/>
      <stop offset="1" stop-color="#8b5e34"/>
    </radialGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="28" stdDeviation="24" flood-color="#5a3219" flood-opacity=".26"/>
    </filter>
  </defs>
  <ellipse cx="360" cy="612" rx="210" ry="38" fill="#5a3219" opacity=".16"/>
  <g filter="url(#shadow)">
    <path d="M168 270c0-70 57-126 126-126h154c70 0 126 57 126 126v112c0 102-83 185-185 185h-36c-102 0-185-83-185-185V270z" fill="url(#cup)"/>
    <path d="M548 316h26c52 0 94 42 94 94s-42 94-94 94h-34v-50h32c24 0 44-20 44-44s-20-44-44-44h-24v-50z" fill="#c7a073"/>
    <ellipse cx="371" cy="270" rx="190" ry="64" fill="#f7ead6"/>
    <ellipse cx="371" cy="270" rx="136" ry="32" fill="#715b48" opacity=".34"/>
  </g>
  <g fill="#5b351d" opacity=".82">
    <circle cx="246" cy="330" r="7"/>
    <circle cx="326" cy="382" r="5"/>
    <circle cx="456" cy="356" r="6"/>
    <circle cx="414" cy="458" r="7"/>
    <circle cx="284" cy="492" r="5"/>
  </g>
</svg>`;

const logoSvg = `
<svg width="160" height="160" viewBox="0 0 160 160" xmlns="http://www.w3.org/2000/svg">
  <circle cx="80" cy="80" r="72" fill="#1c1917"/>
  <path d="M45 86c8-31 36-50 68-42 18 5 30 18 35 33-9-12-22-20-40-22-30-3-54 10-63 31z" fill="#fdba74"/>
  <path d="M44 96h72c-7 22-26 38-50 38-14 0-25-4-34-12l12-26z" fill="#a78bfa"/>
</svg>`;

export const DEFAULT_POSTER_DOCUMENT = {
  version: "capy-poster-v0.1",
  type: "poster",
  title: "Ceramic Morning Poster",
  canvas: {
    width: 1920,
    height: 1080,
    aspectRatio: "16:9",
    background: "#f6f1e8"
  },
  assets: {
    hero_product: {
      type: "image",
      src: svgDataUri(heroSvg),
      mask: "assets/hero_product.mask.png",
      provenance: {
        model: "gpt-image-2",
        prompt: "Handmade ceramic cup product hero, warm daylight, clean poster-safe composition.",
        resolution: "4k",
        size: "16:9",
        refs: ["asset://moodboard/ceramic-craft"],
        task_id: "task_demo_poster_001",
        output_pixels: [3840, 2160],
        created_at: "2026-04-28T00:00:00Z"
      }
    },
    logo_mark: {
      type: "svg",
      src: svgDataUri(logoSvg)
    }
  },
  layers: [
    {
      id: "wash",
      type: "shape",
      shape: "rect",
      x: 0,
      y: 0,
      width: 1920,
      height: 1080,
      z: 0,
      style: {
        fill: "linear-gradient(135deg, #fffaf0 0%, #fff0d8 48%, #ede9fe 100%)"
      }
    },
    {
      id: "accent_blob",
      type: "shape",
      shape: "ellipse",
      x: 980,
      y: 70,
      width: 660,
      height: 660,
      z: 1,
      style: {
        fill: "radial-gradient(circle, rgba(167,139,250,.58), rgba(253,186,116,.16) 58%, transparent 72%)"
      }
    },
    {
      id: "logo",
      type: "image",
      assetId: "logo_mark",
      x: 118,
      y: 104,
      width: 82,
      height: 82,
      z: 4
    },
    {
      id: "headline",
      type: "text",
      text: "CERAMIC\nMORNING",
      x: 118,
      y: 240,
      width: 720,
      height: 250,
      z: 5,
      style: {
        fontFamily: "PingFang SC, Source Han Sans CN, sans-serif",
        fontSize: 118,
        fontWeight: 900,
        color: "#1c1917"
      }
    },
    {
      id: "subhead",
      type: "text",
      text: "Local-first poster document: JSON source, HTML renderer, canvas preview.",
      x: 126,
      y: 540,
      width: 590,
      height: 92,
      z: 5,
      style: {
        fontFamily: "PingFang SC, Source Han Sans CN, sans-serif",
        fontSize: 34,
        fontWeight: 600,
        color: "#57534e"
      }
    },
    {
      id: "hero_product",
      type: "image",
      assetId: "hero_product",
      x: 965,
      y: 238,
      width: 640,
      height: 640,
      z: 6
    },
    {
      id: "cta",
      type: "shape",
      shape: "rect",
      x: 124,
      y: 724,
      width: 360,
      height: 78,
      z: 7,
      style: {
        fill: "#1c1917",
        radius: 39
      }
    },
    {
      id: "cta_text",
      type: "text",
      text: "EXPORT 4K",
      x: 176,
      y: 742,
      width: 250,
      height: 42,
      z: 8,
      style: {
        fontFamily: "SF Pro Display, PingFang SC, sans-serif",
        fontSize: 35,
        fontWeight: 900,
        color: "#fffaf0"
      }
    }
  ]
};

export function cloneDefaultPosterDocument() {
  return cloneDocument(DEFAULT_POSTER_DOCUMENT);
}

export function cloneDocument(document) {
  return JSON.parse(JSON.stringify(document));
}

export function parsePosterDocument(input) {
  const document = typeof input === "string" ? JSON.parse(input) : input;
  return cloneDocument(document);
}

export function validatePosterDocument(document) {
  if (!document || document.type !== "poster") {
    throw new Error("Poster document type must be poster.");
  }
  if (!document.canvas || !positiveNumber(document.canvas.width) || !positiveNumber(document.canvas.height)) {
    throw new Error("Poster document requires positive canvas width and height.");
  }
  if (!document.assets || typeof document.assets !== "object") {
    throw new Error("Poster document requires assets object.");
  }
  if (!Array.isArray(document.layers) || document.layers.length === 0) {
    throw new Error("Poster document requires at least one layer.");
  }
  for (const layer of document.layers) {
    if (!layer.id || !layer.type) {
      throw new Error("Every poster layer requires id and type.");
    }
    if (layer.type === "image" && !document.assets[layer.assetId]) {
      throw new Error(`Missing asset for image layer ${layer.id}.`);
    }
    if (!positiveNumber(layer.width) || !positiveNumber(layer.height)) {
      throw new Error(`Poster layer ${layer.id} requires positive width and height.`);
    }
  }
}

export function renderPosterStage(document, options = {}) {
  validatePosterDocument(document);
  const scale = Number(options.scale) || 1;
  const selectedLayerId = options.selectedLayerId || null;
  const element = globalThis.document.createElement("div");
  element.className = "poster-stage";
  element.dataset.documentVersion = document.version || "unknown";
  element.dataset.layerCount = String(document.layers.length);
  element.style.setProperty("--poster-bg", document.canvas.background || "#f6f1e8");

  document.layers
    .slice()
    .sort((a, b) => (Number(a.z) || 0) - (Number(b.z) || 0))
    .forEach((layer) => element.append(createLayerElement(layer, document, scale, selectedLayerId)));

  return element;
}

export function buildPosterState(document, renderState, error = null) {
  const state = {
    render_state: renderState,
    version: document?.version || null,
    title: document?.title || null,
    canvas: document?.canvas || null,
    layer_count: Array.isArray(document?.layers) ? document.layers.length : 0,
    layers: Array.isArray(document?.layers)
      ? document.layers.map((layer) => ({
        id: layer.id,
        type: layer.type,
        assetId: layer.assetId || null,
        text: layer.type === "text" ? layer.text || "" : null
      }))
      : [],
    generated_assets: Object.entries(document?.assets || {})
      .filter(([, asset]) => asset?.provenance)
      .map(([id, asset]) => ({
        id,
        ...asset.provenance
      }))
  };
  if (error) state.error = error;
  return state;
}

function createLayerElement(layer, document, scale, selectedLayerId) {
  const element = globalThis.document.createElement("div");
  element.className = "poster-layer";
  element.dataset.layerId = layer.id;
  element.dataset.kind = layer.type;
  if (layer.assetId) element.dataset.assetId = layer.assetId;
  if (layer.id === selectedLayerId) element.classList.add("is-selected");
  element.style.zIndex = String(Number(layer.z) || 0);
  Object.assign(element.style, scaleRect(layer, document.canvas));
  applyLayerStyle(element, layer, scale);

  if (layer.type === "text") {
    element.textContent = layer.text || "";
  } else if (layer.type === "image") {
    const asset = document.assets[layer.assetId];
    const image = globalThis.document.createElement("img");
    image.src = asset.src;
    image.alt = layer.id;
    element.append(image);
  } else if (layer.type === "shape" && layer.shape === "ellipse") {
    element.style.borderRadius = "50%";
  }

  return element;
}

function applyLayerStyle(element, layer, scale) {
  const style = layer.style || {};
  if (style.color) element.style.color = style.color;
  if (style.fontFamily) element.style.fontFamily = style.fontFamily;
  if (style.fontSize) element.style.fontSize = `${Math.max(8, Number(style.fontSize) * scale)}px`;
  if (style.fontWeight) element.style.fontWeight = style.fontWeight;
  if (style.fill) element.style.background = style.fill;
  if (style.radius !== undefined) {
    element.style.borderRadius = `${Math.max(0, Number(style.radius) * scale)}px`;
  }
  if (style.blur) {
    element.style.filter = `blur(${Math.max(0, Number(style.blur) * scale)}px)`;
  }
}

function scaleRect(layer, canvas) {
  return {
    left: `${(Number(layer.x) / canvas.width) * 100}%`,
    top: `${(Number(layer.y) / canvas.height) * 100}%`,
    width: `${(Number(layer.width) / canvas.width) * 100}%`,
    height: `${(Number(layer.height) / canvas.height) * 100}%`
  };
}

function positiveNumber(value) {
  return Number.isFinite(Number(value)) && Number(value) > 0;
}

function svgDataUri(svg) {
  return `data:image/svg+xml,${encodeURIComponent(svg.trim())}`;
}

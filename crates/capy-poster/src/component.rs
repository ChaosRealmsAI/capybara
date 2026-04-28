pub const POSTER_COMPONENT_JS: &str = r##"export function mount(root) {
  root.textContent = "";
}

export function update(root, ctx) {
  const poster = ctx && ctx.params ? ctx.params.poster : null;
  if (!poster || !poster.canvas || !Array.isArray(poster.layers)) {
    root.textContent = "";
    root.dataset.renderState = "error";
    return;
  }
  root.dataset.renderState = "ready";
  root.dataset.capyPosterComponent = "capy.poster-document";
  root.style.position = "absolute";
  root.style.inset = "0";
  root.style.overflow = "hidden";
  root.style.background = poster.canvas.background || "#fff";

  const stage = document.createElement("div");
  stage.className = "capy-poster-stage";
  stage.dataset.posterVersion = String(poster.version || "");
  stage.style.position = "absolute";
  stage.style.inset = "0";
  stage.style.width = "100%";
  stage.style.height = "100%";
  stage.style.overflow = "hidden";
  stage.style.background = poster.canvas.background || "#fff";

  const layers = poster.layers.slice().sort((a, b) => Number(a.z || 0) - Number(b.z || 0));
  for (const layer of layers) {
    stage.appendChild(createLayer(layer, poster));
  }
  root.replaceChildren(stage);
}

function createLayer(layer, poster) {
  const element = document.createElement("div");
  element.className = "capy-poster-layer";
  element.dataset.layerId = String(layer.id || "");
  element.dataset.kind = String(layer.type || "");
  element.style.position = "absolute";
  element.style.left = px(layer.x);
  element.style.top = px(layer.y);
  element.style.width = px(layer.width);
  element.style.height = px(layer.height);
  element.style.zIndex = String(Number(layer.z || 0));
  element.style.boxSizing = "border-box";
  element.style.overflow = "hidden";
  element.style.pointerEvents = "none";
  applyStyle(element, layer.style || {}, layer);

  if (layer.type === "text") {
    element.textContent = String(layer.text || "");
    element.style.whiteSpace = "pre-line";
    element.style.display = "flex";
    element.style.alignItems = "flex-start";
    element.style.justifyContent = "flex-start";
    element.style.lineHeight = value(layer.style && layer.style.lineHeight, "1.05");
  } else if (layer.type === "image") {
    const asset = poster.assets && poster.assets[layer.assetId];
    const img = document.createElement("img");
    img.src = asset && asset.src ? String(asset.src) : "";
    img.alt = String(layer.id || "");
    img.style.width = "100%";
    img.style.height = "100%";
    img.style.objectFit = "contain";
    img.style.display = "block";
    element.appendChild(img);
  } else if (layer.type === "shape") {
    if ((layer.shape || "rect") === "ellipse") {
      element.style.borderRadius = "50%";
    }
  }
  return element;
}

function applyStyle(element, style, layer) {
  if (style.fill) element.style.background = String(style.fill);
  if (style.color) element.style.color = String(style.color);
  if (style.fontFamily) element.style.fontFamily = String(style.fontFamily);
  if (style.fontSize) element.style.fontSize = px(style.fontSize);
  if (style.fontWeight) element.style.fontWeight = String(style.fontWeight);
  if (style.opacity !== undefined) element.style.opacity = String(style.opacity);
  if (style.radius !== undefined) element.style.borderRadius = px(style.radius);
  if (style.blur !== undefined && Number(style.blur) > 0) {
    element.style.filter = "blur(" + px(style.blur) + ")";
  }
  if (layer.shape === "ellipse") element.style.borderRadius = "50%";
}

function px(value) {
  const number = Number(value || 0);
  return (Number.isFinite(number) ? number : 0) + "px";
}

function value(raw, fallback) {
  return raw === undefined || raw === null ? fallback : String(raw);
}
"##;

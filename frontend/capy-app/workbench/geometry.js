export function nodeBounds(node) {
  return node?.bounds || node?.geometry || null;
}

export function worldBoxToScreen(bounds, viewport) {
  const zoom = Number(viewport?.zoom) || 1;
  const offset = viewport?.camera_offset || { x: 0, y: 0 };
  return {
    x: Math.round(bounds.x * zoom + (Number(offset.x) || 0)),
    y: Math.round(bounds.y * zoom + (Number(offset.y) || 0)),
    w: Math.round(bounds.w * zoom),
    h: Math.round(bounds.h * zoom)
  };
}

export function clampRectToBounds(rect, bounds) {
  if (!rect || !bounds) return null;
  const x1 = Math.max(bounds.x, rect.x);
  const y1 = Math.max(bounds.y, rect.y);
  const x2 = Math.min(bounds.x + bounds.w, rect.x + rect.w);
  const y2 = Math.min(bounds.y + bounds.h, rect.y + rect.h);
  if (x2 <= x1 || y2 <= y1) return null;
  return { x: x1, y: y1, w: x2 - x1, h: y2 - y1 };
}

export function normalizeRect(x, y, w, h) {
  const nextX = Number(x) || 0;
  const nextY = Number(y) || 0;
  const nextW = Number(w) || 0;
  const nextH = Number(h) || 0;
  return {
    x: nextW < 0 ? nextX + nextW : nextX,
    y: nextH < 0 ? nextY + nextH : nextY,
    w: Math.abs(nextW),
    h: Math.abs(nextH)
  };
}

export function roundGeometry(geometry) {
  return {
    x: round2(geometry.x),
    y: round2(geometry.y),
    w: round2(geometry.w),
    h: round2(geometry.h)
  };
}

export function regionPercent(region, bounds) {
  if (!region || !bounds || !bounds.w || !bounds.h) return null;
  return {
    x: round4((region.x - bounds.x) / bounds.w),
    y: round4((region.y - bounds.y) / bounds.h),
    w: round4(region.w / bounds.w),
    h: round4(region.h / bounds.h)
  };
}

export function compactGeometry(geometry) {
  return [geometry.x, geometry.y, geometry.w, geometry.h].map((value) => Math.round(Number(value) || 0)).join("-");
}

function round2(value) {
  return Math.round((Number(value) || 0) * 100) / 100;
}

function round4(value) {
  return Math.round((Number(value) || 0) * 10000) / 10000;
}

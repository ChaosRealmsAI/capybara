export function base64ToBytes(base64) {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export function contentKindLabel(value) {
  if (value === "poster") return "poster";
  return String(value || "shape").replace(/_/g, " ");
}

export function normalizeValue(value) {
  if (value === null || value === undefined) return value;
  if (typeof value === "bigint") return Number(value);
  if (Array.isArray(value)) return value.map(normalizeValue);
  if (typeof value === "object") {
    const normalized = {};
    for (const [key, inner] of Object.entries(value)) normalized[key] = normalizeValue(inner);
    return normalized;
  }
  return value;
}

export function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

export function stringifyError(error) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.stack || error.message;
  try {
    return JSON.stringify(error, null, 2);
  } catch (_err) {
    return String(error);
  }
}

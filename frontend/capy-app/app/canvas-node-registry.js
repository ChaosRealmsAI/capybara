export function createCanvasNodeRegistry({ nodeRegistry, rpc }) {
  return function registerCanvasNodes(nodes) {
    const ids = nodes
      .map((node) => Number(node?.id))
      .filter((id) => Number.isFinite(id) && id >= 0)
      .sort((a, b) => a - b);
    const key = ids.join(",");
    if (!ids.length || key === nodeRegistry.key) return;
    nodeRegistry.key = key;
    rpc("canvas-nodes-register", { ids }).catch(() => {
      nodeRegistry.key = "";
    });
  };
}

export const pending = new Map();

export const labelSync = {
  refreshFrame: 0,
  liveRefreshFrame: 0,
  liveRefreshActive: false,
  installed: false,
};

export const nodeRegistry = {
  key: "",
};

export const posterDocuments = new Map();

export const state = {
  conversations: [],
  activeId: null,
  messages: [],
  streaming: new Map(),
  dbPath: null,
  selectedId: null,
  blocks: [],
  canvas: {
    ready: false,
    nodeCount: 0,
    selectedNode: null,
    currentTool: "select",
    snapshotText: "",
    darkMode: false,
    error: null
  },
  planner: {
    context: null,
    contextText: "",
    lastOutboundPrompt: "",
    canvasContext: null
  },
  canvasContext: {
    regionMode: false,
    region: null,
    drag: null,
    context: null
  },
  canvasTool: {
    status: "idle",
    runId: null,
    lastResult: null,
    error: null
  },
  poster: {
    renderState: "idle",
    selectedLayerId: "headline",
    lastNodeId: null,
    lastError: null
  },
  timeline: {
    attachments: new Map(),
    inspector: {
      nodeId: null,
      loading: false,
      detail: null,
      error: null
    }
  }
};

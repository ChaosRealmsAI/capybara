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
  workspace: {
    activeTab: "canvas",
    timelineInspectorWasOpen: false
  },
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
    vectorCount: 0,
    objectCount: 0,
    objects: [],
    selectedNode: null,
    selectedVector: null,
    currentTool: "select",
    currentStyle: { stroke: "#8a6fae", fill: "#fef3c7", fillStyle: "hachure" },
    viewport: null,
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
  projectPackage: {
    path: null,
    status: "idle",
    inspection: null,
    workbench: null,
    selectedArtifactId: null,
    selectedCardId: null,
    previewSource: "",
    generation: null,
    error: null
  },
  poster: {
    renderState: "idle",
    selectedLayerId: "headline",
    lastNodeId: null,
    lastError: null
  },
  posterWorkspace: {
    status: "idle",
    path: "",
    document: null,
    pageId: "",
    layerPath: "",
    exportStatus: "",
    error: ""
  },
  timeline: {
    attachments: new Map(),
    inspector: {
      nodeId: null,
      loading: false,
      detail: null,
      error: null
    }
  },
  video: {
    status: "idle",
    compositionPath: "",
    renderSourcePath: "",
    renderSource: null,
    previewUrl: "",
    editor: null,
    selectedTrackId: "",
    selectedField: "",
    playheadMs: 0,
    durationMs: 0,
    exportJob: null,
    error: null
  }
};

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
    surfaceNodes: null,
    artifactNodes: {},
    selectedArtifactId: null,
    selectedCardId: null,
    previewSource: "",
    selectionContext: null,
    campaign: null,
    generation: null,
    review: null,
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
    selectedRange: null,
    clipQueue: [],
    clipQueueManifest: null,
    clipQueuePersistStatus: "idle",
    clipQueuePersistError: null,
    clipSuggestion: null,
    clipSuggestionStatus: "idle",
    clipSuggestionError: null,
    clipProposal: null,
    proposalStatus: "idle",
    playheadMs: 0,
    durationMs: 0,
    exportJob: null,
    lastExport: null,
    error: null
  },
  gameAssets: {
    status: "idle",
    path: "",
    rootUrl: "",
    pack: null,
    selectedAssetId: "",
    verifyStatus: "",
    error: ""
  }
};

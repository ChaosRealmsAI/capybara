export function createStateSnapshot({ state, normalizeValue, posterDocumentsState }) {
  return function stateSnapshot() {
    return normalizeValue({
      canvas: state.canvas,
      selectedId: state.selectedId,
      blocks: state.blocks,
      planner: state.planner,
      workspace: state.workspace,
      video: state.video,
      gameAssets: {
        status: state.gameAssets.status,
        path: state.gameAssets.path,
        selectedAssetId: state.gameAssets.selectedAssetId,
        assetCount: state.gameAssets.pack?.assets?.length || 0,
        frameCount: gameAssetFrameCount(state.gameAssets.pack),
        verifyStatus: state.gameAssets.verifyStatus,
        error: state.gameAssets.error
      },
      posterWorkspace: { status: state.posterWorkspace.status, path: state.posterWorkspace.path, pageId: state.posterWorkspace.pageId, layerPath: state.posterWorkspace.layerPath, pageCount: state.posterWorkspace.document?.pages?.length || 0, exportStatus: state.posterWorkspace.exportStatus, error: state.posterWorkspace.error },
      poster: {
        ...state.poster,
        documents: posterDocumentsState()
      },
      canvasContext: state.canvasContext.context
    });
  };
}

function gameAssetFrameCount(pack) {
  return (pack?.assets || []).reduce((total, asset) =>
    total + (asset.actions || []).reduce((sum, action) => sum + (action.frame_paths || []).length, 0), 0);
}

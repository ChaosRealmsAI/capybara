import { createProjectPackage } from "./project-package.js";

export function createProjectPackageWiring({ state, rpc, dom, stringifyError, appendPlannerMessage, canvasApi }) {
  return createProjectPackage({
    state,
    rpc,
    dom: {
      projectPackagePanelEl: dom.projectPackagePanelEl,
      projectPackageTitleEl: dom.projectPackageTitleEl,
      projectPackageMetaEl: dom.projectPackageMetaEl,
      projectWorkbenchEl: dom.projectWorkbenchEl,
      projectWorkbenchCardsEl: dom.projectWorkbenchCardsEl,
      projectSelectedSummaryEl: dom.projectSelectedSummaryEl,
      projectDesignLanguageEl: dom.projectDesignLanguageEl,
      projectSelectionContextEl: dom.projectSelectionContextEl,
      projectCampaignSummaryEl: dom.projectCampaignSummaryEl,
      projectArtifactListEl: dom.projectArtifactListEl,
      projectPreviewFrameEl: dom.projectPreviewFrameEl,
      promptEl: dom.promptEl,
      providerEl: dom.providerEl,
      modelEl: dom.modelEl,
      effortEl: dom.effortEl,
    },
    stringifyError,
    appendPlannerMessage,
    canvasApi,
  });
}

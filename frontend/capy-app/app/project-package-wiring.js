import { createProjectPackage } from "./project-package.js";

export function createProjectPackageWiring({ state, rpc, dom, stringifyError }) {
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
      projectArtifactListEl: dom.projectArtifactListEl,
      projectPreviewFrameEl: dom.projectPreviewFrameEl,
    },
    stringifyError,
  });
}

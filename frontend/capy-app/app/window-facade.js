export function installWindowFacade({ state, capyApi, workbenchApi }) {
  window.CAPYBARA_STATE = state;
  window.capy = capyApi;
  window.capyWorkbench = workbenchApi;
}

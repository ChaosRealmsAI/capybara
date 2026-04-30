pub const OP_PROJECT_INSPECT: &str = "project-inspect";
pub const OP_ARTIFACT_REGISTER: &str = "artifact-register";
pub const OP_ARTIFACT_READ: &str = "artifact-read";
pub const OP_CONTEXT_BUILD: &str = "context-build";
pub const OP_PATCH_APPLY: &str = "patch-apply";
pub const OP_PROJECT_WORKBENCH: &str = "project-workbench";
pub const OP_PROJECT_GENERATE: &str = "project-generate";
pub const OP_PROJECT_RUN_LIST: &str = "project-run-list";
pub const OP_PROJECT_RUN_SHOW: &str = "project-run-show";
pub const OP_PROJECT_RUN_ACCEPT: &str = "project-run-accept";
pub const OP_PROJECT_RUN_REJECT: &str = "project-run-reject";
pub const OP_PROJECT_RUN_RETRY: &str = "project-run-retry";
pub const OP_PROJECT_RUN_UNDO: &str = "project-run-undo";

#[cfg(test)]
mod tests {
    use super::{
        OP_ARTIFACT_READ, OP_ARTIFACT_REGISTER, OP_CONTEXT_BUILD, OP_PATCH_APPLY,
        OP_PROJECT_GENERATE, OP_PROJECT_INSPECT, OP_PROJECT_RUN_ACCEPT, OP_PROJECT_RUN_LIST,
        OP_PROJECT_RUN_REJECT, OP_PROJECT_RUN_RETRY, OP_PROJECT_RUN_SHOW, OP_PROJECT_RUN_UNDO,
        OP_PROJECT_WORKBENCH,
    };

    #[test]
    fn project_ops_are_stable() {
        assert_eq!(OP_PROJECT_INSPECT, "project-inspect");
        assert_eq!(OP_ARTIFACT_REGISTER, "artifact-register");
        assert_eq!(OP_ARTIFACT_READ, "artifact-read");
        assert_eq!(OP_CONTEXT_BUILD, "context-build");
        assert_eq!(OP_PATCH_APPLY, "patch-apply");
        assert_eq!(OP_PROJECT_WORKBENCH, "project-workbench");
        assert_eq!(OP_PROJECT_GENERATE, "project-generate");
        assert_eq!(OP_PROJECT_RUN_LIST, "project-run-list");
        assert_eq!(OP_PROJECT_RUN_SHOW, "project-run-show");
        assert_eq!(OP_PROJECT_RUN_ACCEPT, "project-run-accept");
        assert_eq!(OP_PROJECT_RUN_REJECT, "project-run-reject");
        assert_eq!(OP_PROJECT_RUN_RETRY, "project-run-retry");
        assert_eq!(OP_PROJECT_RUN_UNDO, "project-run-undo");
    }
}

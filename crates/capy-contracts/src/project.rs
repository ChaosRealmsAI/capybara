pub const OP_PROJECT_INSPECT: &str = "project-inspect";
pub const OP_ARTIFACT_REGISTER: &str = "artifact-register";
pub const OP_ARTIFACT_READ: &str = "artifact-read";
pub const OP_CONTEXT_BUILD: &str = "context-build";
pub const OP_PATCH_APPLY: &str = "patch-apply";
pub const OP_PROJECT_WORKBENCH: &str = "project-workbench";
pub const OP_PROJECT_SURFACE_NODES: &str = "project-surface-nodes";
pub const OP_PROJECT_SURFACE_NODE_UPDATE: &str = "project-surface-node-update";
pub const OP_PROJECT_GENERATE: &str = "project-generate";

#[cfg(test)]
mod tests {
    use super::{
        OP_ARTIFACT_READ, OP_ARTIFACT_REGISTER, OP_CONTEXT_BUILD, OP_PATCH_APPLY,
        OP_PROJECT_GENERATE, OP_PROJECT_INSPECT, OP_PROJECT_SURFACE_NODE_UPDATE,
        OP_PROJECT_SURFACE_NODES, OP_PROJECT_WORKBENCH,
    };

    #[test]
    fn project_ops_are_stable() {
        assert_eq!(OP_PROJECT_INSPECT, "project-inspect");
        assert_eq!(OP_ARTIFACT_REGISTER, "artifact-register");
        assert_eq!(OP_ARTIFACT_READ, "artifact-read");
        assert_eq!(OP_CONTEXT_BUILD, "context-build");
        assert_eq!(OP_PATCH_APPLY, "patch-apply");
        assert_eq!(OP_PROJECT_WORKBENCH, "project-workbench");
        assert_eq!(OP_PROJECT_SURFACE_NODES, "project-surface-nodes");
        assert_eq!(
            OP_PROJECT_SURFACE_NODE_UPDATE,
            "project-surface-node-update"
        );
        assert_eq!(OP_PROJECT_GENERATE, "project-generate");
    }
}

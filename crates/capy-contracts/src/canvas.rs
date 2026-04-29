pub const OP_CANVAS_NODES_REGISTER: &str = "canvas-nodes-register";

#[cfg(test)]
mod tests {
    use super::OP_CANVAS_NODES_REGISTER;

    #[test]
    fn canvas_node_registration_op_is_stable() {
        assert_eq!(OP_CANVAS_NODES_REGISTER, "canvas-nodes-register");
    }
}

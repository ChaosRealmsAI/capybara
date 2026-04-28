//! Connector: arrow line between two shapes that follows when shapes move.
//!
//! A connector links two shapes by their IDs. When either shape moves,
//! the connector's endpoints auto-update because they are computed from
//! live shape positions at render time (via `Shape::edge_point`).

use crate::state::{AppState, Connector, ConnectorStyle};

/// Create a new connector between two shapes by their IDs.
/// Returns `true` if both shapes exist and the connector was added.
pub fn create_connector(state: &mut AppState, from_id: u64, to_id: u64) -> bool {
    create_connector_styled(state, from_id, to_id, ConnectorStyle::default())
}

/// Create a connector with a specific style (Straight or Elbow).
pub fn create_connector_styled(
    state: &mut AppState,
    from_id: u64,
    to_id: u64,
    style: ConnectorStyle,
) -> bool {
    if from_id == to_id {
        return false;
    }
    let from_exists = state.shape_by_id(from_id).is_some();
    let to_exists = state.shape_by_id(to_id).is_some();
    if !from_exists || !to_exists {
        return false;
    }
    state.push_undo();
    state.connectors.push(Connector {
        from_id,
        to_id,
        color: state.color,
        style,
        label: None,
    });
    true
}

/// Compute elbow route points between two edge points.
/// Returns the intermediate bend points for an elbow (horizontal-then-vertical) path.
pub fn elbow_route(p1: (f64, f64), p2: (f64, f64)) -> (f64, f64) {
    // Horizontal-first then vertical: bend at (p2.x, p1.y)
    (p2.0, p1.1)
}

/// Find the closest point on a shape's edge toward a target center.
/// Delegates to `Shape::edge_point` which computes ray-rect intersection.
pub fn find_edge_point(
    state: &AppState,
    shape_id: u64,
    target_cx: f64,
    target_cy: f64,
) -> Option<(f64, f64)> {
    let shape = state.shape_by_id(shape_id)?;
    Some(shape.edge_point(target_cx, target_cy))
}

/// Remove all connectors that reference a given shape ID.
pub fn remove_connectors_for_shape(state: &mut AppState, shape_id: u64) {
    state
        .connectors
        .retain(|c| c.from_id != shape_id && c.to_id != shape_id);
}

/// Clear binding_start / binding_end on any arrows that reference a removed shape.
pub fn clear_bindings_for_shape(state: &mut AppState, shape_id: u64) {
    for shape in &mut state.shapes {
        if shape.binding_start == Some(shape_id) {
            shape.binding_start = None;
        }
        if shape.binding_end == Some(shape_id) {
            shape.binding_end = None;
        }
    }
}

/// List all connector pairs as (from_id, to_id, color).
pub fn list_connectors(state: &AppState) -> Vec<(u64, u64, u32)> {
    state
        .connectors
        .iter()
        .map(|c| (c.from_id, c.to_id, c.color))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Shape, ShapeKind};

    fn make_state_with_two_shapes() -> AppState {
        let mut state = AppState::new();
        let mut s1 = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x1e1e1e);
        s1.w = 100.0;
        s1.h = 80.0;
        state.add_shape(s1);
        let mut s2 = Shape::new(ShapeKind::Rect, 200.0, 200.0, 0x1e1e1e);
        s2.w = 100.0;
        s2.h = 80.0;
        state.add_shape(s2);
        state
    }

    #[test]
    fn create_connector_links_two_shapes() {
        let mut state = make_state_with_two_shapes();
        assert!(create_connector(&mut state, 1, 2));
        assert_eq!(state.connectors.len(), 1);
        assert_eq!(state.connectors[0].from_id, 1);
        assert_eq!(state.connectors[0].to_id, 2);
    }

    #[test]
    fn create_connector_rejects_same_shape() {
        let mut state = make_state_with_two_shapes();
        assert!(!create_connector(&mut state, 1, 1));
        assert!(state.connectors.is_empty());
    }

    #[test]
    fn create_connector_rejects_missing_shape() {
        let mut state = make_state_with_two_shapes();
        assert!(!create_connector(&mut state, 1, 99));
        assert!(state.connectors.is_empty());
    }

    #[test]
    fn remove_connectors_cleans_up() {
        let mut state = make_state_with_two_shapes();
        create_connector(&mut state, 1, 2);
        assert_eq!(state.connectors.len(), 1);
        remove_connectors_for_shape(&mut state, 1);
        assert!(state.connectors.is_empty());
    }

    #[test]
    fn find_edge_point_returns_some() {
        let state = make_state_with_two_shapes();
        let pt = find_edge_point(&state, 1, 250.0, 240.0);
        assert!(pt.is_some());
        let (px, py) = pt.unwrap();
        // Edge point should be on the boundary, not the center
        assert!((0.0..=100.0).contains(&px));
        assert!((0.0..=80.0).contains(&py));
    }

    #[test]
    fn list_connectors_returns_all() {
        let mut state = make_state_with_two_shapes();
        create_connector(&mut state, 1, 2);
        let list = list_connectors(&state);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], (1, 2, state.color));
    }
}

use super::{overlay_scale, toolbar_rect};

#[test]
fn overlay_scale_stays_base_for_normal_canvas() {
    assert!((overlay_scale(1400.0, 900.0) - 1.0).abs() < f64::EPSILON);
}

#[test]
fn overlay_scale_grows_for_zoomed_out_browser_viewport() {
    let scale = overlay_scale(3420.0, 1902.0);
    assert!(scale > 1.4);
    assert!(scale <= 1.65);
}

#[test]
fn toolbar_rect_scales_hit_area_with_overlay() {
    let (_, _, _, normal_h) = toolbar_rect(1400.0, 900.0);
    let (_, _, _, large_h) = toolbar_rect(3420.0, 1902.0);
    assert!(large_h > normal_h * 1.4);
}

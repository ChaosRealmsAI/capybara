use super::{
    ArrowHead, ArrowStyle, CanvasContentKind, CanvasMetadata, CanvasSelectionItem, FillStyle,
    FontFamily, Shape, ShapeGeometry, ShapeKind, StrokeStyle, TextAlign,
};

impl Shape {
    pub fn new(kind: ShapeKind, x: f64, y: f64, color: u32) -> Self {
        Self {
            id: 0,
            kind,
            x,
            y,
            w: 0.0,
            h: 0.0,
            color,
            stroke_color: color,
            stroke_width: 2.0,
            stroke_style: StrokeStyle::default(),
            fill_style: FillStyle::Solid,
            opacity: 1.0,
            flipped_h: false,
            flipped_v: false,
            text: String::new(),
            points: Vec::new(),
            rotation: 0.0,
            group_id: 0,
            arrow_start: ArrowHead::None,
            arrow_end: ArrowHead::default(),
            arrow_style: ArrowStyle::default(),
            label: None,
            font_family: FontFamily::default(),
            font_size: 14.0,
            text_align: TextAlign::default(),
            bold: false,
            italic: false,
            image_path: None,
            metadata: CanvasMetadata::default(),
            image: None,
            binding_start: None,
            binding_end: None,
            rounded: true,
        }
    }

    pub fn content_kind(&self) -> CanvasContentKind {
        if let Some(kind) = self.metadata.content_kind {
            return kind;
        }
        match self.kind {
            ShapeKind::Image => CanvasContentKind::Image,
            ShapeKind::Text | ShapeKind::StickyNote => CanvasContentKind::Text,
            _ => CanvasContentKind::Shape,
        }
    }

    pub fn display_title(&self) -> String {
        if let Some(title) = self
            .metadata
            .title
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            return title.clone();
        }
        if let Some(label) = self.label.as_ref().filter(|value| !value.is_empty()) {
            return label.clone();
        }
        let text = self.text.trim();
        if !text.is_empty() {
            return text
                .lines()
                .next()
                .unwrap_or(text)
                .chars()
                .take(48)
                .collect();
        }
        match self.content_kind() {
            CanvasContentKind::Project => "Project".to_string(),
            CanvasContentKind::ProjectArtifact => "Project Artifact".to_string(),
            CanvasContentKind::Brand => "Brand Kit".to_string(),
            CanvasContentKind::Image => "Image".to_string(),
            CanvasContentKind::Poster => "Poster".to_string(),
            CanvasContentKind::Video => "Video".to_string(),
            CanvasContentKind::Web => "Web".to_string(),
            CanvasContentKind::Text => "Text".to_string(),
            CanvasContentKind::Audio => "Audio".to_string(),
            CanvasContentKind::ThreeD => "3D".to_string(),
            CanvasContentKind::Shape => self.kind.label().to_string(),
        }
    }

    pub fn selection_item(&self, index: usize) -> CanvasSelectionItem {
        CanvasSelectionItem {
            index,
            id: self.id,
            shape_kind: self.kind,
            content_kind: self.content_kind(),
            title: self.display_title(),
            text: self.text.clone(),
            status: self.metadata.status.clone(),
            owner: self.metadata.owner.clone(),
            refs: self.metadata.refs.clone(),
            next_action: self.metadata.next_action.clone(),
            editor_route: self.metadata.editor_route.clone(),
            source_path: self
                .metadata
                .source_path
                .clone()
                .or_else(|| self.image_path.clone()),
            artifact_ref: self.metadata.artifact_ref.clone(),
            mime: self
                .metadata
                .mime
                .clone()
                .or_else(|| self.image.as_ref().map(|image| image.mime.clone())),
            generation_provider: self.metadata.generation_provider.clone(),
            generation_prompt: self.metadata.generation_prompt.clone(),
            geometry: ShapeGeometry {
                x: self.x,
                y: self.y,
                w: self.w,
                h: self.h,
            },
        }
    }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        let (px, py) = self.untransform_point(px, py);
        match self.kind {
            ShapeKind::Rect
            | ShapeKind::Text
            | ShapeKind::Freehand
            | ShapeKind::StickyNote
            | ShapeKind::Highlighter
            | ShapeKind::Image => {
                px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
            }
            ShapeKind::Ellipse => self.contains_ellipse(px, py),
            ShapeKind::Triangle => self.contains_triangle(px, py),
            ShapeKind::Diamond => self.contains_diamond(px, py),
            ShapeKind::Line | ShapeKind::Arrow => {
                point_to_segment_dist(px, py, self.x, self.y, self.x + self.w, self.y + self.h)
                    <= 5.0
            }
        }
    }

    fn contains_ellipse(&self, px: f64, py: f64) -> bool {
        let cx = self.x + self.w / 2.0;
        let cy = self.y + self.h / 2.0;
        let rx = self.w / 2.0;
        let ry = self.h / 2.0;
        if rx <= 0.0 || ry <= 0.0 {
            return false;
        }
        let dx = (px - cx) / rx;
        let dy = (py - cy) / ry;
        dx * dx + dy * dy <= 1.0
    }

    fn contains_triangle(&self, px: f64, py: f64) -> bool {
        let ax = self.x + self.w / 2.0;
        let ay = self.y;
        let bx = self.x;
        let by = self.y + self.h;
        let cx = self.x + self.w;
        let cy = self.y + self.h;
        point_in_triangle((px, py), (ax, ay), (bx, by), (cx, cy))
    }

    fn contains_diamond(&self, px: f64, py: f64) -> bool {
        let top = (self.x + self.w / 2.0, self.y);
        let right = (self.x + self.w, self.y + self.h / 2.0);
        let bottom = (self.x + self.w / 2.0, self.y + self.h);
        let left = (self.x, self.y + self.h / 2.0);
        point_in_triangle((px, py), top, right, left)
            || point_in_triangle((px, py), bottom, right, left)
    }

    fn untransform_point(&self, px: f64, py: f64) -> (f64, f64) {
        if self.rotation.abs() <= 1e-6 && !self.flipped_h && !self.flipped_v {
            return (px, py);
        }

        let (cx, cy) = self.center();
        let mut dx = px - cx;
        let mut dy = py - cy;
        if self.rotation.abs() > 1e-6 {
            let (sin_r, cos_r) = self.rotation.sin_cos();
            (dx, dy) = (dx * cos_r + dy * sin_r, -dx * sin_r + dy * cos_r);
        }
        if self.flipped_h {
            dx = -dx;
        }
        if self.flipped_v {
            dy = -dy;
        }
        (cx + dx, cy + dy)
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn edge_point(&self, tx: f64, ty: f64) -> (f64, f64) {
        if let Some(anchor) = nearest_anchor(self.anchor_points(), tx, ty) {
            return anchor;
        }
        let (cx, cy) = self.center();
        let dx = tx - cx;
        let dy = ty - cy;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return (cx, cy);
        }
        let nx = dx / len;
        let ny = dy / len;
        let tx_edge = if nx.abs() > 1e-6 {
            self.w / 2.0 / nx.abs()
        } else {
            f64::MAX
        };
        let ty_edge = if ny.abs() > 1e-6 {
            self.h / 2.0 / ny.abs()
        } else {
            f64::MAX
        };
        let t = tx_edge.min(ty_edge);
        (cx + nx * t, cy + ny * t)
    }

    pub fn anchor_points(&self) -> [(f64, f64); 4] {
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        [
            (self.x + hw, self.y),
            (self.x + self.w, self.y + hh),
            (self.x + hw, self.y + self.h),
            (self.x, self.y + hh),
        ]
    }

    pub fn translate_by(&mut self, dx: f64, dy: f64) {
        if !dx.is_finite() || !dy.is_finite() || (dx.abs() < 1e-9 && dy.abs() < 1e-9) {
            return;
        }
        self.x += dx;
        self.y += dy;
        self.translate_path_points(dx, dy);
    }

    pub fn move_to(&mut self, x: f64, y: f64) {
        self.translate_by(x - self.x, y - self.y);
    }

    pub fn resize_to_bounds(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let old_x = self.x;
        let old_y = self.y;
        let old_w = self.w;
        let old_h = self.h;
        self.x = x;
        self.y = y;
        self.w = w.max(1.0);
        self.h = h.max(1.0);

        if !self.has_absolute_path_points() || self.points.is_empty() {
            return;
        }
        let sx = if old_w.abs() < 1e-9 {
            1.0
        } else {
            self.w / old_w
        };
        let sy = if old_h.abs() < 1e-9 {
            1.0
        } else {
            self.h / old_h
        };
        for point in &mut self.points {
            point.0 = self.x + (point.0 - old_x) * sx;
            point.1 = self.y + (point.1 - old_y) * sy;
        }
    }

    fn translate_path_points(&mut self, dx: f64, dy: f64) {
        if !self.has_absolute_path_points() {
            return;
        }
        for point in &mut self.points {
            point.0 += dx;
            point.1 += dy;
        }
    }

    fn has_absolute_path_points(&self) -> bool {
        matches!(self.kind, ShapeKind::Freehand | ShapeKind::Highlighter)
    }

    pub fn default_color_for_kind(kind: ShapeKind) -> u32 {
        match kind {
            ShapeKind::Rect => 0x5b8abf,
            ShapeKind::Ellipse => 0x3da065,
            ShapeKind::Triangle => 0xe8a348,
            ShapeKind::Diamond => 0x8a6fae,
            ShapeKind::StickyNote => 0xfef3c7,
            ShapeKind::Text => 0x1e293b,
            ShapeKind::Arrow | ShapeKind::Line | ShapeKind::Freehand => 0x64748b,
            ShapeKind::Highlighter => 0xfbbf24,
            ShapeKind::Image => 0x94a3b8,
        }
    }
}

fn nearest_anchor(anchors: [(f64, f64); 4], tx: f64, ty: f64) -> Option<(f64, f64)> {
    let mut best_anchor = anchors[0];
    let mut best_dist = f64::MAX;
    for &(ax, ay) in &anchors {
        let d = ((tx - ax).powi(2) + (ty - ay).powi(2)).sqrt();
        if d < best_dist {
            best_dist = d;
            best_anchor = (ax, ay);
        }
    }
    (best_dist < 15.0).then_some(best_anchor)
}

pub fn point_to_segment_dist(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = (((px - x1) * dx + (py - y1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

fn point_in_triangle(point: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let d1 = cross_2d(point.0, point.1, a.0, a.1, b.0, b.1);
    let d2 = cross_2d(point.0, point.1, b.0, b.1, c.0, c.1);
    let d3 = cross_2d(point.0, point.1, c.0, c.1, a.0, a.1);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn cross_2d(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    (ax - px) * (by - py) - (bx - px) * (ay - py)
}

pub fn point_in_polygon(px: f64, py: f64, polygon: &[(f64, f64)]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

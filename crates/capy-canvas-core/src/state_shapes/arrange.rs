use crate::state::AppState;

impl AppState {
    pub fn align_left(&mut self) {
        if let Some(target) = self.selected_axis_min(|s| s.x) {
            self.apply_selected_axis(target, |s, value| s.x = value);
        }
    }

    pub fn align_center_h(&mut self) {
        if let Some(target) = self.selected_axis_center(|s| s.x, |s| s.w) {
            self.apply_selected_axis(target, |s, value| s.x = value - s.w / 2.0);
        }
    }

    pub fn align_right(&mut self) {
        if let Some(target) = self.selected_axis_max(|s| s.x + s.w) {
            self.apply_selected_axis(target, |s, value| s.x = value - s.w);
        }
    }

    pub fn align_top(&mut self) {
        if let Some(target) = self.selected_axis_min(|s| s.y) {
            self.apply_selected_axis(target, |s, value| s.y = value);
        }
    }

    pub fn align_center_v(&mut self) {
        if let Some(target) = self.selected_axis_center(|s| s.y, |s| s.h) {
            self.apply_selected_axis(target, |s, value| s.y = value - s.h / 2.0);
        }
    }

    pub fn align_bottom(&mut self) {
        if let Some(target) = self.selected_axis_max(|s| s.y + s.h) {
            self.apply_selected_axis(target, |s, value| s.y = value - s.h);
        }
    }

    pub fn distribute_h(&mut self) {
        self.distribute_selected(|s| (s.x, s.w), |s, value| s.x = value);
    }

    pub fn distribute_v(&mut self) {
        self.distribute_selected(|s| (s.y, s.h), |s, value| s.y = value);
    }

    fn selected_axis_min(&self, value_of: impl Fn(&crate::shape::Shape) -> f64) -> Option<f64> {
        (self.selected.len() >= 2).then(|| {
            self.selected
                .iter()
                .filter_map(|&i| self.shapes.get(i))
                .map(value_of)
                .fold(f64::MAX, f64::min)
        })
    }

    fn selected_axis_max(&self, value_of: impl Fn(&crate::shape::Shape) -> f64) -> Option<f64> {
        (self.selected.len() >= 2).then(|| {
            self.selected
                .iter()
                .filter_map(|&i| self.shapes.get(i))
                .map(value_of)
                .fold(f64::MIN, f64::max)
        })
    }

    fn selected_axis_center(
        &self,
        start: impl Fn(&crate::shape::Shape) -> f64,
        size: impl Fn(&crate::shape::Shape) -> f64,
    ) -> Option<f64> {
        let min = self.selected_axis_min(&start)?;
        let max = self.selected_axis_max(|s| start(s) + size(s))?;
        Some((min + max) / 2.0)
    }

    fn apply_selected_axis(&mut self, target: f64, apply: impl Fn(&mut crate::shape::Shape, f64)) {
        self.push_undo();
        for &i in &self.selected.clone() {
            if let Some(shape) = self.shapes.get_mut(i) {
                apply(shape, target);
            }
        }
    }

    fn distribute_selected(
        &mut self,
        axis: impl Fn(&crate::shape::Shape) -> (f64, f64),
        apply: impl Fn(&mut crate::shape::Shape, f64),
    ) {
        if self.selected.len() < 3 {
            return;
        }
        let mut indexed = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i).map(|s| (i, axis(s))))
            .collect::<Vec<_>>();
        indexed.sort_by(|a, b| {
            a.1.0
                .partial_cmp(&b.1.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let total_size = indexed.iter().map(|(_, (_, size))| *size).sum::<f64>();
        let first = indexed.first().map(|(_, (pos, _))| *pos).unwrap_or(0.0);
        let last = indexed
            .last()
            .map(|(_, (pos, size))| pos + size)
            .unwrap_or(0.0);
        let gap = (last - first - total_size) / (indexed.len() - 1) as f64;
        self.push_undo();
        let mut cursor = first;
        for (i, (_, size)) in indexed {
            if let Some(shape) = self.shapes.get_mut(i) {
                apply(shape, cursor);
            }
            cursor += size + gap;
        }
    }
}

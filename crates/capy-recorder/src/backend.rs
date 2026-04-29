#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecorderBackend {
    CefOsr,
}

impl RecorderBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CefOsr => "cef-osr",
        }
    }

    #[must_use]
    pub fn only() -> Self {
        Self::CefOsr
    }
}

#[cfg(test)]
mod tests {
    use super::RecorderBackend;

    #[test]
    fn only_backend_is_cef_osr() {
        let backend = RecorderBackend::only();
        assert_eq!(backend, RecorderBackend::CefOsr);
        assert_eq!(backend.as_str(), "cef-osr");
    }
}

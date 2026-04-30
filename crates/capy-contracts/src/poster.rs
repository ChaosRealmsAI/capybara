pub const OP_POSTER_DOCUMENT_SAVE: &str = "poster-document-save";
pub const OP_POSTER_DOCUMENT_EXPORT: &str = "poster-document-export";

#[cfg(test)]
mod tests {
    use super::{OP_POSTER_DOCUMENT_EXPORT, OP_POSTER_DOCUMENT_SAVE};

    #[test]
    fn keeps_poster_ops_explicit() {
        assert_eq!(OP_POSTER_DOCUMENT_SAVE, "poster-document-save");
        assert_eq!(OP_POSTER_DOCUMENT_EXPORT, "poster-document-export");
    }
}

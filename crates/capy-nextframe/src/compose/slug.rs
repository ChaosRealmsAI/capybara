const FALLBACK_SLUG: &str = "poster";
const MAX_SLUG_LEN: usize = 64;

pub fn poster_slug(id: Option<&str>, title: Option<&str>) -> String {
    sanitize(id.unwrap_or_default())
        .or_else(|| sanitize(title.unwrap_or_default()))
        .unwrap_or_else(|| FALLBACK_SLUG.to_string())
}

pub fn sanitize(raw: &str) -> Option<String> {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in raw.trim().chars().flat_map(char::to_lowercase) {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch.is_ascii() {
            Some('-')
        } else {
            None
        };
        let Some(ch) = mapped else {
            continue;
        };
        if ch == '-' {
            if slug.is_empty() || previous_dash {
                continue;
            }
            previous_dash = true;
            slug.push(ch);
        } else {
            previous_dash = false;
            slug.push(ch);
        }
        if slug.len() >= MAX_SLUG_LEN {
            break;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() { None } else { Some(slug) }
}

#[cfg(test)]
mod tests {
    use super::poster_slug;

    #[test]
    fn slug_uses_id_before_title() {
        assert_eq!(
            poster_slug(Some("Poster ID 01"), Some("Ignored Title")),
            "poster-id-01"
        );
    }

    #[test]
    fn slug_sanitizes_non_ascii_title_to_ascii_parts() {
        assert_eq!(poster_slug(None, Some("陶瓷 Morning 海报")), "morning");
    }

    #[test]
    fn slug_falls_back_for_empty_or_non_ascii_only_title() {
        assert_eq!(poster_slug(Some("   "), Some("陶瓷海报")), "poster");
    }
}

use crate::brand::tokens::BrandTokens;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

pub fn tokens_hash(tokens: &BrandTokens) -> String {
    let mut hash = FNV_OFFSET;
    for (name, value) in &tokens.values {
        hash = update(hash, name.as_bytes());
        hash = update(hash, b"\0");
        hash = update(hash, value.as_bytes());
        hash = update(hash, b"\0");
    }
    format!("brand-token-fnv1a64-{hash:016x}")
}

fn update(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::tokens_hash;
    use crate::brand::tokens::BrandTokens;

    #[test]
    fn same_tokens_have_same_hash() {
        let left = tokens([("--c-brand-1", "#fff"), ("--c-brand-2", "#000")]);
        let right = tokens([("--c-brand-2", "#000"), ("--c-brand-1", "#fff")]);

        assert_eq!(tokens_hash(&left), tokens_hash(&right));
    }

    #[test]
    fn changed_var_changes_hash() {
        let left = tokens([("--c-brand-1", "#fff")]);
        let right = tokens([("--c-brand-1", "#fefefe")]);

        assert_ne!(tokens_hash(&left), tokens_hash(&right));
    }

    fn tokens<const N: usize>(pairs: [(&str, &str); N]) -> BrandTokens {
        BrandTokens {
            values: pairs
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect::<BTreeMap<_, _>>(),
        }
    }
}

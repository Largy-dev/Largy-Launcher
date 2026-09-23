//! Ordering for dotted loader/game version strings, where lexicographic
//! order is wrong (`20.4.99` < `20.4.190`) and a textual suffix marks a
//! pre-release (`21.0.1-beta` < `21.0.1`).

use std::cmp::Ordering;

fn tokens(v: &str) -> Vec<&str> {
    v.split(['.', '-', '+', '_']).filter(|t| !t.is_empty()).collect()
}

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (ta, tb) = (tokens(a), tokens(b));
    for (x, y) in ta.iter().zip(tb.iter()) {
        let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(nx), Ok(ny)) => nx.cmp(&ny),
            (Ok(_), Err(_)) => Ordering::Greater,
            (Err(_), Ok(_)) => Ordering::Less,
            (Err(_), Err(_)) => x.cmp(y),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    let rest_is_release = |rest: &[&str]| rest.first().is_some_and(|t| t.parse::<u64>().is_ok());
    match ta.len().cmp(&tb.len()) {
        Ordering::Equal => Ordering::Equal,
        Ordering::Greater if rest_is_release(&ta[tb.len()..]) => Ordering::Greater,
        Ordering::Greater => Ordering::Less,
        Ordering::Less if rest_is_release(&tb[ta.len()..]) => Ordering::Less,
        Ordering::Less => Ordering::Greater,
    }
}

/// Newest first.
pub fn sort_desc(versions: &mut [String]) {
    versions.sort_by(|a, b| compare_versions(b, a));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_segments_compare_numerically() {
        assert_eq!(compare_versions("20.4.99", "20.4.190"), Ordering::Less);
        assert_eq!(compare_versions("1.20.10", "1.20.9"), Ordering::Greater);
        assert_eq!(compare_versions("47.2.20", "47.2.20"), Ordering::Equal);
    }

    #[test]
    fn prerelease_suffix_sorts_before_release() {
        assert_eq!(compare_versions("21.0.1-beta", "21.0.1"), Ordering::Less);
        assert_eq!(compare_versions("21.0.1.5", "21.0.1"), Ordering::Greater);
    }

    #[test]
    fn sort_desc_puts_newest_first() {
        let mut v = vec!["20.4.99".to_string(), "20.4.190".to_string(), "20.4.190-beta".to_string()];
        sort_desc(&mut v);
        assert_eq!(v, vec!["20.4.190", "20.4.190-beta", "20.4.99"]);
    }
}

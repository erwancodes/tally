/// Short format, same rules as `src/lib/format.ts`:
/// `1,234` under 10k, then `12.3k`, `1.24M`, `1.2B`.
pub fn compact(n: u64) -> String {
    let f = n as f64;
    if n < 10_000 {
        let s = n.to_string();
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        return out;
    } else if n < 1_000_000 {
        format!("{:.1}k", f / 1e3)
    } else if n < 1_000_000_000 {
        format!("{:.2}M", f / 1e6)
    } else {
        format!("{:.1}B", f / 1e9)
    }
}

#[cfg(test)]
mod tests {
    use super::compact;

    #[test]
    fn formats() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1234), "1,234");
        assert_eq!(compact(12_345), "12.3k");
        assert_eq!(compact(1_240_000), "1.24M");
        assert_eq!(compact(1_200_000_000), "1.2B");
    }
}

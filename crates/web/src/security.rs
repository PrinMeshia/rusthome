//! Loopback bind detection and HTML warning when listening on non-local addresses.

use crate::util::esc_html;

/// True only for loopback binds we treat as "local machine only" (no banner).
pub(crate) fn bind_looks_loopback_only(bind: &str) -> bool {
    let b = bind.trim();
    if b.starts_with('[') {
        if let Some(end) = b.find("]:") {
            let inner = &b[1..end];
            return inner == "::1";
        }
        let inner = b.trim_start_matches('[').trim_end_matches(']');
        return inner == "::1";
    }
    if let Some((host, tail)) = b.rsplit_once(':') {
        if tail.chars().all(|c| c.is_ascii_digit()) && !host.contains(':') {
            return is_loopback_host(host);
        }
    }
    is_loopback_host(b)
}

fn is_loopback_host(host: &str) -> bool {
    let h = host.trim();
    h == "127.0.0.1"
        || h.eq_ignore_ascii_case("localhost")
        || h == "::1"
}

pub(crate) fn security_banner_html(listen: &str) -> String {
    if bind_looks_loopback_only(listen) {
        return String::new();
    }
    format!(
        r#"<div class="security-banner" role="alert"><strong>Network exposure:</strong> listening on <code>{}</code>. This UI and JSON APIs have <strong>no authentication</strong>. Prefer <code>127.0.0.1</code> for local use, or use a reverse proxy with TLS and access control before exposing on a LAN.</div>"#,
        esc_html(listen)
    )
}

#[cfg(test)]
mod tests {
    use super::bind_looks_loopback_only;

    #[test]
    fn bind_loopback_vs_exposure() {
        assert!(bind_looks_loopback_only("127.0.0.1:8080"));
        assert!(bind_looks_loopback_only("127.0.0.1"));
        assert!(bind_looks_loopback_only("localhost:3000"));
        assert!(bind_looks_loopback_only("[::1]:8080"));
        assert!(!bind_looks_loopback_only("0.0.0.0:8080"));
        assert!(!bind_looks_loopback_only("[::]:8080"));
        assert!(!bind_looks_loopback_only("192.168.1.5:8080"));
    }
}

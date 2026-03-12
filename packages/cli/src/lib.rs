use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use mastra_server::RouteDescription;

pub fn default_bind_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000)
}

pub fn render_routes(routes: &[RouteDescription]) -> String {
    routes
        .iter()
        .map(|route| format!("{} {}  # {}", route.method, route.path, route.summary))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn serve_banner(addr: SocketAddr) -> String {
    format!("starting mastra server on {addr}")
}

#[cfg(test)]
mod tests {
    use mastra_server::RouteDescription;

    use super::{default_bind_addr, render_routes, serve_banner};

    #[test]
    fn default_bind_addr_matches_expected_local_endpoint() {
        assert_eq!(default_bind_addr().to_string(), "127.0.0.1:3000");
    }

    #[test]
    fn render_routes_formats_each_route_on_its_own_line() {
        let rendered = render_routes(&[
            RouteDescription {
                method: "GET",
                path: "/health".to_owned(),
                summary: "health check",
            },
            RouteDescription {
                method: "POST",
                path: "/agents/weather/generate".to_owned(),
                summary: "generate",
            },
        ]);

        assert_eq!(
            rendered,
            "GET /health  # health check\nPOST /agents/weather/generate  # generate"
        );
    }

    #[test]
    fn serve_banner_mentions_bind_address() {
        assert!(serve_banner(default_bind_addr()).contains("127.0.0.1:3000"));
    }
}

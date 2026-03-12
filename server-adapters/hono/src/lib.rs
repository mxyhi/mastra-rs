use axum::Router;

use mastra_server::MastraHttpServer;

#[derive(Clone, Default)]
pub struct HonoCompatAdapter {
    inner: MastraHttpServer,
}

impl HonoCompatAdapter {
    pub fn new(inner: MastraHttpServer) -> Self {
        Self { inner }
    }

    pub fn router(&self) -> Router {
        self.inner.router()
    }
}

#[cfg(test)]
mod tests {
    use super::HonoCompatAdapter;
    use mastra_server::MastraHttpServer;

    #[test]
    fn hono_adapter_wraps_server() {
        let adapter = HonoCompatAdapter::new(MastraHttpServer::new());
        let _router = adapter.router();
    }
}

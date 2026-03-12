use axum::Router;
use mastra_server::{MastraServer, RouteDescription};

#[derive(Clone)]
pub struct FastifyCompatAdapter {
    inner: MastraServer,
}

impl FastifyCompatAdapter {
    pub fn new(inner: MastraServer) -> Self {
        Self { inner }
    }

    pub fn router(&self) -> Router {
        self.inner.clone().into_router()
    }

    pub fn route_catalog(&self) -> Vec<RouteDescription> {
        self.inner.route_catalog()
    }
}

#[cfg(test)]
mod tests {
    use super::FastifyCompatAdapter;
    use mastra_server::{MastraRuntimeRegistry, MastraServer};

    #[test]
    fn fastify_adapter_wraps_server_router() {
        let adapter = FastifyCompatAdapter::new(MastraServer::new(MastraRuntimeRegistry::new()));
        let _router = adapter.router();
    }

    #[test]
    fn fastify_adapter_exposes_route_catalog() {
        let adapter = FastifyCompatAdapter::new(MastraServer::new(MastraRuntimeRegistry::new()));
        let routes = adapter.route_catalog();

        assert!(routes.iter().any(|route| route.path == "/api/workflows"));
    }
}

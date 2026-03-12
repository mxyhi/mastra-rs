use std::sync::OnceLock;

use tracing_subscriber::{EnvFilter, fmt};

static INIT_GUARD: OnceLock<()> = OnceLock::new();

pub fn init_tracing(default_filter: &str) {
    let _ = INIT_GUARD.get_or_init(|| {
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
        let _ = fmt().with_env_filter(filter).with_target(false).try_init();
    });
}

#[cfg(test)]
mod tests {
    use super::init_tracing;

    #[test]
    fn init_tracing_is_idempotent() {
        init_tracing("info");
        init_tracing("debug");
    }
}

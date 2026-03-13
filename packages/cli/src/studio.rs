use std::{error::Error, net::SocketAddr, path::Path};

use axum::{
    Router,
    response::{Html, IntoResponse},
    routing::get,
};
use serde_json::Value;

pub type StudioResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioConfig {
    pub address: SocketAddr,
    pub server_url: String,
    pub presets: Option<Value>,
}

pub fn load_request_context_presets(path: &Path) -> StudioResult<Value> {
    let raw = std::fs::read_to_string(path)?;
    let presets = serde_json::from_str::<Value>(&raw)?;
    if !presets.is_object() {
        return Err("request context presets must be a JSON object".into());
    }
    Ok(presets)
}

pub fn render_studio_html(server_url: &str, presets: Option<&Value>) -> String {
    let presets = presets
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    let presets_json = serde_json::to_string_pretty(&presets).expect("presets should serialize");
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Mastra Studio (Rust)</title>
    <style>
      :root {{
        color-scheme: light;
        font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
        background: #f6f7fb;
        color: #17212b;
      }}
      body {{
        margin: 0;
        padding: 32px;
      }}
      main {{
        max-width: 960px;
        margin: 0 auto;
        background: white;
        border-radius: 20px;
        padding: 24px;
        box-shadow: 0 24px 60px rgba(23, 33, 43, 0.12);
      }}
      h1 {{
        margin-top: 0;
      }}
      code, pre {{
        background: #eef2ff;
        border-radius: 10px;
      }}
      code {{
        padding: 2px 6px;
      }}
      pre {{
        padding: 16px;
        overflow: auto;
      }}
      .grid {{
        display: grid;
        gap: 16px;
        grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      }}
      .card {{
        border: 1px solid #dbe2f2;
        border-radius: 16px;
        padding: 16px;
      }}
    </style>
  </head>
  <body>
    <main>
      <h1>Mastra Studio (Rust)</h1>
      <p>This static Studio shell points at <code>{server_url}</code>.</p>
      <div class="grid">
        <section class="card">
          <h2>Route catalog</h2>
          <pre id="routes">Loading…</pre>
        </section>
        <section class="card">
          <h2>Request context presets</h2>
          <pre>{presets_json}</pre>
        </section>
      </div>
    </main>
    <script>
      const serverUrl = {server_url:?};
      fetch(`${{serverUrl}}/routes`)
        .then((response) => response.json())
        .then((payload) => {{
          document.getElementById("routes").textContent = JSON.stringify(payload, null, 2);
        }})
        .catch((error) => {{
          document.getElementById("routes").textContent = `Failed to load routes: ${{error}}`;
        }});
    </script>
  </body>
</html>
"#
    )
}

pub async fn serve_studio(config: StudioConfig) -> StudioResult<()> {
    let html = render_studio_html(&config.server_url, config.presets.as_ref());
    let app = Router::new()
        .route(
            "/",
            get({
                let html = html.clone();
                move || {
                    let html = html.clone();
                    async move { Html(html).into_response() }
                }
            }),
        )
        .route("/health", get(|| async { "ok" }));
    let listener = tokio::net::TcpListener::bind(config.address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{load_request_context_presets, render_studio_html};

    #[test]
    fn presets_loader_requires_json_object() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("presets.json");
        std::fs::write(&path, r#"{ "development": { "userId": "dev" } }"#).expect("write presets");

        let presets = load_request_context_presets(&path).expect("presets");
        assert_eq!(presets["development"]["userId"], "dev");
    }

    #[test]
    fn studio_html_embeds_server_url_and_presets() {
        let html = render_studio_html(
            "http://localhost:4111/api",
            Some(&serde_json::json!({
                "dev": { "userId": "dev-user" }
            })),
        );

        assert!(html.contains("http://localhost:4111/api"));
        assert!(html.contains("dev-user"));
        assert!(html.contains("Route catalog"));
    }
}

use chrono::Utc;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use mastra_auth_auth0::{MastraAuthAuth0, MastraAuthAuth0Options};
use mastra_packages_auth::{AuthRequestParts, Authenticator, JwksSource};
use serde_json::json;

fn signing_secret() -> &'static [u8] {
    b"auth0-provider-secret"
}

fn inline_hs256_jwks() -> JwkSet {
    let mut jwk =
        jsonwebtoken::jwk::Jwk::from_encoding_key(&EncodingKey::from_secret(signing_secret()), Algorithm::HS256)
            .expect("jwk");
    jwk.common.key_id = Some("auth0-key".to_string());
    JwkSet { keys: vec![jwk] }
}

fn bearer_request(token: &str) -> AuthRequestParts {
    AuthRequestParts::default().with_authorization(format!("Bearer {token}"))
}

#[tokio::test]
async fn auth0_authenticates_inline_jwk_tokens() {
    let claims = json!({
        "sub": "auth0|user-123",
        "iss": "https://tenant.auth0.com/",
        "aud": "mastra-api",
        "exp": Utc::now().timestamp() + 3600,
    });
    let token = jsonwebtoken::encode(
        &Header {
            alg: Algorithm::HS256,
            kid: Some("auth0-key".to_string()),
            ..Header::default()
        },
        &claims,
        &EncodingKey::from_secret(signing_secret()),
    )
    .expect("jwt");

    let provider = MastraAuthAuth0::new(
        MastraAuthAuth0Options::default()
            .with_domain("tenant.auth0.com")
            .with_audience("mastra-api")
            .with_jwks(JwksSource::Inline(inline_hs256_jwks()))
            .with_algorithms(vec![Algorithm::HS256])
            .with_leeway_seconds(0),
    )
    .expect("provider");

    let identity = provider
        .authenticate(&bearer_request(&token))
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(provider.domain(), "tenant.auth0.com");
    assert_eq!(provider.audience(), "mastra-api");
    assert_eq!(identity.provider, "auth0");
    assert_eq!(identity.subject, "auth0|user-123");
}

#[test]
fn auth0_reads_domain_and_audience_from_env() {
    unsafe {
        std::env::set_var("AUTH0_DOMAIN", "env-tenant.auth0.com");
        std::env::set_var("AUTH0_AUDIENCE", "env-audience");
    }

    let provider = MastraAuthAuth0::new(MastraAuthAuth0Options::default()).expect("provider");

    assert_eq!(provider.domain(), "env-tenant.auth0.com");
    assert_eq!(provider.audience(), "env-audience");

    unsafe {
        std::env::remove_var("AUTH0_DOMAIN");
        std::env::remove_var("AUTH0_AUDIENCE");
    }
}

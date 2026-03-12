use chrono::Utc;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use mastra_auth_clerk::{MastraAuthClerk, MastraAuthClerkOptions};
use mastra_packages_auth::{AuthRequestParts, Authenticator, JwksSource};
use serde_json::json;

fn signing_secret() -> &'static [u8] {
    b"clerk-provider-secret"
}

fn inline_hs256_jwks() -> JwkSet {
    let mut jwk = jsonwebtoken::jwk::Jwk::from_encoding_key(
        &EncodingKey::from_secret(signing_secret()),
        Algorithm::HS256,
    )
    .expect("jwk");
    jwk.common.key_id = Some("clerk-key".to_string());
    JwkSet { keys: vec![jwk] }
}

#[tokio::test]
async fn clerk_authenticates_inline_jwk_tokens() {
    let claims = json!({
        "sub": "user_123",
        "exp": Utc::now().timestamp() + 3600,
    });
    let token = jsonwebtoken::encode(
        &Header {
            alg: Algorithm::HS256,
            kid: Some("clerk-key".to_string()),
            ..Header::default()
        },
        &claims,
        &EncodingKey::from_secret(signing_secret()),
    )
    .expect("jwt");

    let provider = MastraAuthClerk::new(
        MastraAuthClerkOptions::default()
            .with_jwks(JwksSource::Inline(inline_hs256_jwks()))
            .with_algorithms(vec![Algorithm::HS256])
            .with_leeway_seconds(0),
    )
    .expect("provider");

    let identity = provider
        .authenticate(&AuthRequestParts::default().with_authorization(format!("Bearer {token}")))
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(identity.provider, "clerk");
    assert_eq!(identity.subject, "user_123");
}

#[test]
fn clerk_reads_jwks_uri_from_env() {
    unsafe {
        std::env::set_var("CLERK_JWKS_URI", "https://clerk.example.com/jwks");
    }
    let provider = MastraAuthClerk::new(MastraAuthClerkOptions::default()).expect("provider");
    assert_eq!(provider.jwks_uri(), "https://clerk.example.com/jwks");
    unsafe {
        std::env::remove_var("CLERK_JWKS_URI");
    }
}

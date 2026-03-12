use chrono::Utc;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use mastra_auth_firebase::{MastraAuthFirebase, MastraAuthFirebaseOptions};
use mastra_packages_auth::{AuthRequestParts, Authenticator, JwksSource};
use serde_json::json;

fn signing_secret() -> &'static [u8] {
    b"firebase-provider-secret"
}

fn inline_hs256_jwks() -> JwkSet {
    let mut jwk =
        jsonwebtoken::jwk::Jwk::from_encoding_key(&EncodingKey::from_secret(signing_secret()), Algorithm::HS256)
            .expect("jwk");
    jwk.common.key_id = Some("firebase-key".to_string());
    JwkSet { keys: vec![jwk] }
}

#[tokio::test]
async fn firebase_authenticates_project_scoped_tokens() {
    let claims = json!({
        "sub": "firebase-user-1",
        "iss": "https://securetoken.google.com/demo-project",
        "aud": "demo-project",
        "exp": Utc::now().timestamp() + 3600,
    });
    let token = jsonwebtoken::encode(
        &Header {
            alg: Algorithm::HS256,
            kid: Some("firebase-key".to_string()),
            ..Header::default()
        },
        &claims,
        &EncodingKey::from_secret(signing_secret()),
    )
    .expect("jwt");

    let provider = MastraAuthFirebase::new(
        MastraAuthFirebaseOptions::default()
            .with_project_id("demo-project")
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

    assert_eq!(provider.project_id(), "demo-project");
    assert_eq!(identity.provider, "firebase");
    assert_eq!(identity.subject, "firebase-user-1");
}

#[test]
fn firebase_reads_project_id_from_env() {
    unsafe {
        std::env::set_var("FIREBASE_PROJECT_ID", "env-project");
    }
    let provider = MastraAuthFirebase::new(MastraAuthFirebaseOptions::default()).expect("provider");
    assert_eq!(provider.project_id(), "env-project");
    unsafe {
        std::env::remove_var("FIREBASE_PROJECT_ID");
    }
}

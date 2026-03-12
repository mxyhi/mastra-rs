use crate::AuthError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthRequestParts {
    pub authorization: Option<String>,
    pub cookie_header: Option<String>,
    pub callback_code: Option<String>,
    pub callback_state: Option<String>,
}

impl AuthRequestParts {
    pub fn with_authorization(mut self, authorization: impl Into<String>) -> Self {
        self.authorization = Some(authorization.into());
        self
    }

    pub fn with_cookie_header(mut self, cookie_header: impl Into<String>) -> Self {
        self.cookie_header = Some(cookie_header.into());
        self
    }

    pub fn with_callback(
        mut self,
        callback_code: impl Into<String>,
        callback_state: impl Into<String>,
    ) -> Self {
        self.callback_code = Some(callback_code.into());
        self.callback_state = Some(callback_state.into());
        self
    }

    pub fn bearer_token(&self) -> Result<Option<&str>, AuthError> {
        match self.authorization.as_deref() {
            None => Ok(None),
            Some(value) => {
                let Some((scheme, token)) = value.split_once(' ') else {
                    return Err(AuthError::MissingBearerToken);
                };
                if !scheme.eq_ignore_ascii_case("bearer") {
                    return Err(AuthError::UnsupportedAuthorizationScheme(
                        scheme.to_string(),
                    ));
                }
                if token.is_empty() {
                    return Err(AuthError::MissingBearerToken);
                }
                Ok(Some(token))
            }
        }
    }

    pub fn cookie(&self, name: &str) -> Option<&str> {
        let cookie_header = self.cookie_header.as_deref()?;
        for pair in cookie_header.split(';') {
            let (key, value) = pair.trim().split_once('=')?;
            if key == name {
                return Some(value);
            }
        }
        None
    }

    pub fn callback_code(&self) -> Option<&str> {
        self.callback_code.as_deref()
    }

    pub fn callback_state(&self) -> Option<&str> {
        self.callback_state.as_deref()
    }
}

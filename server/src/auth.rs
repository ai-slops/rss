use crate::error::{AppError, Result};
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// Authentication state is deliberately process-local: daemon restart invalidates sessions.
pub struct Auth {
    digest: [u8; 32],
    sessions: HashMap<String, Instant>,
    last_attempt: Option<Instant>,
    secure: bool,
}

impl Auth {
    pub fn new(password: &str, secure: bool) -> Self {
        Self {
            digest: Sha256::digest(password.as_bytes()).into(),
            sessions: HashMap::new(),
            last_attempt: None,
            secure,
        }
    }

    pub fn login(&mut self, password: &str) -> Result<String> {
        let now = Instant::now();
        if self
            .last_attempt
            .is_some_and(|at| now.duration_since(at) < Duration::from_millis(500))
        {
            return Err(AppError::Unavailable(
                "Too many sign-in attempts. Wait a moment before trying again.".into(),
            ));
        }
        self.last_attempt = Some(now);
        let candidate: [u8; 32] = Sha256::digest(password.as_bytes()).into();
        if !bool::from(self.digest.ct_eq(&candidate)) {
            return Err(AppError::Unauthorized);
        }
        self.sessions.retain(|_, until| *until > now);
        if self.sessions.len() >= 16 {
            return Err(AppError::Unavailable("The 16-session control budget is full. Sign out another browser or wait for its session to expire.".into()));
        }
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        self.sessions
            .insert(token.clone(), now + Duration::from_secs(28_800));
        let secure = if self.secure { "; Secure" } else { "" };
        Ok(format!(
            "lp_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=28800{secure}"
        ))
    }

    pub fn authorize(&self, headers: &HeaderMap) -> Result<()> {
        let token = headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies
                    .split(';')
                    .find_map(|entry| entry.trim().strip_prefix("lp_session="))
            });
        if token
            .and_then(|key| self.sessions.get(key))
            .is_some_and(|until| *until > Instant::now())
        {
            Ok(())
        } else {
            Err(AppError::Unauthorized)
        }
    }

    pub fn logout(&mut self, headers: &HeaderMap) {
        if let Some(token) = headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies
                    .split(';')
                    .find_map(|entry| entry.trim().strip_prefix("lp_session="))
            })
        {
            self.sessions.remove(token);
        }
    }
}

/// Non-simple custom request header plus strict same-site cookies prevents form CSRF.
pub fn mutation_headers(headers: &HeaderMap) -> Result<()> {
    if headers
        .get("x-live-production")
        .and_then(|v| v.to_str().ok())
        != Some("1")
        || headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("cross-site")
    {
        return Err(AppError::Invalid(
            "Use the authenticated same-origin application to send commands.".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_password_without_creating_session() {
        // Given an operator credential.
        let mut auth = Auth::new("a-long-random-operator-password", false);
        // When a different password is presented.
        let result = auth.login("wrong-password");
        // Then no authorized session exists.
        assert!(matches!(result, Err(AppError::Unauthorized)));
        assert!(auth.sessions.is_empty());
    }

    #[test]
    fn cookie_authorizes_only_issued_session() {
        // Given a correctly issued login cookie.
        let mut auth = Auth::new("a-long-random-operator-password", true);
        let cookie = auth
            .login("a-long-random-operator-password")
            .expect("valid login");
        let mut headers = HeaderMap::new();
        headers.insert("cookie", cookie.parse().expect("header"));
        // When the cookie is verified.
        let authorized = auth.authorize(&headers);
        // Then access succeeds without exposing the password.
        assert!(authorized.is_ok());
        assert!(cookie.contains("HttpOnly; SameSite=Strict"));
        assert!(cookie.ends_with("; Secure"));
    }
}

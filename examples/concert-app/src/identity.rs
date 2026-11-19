//! Server-owned fictional accounts and opaque, process-local sessions.

use std::collections::HashMap;
use std::sync::Mutex;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use subtle::ConstantTimeEq;

#[derive(Clone, Copy, Serialize)]
pub struct Profile {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Default)]
pub struct Sessions(Mutex<HashMap<String, Profile>>);

pub enum LoginError {
    InvalidCredentials,
    RandomnessUnavailable,
}

impl Sessions {
    pub fn profile(&self, token: Option<&str>) -> Option<Profile> {
        self.0.lock().unwrap().get(token?).copied()
    }

    pub fn logout(&self, token: Option<&str>) {
        if let Some(token) = token {
            self.0.lock().unwrap().remove(token);
        }
    }

    pub fn login(
        &self,
        previous: Option<&str>,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<(String, Profile), LoginError> {
        // A failed attempt to change identity also signs out the old session.
        self.logout(previous);
        let (id, name, expected) = match username {
            Some("alice") => ("alice", "Alice", "alice-demo"),
            Some("bob") => ("bob", "Bob", "bob-demo"),
            _ => return Err(LoginError::InvalidCredentials),
        };
        if !bool::from(expected.as_bytes().ct_eq(password.unwrap_or("").as_bytes())) {
            return Err(LoginError::InvalidCredentials);
        }
        let profile = Profile { id, name };
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| LoginError::RandomnessUnavailable)?;
        let token = URL_SAFE_NO_PAD.encode(bytes);
        self.0.lock().unwrap().insert(token.clone(), profile);
        Ok((token, profile))
    }
}

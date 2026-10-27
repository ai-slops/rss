use crate::error::{AppError, Result};
use axum::http::{HeaderMap, Uri};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum BrowserMessage {
    #[serde(rename = "setPeerStatus")]
    Register { roles: Vec<String> },
    #[serde(rename = "list")]
    List,
    #[serde(rename = "listConsumers")]
    ListConsumers,
    #[serde(rename = "startSession")]
    Start,
    #[serde(rename = "peer")]
    Peer {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    #[serde(rename = "endSession")]
    End {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "sessionStarted")]
    Started {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    #[serde(rename = "endSession")]
    Ended {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Default)]
pub struct SessionBudget {
    requested: bool,
    active: Option<String>,
}

impl SessionBudget {
    pub fn admit(&mut self, text: &str) -> Result<()> {
        let message: BrowserMessage = serde_json::from_str(text)
            .map_err(|_| AppError::Invalid("Invalid receive-only signalling message.".into()))?;
        match message {
            BrowserMessage::Register { roles } => {
                if roles != ["listener"] {
                    return Err(AppError::Invalid(
                        "Browser monitoring cannot register media producers.".into(),
                    ));
                }
            }
            BrowserMessage::Start => {
                if self.requested {
                    return Err(AppError::Invalid(
                        "Only one private media session is permitted per connection.".into(),
                    ));
                }
                self.requested = true;
            }
            BrowserMessage::Peer { session_id } => {
                if self.active.as_ref() != Some(&session_id) {
                    return Err(AppError::Invalid("Unknown private media session.".into()));
                }
            }
            BrowserMessage::End { session_id } => {
                if self.active.as_ref() != Some(&session_id) {
                    return Err(AppError::Invalid("Unknown private media session.".into()));
                }
                self.active = None;
                self.requested = false;
            }
            BrowserMessage::List | BrowserMessage::ListConsumers => {}
        }
        Ok(())
    }

    pub fn observe(&mut self, text: &str) {
        let Ok(message) = serde_json::from_str::<ServerMessage>(text) else {
            return;
        };
        match message {
            ServerMessage::Started { session_id } => {
                self.active = Some(session_id);
            }
            ServerMessage::Ended { session_id } => {
                if self.active.as_ref() == Some(&session_id) {
                    self.active = None;
                    self.requested = false;
                }
            }
            ServerMessage::Other => {}
        }
    }
}

pub fn origin(headers: &HeaderMap, public_origin: Option<&str>) -> Result<()> {
    let Some(origin) = headers.get("origin") else {
        return Ok(());
    };
    let supplied = origin
        .to_str()
        .ok()
        .and_then(|value| value.parse::<Uri>().ok())
        .ok_or_else(|| AppError::Invalid("Invalid media origin.".into()))?;
    let expected = public_origin
        .and_then(|value| value.parse::<Uri>().ok())
        .and_then(|uri| uri.authority().map(ToString::to_string))
        .or_else(|| {
            headers
                .get("host")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        });
    if !matches!(supplied.scheme_str(), Some("http" | "https"))
        || supplied
            .authority()
            .map(ToString::to_string)
            .map(|value| value.to_lowercase())
            != expected.map(|value| value.to_lowercase())
    {
        return Err(AppError::Invalid(
            "Private media requires the application's own origin.".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_start_or_forged_end_cannot_bypass_encoder_budget() {
        let mut budget = SessionBudget::default();
        assert!(
            budget
                .admit(r#"{"type":"startSession","peerId":"producer"}"#)
                .is_ok()
        );
        budget.observe(r#"{"type":"sessionStarted","sessionId":"owned","peerId":"producer"}"#);
        assert!(
            budget
                .admit(r#"{"type":"endSession","sessionId":"other"}"#)
                .is_err()
        );
        assert!(
            budget
                .admit(r#"{"type":"startSession","peerId":"producer"}"#)
                .is_err()
        );
        assert!(
            budget
                .admit(r#"{"type":"endSession","sessionId":"owned"}"#)
                .is_ok()
        );
        assert!(
            budget
                .admit(r#"{"type":"startSession","peerId":"producer"}"#)
                .is_ok()
        );
    }
    #[test]
    fn sibling_origin_is_rejected_even_with_same_site_cookie_policy() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "studio.example.test".parse().expect("host"));
        headers.insert(
            "origin",
            "https://other.example.test".parse().expect("origin"),
        );
        assert!(origin(&headers, None).is_err());
        headers.insert(
            "origin",
            "https://studio.example.test".parse().expect("origin"),
        );
        assert!(origin(&headers, None).is_ok());
    }
}

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use gst::{glib::prelude::*, prelude::*};
use gstreamer as gst;
use sha2::{Digest, Sha256};

use crate::{
    engine_source_lease::AdmissionLease,
    engine_source_peer::PeerKey,
    protocol::{EngineRunId, SourceId},
};

static REGISTRY: OnceLock<Mutex<HashMap<String, Weak<SourceAdmission>>>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmissionSnapshot {
    pub active: bool,
    pub pending_peer: Option<String>,
    pub active_peer: Option<String>,
    pub accepted: u64,
    pub rejected: u64,
    pub generation: u64,
}

pub fn snapshot(engine_run_id: &EngineRunId, source_id: &SourceId) -> Option<AdmissionSnapshot> {
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let admission = registry
        .lock()
        .ok()?
        .get(&key(engine_run_id, source_id))?
        .upgrade()?;
    admission.snapshot()
}

pub struct SourceAdmission {
    registry_key: Option<String>,
    expected_stream_id: String,
    lease: AdmissionLease,
    epoch_reset_requested: AtomicBool,
    element: gst::glib::WeakRef<gst::Element>,
    handlers: Mutex<Vec<gst::glib::SignalHandlerId>>,
}

#[must_use]
pub fn publisher_stream_id(passphrase: &str) -> String {
    format!("lp-{:x}", Sha256::digest(passphrase.as_bytes()))
}

impl SourceAdmission {
    pub fn install(
        bin: &gst::Bin,
        element_name: &str,
        initial_generation: u64,
        engine_run_id: &EngineRunId,
        source_id: &SourceId,
        passphrase: &str,
    ) -> Result<Arc<Self>, String> {
        let element = bin
            .by_name(element_name)
            .ok_or_else(|| format!("source has no admission element '{element_name}'"))?;
        let registry_key = key(engine_run_id, source_id);
        let admission = Arc::new(Self {
            registry_key: Some(registry_key.clone()),
            expected_stream_id: publisher_stream_id(passphrase),
            lease: AdmissionLease::new(initial_generation),
            epoch_reset_requested: AtomicBool::new(false),
            element: element.downgrade(),
            handlers: Mutex::new(Vec::new()),
        });
        REGISTRY
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| "source admission registry is unavailable".to_owned())?
            .insert(registry_key, Arc::downgrade(&admission));
        let handlers = connect_signals(&element, &admission);
        *admission
            .handlers
            .lock()
            .map_err(|_| "source admission handlers are unavailable".to_owned())? = handlers;
        Ok(admission)
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.lease.generation()
    }

    pub fn pending_expired(&self, now: Instant) -> bool {
        self.lease.pending_expired(now)
    }

    pub fn take_epoch_reset(&self) -> bool {
        self.epoch_reset_requested.swap(false, Ordering::AcqRel)
    }

    pub fn disconnect(&self) {
        let Some(element) = self.element.upgrade() else {
            return;
        };
        let Ok(mut handlers) = self.handlers.lock() else {
            return;
        };
        for handler in handlers.drain(..) {
            element.disconnect(handler);
        }
    }

    fn snapshot(&self) -> Option<AdmissionSnapshot> {
        let lease = self.lease.snapshot()?;
        Some(AdmissionSnapshot {
            active: lease.active_peer.is_some(),
            pending_peer: lease.pending_peer,
            active_peer: lease.active_peer,
            accepted: lease.accepted,
            rejected: lease.rejected,
            generation: lease.generation,
        })
    }
}

fn connect_signals(
    element: &gst::Element,
    admission: &Arc<SourceAdmission>,
) -> Vec<gst::glib::SignalHandlerId> {
    let mut handlers = Vec::with_capacity(4);
    let connecting = Arc::clone(admission);
    handlers.push(element.connect("caller-connecting", false, move |values| {
        let stream_id = values
            .get(2)
            .and_then(|value| value.get::<Option<String>>().ok().flatten())
            .map_or_else(String::new, std::convert::identity);
        let peer = PeerKey::from_signal(values, 1);
        let accepted = if stream_id.is_empty() {
            peer.is_some_and(|peer| connecting.lease.reserve_legacy(peer))
        } else if stream_id == connecting.expected_stream_id {
            peer.is_some_and(|peer| connecting.lease.reserve(peer))
        } else {
            false
        };
        Some(accepted.to_value())
    }));
    let added = Arc::clone(admission);
    handlers.push(element.connect("caller-added", false, move |values| {
        if let Some(peer) = PeerKey::from_signal(values, 2)
            && added.lease.added(peer)
        {
            added.epoch_reset_requested.store(true, Ordering::Release);
        }
        None
    }));
    let removed = Arc::clone(admission);
    handlers.push(element.connect("caller-removed", false, move |values| {
        if let Some(peer) = PeerKey::from_signal(values, 2) {
            removed.lease.removed(&peer);
        }
        None
    }));
    let rejected = Arc::clone(admission);
    handlers.push(element.connect("caller-rejected", false, move |values| {
        rejected
            .lease
            .rejected(PeerKey::from_signal(values, 1).as_ref());
        None
    }));
    handlers
}

impl Drop for SourceAdmission {
    fn drop(&mut self) {
        let Some(key) = &self.registry_key else {
            return;
        };
        if let Some(registry) = REGISTRY.get()
            && let Ok(mut registry) = registry.lock()
            && registry
                .get(key)
                .is_some_and(|current| std::ptr::eq(current.as_ptr(), self))
        {
            registry.remove(key);
        }
    }
}

fn key(engine_run_id: &EngineRunId, source_id: &SourceId) -> String {
    format!("{}/{}", engine_run_id.as_str(), source_id.as_str())
}

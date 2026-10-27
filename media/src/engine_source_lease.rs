use std::{
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crate::engine_source_peer::PeerKey;

const PENDING_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
enum LeaseState {
    #[default]
    Vacant,
    Pending {
        peer: PeerKey,
        reserved_at: Instant,
    },
    Active(PeerKey),
}

pub struct AdmissionLease {
    state: Mutex<LeaseState>,
    accepted: AtomicU64,
    rejected: AtomicU64,
    generation: AtomicU64,
}

pub struct LeaseSnapshot {
    pub pending_peer: Option<String>,
    pub active_peer: Option<String>,
    pub accepted: u64,
    pub rejected: u64,
    pub generation: u64,
}

impl AdmissionLease {
    pub const fn new(initial_generation: u64) -> Self {
        Self {
            state: Mutex::new(LeaseState::Vacant),
            accepted: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            generation: AtomicU64::new(initial_generation.saturating_sub(1)),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn snapshot(&self) -> Option<LeaseSnapshot> {
        let (pending_peer, active_peer) = {
            let state = self.state.lock().ok()?;
            match &*state {
                LeaseState::Vacant => (None, None),
                LeaseState::Pending { peer, .. } => (Some(peer.to_string()), None),
                LeaseState::Active(peer) => (None, Some(peer.to_string())),
            }
        };
        Some(LeaseSnapshot {
            pending_peer,
            active_peer,
            accepted: self.accepted.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            generation: self.generation(),
        })
    }

    pub fn reserve(&self, peer: PeerKey) -> bool {
        self.reserve_at(peer, Instant::now())
    }

    pub fn reserve_legacy(&self, peer: PeerKey) -> bool {
        self.reserve(peer)
    }

    pub fn pending_expired(&self, now: Instant) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        matches!(&*state, LeaseState::Pending { reserved_at, .. } if now.saturating_duration_since(*reserved_at) >= PENDING_TIMEOUT)
    }

    pub fn added(&self, peer: PeerKey) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if matches!(&*state, LeaseState::Pending { peer: current, .. } if current == &peer) {
            *state = LeaseState::Active(peer);
            self.accepted.fetch_add(1, Ordering::Relaxed);
            self.generation.fetch_add(1, Ordering::AcqRel);
            true
        } else {
            false
        }
    }

    pub fn removed(&self, peer: &PeerKey) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if matches!(&*state, LeaseState::Pending { peer: current, .. } | LeaseState::Active(current) if current == peer)
        {
            *state = LeaseState::Vacant;
        }
    }

    pub fn rejected(&self, peer: Option<&PeerKey>) {
        self.rejected.fetch_add(1, Ordering::Relaxed);
        let Some(peer) = peer else {
            return;
        };
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if matches!(&*state, LeaseState::Pending { peer: current, .. } if current == peer) {
            *state = LeaseState::Vacant;
        }
    }

    fn reserve_at(&self, peer: PeerKey, now: Instant) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        match &*state {
            LeaseState::Vacant => {
                *state = LeaseState::Pending {
                    peer,
                    reserved_at: now,
                };
                true
            }
            LeaseState::Pending { peer: current, .. } if current == &peer => true,
            LeaseState::Pending { .. } => false,
            LeaseState::Active(current) => current == &peer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_and_removal_only_release_matching_peer() -> anyhow::Result<()> {
        let lease = AdmissionLease::new(1);
        let a = PeerKey::test("127.0.0.1:10001")?;
        let b = PeerKey::test("127.0.0.1:10002")?;
        let c = PeerKey::test("127.0.0.1:10003")?;
        let now = Instant::now();
        assert!(lease.reserve_at(a.clone(), now));
        assert!(!lease.reserve_at(b.clone(), now + Duration::from_millis(10)));
        lease.rejected(Some(&b));
        lease.removed(&b);
        assert!(lease.added(a.clone()));
        assert!(!lease.reserve_at(c.clone(), now + Duration::from_secs(3)));
        lease.rejected(Some(&c));
        let active = lease
            .snapshot()
            .ok_or_else(|| anyhow::anyhow!("snapshot"))?;
        assert_eq!(active.active_peer.as_deref(), Some("127.0.0.1:10001"));
        assert_eq!(active.accepted, 1);
        assert_eq!(active.rejected, 2);
        lease.removed(&a);
        assert!(lease.reserve(c.clone()));
        assert!(lease.added(c));
        assert_eq!(lease.generation(), 2);
        Ok(())
    }

    #[test]
    fn own_rejection_releases_pending_and_expiry_never_reassigns() -> anyhow::Result<()> {
        let lease = AdmissionLease::new(1);
        let a = PeerKey::test("127.0.0.1:11001")?;
        let b = PeerKey::test("127.0.0.1:11002")?;
        let now = Instant::now();
        assert!(lease.reserve_at(a.clone(), now));
        lease.rejected(Some(&a));
        assert!(lease.reserve_at(a, now + Duration::from_millis(1)));
        assert!(lease.pending_expired(now + Duration::from_secs(3)));
        assert!(!lease.reserve_at(b, now + Duration::from_secs(3)));
        Ok(())
    }
}

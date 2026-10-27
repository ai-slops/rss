pub const fn after_pending(runtime: u64, admission: Option<u64>) -> u64 {
    match admission {
        Some(accepted) if accepted >= runtime => accepted.saturating_add(1),
        Some(_) | None => runtime,
    }
}

#[cfg(test)]
mod tests {
    use super::after_pending;

    #[test]
    fn pending_recovery_uses_accepted_high_water_not_last_snapshot() {
        assert_eq!(after_pending(1, Some(0)), 1);
        assert_eq!(after_pending(1, Some(1)), 2);
        assert_eq!(after_pending(1, Some(2)), 3);
        assert_eq!(after_pending(3, Some(2)), 3);
        assert_eq!(after_pending(3, None), 3);
    }
}

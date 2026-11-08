#[derive(Default)]
pub struct SourceSequence {
    sequence: Option<u64>,
    generation: Option<u64>,
}

impl SourceSequence {
    pub fn accept(&mut self, sequence: u64, generation: u64) -> Result<(), &'static str> {
        if let Some(previous) = self.sequence
            && sequence != previous.saturating_add(1)
        {
            return Err("source capture sequence gap or replay");
        }
        if self
            .generation
            .is_some_and(|previous| generation < previous)
        {
            return Err("source capture generation regressed");
        }
        self.sequence = Some(sequence);
        self.generation = Some(generation);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SourceSequence;

    #[test]
    fn duplicate_and_gap_sequences_fail_instead_of_replaying_provenance() {
        let mut duplicate = SourceSequence::default();
        assert_eq!(duplicate.accept(7, 1), Ok(()));
        assert_eq!(
            duplicate.accept(7, 1),
            Err("source capture sequence gap or replay")
        );

        let mut gap = SourceSequence::default();
        assert_eq!(gap.accept(7, 1), Ok(()));
        assert_eq!(
            gap.accept(9, 1),
            Err("source capture sequence gap or replay")
        );
    }

    #[test]
    fn generation_regression_fails_instead_of_restamping() {
        let mut sequence = SourceSequence::default();
        assert_eq!(sequence.accept(7, 2), Ok(()));
        assert_eq!(
            sequence.accept(8, 1),
            Err("source capture generation regressed")
        );
    }
}

use crate::snapshot::StorageSnapshot;
use nix::sys::statvfs::{FsFlags, statvfs};
use std::path::Path;

pub fn snapshot(path: &Path) -> StorageSnapshot {
    let Ok(stat) = statvfs(path) else {
        return StorageSnapshot {
            free_bytes: "unknown".into(),
            total_bytes: "unknown".into(),
            writable: false,
        };
    };
    let bytes_per_block = u128::from(stat.fragment_size());
    StorageSnapshot {
        free_bytes: (u128::from(stat.blocks_available()) * bytes_per_block).to_string(),
        total_bytes: (u128::from(stat.blocks()) * bytes_per_block).to_string(),
        writable: stat.blocks_available() > 0
            && !stat.flags().contains(FsFlags::ST_RDONLY)
            && path
                .metadata()
                .is_ok_and(|metadata| !metadata.permissions().readonly()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_reports_actual_available_and_total_bytes() {
        let directory = tempfile::tempdir().expect("private storage fixture");
        let observed = snapshot(directory.path());
        let free = observed
            .free_bytes
            .parse::<u128>()
            .expect("available bytes");
        let total = observed.total_bytes.parse::<u128>().expect("total bytes");
        assert!(free > 0 && total >= free);
        assert!(observed.writable);
    }

    #[test]
    fn unavailable_storage_does_not_invent_zero_capacity() {
        let directory = tempfile::tempdir().expect("private fixture");
        let observed = snapshot(&directory.path().join("missing"));
        assert_eq!(observed.free_bytes, "unknown");
        assert!(!observed.writable);
    }
}

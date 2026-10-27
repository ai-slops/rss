use nix::unistd::Pid;
use std::{collections::HashSet, fs, io};

#[derive(Clone, Debug)]
pub struct Identity {
    pub pid: Pid,
    pub parent: Pid,
    pub start_ticks: u64,
}

pub fn identity(pid: Pid) -> io::Result<Identity> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields: Vec<_> = text
        .rsplit_once(") ")
        .ok_or_else(invalid_stat)?
        .1
        .split_whitespace()
        .collect();
    let parent = fields
        .get(1)
        .ok_or_else(invalid_stat)?
        .parse::<i32>()
        .map_err(|_| invalid_stat())?;
    let start_ticks = fields
        .get(19)
        .ok_or_else(invalid_stat)?
        .parse()
        .map_err(|_| invalid_stat())?;
    Ok(Identity {
        pid,
        parent: Pid::from_raw(parent),
        start_ticks,
    })
}

pub fn descendants(owner: Pid) -> io::Result<Vec<Identity>> {
    let mut processes = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        if let Ok(raw) = entry.file_name().to_string_lossy().parse::<i32>()
            && let Ok(process) = identity(Pid::from_raw(raw))
        {
            processes.push(process);
        }
    }
    let mut owned = HashSet::from([owner]);
    loop {
        let before = owned.len();
        for process in &processes {
            if owned.contains(&process.parent) {
                owned.insert(process.pid);
            }
        }
        if owned.len() == before {
            break;
        }
    }
    Ok(processes
        .into_iter()
        .filter(|process| process.pid != owner && owned.contains(&process.pid))
        .collect())
}

pub fn still_same(process: &Identity) -> bool {
    identity(process.pid).is_ok_and(|current| current.start_ticks == process.start_ticks)
}

fn invalid_stat() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid process identity")
}

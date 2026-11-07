//! Pure inventory planning and a crash-released, checksummed write-ahead store.
//! The server must checkpoint BEFORE publishing an intent, and acknowledge only
//! after the resulting player snapshot (including sequence) has been persisted.
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

// DFQTX1 uses slot zero for one-time quests and a 1-based cycle for this exchange.
pub const HAM_EXCHANGE: u64 = 0x2AE651270FD58200;

pub fn operation_slot(quest: u64, cycle: Option<u64>) -> Option<u64> {
    match (quest == HAM_EXCHANGE, cycle) {
        (true, None) => Some(1),
        (true, Some(n)) if n > 0 => Some(n),
        (false, None) => Some(0),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub quest: u64,
    pub costs: Vec<(u64, String, u64)>,
    pub items: Vec<(String, u64)>,
    pub levels: i32,
    pub blocked: Option<&'static str>,
}
pub fn definitions() -> Vec<Definition> {
    include!("economy_catalog.rs")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
    pub plain: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum PlanError {
    Invalid,
    MissingItems,
    FullInventory,
    UnknownItem,
    Overflow,
}
/// Return an entirely planned copy. Never drop a reward or partially mutate input.
pub fn plan(
    inventory: &[Option<Stack>],
    costs: &[(String, u64)],
    items: &[(String, u64)],
    levels: i32,
    current_levels: i32,
    max_stack: impl Fn(&str) -> Option<u32>,
) -> Result<(Vec<Option<Stack>>, i32), PlanError> {
    if inventory.len() != 36 || levels < 0 || current_levels < 0 {
        return Err(PlanError::Invalid);
    }
    let new_levels = current_levels
        .checked_add(levels)
        .ok_or(PlanError::Overflow)?;
    let mut totals = BTreeMap::<String, u64>::new();
    for (item, count) in costs {
        if item.is_empty() || *count == 0 {
            return Err(PlanError::Invalid);
        }
        let old = totals.entry(item.clone()).or_default();
        *old = old.checked_add(*count).ok_or(PlanError::Overflow)?;
    }
    let mut result = inventory.to_vec();
    for (item, mut needed) in totals {
        for slot in &mut result {
            if let Some(stack) = slot.as_mut().filter(|s| s.plain && s.item == item) {
                let n = needed.min(u64::from(stack.count));
                stack.count -= n as u32;
                needed -= n;
                if stack.count == 0 {
                    *slot = None;
                }
            }
        }
        if needed != 0 {
            return Err(PlanError::MissingItems);
        }
    }
    for (item, count) in items {
        if item.is_empty() || *count == 0 {
            return Err(PlanError::Invalid);
        }
        let max = max_stack(item)
            .filter(|m| *m > 0)
            .ok_or(PlanError::UnknownItem)?;
        let mut needed = *count;
        for slot in &mut result {
            if let Some(s) = slot.as_mut().filter(|s| s.plain && s.item == *item) {
                let n = needed.min(u64::from(max.saturating_sub(s.count)));
                s.count += n as u32;
                needed -= n;
            }
        }
        for slot in &mut result {
            if slot.is_none() && needed > 0 {
                let n = needed.min(u64::from(max)) as u32;
                *slot = Some(Stack {
                    item: item.clone(),
                    count: n,
                    plain: true,
                });
                needed -= u64::from(n);
            }
        }
        if needed != 0 {
            return Err(PlanError::FullInventory);
        }
    }
    Ok((result, new_levels))
}

pub fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |a, b| {
        (a ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid Dragonfyre transaction journal",
    )
}
#[derive(Clone, Debug)]
pub struct Record {
    pub sequence: u64,
    pub quest: u64,
    pub task: u64,
    pub kind: u8,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}
impl Record {
    fn encode(&self, actor: u128) -> io::Result<Vec<u8>> {
        if self.before.len() + self.after.len() > 16 * 1024 * 1024 || !matches!(self.kind, 1 | 2) {
            return Err(invalid());
        }
        let mut b = b"DFQTX1".to_vec();
        b.extend(crate::PACK.as_bytes());
        b.extend(actor.to_le_bytes());
        for n in [self.sequence, self.quest, self.task] {
            b.extend(n.to_le_bytes());
        }
        b.push(self.kind);
        b.extend((self.before.len() as u32).to_le_bytes());
        b.extend((self.after.len() as u32).to_le_bytes());
        b.extend(&self.before);
        b.extend(&self.after);
        b.extend(checksum(&b).to_le_bytes());
        Ok(b)
    }
    fn decode(b: &[u8], actor: u128) -> io::Result<Self> {
        if b.len() < 127
            || &b[..6] != b"DFQTX1"
            || &b[6..70] != crate::PACK.as_bytes()
            || b[70..86] != actor.to_le_bytes()
        {
            return Err(invalid());
        }
        let u64at = |i| u64::from_le_bytes(b[i..i + 8].try_into().unwrap());
        let n = u32::from_le_bytes(b[111..115].try_into().unwrap()) as usize;
        let m = u32::from_le_bytes(b[115..119].try_into().unwrap()) as usize;
        if n + m > 16 * 1024 * 1024
            || b.len() != 127 + n + m
            || !matches!(b[110], 1 | 2)
            || u64at(b.len() - 8) != checksum(&b[..b.len() - 8])
        {
            return Err(invalid());
        }
        Ok(Self {
            sequence: u64at(86),
            quest: u64at(94),
            task: u64at(102),
            kind: b[110],
            before: b[119..119 + n].to_vec(),
            after: b[119 + n..119 + n + m].to_vec(),
        })
    }
}

/// The OS file lock is released even on process termination. Never delete it.
pub struct Store {
    directory: PathBuf,
    actor: u128,
    _lock: File,
    pub records: Vec<Record>,
}
impl Store {
    pub fn open(world: &Path, actor: u128) -> io::Result<Self> {
        let directory = world
            .join("dragonfyre-transactions-v1")
            .join(format!("{actor:032x}"));
        fs::create_dir_all(&directory)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("coordinator.lock"))?;
        lock.try_lock().map_err(io::Error::other)?;
        let mut paths = Vec::new();
        for e in fs::read_dir(&directory)? {
            let p = e?.path();
            if p.extension().is_some_and(|e| e == "txn") {
                paths.push(p);
            }
        }
        paths.sort();
        let mut records = Vec::new();
        for p in paths {
            let mut b = Vec::new();
            File::open(&p)?
                .take(16 * 1024 * 1024 + 128)
                .read_to_end(&mut b)?;
            let r = Record::decode(&b, actor)?;
            if r.sequence != records.len() as u64 + 1
                || p.file_name().unwrap() != format!("{:020}.txn", r.sequence).as_str()
            {
                return Err(invalid());
            }
            if records
                .iter()
                .any(|old: &Record| (old.kind, old.quest, old.task) == (r.kind, r.quest, r.task))
            {
                return Err(invalid());
            }
            records.push(r);
        }
        Ok(Self {
            directory,
            actor,
            _lock: lock,
            records,
        })
    }
    pub fn acknowledged(&self, r: &Record) -> io::Result<bool> {
        let path = self.directory.join(format!("{:020}.ack", r.sequence));
        match fs::read(&path) {
            Ok(b) if b == checksum(&r.encode(self.actor)?).to_le_bytes() => Ok(true),
            Ok(_) => Err(invalid()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }
    pub fn has(&self, kind: u8, quest: u64, task: u64) -> bool {
        self.records
            .iter()
            .any(|r| (r.kind, r.quest, r.task) == (kind, quest, task))
    }
    pub fn publish(&mut self, mut r: Record, phase: impl Fn(&str)) -> io::Result<()> {
        if self.has(r.kind, r.quest, r.task) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "already transacted",
            ));
        }
        r.sequence = self.records.len() as u64 + 1;
        let b = r.encode(self.actor)?;
        let temp = self.directory.join("unpublished.pending");
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp)?;
        f.write_all(&b)?;
        f.sync_all()?;
        phase("after_temp_sync");
        drop(f);
        fs::rename(temp, self.directory.join(format!("{:020}.txn", r.sequence)))?;
        sync_directory(&self.directory)?;
        self.records.push(r);
        phase("after_intent");
        Ok(())
    }
    pub fn acknowledge(&self, r: &Record) -> io::Result<()> {
        if self.acknowledged(r)? {
            return Ok(());
        }
        let p = self.directory.join(format!("{:020}.ack", r.sequence));
        let temp = self.directory.join("ack.pending");
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp)?;
        f.write_all(&checksum(&r.encode(self.actor)?).to_le_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(temp, p)?;
        sync_directory(&self.directory)
    }
}
#[cfg(unix)]
fn sync_directory(p: &Path) -> io::Result<()> {
    File::open(p)?.sync_all()
}
#[cfg(not(unix))]
fn sync_directory(_p: &Path) -> io::Result<()> {
    Ok(())
} // Process-crash contract; power-loss durability is not claimed on Windows.

#[cfg(test)]
mod tests {
    use super::*;
    fn slots() -> Vec<Option<Stack>> {
        vec![None; 36]
    }
    fn stack(n: u32) -> Option<Stack> {
        Some(Stack {
            item: "minecraft:obsidian".into(),
            count: n,
            plain: true,
        })
    }
    #[test]
    fn split_cost_and_reward_are_atomic() {
        let mut inv = slots();
        inv[0] = stack(2);
        inv[35] = stack(3);
        let (after, levels) = plan(
            &inv,
            &[("minecraft:obsidian".into(), 5)],
            &[("minecraft:diamond".into(), 2)],
            3,
            7,
            |_| Some(64),
        )
        .unwrap();
        assert_eq!(inv[0], stack(2));
        assert_eq!(levels, 10);
        assert_eq!(after[0].as_ref().unwrap().item, "minecraft:diamond");
        assert!(after[35].is_none());
    }
    #[test]
    fn short_cost_never_takes_items() {
        let mut i = slots();
        i[0] = stack(4);
        assert_eq!(
            plan(
                &i,
                &[("minecraft:obsidian".into(), 5)],
                &[],
                0,
                0,
                |_| Some(64)
            ),
            Err(PlanError::MissingItems)
        );
        assert_eq!(i[0], stack(4));
    }
    #[test]
    fn custom_components_are_never_spent_or_merged() {
        let mut i = slots();
        i[0] = stack(5);
        i[0].as_mut().unwrap().plain = false;
        assert_eq!(
            plan(
                &i,
                &[("minecraft:obsidian".into(), 1)],
                &[],
                0,
                0,
                |_| Some(64)
            ),
            Err(PlanError::MissingItems)
        );
    }
    #[test]
    fn full_inventory_rejects_every_reward_and_xp() {
        let i = vec![stack(64); 36];
        assert_eq!(
            plan(&i, &[], &[("minecraft:diamond".into(), 1)], 5, 2, |_| Some(
                64
            )),
            Err(PlanError::FullInventory)
        );
    }
    #[test]
    fn consumed_slot_can_hold_reward() {
        let i = vec![stack(64); 36];
        let (a, _) = plan(
            &i,
            &[("minecraft:obsidian".into(), 64)],
            &[("minecraft:diamond".into(), 64)],
            0,
            0,
            |_| Some(64),
        )
        .unwrap();
        assert_eq!(a[0].as_ref().unwrap().item, "minecraft:diamond");
    }
    #[test]
    fn unknown_item_and_xp_overflow_fail_closed() {
        assert_eq!(
            plan(&slots(), &[], &[("absent:item".into(), 1)], 0, 0, |_| None),
            Err(PlanError::UnknownItem)
        );
        assert_eq!(
            plan(&slots(), &[], &[], 1, i32::MAX, |_| Some(64)),
            Err(PlanError::Overflow)
        );
    }
    #[test]
    fn duplicate_costs_are_summed() {
        let mut i = slots();
        i[0] = stack(5);
        assert_eq!(
            plan(
                &i,
                &[
                    ("minecraft:obsidian".into(), 3),
                    ("minecraft:obsidian".into(), 3)
                ],
                &[],
                0,
                0,
                |_| Some(64)
            ),
            Err(PlanError::MissingItems)
        );
    }
    #[test]
    fn records_reject_every_truncation_and_wrong_actor() {
        let r = Record {
            sequence: 1,
            quest: 2,
            task: 0,
            kind: 1,
            before: vec![1],
            after: vec![2],
        };
        let b = r.encode(7).unwrap();
        for n in 0..b.len() {
            assert!(Record::decode(&b[..n], 7).is_err());
        }
        assert!(Record::decode(&b, 8).is_err());
        assert_eq!(Record::decode(&b, 7).unwrap().after, vec![2]);
    }

    fn world() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "dfq-tx-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn record() -> Record {
        Record {
            sequence: 0,
            quest: 1,
            task: 0,
            kind: 1,
            before: vec![1],
            after: vec![2],
        }
    }
    #[test]
    fn store_locks_reopens_and_refuses_duplicate_claims() {
        let p = world();
        {
            let mut s = Store::open(&p, 7).unwrap();
            assert!(Store::open(&p, 7).is_err());
            s.publish(record(), |_| {}).unwrap();
            assert!(s.publish(record(), |_| {}).is_err());
            assert!(!s.acknowledged(&s.records[0]).unwrap());
            s.acknowledge(&s.records[0]).unwrap();
        }
        {
            let s = Store::open(&p, 7).unwrap();
            assert_eq!(s.records.len(), 1);
            assert!(s.acknowledged(&s.records[0]).unwrap());
        }
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn unpublished_files_do_not_commit_and_corrupt_ack_is_rejected() {
        let p = world();
        {
            let mut s = Store::open(&p, 8).unwrap();
            fs::write(s.directory.join("unpublished.pending"), b"partial").unwrap();
            s.publish(record(), |_| {}).unwrap();
            fs::write(s.directory.join("00000000000000000001.ack"), b"partial").unwrap();
            assert!(s.acknowledged(&s.records[0]).is_err());
        }
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn journal_corruption_and_sequence_gaps_fail_closed() {
        let p = world();
        let file;
        {
            let mut s = Store::open(&p, 9).unwrap();
            s.publish(record(), |_| {}).unwrap();
            file = s.directory.join("00000000000000000001.txn");
        }
        let original = fs::read(&file).unwrap();
        fs::write(&file, b"partial").unwrap();
        assert!(Store::open(&p, 9).is_err());
        fs::write(&file, &original).unwrap();
        let moved = file.with_file_name("00000000000000000002.txn");
        fs::rename(&file, moved).unwrap();
        assert!(Store::open(&p, 9).is_err());
        fs::remove_dir_all(p).unwrap();
    }
}

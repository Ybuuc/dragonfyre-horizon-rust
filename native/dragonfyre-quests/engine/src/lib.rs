//! Original Rust progression experiment. Not an FTB implementation or a playable port.
//! Only non-repeatable, all-dependencies, checkmark-only task completion is enabled.
//! Rewards are deliberately unavailable until delivery can be committed with game saves.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub const PACK: &str = "a5d24acb0665fd7e5c161f6aba1e3d80add8118429c6e53530e30c0c7880be7b";
pub type Id = u64;

pub mod command;
pub mod economy;

static REQUERY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn request_requery() {
    REQUERY.store(true, std::sync::atomic::Ordering::Release);
}

pub fn take_requery() -> bool {
    REQUERY.swap(false, std::sync::atomic::Ordering::AcqRel)
}

struct WriterLock(PathBuf);
impl WriterLock {
    fn acquire(directory: &Path) -> io::Result<Self> {
        let path = directory.join("writer.lock");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self(path))
    }
}
impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Debug, Clone)]
pub struct Quest {
    pub id: Id,
    pub dependencies: Vec<Id>,
    pub tasks: Vec<Id>,
    /// None means the narrowly supported semantics above, not a ported quest.
    pub blocked: Option<&'static str>,
}

pub fn catalog() -> Vec<Quest> {
    include!("catalog.rs")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSpec {
    pub quest: Id,
    pub task: Id,
    pub item: String,
    pub count: u64,
}

pub fn items() -> Vec<ItemSpec> {
    include!("items.rs")
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    InvalidCatalog,
    UnknownQuest,
    UnknownTask,
    Locked,
    Unsupported(&'static str),
    CorruptJournal,
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// One authoritative writer per world/team. Callers supply the server's real team identity.
pub struct Progress {
    quests: BTreeMap<Id, Quest>,
    completed_tasks: BTreeSet<(Id, Id)>,
    items: Vec<ItemSpec>,
    directory: PathBuf,
    team: u128,
    sequence: u64,
}

impl Progress {
    pub fn open(
        world: &Path,
        team: u128,
        quests: Vec<Quest>,
        mut items: Vec<ItemSpec>,
    ) -> Result<Self, Error> {
        let count = quests.len();
        let map: BTreeMap<_, _> = quests.into_iter().map(|q| (q.id, q)).collect();
        let mut task_ids = BTreeSet::new();
        if map.len() != count
            || map.values().any(|q| {
                q.tasks.is_empty()
                    || q.tasks.iter().any(|t| !task_ids.insert(*t))
                    || q.dependencies.iter().any(|id| !map.contains_key(id))
            })
        {
            return Err(Error::InvalidCatalog);
        }
        // Detect cycles including paths through unsupported quests.
        fn visit(
            id: Id,
            map: &BTreeMap<Id, Quest>,
            active: &mut BTreeSet<Id>,
            done: &mut BTreeSet<Id>,
        ) -> bool {
            if done.contains(&id) {
                return true;
            }
            if !active.insert(id) {
                return false;
            }
            if map[&id]
                .dependencies
                .iter()
                .any(|d| !visit(*d, map, active, done))
            {
                return false;
            }
            active.remove(&id);
            done.insert(id);
            true
        }
        let mut done = BTreeSet::new();
        for id in map.keys() {
            if !visit(*id, &map, &mut BTreeSet::new(), &mut done) {
                return Err(Error::InvalidCatalog);
            }
        }
        let mut item_task_ids = BTreeSet::new();
        for spec in &items {
            if spec.count < 1 || spec.item.is_empty() || !item_task_ids.insert(spec.task) {
                return Err(Error::InvalidCatalog);
            }
            let Some(quest) = map.get(&spec.quest) else {
                return Err(Error::InvalidCatalog);
            };
            if quest.blocked.is_some() || !quest.tasks.contains(&spec.task) {
                return Err(Error::InvalidCatalog);
            }
        }
        items.sort_by_key(|s| (s.quest, s.task));
        let directory = world
            .join("dragonfyre-quests-prototype")
            .join(format!("{team:032x}"));
        fs::create_dir_all(&directory)?;
        let mut progress = Self {
            quests: map,
            completed_tasks: BTreeSet::new(),
            items,
            directory,
            team,
            sequence: 0,
        };
        let mut files = Vec::new();
        for entry in fs::read_dir(&progress.directory)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".event") {
                continue;
            } // Unpublished .pending is never progress.
            // The journal filename is an ASCII sequence number, not arbitrary text.
            if name.len() != 26 || !name.as_bytes()[..20].iter().all(u8::is_ascii_digit) {
                return Err(Error::CorruptJournal);
            }
            files.push((name, entry.path()));
        }
        files.sort();
        for (name, path) in files {
            let next = progress
                .sequence
                .checked_add(1)
                .ok_or(Error::CorruptJournal)?;
            if name != format!("{next:020}.event") {
                return Err(Error::CorruptJournal);
            }
            // Read at most one byte beyond the format limit. A corrupted file
            // must not allocate its entire contents before rejecting it.
            let mut bytes = Vec::new();
            fs::File::open(&path)?.take(513).read_to_end(&mut bytes)?;
            if bytes.len() > 512 {
                return Err(Error::CorruptJournal);
            }
            let text = std::str::from_utf8(&bytes).map_err(|_| Error::CorruptJournal)?;
            let fields: Vec<_> = text.trim_end_matches('\n').split(' ').collect();
            if fields.len() != 8
                || fields[0] != "DFQ1"
                || fields[1] != PACK
                || fields[2] != format!("{team:032x}")
                || fields[3] != next.to_string()
            {
                return Err(Error::CorruptJournal);
            }
            let id = Id::from_str_radix(fields[4], 16).map_err(|_| Error::CorruptJournal)?;
            let task = Id::from_str_radix(fields[5], 16).map_err(|_| Error::CorruptJournal)?;
            let canonical = progress.record(next, id, task);
            if text != canonical {
                return Err(Error::CorruptJournal);
            }
            if progress.validate(id, task).is_err() || !progress.completed_tasks.insert((id, task))
            {
                return Err(Error::CorruptJournal);
            }
            progress.sequence = next;
        }
        Ok(progress)
    }

    pub fn complete(&self, id: Id) -> bool {
        self.quests.get(&id).is_some_and(|q| {
            q.blocked.is_none()
                && q.tasks
                    .iter()
                    .all(|t| self.completed_tasks.contains(&(id, *t)))
        })
    }

    pub fn status(&self, id: Id) -> Result<command::Reply, Error> {
        let q = self.quests.get(&id).ok_or(Error::UnknownQuest)?;
        if let Some(reason) = q.blocked {
            return Err(Error::Unsupported(reason));
        }
        if self.complete(id) {
            return Ok(command::Reply::Complete);
        }
        if !q.dependencies.iter().all(|d| self.complete(*d)) {
            return Ok(command::Reply::Locked);
        }
        Ok(command::Reply::Available)
    }

    fn validate(&self, id: Id, task: Id) -> Result<(), Error> {
        let quest = self.quests.get(&id).ok_or(Error::UnknownQuest)?;
        if let Some(reason) = quest.blocked {
            return Err(Error::Unsupported(reason));
        }
        if !quest.tasks.contains(&task) {
            return Err(Error::UnknownTask);
        }
        if !quest.dependencies.iter().all(|d| self.complete(*d)) {
            return Err(Error::Locked);
        }
        Ok(())
    }

    /// Returns false for a duplicate completion. No writes or reward delivery on replay.
    pub fn checkmark(&mut self, id: Id, task: Id) -> Result<bool, Error> {
        self.validate(id, task)?;
        if self.completed_tasks.contains(&(id, task)) {
            return Ok(false);
        }
        let next = self.sequence.checked_add(1).ok_or(Error::CorruptJournal)?;
        let pending = self
            .directory
            .join(format!("{next:020}.{}.pending", std::process::id()));
        let committed = self.directory.join(format!("{next:020}.event"));
        let _writer_lock = WriterLock::acquire(&self.directory)?;
        if committed.try_exists()? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "journal advanced; reopen progress",
            )
            .into());
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        let result = (|| -> io::Result<()> {
            file.write_all(self.record(next, id, task).as_bytes())?;
            file.sync_all()?;
            drop(file);
            // All writers hold writer.lock, and the destination was checked under it.
            // A process crash leaves the lock in place; never steal it automatically.
            fs::rename(&pending, &committed)
        })();
        let _ = fs::remove_file(&pending);
        result?;
        self.completed_tasks.insert((id, task));
        self.sequence = next;
        Ok(true)
    }

    pub fn claim_rewards(&self, _quest: Id) -> Result<(), Error> {
        Err(Error::Unsupported(
            "reward delivery and its transaction with Horizon saves are not implemented",
        ))
    }

    pub fn item_detection(
        &mut self,
        stacks: impl IntoIterator<Item = (String, u64)>,
    ) -> Result<Vec<(Id, Id)>, Error> {
        let mut totals: BTreeMap<String, u64> = BTreeMap::new();
        for (item, count) in stacks {
            totals
                .entry(item)
                .and_modify(|c| *c = c.saturating_add(count))
                .or_insert(count);
        }
        let mut completed_in_pass = Vec::new();
        loop {
            let mut newly_completed = false;
            for spec in self.items.clone() {
                if self.completed_tasks.contains(&(spec.quest, spec.task)) {
                    continue;
                }
                let quest = &self.quests[&spec.quest];
                if quest.blocked.is_some() || !quest.dependencies.iter().all(|d| self.complete(*d))
                {
                    continue;
                }
                let available = totals.get(&spec.item).copied().unwrap_or(0);
                if available >= spec.count {
                    let changed = self.checkmark(spec.quest, spec.task)?;
                    if changed {
                        completed_in_pass.push((spec.quest, spec.task));
                        newly_completed = true;
                    }
                }
            }
            if !newly_completed {
                break;
            }
        }
        completed_in_pass.sort();
        Ok(completed_in_pass)
    }

    fn record(&self, sequence: u64, id: Id, task: Id) -> String {
        let body = format!(
            "DFQ1 {PACK} {:032x} {sequence} {id:016X} {task:016X} checkmark",
            self.team
        );
        // Corruption check, not an authentication/security boundary.
        let checksum = body.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        });
        format!("{body} {checksum:016X}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NUMBER: AtomicU64 = AtomicU64::new(0);
    struct World(PathBuf);
    impl World {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "dfq-{}-{}",
                std::process::id(),
                NUMBER.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for World {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    // Exact original Dragonfyre quest/task IDs from age_of_dragons.snbt.
    const ROOT: Id = 0x290D9D572A5D6C04;
    const ROOT_TASK: Id = 0x54FE38A7CC3A8260;
    const CHILD: Id = 0x292408BF95DC3F81;
    const CHILD_TASK: Id = 0x764B012760F10FF4;

    #[test]
    fn original_chain_is_locked_then_survives_restart_without_duplicate_completion() {
        let world = World::new();
        let mut p = Progress::open(&world.0, 7, catalog(), items()).unwrap();
        assert!(matches!(p.checkmark(CHILD, CHILD_TASK), Err(Error::Locked)));
        assert!(p.checkmark(ROOT, ROOT_TASK).unwrap());
        assert!(!p.checkmark(ROOT, ROOT_TASK).unwrap());
        drop(p);
        let mut p = Progress::open(&world.0, 7, catalog(), items()).unwrap();
        assert!(p.complete(ROOT));
        assert!(p.checkmark(CHILD, CHILD_TASK).unwrap());
        assert!(p.complete(CHILD));
        assert!(matches!(p.claim_rewards(CHILD), Err(Error::Unsupported(_))));
        assert!(
            !Progress::open(&world.0, 8, catalog(), items())
                .unwrap()
                .complete(ROOT)
        );
    }

    #[test]
    fn unsupported_item_or_advancement_cannot_be_completed_as_checkmark() {
        let world = World::new();
        let mut p = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        assert!(matches!(
            p.checkmark(0x0741942A0CF808E5, 0x45479C715B1496AC),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(p.checkmark(ROOT, 123), Err(Error::UnknownTask)));
        assert!(matches!(p.checkmark(123, 123), Err(Error::UnknownQuest)));
        assert!(matches!(
            p.checkmark(0x3C1CDBC749329522, 0x26D9FA8E0423F0F9),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn competing_writer_cannot_overwrite_or_claim_a_committed_event() {
        let world = World::new();
        let mut first = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        let mut stale = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        first.checkmark(ROOT, ROOT_TASK).unwrap();
        assert!(matches!(
            stale.checkmark(ROOT, ROOT_TASK),
            Err(Error::Io(_))
        ));
        assert!(!stale.complete(ROOT));
        assert!(
            Progress::open(&world.0, 1, catalog(), items())
                .unwrap()
                .complete(ROOT)
        );
    }

    #[test]
    fn corruption_gaps_and_wrong_pack_are_rejected_not_reset() {
        let world = World::new();
        let mut p = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        p.checkmark(ROOT, ROOT_TASK).unwrap();
        let path = p.directory.join("00000000000000000001.event");
        let bytes = fs::read(&path).unwrap();
        let mut altered = bytes.clone();
        altered[5] = b'0';
        fs::write(&path, &altered).unwrap();
        assert!(matches!(
            Progress::open(&world.0, 1, catalog(), items()),
            Err(Error::CorruptJournal)
        ));
        fs::write(&path, &bytes).unwrap();
        fs::rename(&path, p.directory.join("00000000000000000002.event")).unwrap();
        assert!(matches!(
            Progress::open(&world.0, 1, catalog(), items()),
            Err(Error::CorruptJournal)
        ));
    }

    #[test]
    fn unfinished_write_never_grants_progress_and_failed_write_never_changes_memory() {
        let world = World::new();
        let mut p = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        let pending = p.directory.join(format!(
            "00000000000000000001.{}.pending",
            std::process::id()
        ));
        fs::write(&pending, b"incomplete").unwrap();
        assert!(matches!(p.checkmark(ROOT, ROOT_TASK), Err(Error::Io(_))));
        assert!(!p.complete(ROOT));
        assert!(
            !Progress::open(&world.0, 1, catalog(), items())
                .unwrap()
                .complete(ROOT)
        );
    }

    #[test]
    fn catalog_rejects_cycles_and_missing_dependencies() {
        let world = World::new();
        let q = |id, dependencies| Quest {
            id,
            dependencies,
            tasks: vec![id + 10],
            blocked: None,
        };
        assert!(matches!(
            Progress::open(&world.0, 1, vec![q(1, vec![2])], vec![]),
            Err(Error::InvalidCatalog)
        ));
        assert!(matches!(
            Progress::open(&world.0, 1, vec![q(1, vec![2]), q(2, vec![1])], vec![]),
            Err(Error::InvalidCatalog)
        ));
    }

    #[test]
    fn abandoned_writer_lock_blocks_new_writes_without_resetting_progress() {
        let world = World::new();
        let mut p = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        p.checkmark(ROOT, ROOT_TASK).unwrap();
        fs::write(p.directory.join("writer.lock"), b"abandoned").unwrap();
        let mut reopened = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        assert!(reopened.complete(ROOT));
        assert!(matches!(
            reopened.checkmark(CHILD, CHILD_TASK),
            Err(Error::Io(_))
        ));
        assert!(!reopened.complete(CHILD));
    }

    #[test]
    fn malformed_names_and_oversized_events_are_rejected_without_reset() {
        let world = World::new();
        let mut p = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        p.checkmark(ROOT, ROOT_TASK).unwrap();
        let event = p.directory.join("00000000000000000001.event");
        let original = fs::read(&event).unwrap();
        let malformed = p.directory.join("000000000000000000é.event");
        fs::rename(&event, &malformed).unwrap();
        assert!(matches!(
            Progress::open(&world.0, 1, catalog(), items()),
            Err(Error::CorruptJournal)
        ));
        fs::rename(&malformed, &event).unwrap();
        fs::write(&event, vec![b'x'; 1024 * 1024]).unwrap();
        assert!(matches!(
            Progress::open(&world.0, 1, catalog(), items()),
            Err(Error::CorruptJournal)
        ));
        // Rejection preserves the file, rather than silently repairing/resetting it.
        assert_eq!(fs::metadata(&event).unwrap().len(), 1024 * 1024);
        fs::write(&event, original).unwrap();
        assert!(
            Progress::open(&world.0, 1, catalog(), items())
                .unwrap()
                .complete(ROOT)
        );
    }

    #[test]
    fn valid_checksum_does_not_bypass_replay_semantics() {
        let world = World::new();
        let p = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        let event = p.directory.join("00000000000000000001.event");
        // These records have a valid checksum but cannot be legitimate progress.
        for (quest, task) in [(CHILD, CHILD_TASK), (ROOT, 123), (123, ROOT_TASK)] {
            fs::write(&event, p.record(1, quest, task)).unwrap();
            assert!(matches!(
                Progress::open(&world.0, 1, catalog(), items()),
                Err(Error::CorruptJournal)
            ));
        }
        fs::write(&event, p.record(1, ROOT, ROOT_TASK)).unwrap();
        fs::write(
            p.directory.join("00000000000000000002.event"),
            p.record(2, ROOT, ROOT_TASK),
        )
        .unwrap();
        assert!(matches!(
            Progress::open(&world.0, 1, catalog(), items()),
            Err(Error::CorruptJournal)
        ));
    }

    #[test]
    fn every_truncated_record_is_rejected_and_actor_transplant_is_rejected() {
        let world = World::new();
        let p = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        let event = p.directory.join("00000000000000000001.event");
        let record = p.record(1, ROOT, ROOT_TASK);
        for length in 0..record.len() {
            fs::write(&event, &record.as_bytes()[..length]).unwrap();
            assert!(
                matches!(
                    Progress::open(&world.0, 1, catalog(), items()),
                    Err(Error::CorruptJournal)
                ),
                "accepted truncated event at byte {length}"
            );
        }
        fs::write(&event, &record).unwrap();
        assert!(
            Progress::open(&world.0, 1, catalog(), items())
                .unwrap()
                .complete(ROOT)
        );
        let other = Progress::open(&world.0, 2, catalog(), items()).unwrap();
        fs::write(other.directory.join("00000000000000000001.event"), record).unwrap();
        assert!(matches!(
            Progress::open(&world.0, 2, catalog(), items()),
            Err(Error::CorruptJournal)
        ));
    }

    #[test]
    fn item_detection_exact_match_completes_task() {
        let world = World::new();
        let q = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let spec = ItemSpec {
            quest: 100,
            task: 101,
            item: "minecraft:iron_ingot".into(),
            count: 5,
        };
        let mut p = Progress::open(&world.0, 1, vec![q], vec![spec]).unwrap();
        assert_eq!(p.status(100).unwrap(), command::Reply::Available);
        let completed = p
            .item_detection([("minecraft:iron_ingot".into(), 5)])
            .unwrap();
        assert_eq!(completed, vec![(100, 101)]);
        assert!(p.complete(100));
        assert_eq!(p.status(100).unwrap(), command::Reply::Complete);
    }

    #[test]
    fn item_detection_short_count_does_not_complete() {
        let world = World::new();
        let q = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let spec = ItemSpec {
            quest: 100,
            task: 101,
            item: "minecraft:iron_ingot".into(),
            count: 5,
        };
        let mut p = Progress::open(&world.0, 1, vec![q], vec![spec]).unwrap();
        let completed = p
            .item_detection([("minecraft:iron_ingot".into(), 4)])
            .unwrap();
        assert!(completed.is_empty());
        assert!(!p.complete(100));
        assert_eq!(p.status(100).unwrap(), command::Reply::Available);
    }

    #[test]
    fn item_detection_wrong_item_does_not_complete() {
        let world = World::new();
        let q = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let spec = ItemSpec {
            quest: 100,
            task: 101,
            item: "minecraft:iron_ingot".into(),
            count: 1,
        };
        let mut p = Progress::open(&world.0, 1, vec![q], vec![spec]).unwrap();
        let completed = p
            .item_detection([("minecraft:gold_ingot".into(), 64)])
            .unwrap();
        assert!(completed.is_empty());
        assert!(!p.complete(100));
    }

    #[test]
    fn item_detection_count_cap_parity_and_saturating_add() {
        let world = World::new();
        let q = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let spec = ItemSpec {
            quest: 100,
            task: 101,
            item: "cisco_mod:champion_coin".into(),
            count: 50_000,
        };
        let mut p = Progress::open(&world.0, 1, vec![q], vec![spec]).unwrap();
        // Stacks: 35,000 + 20,000 = 55,000 >= 50,000; also test near u64::MAX without overflow
        let completed = p
            .item_detection([
                ("cisco_mod:champion_coin".into(), 35_000),
                ("cisco_mod:champion_coin".into(), 20_000),
                ("other:item".into(), u64::MAX),
                ("other:item".into(), 10),
            ])
            .unwrap();
        assert_eq!(completed, vec![(100, 101)]);
        assert!(p.complete(100));
    }

    #[test]
    fn item_detection_cascade_with_pre_placed_items() {
        let world = World::new();
        let q1 = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let q2 = Quest {
            id: 200,
            dependencies: vec![100],
            tasks: vec![201],
            blocked: None,
        };
        let s1 = ItemSpec {
            quest: 100,
            task: 101,
            item: "minecraft:oak_log".into(),
            count: 1,
        };
        let s2 = ItemSpec {
            quest: 200,
            task: 201,
            item: "minecraft:oak_planks".into(),
            count: 4,
        };
        let mut p = Progress::open(&world.0, 1, vec![q1, q2], vec![s1, s2]).unwrap();
        assert_eq!(p.status(200).unwrap(), command::Reply::Locked);
        let completed = p
            .item_detection([
                ("minecraft:oak_log".into(), 1),
                ("minecraft:oak_planks".into(), 4),
            ])
            .unwrap();
        assert_eq!(completed, vec![(100, 101), (200, 201)]);
        assert!(p.complete(100));
        assert!(p.complete(200));
    }

    #[test]
    fn item_detection_locked_skip() {
        let world = World::new();
        let q1 = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let q2 = Quest {
            id: 200,
            dependencies: vec![100],
            tasks: vec![201],
            blocked: None,
        };
        let s2 = ItemSpec {
            quest: 200,
            task: 201,
            item: "minecraft:diamond".into(),
            count: 1,
        };
        let mut p = Progress::open(&world.0, 1, vec![q1, q2], vec![s2]).unwrap();
        assert_eq!(p.status(200).unwrap(), command::Reply::Locked);
        let completed = p
            .item_detection([("minecraft:diamond".into(), 10)])
            .unwrap();
        assert!(completed.is_empty());
        assert!(!p.complete(200));
    }

    #[test]
    fn invalid_spec_rejections() {
        let world = World::new();
        let valid_q = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let blocked_q = Quest {
            id: 200,
            dependencies: vec![],
            tasks: vec![201],
            blocked: Some("unsupported"),
        };
        let mk_s = |q, t, item: &str, count| ItemSpec {
            quest: q,
            task: t,
            item: item.into(),
            count,
        };

        // Blocked quest
        assert!(matches!(
            Progress::open(
                &world.0,
                1,
                vec![valid_q.clone(), blocked_q.clone()],
                vec![mk_s(200, 201, "item", 1)]
            ),
            Err(Error::InvalidCatalog)
        ));
        // Unknown quest
        assert!(matches!(
            Progress::open(
                &world.0,
                1,
                vec![valid_q.clone()],
                vec![mk_s(999, 101, "item", 1)]
            ),
            Err(Error::InvalidCatalog)
        ));
        // Unknown task
        assert!(matches!(
            Progress::open(
                &world.0,
                1,
                vec![valid_q.clone()],
                vec![mk_s(100, 999, "item", 1)]
            ),
            Err(Error::InvalidCatalog)
        ));
        // Duplicate task ID
        assert!(matches!(
            Progress::open(
                &world.0,
                1,
                vec![valid_q.clone()],
                vec![mk_s(100, 101, "item_a", 1), mk_s(100, 101, "item_b", 1),]
            ),
            Err(Error::InvalidCatalog)
        ));
        // Count == 0
        assert!(matches!(
            Progress::open(
                &world.0,
                1,
                vec![valid_q.clone()],
                vec![mk_s(100, 101, "item", 0)]
            ),
            Err(Error::InvalidCatalog)
        ));
        // Empty item string
        assert!(matches!(
            Progress::open(
                &world.0,
                1,
                vec![valid_q.clone()],
                vec![mk_s(100, 101, "", 1)]
            ),
            Err(Error::InvalidCatalog)
        ));
    }

    #[test]
    fn item_detection_duplicate_detection_no_second_journal() {
        let world = World::new();
        let q = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let s = ItemSpec {
            quest: 100,
            task: 101,
            item: "minecraft:feather".into(),
            count: 1,
        };
        let mut p = Progress::open(&world.0, 1, vec![q], vec![s]).unwrap();
        let completed = p.item_detection([("minecraft:feather".into(), 1)]).unwrap();
        assert_eq!(completed, vec![(100, 101)]);
        let files_before = fs::read_dir(&p.directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "event"))
            .count();
        assert_eq!(files_before, 1);

        // Run detection again with the same item
        let completed_again = p.item_detection([("minecraft:feather".into(), 1)]).unwrap();
        assert!(completed_again.is_empty());
        let files_after = fs::read_dir(&p.directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "event"))
            .count();
        assert_eq!(files_after, 1);
    }

    #[test]
    fn item_detection_io_writer_lock_error_propagation_without_memory_change() {
        let world = World::new();
        let q = Quest {
            id: 100,
            dependencies: vec![],
            tasks: vec![101],
            blocked: None,
        };
        let s = ItemSpec {
            quest: 100,
            task: 101,
            item: "minecraft:coal".into(),
            count: 1,
        };
        let mut p = Progress::open(&world.0, 1, vec![q], vec![s]).unwrap();
        // Drop an abandoned writer lock to simulate writer contention
        fs::write(p.directory.join("writer.lock"), b"locked").unwrap();

        let res = p.item_detection([("minecraft:coal".into(), 1)]);
        assert!(matches!(res, Err(Error::Io(_))));
        assert!(!p.complete(100));
        assert!(!p.completed_tasks.contains(&(100, 101)));
    }

    #[test]
    fn item_detection_real_catalog_obsidian_smoke() {
        let world = World::new();
        let obsidian_quest: Id = 0x643A6BD7BE518653;
        let obsidian_task: Id = 0x6CF1A08007E1F61A;

        let mut p = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        assert_eq!(p.status(obsidian_quest).unwrap(), command::Reply::Available);

        let completed = p
            .item_detection([("minecraft:obsidian".into(), 1)])
            .unwrap();
        assert_eq!(completed, vec![(obsidian_quest, obsidian_task)]);
        assert!(p.complete(obsidian_quest));
        assert_eq!(p.status(obsidian_quest).unwrap(), command::Reply::Complete);

        drop(p);
        let reopened = Progress::open(&world.0, 1, catalog(), items()).unwrap();
        assert!(reopened.complete(obsidian_quest));
        assert_eq!(
            reopened.status(obsidian_quest).unwrap(),
            command::Reply::Complete
        );
    }
}

// Test the engine's persistence in two processes. The payload bytes here are
// opaque fixtures; the live Horizon NBT, inventory and UUID routing are tested
// separately by verify_native_stage07.py.
use dragonfyre_quests_engine::{
    Progress, catalog, items,
    economy::{HAM_EXCHANGE, Record, Store, operation_slot},
};
use std::{io::ErrorKind, path::Path};

const RUBY: u64 = 0x5AFA5CB101E31BE4;
const CORE: u64 = 0x288608ED892DC5EB;
const A: u128 = 0x7a;
const B: u128 = 0x7b;

fn record(kind: u8, cycle: Option<u64>) -> Record {
    Record {
        sequence: 0,
        quest: HAM_EXCHANGE,
        task: operation_slot(HAM_EXCHANGE, cycle).unwrap(),
        kind,
        before: b"fixture before".to_vec(),
        after: b"fixture after".to_vec(),
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let world = Path::new(&args[1]);
    let mut progress = Progress::open(world, A, catalog(), items()).unwrap();
    let mut journal = Store::open(world, A).unwrap();
    if args[2] == "write" {
        let mut completed = progress
            .item_detection([
                ("cisco_mod:radiant_ruby".into(), 1),
                ("cisco_mod:divine_core".into(), 1),
            ])
            .unwrap();
        completed.sort_unstable();
        assert_eq!(
            completed,
            [(CORE, 0x4F241D893DBF361F), (RUBY, 0x022397B35A0E22E3)]
        );
        for (kind, cycle) in [(2, None), (1, Some(1)), (2, Some(2))] {
            journal.publish(record(kind, cycle), |_| {}).unwrap();
            journal.acknowledge(journal.records.last().unwrap()).unwrap();
        }
        println!("PASS: original item tasks and two distinct repeat cycles recorded");
    } else {
        assert_eq!(args[2], "reopen");
        assert!(progress.complete(RUBY) && progress.complete(CORE));
        let other = Progress::open(world, B, catalog(), items()).unwrap();
        assert!(!other.complete(RUBY) && !other.complete(CORE));
        assert!(Store::open(world, B).unwrap().records.is_empty());
        assert_eq!(journal.records.len(), 3);
        assert!(journal.has(2, HAM_EXCHANGE, 1) && journal.has(1, HAM_EXCHANGE, 1));
        assert!(journal.has(2, HAM_EXCHANGE, 2) && !journal.has(1, HAM_EXCHANGE, 2));
        for row in &journal.records {
            assert!(journal.acknowledged(row).unwrap());
        }
        assert_eq!(
            journal.publish(record(2, Some(1)), |_| {}).unwrap_err().kind(),
            ErrorKind::AlreadyExists,
        );
        assert_eq!(
            journal.publish(record(2, Some(2)), |_| {}).unwrap_err().kind(),
            ErrorKind::AlreadyExists,
        );
        println!("PASS: reopened in a new process; quest completion, cycle state and actor isolation persisted");
    }
}

//! Development commands; the actor comes from the server, never from command arguments.
use crate::{Error, Id, Progress, catalog};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Complete,
    Available,
    Locked,
    Changed,
    AlreadyCompleted,
    Unsupported,
    UnknownQuest,
    UnknownTask,
    InvalidSyntax,
    Forbidden,
    StorageFailure,
}

fn parse_id(text: &str) -> Option<Id> {
    (text.len() == 16 && text.bytes().all(|c| c.is_ascii_hexdigit()))
        .then(|| Id::from_str_radix(text, 16).ok())
        .flatten()
}

/// None leaves unrelated commands to other mods. No inventory/XP mutation occurs here.
pub fn perform(world: &Path, actor: u128, permission: u8, line: &str) -> Option<Reply> {
    let words: Vec<_> = line.split_whitespace().collect();
    if words.first().copied() != Some("dragonfyredev") {
        return None;
    }
    if permission < 3 {
        return Some(Reply::Forbidden);
    }
    let result = match words.as_slice() {
        [_, "status", quest] => parse_id(quest).map(|id| (false, id, None)),
        [_, "checkmark", quest, task] => parse_id(quest)
            .zip(parse_id(task))
            .map(|(q, t)| (true, q, Some(t))),
        _ => None,
    };
    let Some((check, quest, task)) = result else {
        return Some(Reply::InvalidSyntax);
    };
    let quests = catalog();
    let Some(definition) = quests.iter().find(|q| q.id == quest) else {
        return Some(Reply::UnknownQuest);
    };
    if definition.blocked.is_some() {
        return Some(Reply::Unsupported);
    }
    if task.is_some_and(|id| !definition.tasks.contains(&id)) {
        return Some(Reply::UnknownTask);
    }
    let result = Progress::open(world, actor, quests, crate::items()).and_then(|mut state| {
        if check {
            state
                .checkmark(quest, task.expect("checkmark has a parsed task"))
                .map(|changed| {
                    if changed {
                        crate::request_requery();
                        Reply::Changed
                    } else {
                        Reply::AlreadyCompleted
                    }
                })
        } else {
            state.status(quest)
        }
    });
    Some(match result {
        Ok(reply) => reply,
        Err(Error::Locked) => Reply::Locked,
        Err(Error::UnknownQuest) => Reply::UnknownQuest,
        Err(Error::UnknownTask) => Reply::UnknownTask,
        Err(Error::Unsupported(_)) => Reply::Unsupported,
        Err(error) => {
            eprintln!("dragonfyre quest development storage error: {error:?}");
            Reply::StorageFailure
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct World(std::path::PathBuf);
    impl World {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "dfq-router-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
        fn call(&self, actor: u128, line: &str) -> Option<Reply> {
            perform(&self.0, actor, 3, line)
        }
    }
    impl Drop for World {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn authoritative_actor_and_world_isolation_survive_reopening_on_each_command() {
        let w = World::new();
        let other = World::new();
        assert_eq!(
            w.call(1, "dragonfyredev status 292408BF95DC3F81"),
            Some(Reply::Locked)
        );
        assert_eq!(
            w.call(
                1,
                "dragonfyredev checkmark 290D9D572A5D6C04 54FE38A7CC3A8260"
            ),
            Some(Reply::Changed)
        );
        assert_eq!(
            w.call(1, "dragonfyredev status 290D9D572A5D6C04"),
            Some(Reply::Complete)
        );
        assert_eq!(
            w.call(2, "dragonfyredev status 290D9D572A5D6C04"),
            Some(Reply::Available)
        );
        assert_eq!(
            other.call(1, "dragonfyredev status 290D9D572A5D6C04"),
            Some(Reply::Available)
        );
        assert_eq!(
            w.call(
                1,
                "dragonfyredev checkmark 292408BF95DC3F81 764B012760F10FF4"
            ),
            Some(Reply::Changed)
        );
        assert_eq!(
            w.call(
                1,
                "dragonfyredev checkmark 292408BF95DC3F81 764B012760F10FF4"
            ),
            Some(Reply::AlreadyCompleted)
        );
    }
    #[test]
    fn commands_cannot_choose_another_actor_or_bypass_unsupported_tasks() {
        let w = World::new();
        assert_eq!(
            w.call(
                1,
                "dragonfyredev checkmark 290D9D572A5D6C04 54FE38A7CC3A8260 2"
            ),
            Some(Reply::InvalidSyntax)
        );
        assert_eq!(
            w.call(
                1,
                "dragonfyredev checkmark 0741942A0CF808E5 45479C715B1496AC"
            ),
            Some(Reply::Unsupported)
        );
        assert_eq!(
            perform(&w.0, 1, 0, "dragonfyredev status 290D9D572A5D6C04"),
            Some(Reply::Forbidden)
        );
        assert!(!w.0.exists());
    }
    #[test]
    fn unrelated_commands_are_untouched_and_invalid_ids_do_not_create_state() {
        let w = World::new();
        for line in ["", "say hi", "dragonfyredevother status"] {
            assert_eq!(w.call(1, line), None);
        }
        for line in [
            "dragonfyredev",
            "dragonfyredev status ../escape",
            "dragonfyredev status +000000000000001",
            "dragonfyredev claim 290D9D572A5D6C04",
        ] {
            assert_eq!(w.call(1, line), Some(Reply::InvalidSyntax));
        }
        assert!(!w.0.exists());
    }
}

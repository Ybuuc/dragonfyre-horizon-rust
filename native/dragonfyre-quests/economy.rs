// Same-crate transactional server glue. Only the server tick owns mutations.
use crate::game::Game;
use dragonfyre_quests_engine::{
    self as engine,
    economy::{self, Definition, Record, Stack, Store},
};
use horizon_client::{items::ItemStack, lan::playerdata, save::Level};
use horizon_world::{
    attachments::{AttachmentCodec, AttachmentError, AttachmentType},
    nbt::{self, Compound, Limits, NamedTag, Tag},
};
use std::{
    sync::mpsc::Sender,
    path::{Path, PathBuf},
    sync::Mutex,
};

struct Request {
    world: PathBuf,
    actor: u128,
    quest: u64,
    kind: u8,
    slot: u64,
    reply: Option<Sender<Result<(), String>>>,
}
static QUEUE: Mutex<Vec<Request>> = Mutex::new(Vec::new());
static MARKER: AttachmentType<String> = AttachmentType {
    namespace: "dragonfyre_quests",
    name: "economy_sequence_v1",
    default: String::new,
    codec: Some(AttachmentCodec {
        encode: |s| {
            Ok([("identity".into(), Tag::String(s.as_str().into()))]
                .into_iter()
                .collect())
        },
        decode: |m| match m.get(&"identity".into()) {
            Some(Tag::String(s)) => s.to_string().map_err(|_| AttachmentError::Decode),
            _ => Err(AttachmentError::Decode),
        },
    }),
    copy_on_death: true,
    sync: false,
};
fn seq(game: &mut Game<'_>, actor: u128) -> Result<u64, String> {
    let a = game.guest_attachments.entry(actor).or_default();
    let s = a.get(&MARKER).map_err(|e| format!("marker: {e:?}"))?;
    if s.is_empty() {
        return Ok(0);
    }
    let prefix = format!("{actor:032x}:{}:", engine::PACK);
    s.strip_prefix(&prefix)
        .ok_or("transaction marker belongs to another actor/pack")?
        .parse()
        .map_err(|_| "invalid sequence".into())
}
fn setseq(game: &mut Game<'_>, actor: u128, n: u64) -> Result<(), String> {
    game.guest_attachments
        .entry(actor)
        .or_default()
        .set(&MARKER, format!("{actor:032x}:{}:{n}", engine::PACK))
        .map_err(|e| format!("marker encode: {e:?}"))
}
fn definitions() -> Vec<Definition> {
    let mut d = economy::definitions();
    // Explicit developer fixture, never a substitute for original pack content.
    if std::env::var_os("HORIZON_DFQ_ECONOMY_FIXTURE").is_some() {
        d.push(Definition {
            quest: 0xFFFFFFFFFFFFFF06,
            costs: vec![(0xFFFFFFFFFFFFFF16, "minecraft:obsidian".into(), 5)],
            items: vec![("minecraft:diamond".into(), 2)],
            levels: 3,
            blocked: None,
        });
    }
    d
}
fn definition(q: u64) -> Result<Definition, String> {
    definitions()
        .into_iter()
        .find(|d| d.quest == q)
        .filter(|d| d.blocked.is_none())
        .ok_or("unsupported".into())
}
fn complete(world: &Path, actor: u128, q: u64, d: &Definition, s: &Store, slot: u64) -> Result<bool, String> {
    if !d.costs.is_empty() {
        return Ok(s.has(2, q, slot));
    }
    engine::Progress::open(world, actor, engine::catalog(), engine::items())
        .map(|p| p.complete(q))
        .map_err(|e| format!("progress: {e:?}"))
}
pub fn enqueue(game: &Game<'_>, actor: u128, quest: u64, kind: u8, slot: u64) {
    if let Some(world) = game.native_world_dir() {
        QUEUE.lock().unwrap().push(Request { world, actor, quest, kind, slot, reply: None });
    }
}
pub const JOURNAL_QUESTS: [u64; 6] = [
    0x5AFA5CB101E31BE4,
    0x288608ED892DC5EB,
    economy::HAM_EXCHANGE,
    0x643A6BD7BE518653,
    0x7D59BD1308E6778F,
    0x3E13EB56EE51145F,
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JournalState {
    Locked,
    Collect,
    Submit,
    Claim,
    Claimed,
}

#[derive(Clone, Copy)]
pub struct JournalRow {
    pub quest: u64,
    pub cycle: u64,
    pub count: u64,
    pub state: JournalState,
}

pub struct Journal {
    pub rows: [JournalRow; JOURNAL_QUESTS.len()],
    pub exchanges: u64,
}

fn journal_owner(game: &Game<'_>) -> Result<u128, String> {
    let host = game.lan.as_ref().ok_or("player unavailable")?;
    let actor = host.owner.as_ref().ok_or("player unavailable")?.uuid;
    host.hub.guests.iter()
        .find(|g| g.uuid == actor && g.placed && !g.dead)
        .map(|g| g.uuid)
        .ok_or("player unavailable".into())
}

pub fn journal(game: &mut Game<'_>) -> Result<Journal, String> {
    let actor = journal_owner(game)?;
    let world = game.native_world_dir().ok_or("no world")?;
    let store = Store::open(&world, actor).map_err(|e| e.to_string())?;
    if seq(game, actor)? != store.records.len() as u64 {
        return Err("unrecovered transaction sequence".into());
    }
    let progress = engine::Progress::open(&world, actor, engine::catalog(), engine::items())
        .map_err(|e| format!("progress: {e:?}"))?;
    let guest = game.lan.as_ref().and_then(|host| host.hub.guests.iter().find(|g| g.uuid == actor))
        .ok_or("player unavailable")?;
    let amount = |id: &str, plain: bool| -> u64 {
        guest.inventory.iter().take(36).flatten()
            .filter(|stack| &*stack.item == id && (!plain || (stack.damage == 0 && stack.extra.is_none())))
            .map(|stack| u64::from(stack.count))
            .sum()
    };
    let exchanges = store.records.iter().filter(|r| r.quest == economy::HAM_EXCHANGE && r.kind == 1)
        .map(|r| r.task).max().unwrap_or(0);
    let cycle = exchanges.checked_add(1).ok_or("exchange cycle overflow")?;
    let quest_row = |quest: u64, count: u64| {
        let state = if store.has(1, quest, 0) {
            JournalState::Claimed
        } else if progress.complete(quest) {
            JournalState::Claim
        } else if matches!(progress.status(quest), Ok(engine::command::Reply::Locked)) {
            JournalState::Locked
        } else {
            JournalState::Collect
        };
        JournalRow { quest, cycle: 0, count, state }
    };
    let coins = amount("cisco_mod:champion_coin", true);
    Ok(Journal {
        rows: [
            quest_row(JOURNAL_QUESTS[0], amount("cisco_mod:radiant_ruby", false)),
            quest_row(JOURNAL_QUESTS[1], amount("cisco_mod:divine_core", false)),
            JournalRow {
                quest: economy::HAM_EXCHANGE, cycle, count: coins,
                state: if store.has(2, economy::HAM_EXCHANGE, cycle) {
                    JournalState::Claim
                } else if coins >= 2 {
                    JournalState::Submit
                } else {
                    JournalState::Collect
                },
            },
            quest_row(JOURNAL_QUESTS[3], amount("minecraft:obsidian", false)),
            quest_row(JOURNAL_QUESTS[4], 0),
            quest_row(JOURNAL_QUESTS[5], amount("minecraft:quartz", false)),
        ],
        exchanges,
    })
}

pub fn journal_action(
    game: &Game<'_>, quest: u64, kind: u8, slot: u64, reply: Sender<Result<(), String>>,
) {
    let request = (|| {
        let actor = journal_owner(game)?;
        let world = game.native_world_dir().ok_or("no world")?;
        let valid = JOURNAL_QUESTS.contains(&quest)
            && (kind == 1 || (quest == economy::HAM_EXCHANGE && kind == 2))
            && if quest == economy::HAM_EXCHANGE { slot > 0 } else { slot == 0 };
        if !valid {
            return Err("invalid journal action".to_owned());
        }
        Ok((world, actor))
    })();
    match request {
        Ok((world, actor)) => QUEUE.lock().unwrap().push(Request {
            world, actor, quest, kind, slot, reply: Some(reply),
        }),
        Err(error) => { let _ = reply.send(Err(error)); }
    }
}

pub fn status(game: &mut Game<'_>, actor: u128, q: u64, slot: u64) -> Result<&'static str, String> {
    let world = game.native_world_dir().ok_or("no world")?;
    let d = definition(q)?;
    let store = Store::open(&world, actor).map_err(|e| e.to_string())?;
    if store.has(1, q, slot) {
        return Ok("claimed");
    }
    if slot > 1 && !store.has(1, q, slot - 1) {
        return Ok("locked");
    }
    if complete(&world, actor, q, &d, &store, slot)? {
        return Ok("ready");
    }
    Ok(if d.costs.is_empty() {
        "locked"
    } else {
        "awaiting_items"
    })
}
fn snapshot(game: &mut Game<'_>, actor: u128) -> Result<Vec<u8>, String> {
    let n = seq(game, actor)?;
    let g = game
        .lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == actor))
        .ok_or("player missing")?;
    let tag = playerdata::player_tag(g);
    let mut map = Compound::new();
    for key in ["Inventory", "XpLevel", "XpP", "XpTotal", "XpSeed"] {
        map.insert(
            key.into(),
            tag.get(key).ok_or("player snapshot field missing")?.clone(),
        );
    }
    map.insert("DfqSequence".into(), Tag::Long(n as i64));
    nbt::write_named(
        &NamedTag {
            name: "".into(),
            tag: Tag::Compound(map),
        },
        Limits::saved(),
    )
    .map_err(|e| e.to_string())
}
fn restore(game: &mut Game<'_>, actor: u128, bytes: &[u8]) -> Result<(), String> {
    let (root, used) = nbt::read_named(bytes, Limits::saved()).map_err(|e| e.to_string())?;
    if used != bytes.len() {
        return Err("trailing snapshot data".into());
    }
    let n = match root.tag.get("DfqSequence") {
        Some(Tag::Long(n)) if *n >= 0 => *n as u64,
        _ => return Err("bad snapshot sequence".into()),
    };
    let mut level = Level::default();
    level.read_player(&root.tag);
    let g = game
        .lan
        .as_mut()
        .and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == actor))
        .ok_or("player missing")?;
    g.inventory.fill(None);
    // Equipment is outside the transaction: preserve armor and offhand.
    for (slot, item, count, damage) in level.inventory {
        let stack = Some(ItemStack {
            item: item.into(),
            count,
            damage,
            extra: level.inventory_components.get(&slot).cloned(),
        });
        match slot {
            0..=35 => g.inventory[slot as usize] = stack,
            100 => g.armor[3] = stack,
            101 => g.armor[2] = stack,
            102 => g.armor[1] = stack,
            103 => g.armor[0] = stack,
            150 => g.offhand = stack,
            _ => return Err("invalid inventory slot".into()),
        }
    }
    g.xp_level = level.xp_level;
    g.xp_progress = level.xp_progress;
    g.xp_total = level.xp_total;
    g.xp_seed = level.xp_seed;
    setseq(game, actor, n)
}
fn checkpoint(game: &mut Game<'_>, actor: u128) -> Result<(), String> {
    game.finish_player_autosaves(true)
        .map_err(|e| e.to_string())?;
    if let Some(mut h) = game.lan.take() {
        for g in &mut h.hub.guests {
            game.guest_attachments_into(g);
        }
        game.lan = Some(h);
    }
    game.save_game_metadata().map_err(|e| e.to_string())?;
    game.finish_player_autosaves(true)
        .map_err(|e| e.to_string())?;
    let world = game.native_world_dir().ok_or("no world")?;
    let mut host = game.lan.take().ok_or("no server")?;
    let result = (|| {
        let g = host
            .hub
            .guests
            .iter_mut()
            .find(|g| g.uuid == actor)
            .ok_or("no actor")?;
        game.guest_attachments_into(g);
        horizon_client::save::write_dat(
            &playerdata::path(&world, actor),
            playerdata::player_tag(g),
        )
        .map_err(|e| e.to_string())?;
        // Flush all player snapshots touched by the ordinary owner/server saver.
        for e in std::fs::read_dir(playerdata::directory(&world)).map_err(|e| e.to_string())? {
            let p = e.map_err(|e| e.to_string())?.path();
            if p.extension().is_some_and(|x| x == "dat") {
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(p)
                    .and_then(|f| f.sync_all())
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })();
    game.lan = Some(host);
    result
}
fn failpoint(name: &str, kind: u8) {
    if std::env::var("HORIZON_DFQ_CRASH_OP")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .is_some_and(|k| k != kind)
    {
        return;
    }
    if std::env::var("HORIZON_DFQ_CRASH").as_deref() == Ok(name) {
        let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("DFQ injected process exit: {name}"));
        std::process::exit(86);
    }
}
fn fatal(e: impl std::fmt::Display) -> ! {
    let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("DFQ transaction cannot safely continue; restart required: {e}"));
    std::process::exit(87);
}
/// Runs during login, before the next tick can process client inventory actions.
pub fn recover(game: &mut Game<'_>, actor: u128) {
    if let Err(e) = recover_inner(game, actor) {
        fatal(e);
    }
}
fn recover_inner(game: &mut Game<'_>, actor: u128) -> Result<(), String> {
    let world = game.native_world_dir().ok_or("no world")?;
    // Do not create stores or markers for players who never used the economy.
    if !world
        .join("dragonfyre-transactions-v1")
        .join(format!("{actor:032x}"))
        .exists()
    {
        return if seq(game, actor)? == 0 {
            Ok(())
        } else {
            Err("transaction marker exists without its journal".into())
        };
    }
    let store = Store::open(&world, actor).map_err(|e| e.to_string())?;
    let loaded = seq(game, actor)?;
    if loaded > store.records.len() as u64 {
        return Err("player marker ahead of transaction log".into());
    }
    for r in &store.records {
        if r.sequence <= loaded {
            continue;
        }
        if store.acknowledged(r).map_err(|e| e.to_string())? {
            return Err(
                "acknowledged transaction missing from player save; restore a consistent backup"
                    .into(),
            );
        }
        if snapshot(game, actor)? != r.before {
            return Err(
                "recovery precondition differs from durable baseline; refusing overwrite".into(),
            );
        }
        restore(game, actor, &r.after)?;
    }
    if let Some(last) = store.records.last() {
        if loaded < last.sequence || !store.acknowledged(last).map_err(|e| e.to_string())? {
            checkpoint(game, actor)?;
            for r in &store.records {
                store.acknowledge(r).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
fn execute(game: &mut Game<'_>, actor: u128, q: u64, kind: u8, slot: u64) -> Result<(), String> {
    let world = game.native_world_dir().ok_or("no world")?;
    let d = definition(q)?;
    let mut store = Store::open(&world, actor).map_err(|e| e.to_string())?;
    if seq(game, actor)? != store.records.len() as u64 {
        return Err("unrecovered transaction sequence".into());
    }
    if store.has(kind, q, slot) {
        return Ok(());
    }
    if slot > 1 && !store.has(1, q, slot - 1) {
        return Err("previous exchange cycle is not claimed".into());
    }
    if kind == 1 && !complete(&world, actor, q, &d, &store, slot)? {
        return Err("quest not complete".into());
    }
    if kind == 2 {
        if d.costs.is_empty() {
            return Err("quest has no supported consuming tasks".into());
        }
        if let Some(original) = engine::catalog().into_iter().find(|c| c.id == q) {
            let p = engine::Progress::open(&world, actor, engine::catalog(), engine::items())
                .map_err(|e| format!("{e:?}"))?;
            if !original.dependencies.iter().all(|id| p.complete(*id)) {
                return Err("dependencies incomplete".into());
            }
        }
    }
    let g = game
        .lan
        .as_ref()
        .and_then(|h| {
            h.hub
                .guests
                .iter()
                .find(|g| g.uuid == actor && g.placed && !g.dead)
        })
        .ok_or("player unavailable")?;
    if g.menu.is_some() || g.carried.is_some() {
        return Err("close the container first".into());
    }
    let inv: Vec<_> = g
        .inventory
        .iter()
        .take(36)
        .map(|s| {
            s.as_ref().map(|s| Stack {
                item: s.item.to_string(),
                count: s.count,
                plain: s.damage == 0 && s.extra.is_none(),
            })
        })
        .collect();
    let costs: Vec<_> = if kind == 2 {
        d.costs.iter().map(|(_, i, n)| (i.clone(), *n)).collect()
    } else {
        vec![]
    };
    let (planned, levels) = economy::plan(
        &inv,
        &costs,
        if kind == 1 { &d.items } else { &[] },
        if kind == 1 { d.levels } else { 0 },
        g.xp_level,
        |i| game.res.items().get(i).map(|x| x.max_stack),
    )
    .map_err(|e| format!("{e:?}"))?;
    // Freeze on the tick thread and establish the durable pre-transaction baseline.
    checkpoint(game, actor)?;
    failpoint("after_baseline", kind);
    let before = snapshot(game, actor)?;
    let old_inv = game
        .lan
        .as_ref()
        .unwrap()
        .hub
        .guests
        .iter()
        .find(|g| g.uuid == actor)
        .unwrap()
        .inventory
        .clone();
    let old_levels = game
        .lan
        .as_ref()
        .unwrap()
        .hub
        .guests
        .iter()
        .find(|g| g.uuid == actor)
        .unwrap()
        .xp_level;
    {
        let g = game
            .lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .iter_mut()
            .find(|g| g.uuid == actor)
            .unwrap();
        for (slot, p) in planned.iter().enumerate() {
            match p {
                None => g.inventory[slot] = None,
                Some(p) if !p.plain => {}
                Some(p) => {
                    if let Some(s) = g.inventory[slot]
                        .as_mut()
                        .filter(|s| s.damage == 0 && s.extra.is_none() && *s.item == p.item)
                    {
                        s.count = p.count;
                    } else {
                        g.inventory[slot] = Some(ItemStack {
                            item: p.item.as_str().into(),
                            count: p.count,
                            damage: 0,
                            extra: None,
                        });
                    }
                }
            }
        }
        g.xp_level = levels;
    }
    let old_seq = seq(game, actor)?;
    setseq(game, actor, old_seq + 1)?;
    let after_result = snapshot(game, actor);
    {
        let g = game
            .lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .iter_mut()
            .find(|g| g.uuid == actor)
            .unwrap();
        g.inventory = old_inv;
        g.xp_level = old_levels;
    }
    setseq(game, actor, old_seq)?;
    let after = after_result?;
    store
        .publish(
            Record {
                sequence: 0,
                quest: q,
                task: slot,
                kind,
                before,
                after,
            },
            |name| failpoint(name, kind),
        )
        .unwrap_or_else(|e| fatal(e));
    // Any error after intent publication must fail-stop; never permit later
    // player/world mutations while an outcome might be ambiguous.
    let r = store.records.last().unwrap();
    restore(game, actor, &r.after).unwrap_or_else(|e| fatal(e));
    failpoint("after_apply", kind);
    checkpoint(game, actor).unwrap_or_else(|e| fatal(e));
    failpoint("after_player_save", kind);
    store.acknowledge(r).unwrap_or_else(|e| fatal(e));
    failpoint("after_ack", kind);
    let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "DFQ transaction committed: {actor:032x} {q:016X} kind={kind} sequence={}",
        r.sequence
    ));
    Ok(())
}
pub fn tick(game: &mut Game<'_>) {
    let Some(world) = game.native_world_dir() else {
        return;
    };
    let requests = {
        let mut all = QUEUE.lock().unwrap();
        let (picked, pending): (Vec<_>, Vec<_>) = std::mem::take(&mut *all)
            .into_iter().partition(|request| request.world == world);
        *all = pending;
        picked
    };
    for Request { actor, quest, kind, slot, reply, .. } in requests {
        let result = execute(game, actor, quest, kind, slot);
        if let Err(e) = &result {
            let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("DFQ transaction rejected: {actor:032x} {quest:016X}: {e}"));
        }
        if let Some(reply) = reply {
            let _ = reply.send(result);
        }
    }
}

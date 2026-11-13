fn dragonfyre_tick(
    game: &mut crate::game::Game<'_>,
    context: &horizon_client::mod_api::NativeLifecycleContext,
    phase: horizon_client::mod_api::ServerTickPhase,
) {
    use std::{collections::BTreeMap, sync::Mutex};

    let horizon_client::mod_api::ServerTickPhase::ServerTickEnd {
        world_running: true,
    } = phase
    else {
        return;
    };

    if context.authority == horizon_client::mod_api::NativeAuthority::RemoteClient {
        return;
    }

    crate::game::native_hooks::dragonfyre_economy::tick(game);

    let forced_requery = dragonfyre_quests_engine::take_requery();

    let Some(lan) = &game.lan else {
        return;
    };
    let Some(world) = game.native_world_dir() else {
        return;
    };

    const DETECTION_DELAY_TICKS: u64 = 20;

    type DetectionState = (u64, BTreeMap<String, u64>, Option<u64>, String);
    static STATE: Mutex<BTreeMap<u128, DetectionState>> =
        Mutex::new(BTreeMap::new());

    let current_epoch = context.epoch.get();
    let current_tick = context.tick.get();

    let mut state_map = match STATE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    if state_map.values().any(|entry| entry.0 != current_epoch) {
        state_map.clear();
    }

    for guest in &lan.hub.guests {
        if !guest.placed || guest.dead {
            continue;
        }

        let mut current_totals: BTreeMap<String, u64> = BTreeMap::new();
        for slot in guest.inventory.iter().take(36).flatten() {
            let count = slot.count as u64;
            if count > 0 {
                current_totals
                    .entry(slot.item.to_string())
                    .and_modify(|c| *c = c.saturating_add(count))
                    .or_insert(count);
            }
        }

        let entry = state_map.entry(guest.uuid).or_insert_with(|| {
            (
                current_epoch,
                current_totals.clone(),
                Some(current_tick + DETECTION_DELAY_TICKS),
                guest.dimension.clone(),
            )
        });

        if entry.0 != current_epoch {
            *entry = (
                current_epoch,
                current_totals.clone(),
                Some(current_tick + DETECTION_DELAY_TICKS),
                guest.dimension.clone(),
            );
        } else if entry.1 != current_totals {
            entry.1 = current_totals.clone();
            let target = current_tick + DETECTION_DELAY_TICKS;
            entry.2 = Some(entry.2.map_or(target, |p| p.min(target)));
        }

        if entry.3 != guest.dimension {
            entry.3.clone_from(&guest.dimension);
            entry.2 = Some(current_tick);
        }

        if forced_requery {
            entry.2 = Some(entry.2.map_or(current_tick, |p| p.min(current_tick)));
        }

        let should_run = entry.2.map_or(false, |due| current_tick >= due);
        if should_run {
            let res = dragonfyre_quests_engine::Progress::open(
                &world,
                guest.uuid,
                dragonfyre_quests_engine::catalog(),
                dragonfyre_quests_engine::items(),
            )
            .and_then(|mut progress| {
                progress.item_detection(current_totals.clone())?;
                if progress.dimension_detection(&guest.dimension)? {
                    progress.item_detection(current_totals.clone())?;
                }
                Ok(())
            });

            match res {
                Ok(_) => {
                    entry.2 = None;
                }
                Err(err) => {
                    eprintln!(
                        "dragonfyre quest item detection error for {:032x}: {err:?}",
                        guest.uuid
                    );
                    // Keep entry.2 as Some(due) so it retries next tick
                }
            }
        }
    }
}

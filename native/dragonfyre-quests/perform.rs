fn dragonfyre_perform(
    game: &mut crate::game::Game<'_>,
    player: &horizon_client::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    use dragonfyre_quests_engine::command::{self, Reply};
    use horizon_client::text::Text;
    if *ran || command.split_whitespace().next() != Some("dragonfyredev") {
        return;
    }
    *ran = true;
    static MESSAGES: std::sync::OnceLock<std::collections::BTreeMap<String, String>> =
        std::sync::OnceLock::new();
    let messages = MESSAGES.get_or_init(|| {
        serde_json::from_slice(horizon_client::native_data!(
            "dragonfyre_quests",
            "messages.ru.json"
        ))
        .unwrap_or_default()
    });
    let message = |key: &str| {
        Text::translate_with_fallback(
            key,
            messages.get(key).map(String::as_str).unwrap_or(key),
            &[],
        )
    };
    let Some(world) = game.native_world_dir() else {
        game.native_feedback(message("dragonfyre_quests.dev.no_world"));
        return;
    };
    let words: Vec<_> = command.split_whitespace().collect();
    if let [_, operation @ ("claim" | "submit" | "economy"), quest, cycle @ ..] = words.as_slice() {
        let allowed = player.permission >= 3;
        let parsed = (quest.len() == 16 && quest.bytes().all(|c| c.is_ascii_hexdigit()))
            .then(|| u64::from_str_radix(quest, 16).ok())
            .flatten()
            .and_then(|q| {
                let cycle = match cycle {
                    [] => None,
                    [n] if n.bytes().all(|b| b.is_ascii_digit()) => Some(n.parse::<u64>().ok()?),
                    _ => return None,
                };
                dragonfyre_quests_engine::economy::operation_slot(q, cycle).map(|slot| (q, slot))
            });
        let key = if !allowed {
            "dragonfyre_quests.dev.forbidden"
        } else if let Some((q, slot)) = parsed {
            if *operation == "economy" {
                match crate::game::native_hooks::dragonfyre_economy::status(game, player.uuid, q, slot) {
                    Ok("claimed") => "dragonfyre_quests.dev.claimed",
                    Ok("ready") => "dragonfyre_quests.dev.ready",
                    Ok("awaiting_items") => "dragonfyre_quests.dev.awaiting_items",
                    Ok(_) => "dragonfyre_quests.dev.locked",
                    Err(e) if e == "unsupported" => "dragonfyre_quests.dev.unsupported",
                    Err(_) => "dragonfyre_quests.dev.storage_failure",
                }
            } else {
                crate::game::native_hooks::dragonfyre_economy::enqueue(
                    game,
                    player.uuid,
                    q,
                    if *operation == "claim" { 1 } else { 2 },
                    slot,
                );
                "dragonfyre_quests.dev.queued"
            }
        } else {
            "dragonfyre_quests.dev.syntax"
        };
        game.native_feedback(message(key));
        return;
    }
    // UUID and operator permission come from native_perform_command's server player.
    // FTB Teams is not implemented: this experiment has personal progress only.
    let reply = command::perform(&world, player.uuid, player.permission, command)
        .unwrap_or(Reply::InvalidSyntax);
    let key = match reply {
        Reply::Complete => "dragonfyre_quests.dev.complete",
        Reply::Available => "dragonfyre_quests.dev.available",
        Reply::Locked => "dragonfyre_quests.dev.locked",
        Reply::Changed => "dragonfyre_quests.dev.changed",
        Reply::AlreadyCompleted => "dragonfyre_quests.dev.already_completed",
        Reply::Unsupported => "dragonfyre_quests.dev.unsupported",
        Reply::UnknownQuest => "dragonfyre_quests.dev.unknown_quest",
        Reply::UnknownTask => "dragonfyre_quests.dev.unknown_task",
        Reply::InvalidSyntax => "dragonfyre_quests.dev.syntax",
        Reply::Forbidden => "dragonfyre_quests.dev.forbidden",
        Reply::StorageFailure => "dragonfyre_quests.dev.storage_failure",
    };
    game.native_feedback(message(key));
}

use dragonfyre_quests_engine::{Progress, catalog};
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 6 {
        return Err(
            "Development probe only: quest-probe WORLD TEAM_HEX checkmark QUEST_HEX TASK_HEX"
                .into(),
        );
    }
    let team = u128::from_str_radix(&args[2], 16).map_err(|e| e.to_string())?;
    if args[3] != "checkmark" {
        return Err("Only checkmark probe is available; no game/rewards integration".into());
    }
    let quest = u64::from_str_radix(&args[4], 16).map_err(|e| e.to_string())?;
    let task = u64::from_str_radix(&args[5], 16).map_err(|e| e.to_string())?;
    let mut state = Progress::open(
        Path::new(&args[1]),
        team,
        catalog(),
        dragonfyre_quests_engine::items(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let changed = state.checkmark(quest, task).map_err(|e| format!("{e:?}"))?;
    println!(
        "prototype_checkmark_changed={changed} prototype_tasks_complete={} rewards=UNIMPLEMENTED horizon=NOT_CONNECTED",
        state.complete(quest)
    );
    Ok(())
}

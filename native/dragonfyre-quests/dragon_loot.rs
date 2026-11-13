use crate::game::Game;

/// Roll `minecraft:entities/ender_dragon` once at the dragon's death position.
/// Horizon's dragon death tick only spawns experience; the vanilla table is not
/// rolled, so the original Forge global loot modifier would never run. The
/// caller drops this before the death animation moves the dragon upwards.
pub(super) fn drop_loot(game: &mut Game<'_>, player_credit: bool) {
    let Some(d) = game.dragon.as_ref() else {
        return;
    };
    let at = [d.position.x, d.position.y, d.position.z];
    let (killed_by_player, held) = if player_credit {
        match d.hurt_by_player {
            Some((_who, hit)) if d.ticks.saturating_sub(hit) <= 100 => {
                // During a guest's turn the game swaps that guest's inventory
                // and selected slot in, so this is the killer's held item for
                // the host and for a LAN guest alike.
                (true, game.inventory[game.selected].clone())
            }
            _ => (false, None),
        }
    } else {
        (false, None)
    };
    let mut drops = world_items!(game).loot_table(
        "minecraft:entities/ender_dragon",
        killed_by_player,
        held.as_ref(),
        &mut game.world.random,
    );
    for stack in drops.drain(..) {
        game.spawn_item(
            stack,
            at,
            crate::game::dispenser::new_item_motion(),
            10,
        );
    }
}

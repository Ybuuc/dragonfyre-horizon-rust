fn dragonfyre_dragon_loot_kill(game: &mut crate::game::Game<'_>) {
    // `/kill` removes a live dragon at once, without the death flight, so the
    // death tick never rolls its table. A dragon already in its death flight
    // has death_time > 0 and keeps the roll made at its first death tick.
    if game.dragon.as_ref().is_some_and(|d| d.death_time == 0) {
        crate::game::dragon::dragonfyre_dragon_loot::drop_loot(game, false);
    }
}

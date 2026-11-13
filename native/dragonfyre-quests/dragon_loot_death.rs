fn dragonfyre_dragon_loot_death(game: &mut crate::game::Game<'_>) {
    // `tick_dragon_death` runs every tick of the 200-tick death animation;
    // death_time is still 0 on its first call, before the dragon starts rising.
    if game
        .dragon
        .as_ref()
        .is_some_and(|d| d.death_time == 0)
    {
        crate::game::dragon::dragonfyre_dragon_loot::drop_loot(game, true);
    }
}

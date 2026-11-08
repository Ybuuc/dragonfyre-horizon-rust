fn drop_necklace_on_death(
    game: &mut crate::game::Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) {
    crate::game::native_hooks::dragonfyre_necklace::death_drop(game, context, drops);
}

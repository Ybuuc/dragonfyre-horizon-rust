fn dragonfyre_journal_draw(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::native_hooks::dragonfyre_journal::draw(game, batch, screen);
    crate::game::native_hooks::dragonfyre_necklace_screen::draw(game, batch, screen);
}

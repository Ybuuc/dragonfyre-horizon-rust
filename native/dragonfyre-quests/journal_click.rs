fn dragonfyre_journal_click(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::native_hooks::dragonfyre_journal::click(game, screen, button, canceled);
    crate::game::native_hooks::dragonfyre_necklace_screen::click(game, screen, button, canceled);
}

fn dragonfyre_journal_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::native_hooks::dragonfyre_journal::key(game, key, pressed);
    crate::game::native_hooks::dragonfyre_necklace_screen::key(game, key, pressed);
}

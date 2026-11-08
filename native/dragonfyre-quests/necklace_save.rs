fn save_owner_necklace(game: &mut crate::game::Game<'_>) {
    crate::game::native_hooks::dragonfyre_necklace::prepare_owner_save(game);
}

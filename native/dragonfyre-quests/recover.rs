fn recover_player_transactions(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::native_hooks::dragonfyre_economy::recover(game, player);
}

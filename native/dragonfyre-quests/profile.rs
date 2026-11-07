fn isolate_integrated_owner_profile(game: &mut crate::game::Game<'_>, host: &mut LanHost, conn: u32) {
    if let Some(guest) = host.hub.guest(conn)
        && let Some(owner) = host.owner.as_mut().filter(|owner| owner.uuid == guest.uuid)
    {
        // The cached single-player snapshot can belong to the previous owner.
        // Keep Dragonfyre player state in the files named by their UUID.
        owner.saved = None;
        playerdata::store(guest, &mut game.level, None);
        game.level.position = None;
        game.level.singleplayer_file = false;
        game.level.player_uuid = Some(playerdata::uuid_ints(guest.uuid));
    }
}

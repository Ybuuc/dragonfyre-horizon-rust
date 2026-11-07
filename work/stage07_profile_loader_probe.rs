use horizon_client_lib::{
    items::ItemStack,
    lan::{Joined, PlayerSender, guest::Guest, playerdata},
    save::Level,
};
use horizon_protocol::packets::configuration::ClientInformation;
use std::{path::Path, sync::{Arc, atomic::AtomicI32, mpsc}};

const A: u128 = 1;
const B: u128 = 2;

fn guest(id: u128) -> Guest {
    let (tx, _rx) = mpsc::channel();
    Guest::new(
        Joined {
            conn: id as u32,
            uuid: id,
            name: format!("Owner{id}"),
            properties: Vec::new(),
            info: ClientInformation::new(8),
            brand: "stage07-fixture".into(),
            sender: PlayerSender::new(tx, Arc::default()),
            latency: Arc::new(AtomicI32::new(0)),
            address: "127.0.0.1:25565".parse().unwrap(),
            expected: false,
        },
        id as i32,
        [0.0, 64.0, 0.0],
        8,
    )
}

fn add(guest: &mut Guest, xp: i32, item: &str) {
    guest.xp_level = xp;
    guest.inventory[0] = Some(ItemStack { item: item.to_owned().into(), count: 1, damage: 0, extra: None });
}

fn check(guest: &Guest, xp: i32, item: &str) {
    assert_eq!(guest.xp_level, xp);
    assert_eq!(guest.inventory[0].as_ref().unwrap().item.to_string(), item);
}

fn main() {
    let argv: Vec<_> = std::env::args().collect();
    let world = Path::new(&argv[1]);
    if argv[2] == "write" {
        let mut a = guest(A);
        let mut b = guest(B);
        add(&mut a, 5, "minecraft:flint_and_steel");
        add(&mut b, 3, "minecraft:diamond");
        let mut level = Level::default();
        playerdata::save_owner(&a, &mut level, world);
        level.write(world).unwrap();
        playerdata::save(&b, world);
        println!("PASS: one world saved with two distinct UUIDs and A as the cached owner");
    } else {
        let cached = Level::read(world).unwrap();
        assert!(cached.singleplayer_file);
        assert_eq!(cached.player_uuid, Some(playerdata::uuid_ints(A)));
        let mut old = guest(B);
        assert!(playerdata::load_owner(&mut old, world, Some(&cached)));
        check(&old, 5, "minecraft:flint_and_steel");
        let mut own = guest(B);
        assert!(playerdata::load_owner(&mut own, world, None));
        check(&own, 3, "minecraft:diamond");
        let mut a = guest(A);
        assert!(playerdata::load_owner(&mut a, world, None));
        check(&a, 5, "minecraft:flint_and_steel");
        let bpath = playerdata::path(world, B);
        std::fs::rename(&bpath, bpath.with_extension("dat_old")).unwrap();
        let mut backup = guest(B);
        assert!(playerdata::load_owner(&mut backup, world, None));
        check(&backup, 3, "minecraft:diamond");
        println!("PASS: old cached-owner route loads A into B; the UUID-only route loads each own file and B's backup");
    }
}

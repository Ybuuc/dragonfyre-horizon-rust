use crate::game::Game;
use horizon_client::{
    items::ItemStack,
    motion::PlayerMotion,
    save::{item_from_nbt, item_to_nbt},
};
use horizon_world::attachments::{AttachmentCodec, AttachmentError, AttachmentType, Attachments};

pub(super) const ITEM: &str = "cisco_mod:amuletof_vitality";
const MAX_HEALTH: &str = "minecraft:max_health";
const MODIFIER: &str = "dragonfyre_quests:vitality_amulet";
const BONUS: f64 = 10.0;

static NECKLACE: AttachmentType<ItemStack> = AttachmentType {
    namespace: "dragonfyre_quests",
    name: "necklace",
    default: || ItemStack::new(ITEM, 1),
    codec: Some(AttachmentCodec {
        encode: |stack| match item_to_nbt(stack) {
            horizon_world::nbt::Tag::Compound(tag) => Ok(tag),
            _ => Err(AttachmentError::Decode),
        },
        decode: |tag| {
            item_from_nbt(&horizon_world::nbt::Tag::Compound(tag.clone()))
                .filter(|stack| stack.item.as_ref() == ITEM && stack.count == 1)
                .ok_or(AttachmentError::Decode)
        },
    }),
    copy_on_death: true,
    sync: false,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Inspect,
    Equip,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Unavailable,
    Missing,
    Full,
    Storage,
}

pub(super) struct View {
    pub equipped: Option<ItemStack>,
}

fn move_item(
    attachments: &mut Attachments,
    inventory: &mut [Option<ItemStack>],
    action: Action,
) -> Result<View, Error> {
    let worn = attachments
        .get_existing(&NECKLACE)
        .map_err(|_| Error::Storage)?;
    let equipped = match (action, worn) {
        (Action::Equip, None) => {
            let source = inventory
                .iter_mut()
                .find(|cell| {
                    cell.as_ref()
                        .is_some_and(|stack| stack.item.as_ref() == ITEM && stack.count != 0)
                })
                .ok_or(Error::Missing)?;
            let stack = source.as_mut().unwrap();
            let amulet = ItemStack {
                count: 1,
                ..stack.clone()
            };
            attachments
                .set(&NECKLACE, amulet.clone())
                .map_err(|_| Error::Storage)?;
            stack.count -= 1;
            if stack.count == 0 {
                *source = None;
            }
            Some(amulet)
        }
        (Action::Remove, Some(worn)) => {
            let destination = inventory
                .iter_mut()
                .find(|cell| cell.is_none())
                .ok_or(Error::Full)?;
            attachments.remove(&NECKLACE).map_err(|_| Error::Storage)?;
            *destination = Some(worn);
            None
        }
        (_, worn) => worn,
    };
    Ok(View { equipped })
}

/// The guest's attachment is installed before the engine restores its current HP.
pub(crate) fn sync_bonus(attachments: &mut Attachments, body: &mut PlayerMotion) {
    let Some(attribute) = body.attributes.key(MAX_HEALTH) else {
        return;
    };
    let wearing = attachments.get_existing(&NECKLACE).ok().flatten().is_some();
    if body.attributes.has_modifier(Some(attribute), MODIFIER).ok() != Some(wearing) {
        let _ = body.set_modifier(MAX_HEALTH, MODIFIER, BONUS, "add_value", wearing);
    }
}

pub(super) fn access(game: &mut Game<'_>, action: Action) -> Result<View, Error> {
    if game.items().get(ITEM).is_none() {
        return Err(Error::Unavailable);
    }
    let mut host = game.lan.take().ok_or(Error::Unavailable)?;
    let result = (|| {
        let owner = host.owner.as_ref().ok_or(Error::Unavailable)?.uuid;
        let guest = host
            .hub
            .guests
            .iter()
            .find(|guest| {
                guest.uuid == owner
                    && guest.placed
                    && !guest.dead
                    && guest.health > 0.0
                    && !guest.spectator()
                    && guest.handover.is_none()
            })
            .ok_or(Error::Unavailable)?;
        if action != Action::Inspect && (guest.menu.is_some() || guest.carried.is_some()) {
            return Err(Error::Unavailable);
        }
        let conn = guest.conn;
        game.as_guest(&mut host, conn, |game| {
            let view = move_item(
                &mut game.player_attachments,
                &mut game.inventory[..36],
                action,
            )?;
            if let Some(body) = game.player.as_mut() {
                sync_bonus(&mut game.player_attachments, body);
            }
            Ok(view)
        })
        .ok_or(Error::Unavailable)?
    })();
    game.lan = Some(host);
    result
}

/// The owner autosave serializes the inventory and this attachment together.
pub(crate) fn prepare_owner_save(game: &mut Game<'_>) {
    let Some(host) = game.lan.as_mut() else {
        return;
    };
    let Some(owner) = host.owner.as_ref() else {
        return;
    };
    let Some(guest) = host
        .hub
        .guests
        .iter_mut()
        .find(|g| g.uuid == owner.uuid && g.placed)
    else {
        return;
    };
    if let Some(live) = game.guest_attachments.get(&guest.uuid)
        && (live.has(&NECKLACE) || guest.attachments.has(&NECKLACE))
    {
        guest.attachments = live.clone();
    }
}

pub(super) fn death_drop(
    game: &mut Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) {
    // drop_inventory invokes this only after checking keep_inventory.
    if context.table.is_empty()
        && context
            .entity
            .is_some_and(|(_, kind)| kind == "minecraft:player")
        && game
            .lan_acting_uuid
            .is_some_and(|uuid| context.player == Some(uuid))
        && let Ok(Some(worn)) = game.player_attachments.get_existing(&NECKLACE)
        && game.player_attachments.remove(&NECKLACE).is_ok()
    {
        drops.push(worn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn necklace_transfer_is_idempotent_and_keeps_the_exact_stack() {
        let mut saved = Attachments::default();
        let amulet = ItemStack::new(ITEM, 1);
        let mut inventory = [
            Some(ItemStack::new("minecraft:stone", 64)),
            Some(amulet.clone()),
        ];
        assert_eq!(
            move_item(&mut saved, &mut inventory, Action::Equip)
                .unwrap()
                .equipped,
            Some(amulet.clone())
        );
        assert!(inventory[1].is_none());
        assert_eq!(
            move_item(&mut saved, &mut inventory, Action::Equip)
                .unwrap()
                .equipped,
            Some(amulet.clone())
        );
        inventory[1] = Some(ItemStack::new("minecraft:dirt", 64));
        assert!(matches!(
            move_item(&mut saved, &mut inventory, Action::Remove),
            Err(Error::Full)
        ));
        assert_eq!(saved.get_existing(&NECKLACE).unwrap(), Some(amulet.clone()));
        inventory[1] = None;
        assert!(
            move_item(&mut saved, &mut inventory, Action::Remove)
                .unwrap()
                .equipped
                .is_none()
        );
        assert!(
            move_item(&mut saved, &mut inventory, Action::Remove)
                .unwrap()
                .equipped
                .is_none()
        );
        assert_eq!(inventory[1], Some(amulet));
    }

    #[test]
    fn necklace_round_trips_through_player_attachments() {
        let mut original = Attachments::default();
        let mut inventory = [Some(ItemStack::new(ITEM, 1))];
        move_item(&mut original, &mut inventory, Action::Equip).unwrap();
        let mut reopened = Attachments::from_nbt(original.to_nbt());
        let mut other_player = Attachments::default();
        assert!(
            move_item(&mut other_player, &mut inventory, Action::Inspect)
                .unwrap()
                .equipped
                .is_none()
        );
        assert_eq!(
            move_item(&mut reopened, &mut inventory, Action::Inspect)
                .unwrap()
                .equipped,
            Some(ItemStack::new(ITEM, 1))
        );
        move_item(&mut reopened, &mut inventory, Action::Remove).unwrap();
        assert!(!Attachments::from_nbt(reopened.to_nbt()).has(&NECKLACE));
    }

    #[test]
    fn vitality_bonus_stacks_once_and_preserves_other_health_modifiers() {
        use horizon_client::{assets::Assets, motion::MotionResources};
        use horizon_core::geometry::Vec3;
        let Some(root) = std::env::var_os("HORIZON_TEST_RESOURCES") else {
            eprintln!("prepared resources required for the vitality health test");
            return;
        };
        let root = std::path::PathBuf::from(root);
        let assets = Assets::load(&root).unwrap();
        let resources = MotionResources::load(&root, &assets).unwrap();
        let mut motion = PlayerMotion::new(&resources, Vec3::ZERO).unwrap();
        let mut saved = Attachments::default();
        let mut inventory = [Some(ItemStack::new(ITEM, 1))];
        move_item(&mut saved, &mut inventory, Action::Equip).unwrap();
        let mut reopened = Attachments::from_nbt(saved.to_nbt());
        sync_bonus(&mut reopened, &mut motion);
        sync_bonus(&mut reopened, &mut motion);
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 30.0);
        motion.set_health(27.0, false).unwrap();
        assert_eq!(motion.health.health(), 27.0);
        motion
            .set_modifier(
                MAX_HEALTH,
                "minecraft:effect.health_boost",
                4.0,
                "add_value",
                true,
            )
            .unwrap();
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 34.0);
        move_item(&mut reopened, &mut inventory, Action::Remove).unwrap();
        sync_bonus(&mut reopened, &mut motion);
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 24.0);
        assert_eq!(motion.health.health(), 24.0);
    }
}

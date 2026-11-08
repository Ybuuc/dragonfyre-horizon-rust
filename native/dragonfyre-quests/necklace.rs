use crate::game::Game;
use horizon_client::{
    items::ItemStack,
    motion::PlayerMotion,
    save::{item_from_nbt, item_to_nbt},
};
use horizon_world::attachments::{AttachmentCodec, AttachmentError, AttachmentType, Attachments};

const MAX_HEALTH: &str = "minecraft:max_health";
const LUCK: &str = "minecraft:luck";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Amulet {
    Vitality,
    Luck,
}

impl Amulet {
    pub const ALL: [Self; 2] = [Self::Vitality, Self::Luck];

    pub fn id(self) -> &'static str {
        match self {
            Self::Vitality => "cisco_mod:amuletof_vitality",
            Self::Luck => "cisco_mod:amuletof_luck",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|amulet| amulet.id() == id)
    }

    fn modifier(self) -> (&'static str, &'static str, f64) {
        match self {
            Self::Vitality => (MAX_HEALTH, "dragonfyre_quests:vitality_amulet", 10.0),
            Self::Luck => (LUCK, "dragonfyre_quests:luck_amulet", 5.0),
        }
    }
}

static NECKLACE: AttachmentType<ItemStack> = AttachmentType {
    namespace: "dragonfyre_quests",
    name: "necklace",
    default: || ItemStack::new(Amulet::Vitality.id(), 1),
    codec: Some(AttachmentCodec {
        encode: |stack| match item_to_nbt(stack) {
            horizon_world::nbt::Tag::Compound(tag) => Ok(tag),
            _ => Err(AttachmentError::Decode),
        },
        decode: |tag| {
            item_from_nbt(&horizon_world::nbt::Tag::Compound(tag.clone()))
                .filter(|stack| Amulet::from_id(&stack.item).is_some() && stack.count == 1)
                .ok_or(AttachmentError::Decode)
        },
    }),
    copy_on_death: true,
    sync: false,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Inspect,
    Equip(Amulet),
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

fn matches_amulet(cell: &Option<ItemStack>, amulet: Amulet) -> bool {
    cell.as_ref()
        .is_some_and(|stack| stack.item.as_ref() == amulet.id() && stack.count != 0)
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
        (Action::Equip(amulet), None) => {
            let source = inventory
                .iter_mut()
                .find(|cell| matches_amulet(cell, amulet))
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
    let equipped = attachments
        .get_existing(&NECKLACE)
        .ok()
        .flatten()
        .and_then(|stack| Amulet::from_id(&stack.item));
    for amulet in Amulet::ALL {
        let (attribute, modifier, amount) = amulet.modifier();
        let Some(key) = body.attributes.key(attribute) else {
            continue;
        };
        let wearing = equipped == Some(amulet);
        if body.attributes.has_modifier(Some(key), modifier).ok() != Some(wearing) {
            let _ = body.set_modifier(attribute, modifier, amount, "add_value", wearing);
        }
    }
}

pub(super) fn access(game: &mut Game<'_>, action: Action) -> Result<View, Error> {
    if !Amulet::ALL
        .iter()
        .any(|amulet| game.items().get(amulet.id()).is_some())
    {
        return Err(Error::Unavailable);
    }
    if let Action::Equip(amulet) = action
        && game.items().get(amulet.id()).is_none()
    {
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

    fn test_motion() -> Option<PlayerMotion> {
        use horizon_client::{assets::Assets, motion::MotionResources};
        use horizon_core::geometry::Vec3;
        let Some(root) = std::env::var_os("HORIZON_TEST_RESOURCES") else {
            eprintln!("prepared resources required for the necklace attribute tests");
            return None;
        };
        let root = std::path::PathBuf::from(root);
        let assets = Assets::load(&root).unwrap();
        let resources = MotionResources::load(&root, &assets).unwrap();
        Some(PlayerMotion::new(&resources, Vec3::ZERO).unwrap())
    }

    #[test]
    fn necklace_transfer_is_idempotent_and_keeps_the_exact_stack() {
        let mut saved = Attachments::default();
        let amulet = ItemStack::new(Amulet::Vitality.id(), 1);
        let mut inventory = [
            Some(ItemStack::new("minecraft:stone", 64)),
            Some(amulet.clone()),
        ];
        assert_eq!(
            move_item(&mut saved, &mut inventory, Action::Equip(Amulet::Vitality))
                .unwrap()
                .equipped,
            Some(amulet.clone())
        );
        assert!(inventory[1].is_none());
        assert_eq!(
            move_item(&mut saved, &mut inventory, Action::Equip(Amulet::Vitality))
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
        let mut inventory = [Some(ItemStack::new(Amulet::Vitality.id(), 1))];
        move_item(
            &mut original,
            &mut inventory,
            Action::Equip(Amulet::Vitality),
        )
        .unwrap();
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
            Some(ItemStack::new(Amulet::Vitality.id(), 1))
        );
        move_item(&mut reopened, &mut inventory, Action::Remove).unwrap();
        assert!(!Attachments::from_nbt(reopened.to_nbt()).has(&NECKLACE));
    }

    #[test]
    fn vitality_bonus_stacks_once_and_preserves_other_health_modifiers() {
        let Some(mut motion) = test_motion() else {
            return;
        };
        let mut saved = Attachments::default();
        let mut inventory = [Some(ItemStack::new(Amulet::Vitality.id(), 1))];
        move_item(&mut saved, &mut inventory, Action::Equip(Amulet::Vitality)).unwrap();
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

    #[test]
    fn luck_uses_the_same_slot_and_adds_five_once_after_reopening() {
        let vitality = ItemStack::new(Amulet::Vitality.id(), 1);
        let luck = ItemStack::new(Amulet::Luck.id(), 1);
        let mut inventory = [Some(vitality.clone()), Some(luck.clone())];
        let mut saved = Attachments::default();
        move_item(&mut saved, &mut inventory, Action::Equip(Amulet::Luck)).unwrap();
        let mut reopened = Attachments::from_nbt(saved.to_nbt());
        assert_eq!(inventory, [Some(vitality.clone()), None]);
        let result = move_item(
            &mut reopened,
            &mut inventory,
            Action::Equip(Amulet::Vitality),
        )
        .unwrap();
        assert_eq!(result.equipped, Some(luck.clone()));
        assert_eq!(inventory, [Some(vitality.clone()), None]);

        let Some(mut motion) = test_motion() else {
            return;
        };
        motion
            .set_modifier(LUCK, "minecraft:effect.luck", 1.0, "add_value", true)
            .unwrap();
        sync_bonus(&mut reopened, &mut motion);
        sync_bonus(&mut reopened, &mut motion);
        assert_eq!(motion.attribute(LUCK).unwrap(), 6.0);
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 20.0);

        move_item(&mut reopened, &mut inventory, Action::Remove).unwrap();
        assert_eq!(inventory[1], Some(luck));
        move_item(
            &mut reopened,
            &mut inventory,
            Action::Equip(Amulet::Vitality),
        )
        .unwrap();
        sync_bonus(&mut reopened, &mut motion);
        assert_eq!(motion.attribute(LUCK).unwrap(), 1.0);
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 30.0);
    }
}

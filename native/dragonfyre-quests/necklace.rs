use crate::game::Game;
use horizon_client::{
    items::ItemStack,
    motion::PlayerMotion,
    save::{item_from_nbt, item_to_nbt},
};
use horizon_world::attachments::{AttachmentCodec, AttachmentError, AttachmentType, Attachments};

const MAX_HEALTH: &str = "minecraft:max_health";
const ARMOR: &str = "minecraft:armor";
const LUCK: &str = "minecraft:luck";
const KNOCKBACK_RESISTANCE: &str = "minecraft:knockback_resistance";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Accessory {
    Vitality,
    Luck,
    SwimSpeed,
    BracingBelt,
    EmperorCrown,
}

impl Accessory {
    pub const ALL: [Self; 5] = [
        Self::Vitality,
        Self::Luck,
        Self::SwimSpeed,
        Self::BracingBelt,
        Self::EmperorCrown,
    ];

    pub fn slot(self) -> Slot {
        match self {
            Self::Vitality | Self::Luck | Self::SwimSpeed => Slot::Necklace,
            Self::BracingBelt => Slot::Belt,
            Self::EmperorCrown => Slot::Head,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Vitality => "cisco_mod:amuletof_vitality",
            Self::Luck => "cisco_mod:amuletof_luck",
            Self::SwimSpeed => "cisco_mod:amuletof_swim_speed",
            Self::BracingBelt => "cisco_mod:beltof_bracing",
            Self::EmperorCrown => "cisco_mod:crownofthe_emperor",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|accessory| accessory.id() == id)
    }

    fn modifier(self) -> (&'static str, &'static str, f64) {
        match self {
            Self::Vitality => (MAX_HEALTH, "dragonfyre_quests:vitality_amulet", 10.0),
            Self::Luck => (LUCK, "dragonfyre_quests:luck_amulet", 5.0),
            Self::SwimSpeed => (
                PlayerMotion::SWIM_AMULET_SYNC_ATTRIBUTE,
                PlayerMotion::SWIM_AMULET_SYNC_MODIFIER,
                0.0,
            ),
            Self::BracingBelt => (KNOCKBACK_RESISTANCE, "dragonfyre_quests:bracing_belt", 0.5),
            Self::EmperorCrown => (ARMOR, "dragonfyre_quests:emperor_crown", 6.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    Necklace,
    Belt,
    Head,
}

impl Slot {
    pub const ALL: [Self; 3] = [Self::Necklace, Self::Belt, Self::Head];

    fn attachment(self) -> &'static AttachmentType<ItemStack> {
        match self {
            Self::Necklace => &NECKLACE,
            Self::Belt => &BELT,
            Self::Head => &HEAD,
        }
    }
}

fn encode_stack(stack: &ItemStack) -> Result<horizon_world::nbt::Compound, AttachmentError> {
    match item_to_nbt(stack) {
        horizon_world::nbt::Tag::Compound(tag) => Ok(tag),
        _ => Err(AttachmentError::Decode),
    }
}

fn decode_slot(
    tag: &horizon_world::nbt::Compound,
    slot: Slot,
) -> Result<ItemStack, AttachmentError> {
    item_from_nbt(&horizon_world::nbt::Tag::Compound(tag.clone()))
        .filter(|stack| {
            stack.count == 1
                && Accessory::from_id(&stack.item).is_some_and(|accessory| accessory.slot() == slot)
        })
        .ok_or(AttachmentError::Decode)
}

static NECKLACE: AttachmentType<ItemStack> = AttachmentType {
    namespace: "dragonfyre_quests",
    name: "necklace",
    default: || ItemStack::new(Accessory::Vitality.id(), 1),
    codec: Some(AttachmentCodec {
        encode: encode_stack,
        decode: |tag| decode_slot(tag, Slot::Necklace),
    }),
    copy_on_death: true,
    sync: false,
};

static BELT: AttachmentType<ItemStack> = AttachmentType {
    namespace: "dragonfyre_quests",
    name: "belt",
    default: || ItemStack::new(Accessory::BracingBelt.id(), 1),
    codec: Some(AttachmentCodec {
        encode: encode_stack,
        decode: |tag| decode_slot(tag, Slot::Belt),
    }),
    copy_on_death: true,
    sync: false,
};

static HEAD: AttachmentType<ItemStack> = AttachmentType {
    namespace: "dragonfyre_quests",
    name: "head",
    default: || ItemStack::new(Accessory::EmperorCrown.id(), 1),
    codec: Some(AttachmentCodec {
        encode: encode_stack,
        decode: |tag| decode_slot(tag, Slot::Head),
    }),
    copy_on_death: true,
    sync: false,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Inspect,
    Equip(Accessory),
    Remove(Slot),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Unavailable,
    Missing,
    Full,
    Storage,
}

pub(super) struct View {
    necklace: Option<ItemStack>,
    belt: Option<ItemStack>,
    head: Option<ItemStack>,
}

impl View {
    fn read(attachments: &mut Attachments) -> Result<Self, Error> {
        Ok(Self {
            necklace: attachments
                .get_existing(&NECKLACE)
                .map_err(|_| Error::Storage)?,
            belt: attachments
                .get_existing(&BELT)
                .map_err(|_| Error::Storage)?,
            head: attachments
                .get_existing(&HEAD)
                .map_err(|_| Error::Storage)?,
        })
    }

    pub fn equipped(&self, slot: Slot) -> Option<&ItemStack> {
        match slot {
            Slot::Necklace => self.necklace.as_ref(),
            Slot::Belt => self.belt.as_ref(),
            Slot::Head => self.head.as_ref(),
        }
    }
}

fn move_item(
    attachments: &mut Attachments,
    inventory: &mut [Option<ItemStack>],
    action: Action,
) -> Result<View, Error> {
    let mut view = View::read(attachments)?;
    let slot = match action {
        Action::Inspect => return Ok(view),
        Action::Equip(accessory) => accessory.slot(),
        Action::Remove(slot) => slot,
    };
    let attachment = slot.attachment();
    let worn = match slot {
        Slot::Necklace => &mut view.necklace,
        Slot::Belt => &mut view.belt,
        Slot::Head => &mut view.head,
    };
    match (action, worn.as_ref()) {
        (Action::Equip(accessory), None) => {
            let source = inventory
                .iter_mut()
                .find(|cell| {
                    cell.as_ref().is_some_and(|stack| {
                        stack.item.as_ref() == accessory.id() && stack.count != 0
                    })
                })
                .ok_or(Error::Missing)?;
            let stack = source.as_mut().unwrap();
            let equipped = ItemStack {
                count: 1,
                ..stack.clone()
            };
            attachments
                .set(attachment, equipped.clone())
                .map_err(|_| Error::Storage)?;
            *worn = Some(equipped);
            stack.count -= 1;
            if stack.count == 0 {
                *source = None;
            }
        }
        (Action::Remove(_), Some(_)) => {
            let destination = inventory
                .iter_mut()
                .find(|cell| cell.is_none())
                .ok_or(Error::Full)?;
            attachments.remove(attachment).map_err(|_| Error::Storage)?;
            *destination = worn.take();
        }
        _ => {}
    }
    Ok(view)
}

/// The guest's accessories are installed before the engine restores its current HP.
pub(crate) fn sync_bonus(attachments: &mut Attachments, body: &mut PlayerMotion) {
    for slot in Slot::ALL {
        let equipped = attachments
            .get_existing(slot.attachment())
            .ok()
            .flatten()
            .and_then(|stack| Accessory::from_id(&stack.item));
        for accessory in Accessory::ALL
            .into_iter()
            .filter(|accessory| accessory.slot() == slot)
        {
            let (attribute, modifier, amount) = accessory.modifier();
            let Some(key) = body.attributes.key(attribute) else {
                continue;
            };
            let wearing = equipped == Some(accessory);
            if body.attributes.has_modifier(Some(key), modifier).ok() != Some(wearing) {
                let _ = body.set_modifier(attribute, modifier, amount, "add_value", wearing);
            }
        }
    }
}

pub(super) fn access(game: &mut Game<'_>, action: Action) -> Result<View, Error> {
    if !Accessory::ALL
        .iter()
        .any(|accessory| game.items().get(accessory.id()).is_some())
    {
        return Err(Error::Unavailable);
    }
    if let Action::Equip(accessory) = action
        && game.items().get(accessory.id()).is_none()
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
        && Slot::ALL
            .into_iter()
            .any(|slot| live.has(slot.attachment()) || guest.attachments.has(slot.attachment()))
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
    {
        drop_accessories(&mut game.player_attachments, drops);
    }
}

fn drop_accessories(attachments: &mut Attachments, drops: &mut Vec<ItemStack>) {
    for slot in Slot::ALL {
        let attachment = slot.attachment();
        if let Ok(Some(worn)) = attachments.get_existing(attachment)
            && attachments.remove(attachment).is_ok()
        {
            drops.push(worn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use horizon_client::{assets::Assets, motion::MotionResources};
    use horizon_core::geometry::Vec3;

    fn test_resources() -> Option<MotionResources> {
        let Some(root) = std::env::var_os("HORIZON_TEST_RESOURCES") else {
            eprintln!("prepared resources required for the accessory attribute tests");
            return None;
        };
        let root = std::path::PathBuf::from(root);
        let assets = Assets::load(&root).unwrap();
        Some(MotionResources::load(&root, &assets).unwrap())
    }

    fn test_motion() -> Option<PlayerMotion> {
        let resources = test_resources()?;
        Some(PlayerMotion::new(&resources, Vec3::ZERO).unwrap())
    }

    #[test]
    fn necklace_transfer_is_idempotent_and_keeps_the_exact_stack() {
        let mut saved = Attachments::default();
        let amulet = ItemStack::new(Accessory::Vitality.id(), 1);
        let mut inventory = [
            Some(ItemStack::new("minecraft:stone", 64)),
            Some(amulet.clone()),
        ];
        assert_eq!(
            move_item(
                &mut saved,
                &mut inventory,
                Action::Equip(Accessory::Vitality)
            )
            .unwrap()
            .necklace,
            Some(amulet.clone())
        );
        assert!(inventory[1].is_none());
        assert_eq!(
            move_item(
                &mut saved,
                &mut inventory,
                Action::Equip(Accessory::Vitality)
            )
            .unwrap()
            .necklace,
            Some(amulet.clone())
        );
        inventory[1] = Some(ItemStack::new("minecraft:dirt", 64));
        assert!(matches!(
            move_item(&mut saved, &mut inventory, Action::Remove(Slot::Necklace)),
            Err(Error::Full)
        ));
        assert_eq!(saved.get_existing(&NECKLACE).unwrap(), Some(amulet.clone()));
        inventory[1] = None;
        assert!(
            move_item(&mut saved, &mut inventory, Action::Remove(Slot::Necklace))
                .unwrap()
                .necklace
                .is_none()
        );
        assert!(
            move_item(&mut saved, &mut inventory, Action::Remove(Slot::Necklace))
                .unwrap()
                .necklace
                .is_none()
        );
        assert_eq!(inventory[1], Some(amulet));
    }

    #[test]
    fn necklace_round_trips_through_player_attachments() {
        let mut original = Attachments::default();
        let mut inventory = [Some(ItemStack::new(Accessory::Vitality.id(), 1))];
        move_item(
            &mut original,
            &mut inventory,
            Action::Equip(Accessory::Vitality),
        )
        .unwrap();
        let mut reopened = Attachments::from_nbt(original.to_nbt());
        let mut other_player = Attachments::default();
        assert!(
            move_item(&mut other_player, &mut inventory, Action::Inspect)
                .unwrap()
                .necklace
                .is_none()
        );
        assert_eq!(
            move_item(&mut reopened, &mut inventory, Action::Inspect)
                .unwrap()
                .necklace,
            Some(ItemStack::new(Accessory::Vitality.id(), 1))
        );
        move_item(
            &mut reopened,
            &mut inventory,
            Action::Remove(Slot::Necklace),
        )
        .unwrap();
        assert!(!Attachments::from_nbt(reopened.to_nbt()).has(&NECKLACE));
    }

    #[test]
    fn vitality_bonus_stacks_once_and_preserves_other_health_modifiers() {
        let Some(mut motion) = test_motion() else {
            return;
        };
        let mut saved = Attachments::default();
        let mut inventory = [Some(ItemStack::new(Accessory::Vitality.id(), 1))];
        move_item(
            &mut saved,
            &mut inventory,
            Action::Equip(Accessory::Vitality),
        )
        .unwrap();
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
        move_item(
            &mut reopened,
            &mut inventory,
            Action::Remove(Slot::Necklace),
        )
        .unwrap();
        sync_bonus(&mut reopened, &mut motion);
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 24.0);
        assert_eq!(motion.health.health(), 24.0);
    }

    #[test]
    fn luck_uses_the_same_slot_and_adds_five_once_after_reopening() {
        let vitality = ItemStack::new(Accessory::Vitality.id(), 1);
        let luck = ItemStack::new(Accessory::Luck.id(), 1);
        let mut inventory = [Some(vitality.clone()), Some(luck.clone())];
        let mut saved = Attachments::default();
        move_item(&mut saved, &mut inventory, Action::Equip(Accessory::Luck)).unwrap();
        let mut reopened = Attachments::from_nbt(saved.to_nbt());
        assert_eq!(inventory, [Some(vitality.clone()), None]);
        let result = move_item(
            &mut reopened,
            &mut inventory,
            Action::Equip(Accessory::Vitality),
        )
        .unwrap();
        assert_eq!(result.necklace, Some(luck.clone()));
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

        move_item(
            &mut reopened,
            &mut inventory,
            Action::Remove(Slot::Necklace),
        )
        .unwrap();
        assert_eq!(inventory[1], Some(luck));
        move_item(
            &mut reopened,
            &mut inventory,
            Action::Equip(Accessory::Vitality),
        )
        .unwrap();
        sync_bonus(&mut reopened, &mut motion);
        assert_eq!(motion.attribute(LUCK).unwrap(), 1.0);
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 30.0);
    }

    #[test]
    fn swim_amulet_restores_and_syncs_to_the_client_without_changing_water_efficiency() {
        use horizon_client::lan::attributes::{apply, take_dirty};

        let swim = ItemStack::new(Accessory::SwimSpeed.id(), 1);
        let mut inventory = [Some(swim.clone())];
        let mut saved = Attachments::default();
        move_item(
            &mut saved,
            &mut inventory,
            Action::Equip(Accessory::SwimSpeed),
        )
        .unwrap();
        assert!(inventory[0].is_none());
        let mut reopened = Attachments::from_nbt(saved.to_nbt());
        assert_eq!(
            reopened.get_existing(&NECKLACE).unwrap(),
            Some(swim.clone())
        );

        let (Some(mut server), Some(mut client)) = (test_motion(), test_motion()) else {
            return;
        };
        let attr = PlayerMotion::SWIM_AMULET_SYNC_ATTRIBUTE;
        let efficiency = client.attribute(attr).unwrap();
        sync_bonus(&mut reopened, &mut server);
        apply(&mut client.attributes, &take_dirty(&mut server.attributes));
        assert_eq!(server.dragonfyre_swim_factor(), 1.2);
        assert_eq!(client.dragonfyre_swim_factor(), 1.2);
        assert_eq!(client.attribute(attr).unwrap(), efficiency);
        assert_eq!(client.attribute(MAX_HEALTH).unwrap(), 20.0);
        assert_eq!(client.attribute(LUCK).unwrap(), 0.0);

        move_item(
            &mut reopened,
            &mut inventory,
            Action::Remove(Slot::Necklace),
        )
        .unwrap();
        sync_bonus(&mut reopened, &mut server);
        apply(&mut client.attributes, &take_dirty(&mut server.attributes));
        assert_eq!(client.dragonfyre_swim_factor(), 1.0);
        assert_eq!(inventory[0], Some(swim));
    }

    #[test]
    fn bracing_belt_uses_its_own_saved_slot_and_preserves_armor_resistance() {
        let belt = ItemStack::new(Accessory::BracingBelt.id(), 1);
        let amulet = ItemStack::new(Accessory::Vitality.id(), 1);
        let mut inventory = [Some(belt.clone()), Some(amulet.clone())];
        let mut saved = Attachments::default();
        move_item(
            &mut saved,
            &mut inventory,
            Action::Equip(Accessory::Vitality),
        )
        .unwrap();
        move_item(
            &mut saved,
            &mut inventory,
            Action::Equip(Accessory::BracingBelt),
        )
        .unwrap();
        move_item(
            &mut saved,
            &mut inventory,
            Action::Equip(Accessory::BracingBelt),
        )
        .unwrap();
        assert_eq!(inventory, [None, None]);

        let mut reopened = Attachments::from_nbt(saved.to_nbt());
        let view = View::read(&mut reopened).unwrap();
        assert_eq!(view.equipped(Slot::Belt), Some(&belt));
        assert_eq!(view.equipped(Slot::Necklace), Some(&amulet));

        let Some(mut motion) = test_motion() else {
            return;
        };
        motion
            .set_modifier(
                KNOCKBACK_RESISTANCE,
                "minecraft:armor.boots",
                0.1,
                "add_value",
                true,
            )
            .unwrap();
        sync_bonus(&mut reopened, &mut motion);
        sync_bonus(&mut reopened, &mut motion);
        assert_eq!(motion.attribute(KNOCKBACK_RESISTANCE).unwrap(), 0.6);
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 30.0);

        move_item(&mut reopened, &mut inventory, Action::Remove(Slot::Belt)).unwrap();
        sync_bonus(&mut reopened, &mut motion);
        assert_eq!(motion.attribute(KNOCKBACK_RESISTANCE).unwrap(), 0.1);
        assert_eq!(motion.attribute(MAX_HEALTH).unwrap(), 30.0);
        assert_eq!(inventory, [Some(belt), None]);
        assert!(!Attachments::from_nbt(reopened.to_nbt()).has(&BELT));
    }

    #[test]
    fn equipped_belt_and_necklace_drop_once_and_reject_other_slot_stacks() {
        let belt = ItemStack::new(Accessory::BracingBelt.id(), 1);
        let amulet = ItemStack::new(Accessory::Luck.id(), 1);
        let mut inventory = [Some(belt.clone()), Some(amulet.clone())];
        let mut saved = Attachments::default();
        move_item(&mut saved, &mut inventory, Action::Equip(Accessory::Luck)).unwrap();
        move_item(
            &mut saved,
            &mut inventory,
            Action::Equip(Accessory::BracingBelt),
        )
        .unwrap();
        assert!(decode_slot(&encode_stack(&belt).unwrap(), Slot::Necklace).is_err());
        assert!(decode_slot(&encode_stack(&amulet).unwrap(), Slot::Belt).is_err());

        let mut drops = Vec::new();
        drop_accessories(&mut saved, &mut drops);
        drop_accessories(&mut saved, &mut drops);
        assert_eq!(drops, [amulet, belt]);
        let mut reopened = Attachments::from_nbt(saved.to_nbt());
        let view = View::read(&mut reopened).unwrap();
        assert!(view.equipped(Slot::Necklace).is_none());
        assert!(view.equipped(Slot::Belt).is_none());
    }

    #[test]
    fn emperor_crown_restores_with_other_slots_and_reduces_damage_over_helmet() {
        use horizon_client::{
            health::HealthRules,
            lan::attributes::{apply, take_dirty},
        };

        let crown = ItemStack::new(Accessory::EmperorCrown.id(), 1);
        let belt = ItemStack::new(Accessory::BracingBelt.id(), 1);
        let amulet = ItemStack::new(Accessory::Vitality.id(), 1);
        let mut inventory = [
            Some(crown.clone()),
            Some(belt.clone()),
            Some(amulet.clone()),
        ];
        let mut saved = Attachments::default();
        for accessory in [
            Accessory::EmperorCrown,
            Accessory::BracingBelt,
            Accessory::Vitality,
        ] {
            move_item(&mut saved, &mut inventory, Action::Equip(accessory)).unwrap();
        }
        assert_eq!(inventory, [None, None, None]);
        let mut reopened = Attachments::from_nbt(saved.to_nbt());
        let view = View::read(&mut reopened).unwrap();
        assert_eq!(view.equipped(Slot::Head), Some(&crown));
        assert_eq!(view.equipped(Slot::Belt), Some(&belt));
        assert_eq!(view.equipped(Slot::Necklace), Some(&amulet));
        assert!(decode_slot(&encode_stack(&crown).unwrap(), Slot::Belt).is_err());

        let Some(resources) = test_resources() else {
            return;
        };
        let helmet = || {
            let mut player = PlayerMotion::new(&resources, Vec3::ZERO).unwrap();
            player
                .set_modifier(ARMOR, "minecraft:armor.helmet", 3.0, "add_value", true)
                .unwrap();
            player
        };
        let mut without_crown = helmet();
        let mut with_crown = helmet();
        sync_bonus(&mut reopened, &mut with_crown);
        sync_bonus(&mut reopened, &mut with_crown);
        assert_eq!(with_crown.attribute(ARMOR).unwrap(), 9.0);
        assert_eq!(with_crown.attribute(KNOCKBACK_RESISTANCE).unwrap(), 0.5);
        assert_eq!(with_crown.attribute(MAX_HEALTH).unwrap(), 30.0);

        let mut client = PlayerMotion::new(&resources, Vec3::ZERO).unwrap();
        apply(
            &mut client.attributes,
            &take_dirty(&mut with_crown.attributes),
        );
        assert_eq!(client.attribute(ARMOR).unwrap(), 9.0);

        let attack = resources.health.damage.id("minecraft:mob_attack").unwrap();
        for player in [&mut with_crown, &mut without_crown] {
            assert!(
                player
                    .hurt(&resources.health, HealthRules::default(), attack, 8.0)
                    .unwrap()
            );
        }
        assert!(
            with_crown.hurt_log.last().unwrap().taken
                < without_crown.hurt_log.last().unwrap().taken
        );

        move_item(&mut reopened, &mut inventory, Action::Remove(Slot::Head)).unwrap();
        sync_bonus(&mut reopened, &mut with_crown);
        assert_eq!(with_crown.attribute(ARMOR).unwrap(), 3.0);
        assert_eq!(with_crown.attribute(KNOCKBACK_RESISTANCE).unwrap(), 0.5);
        assert_eq!(inventory, [Some(crown), None, None]);
        assert!(!Attachments::from_nbt(reopened.to_nbt()).has(&HEAD));
    }
}

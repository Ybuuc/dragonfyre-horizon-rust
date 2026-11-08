use crate::items::ItemStack;

pub(super) const ID: &str = "cisco_mod:arcane_apparatus";
const MAX_DAMAGE: u32 = 50;

pub(super) fn remainders(
    mut remaining: Vec<Option<ItemStack>>,
    grid: &[Option<ItemStack>],
) -> Vec<Option<ItemStack>> {
    for (slot, ingredient) in remaining.iter_mut().zip(grid) {
        let Some(stack) = ingredient
            .as_ref()
            .filter(|stack| stack.item.as_ref() == ID)
        else {
            continue;
        };
        // Cisco creates a fresh single stack with the consumed tool's wear plus one.
        *slot = (stack.damage < MAX_DAMAGE - 1).then(|| ItemStack {
            damage: stack.damage + 1,
            ..ItemStack::new(ID, 1)
        });
    }
    remaining
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::{Facts, Recipe, Recipes, Special};

    #[test]
    fn apparatus_returns_in_its_slot_until_the_fiftieth_craft() {
        let mut recipes = Recipes::empty();
        recipes.facts.insert(
            "minecraft:milk_bucket".into(),
            Facts {
                remainder: Some("minecraft:bucket".into()),
                ..Facts::default()
            },
        );
        // Older saves may contain more than one unused apparatus in a stack.
        let mut apparatus = ItemStack::new(ID, 2);
        for wear in 0..MAX_DAMAGE {
            let grid = [
                None,
                Some(ItemStack::new("minecraft:milk_bucket", 1)),
                None,
                Some(apparatus),
            ];
            let result = recipes.remainders("cisco_mod:bright_essence_rec", &grid, 2);
            assert_eq!(result[1], Some(ItemStack::new("minecraft:bucket", 1)));
            assert!(result[0].is_none() && result[2].is_none());
            match &result[3] {
                Some(tool) => {
                    assert_eq!((tool.count, tool.damage), (1, wear + 1));
                    apparatus = tool.clone();
                }
                None => {
                    assert_eq!(wear + 1, MAX_DAMAGE);
                    return;
                }
            }
        }
        panic!("apparatus did not break");
    }

    #[test]
    fn crafting_repair_rejects_apparatus_and_accepts_vanilla_tools() {
        let mut recipes = Recipes::empty();
        for id in [ID, "minecraft:iron_pickaxe"] {
            recipes.facts.insert(
                id.into(),
                Facts {
                    max_damage: MAX_DAMAGE,
                    ..Facts::default()
                },
            );
        }
        recipes
            .crafting
            .push(("repair_item".into(), Recipe::Special(Special::RepairItem)));
        for (id, repairable) in [(ID, false), ("minecraft:iron_pickaxe", true)] {
            let tool = ItemStack {
                damage: 12,
                ..ItemStack::new(id, 1)
            };
            let grid = [Some(tool.clone()), Some(tool), None, None];
            assert_eq!(recipes.recipe_matches("repair_item", &grid, 2), repairable);
        }
    }
}

use super::PlayerMotion;

impl PlayerMotion {
    /// A zero-valued modifier travels in normal attribute packets without
    /// changing the engine's water efficiency or its drag calculation.
    pub const SWIM_AMULET_SYNC_ATTRIBUTE: &str = "minecraft:water_movement_efficiency";
    pub const SWIM_AMULET_SYNC_MODIFIER: &str = "dragonfyre_quests:swim_amulet";

    pub fn dragonfyre_swim_factor(&mut self) -> f64 {
        let key = self.attributes.key(Self::SWIM_AMULET_SYNC_ATTRIBUTE);
        if self
            .attributes
            .has_modifier(key, Self::SWIM_AMULET_SYNC_MODIFIER)
            .unwrap_or(false)
        {
            1.2
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assets::Assets,
        collisions,
        motion::{MotionResources, MotionWorld},
        world::World,
    };
    use horizon_core::geometry::Vec3;
    use horizon_world::living_fluid::travel_in_water;

    #[test]
    fn swim_amulet_scales_propulsion_and_vertical_input_with_normal_water_drag() {
        let Some(root) = std::env::var_os("HORIZON_TEST_RESOURCES") else {
            eprintln!("prepared resources required for the swimming test");
            return;
        };
        let root = std::path::PathBuf::from(root);
        let assets = Assets::load(&root).unwrap();
        let render = assets.reload(&root).unwrap();
        let collision_binding = collisions::load(&root, assets.registry).unwrap();
        let resources = MotionResources::load(&root, &assets).unwrap();
        let mut scene = World::test_scene(&assets, &render, 3).unwrap();
        let water = assets.registry.default_state("minecraft:water").unwrap();
        for x in 19..=21 {
            for y in 3..=7 {
                for z in 19..=21 {
                    scene.set(x, y, z, water).unwrap();
                }
            }
        }

        let mut travel = |equipped| {
            let mut player = PlayerMotion::new(&resources, Vec3::new(20.5, 4.0, 20.5)).unwrap();
            player
                .set_modifier(
                    PlayerMotion::SWIM_AMULET_SYNC_ATTRIBUTE,
                    PlayerMotion::SWIM_AMULET_SYNC_MODIFIER,
                    0.0,
                    "add_value",
                    equipped,
                )
                .unwrap();
            let efficiency = player
                .attribute(PlayerMotion::SWIM_AMULET_SYNC_ATTRIBUTE)
                .unwrap();
            let mut world = MotionWorld {
                entities: &[],
                player: &mut player,
                scene: &mut scene,
                assets: &assets,
                collision_binding: &collision_binding,
                resources: &resources,
                health: &resources.health,
            };
            world
                .controlled_simulation_tick(Default::default())
                .unwrap();
            world.player.velocity = Vec3::ZERO;
            travel_in_water(&mut world, Vec3::new(0.0, 0.0, 1.0), 0.0, false, 4.0).unwrap();
            let forward = world.player.velocity.z;
            world.player.jumping = true;
            world.player.velocity = Vec3::ZERO;
            world.jump_phase().unwrap();
            let ascent = world.player.velocity.y;
            world.player.velocity = Vec3::ZERO;
            world
                .controlled_simulation_tick(crate::sprint::Keys {
                    shift: true,
                    ..Default::default()
                })
                .unwrap();
            (forward, ascent, world.player.velocity.y, efficiency)
        };
        let (forward, ascent, descent, efficiency) = travel(false);
        let (boosted_forward, boosted_ascent, boosted_descent, boosted_efficiency) = travel(true);
        assert!(forward > 0.0 && ascent > 0.0, "{forward}, {ascent}");
        assert!((boosted_forward / forward - 1.2).abs() < 1e-6);
        assert!((boosted_ascent / ascent - 1.2).abs() < 1e-6);
        // Water's gravitational step is separate from the player's sinking input.
        assert!(descent < -0.005);
        assert!(((boosted_descent + 0.005) / (descent + 0.005) - 1.2).abs() < 1e-6);
        assert_eq!(boosted_efficiency, efficiency);
    }
}

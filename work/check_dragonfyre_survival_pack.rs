//! Verify Cisco content registration, the original loot odds and natural tower generation.
use horizon_core::position::ChunkPos;
use horizon_world::{
    DATA_VERSION,
    nbt::Tag,
    registry::BlockRegistry,
    worldgen::generator::{GeneratorData, OverworldGenerator, ProtoChunk},
};
use serde_json::Value;
use std::{fs, path::Path};

type Error = Box<dyn std::error::Error + Send + Sync>;

fn tag(v: &Value) -> Tag {
    match v {
        Value::String(s) => Tag::String(s.as_str().into()),
        Value::Number(n) => Tag::Int(n.as_i64().unwrap() as i32),
        Value::Array(a) => Tag::List(a.iter().map(tag).collect()),
        Value::Object(o) => {
            Tag::Compound(o.iter().map(|(k, v)| (k.as_str().into(), tag(v))).collect())
        }
        _ => panic!("invalid registry metadata"),
    }
}
fn registry(resources: &Path) -> Result<&'static BlockRegistry, Error> {
    let facts: Value = serde_json::from_str(&fs::read_to_string(resources.join("facts.json"))?)?;
    let states = facts["states"].as_array().ok_or("facts states")?;
    let registry = BlockRegistry::from_nbt(&Tag::compound([
        ("DataVersion", Tag::Int(DATA_VERSION)),
        (
            "states",
            Tag::List(states.iter().map(|v| tag(&v[4])).collect()),
        ),
        ("defaults", tag(&facts["defaults"])),
    ]))?;
    Ok(Box::leak(Box::new(registry)))
}


use horizon_mods::{Content, Package};
use horizon_world::{
    loot::{LootData, PackOverlay},
    worldgen::overrides::WorldgenOverrides,
};
fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let resources = std::path::PathBuf::from(args.next().ok_or("client resource directory")?);
    let scratch = std::path::PathBuf::from(args.next().ok_or("scratch directory")?);
    let pack_file = std::path::PathBuf::from(args.next().ok_or("content pack file")?);
    let package = Package::read(&pack_file)?;
    let content = Content::read(&pack_file, &package.manifest)?;
    assert_eq!(content.items.len(), 20);
    assert!(content.items.iter().any(|item| item.id == "cisco_mod:amuletof_vitality"));
    assert!(content.items.iter().any(|item| item.id == "cisco_mod:amuletof_luck"));
    assert!(content.items.iter().any(|item| item.id == "cisco_mod:amuletof_swim_speed"));
    let relative = "data/cisco_mod/loot_table/chests/tower_loot.json";
    let pack = scratch.join("pack");
    let table = "cisco_mod:chests/tower_loot";
    let files = [(format!("loot_table/{table}"), vec![fs::read(pack.join(relative))?])];
    let loot = LootData::load(&resources, &PackOverlay { files: &files })?;
    let core = |seed| {
        loot.fill(table, seed, 27, Some("minecraft:plains")).iter().flatten()
            .any(|stack| stack.item == "cisco_mod:divine_core" && stack.count == 1)
    };
    let wins = (1..=4096).filter(|&seed| core(seed)).collect::<Vec<_>>();
    assert!((120..230).contains(&wins.len()), "unexpected rolls {wins:?}");
    println!("core rolls {}/4096; test seed {}", wins.len(), wins[0]);

    let mut overlay = WorldgenOverrides::default();
    for kind in ["worldgen/structure", "worldgen/structure_set", "worldgen/template_pool",
                 "tags/worldgen/biome/has_structure"] {
        let file = format!("data/cisco_mod/{kind}/owl_mage_tower.json");
        let json = fs::read_to_string(pack.join(&file))?;
        overlay.add_file("cisco_mod", &file, || Some(json));
    }
    let registry = registry(&resources)?;
    let data = GeneratorData::load_with(&scratch.join("worldgen"), registry, &overlay)?;
    for seed in 9000..9064 {
        let generator = OverworldGenerator::for_dimension(data.clone(), seed, "minecraft:normal", "minecraft:overworld")?;
        let mut worker = generator.worker();
        worker.set_structures_only(true);
        let name = "cisco_mod:owl_mage_tower".to_owned();
        let Some((_, pos)) = worker.locate(&[name.clone()], [0, 64, 0], 5) else { continue };
        let cx = pos[0].div_euclid(16);
        let cz = pos[2].div_euclid(16);
        let start = worker.structure_starts(ChunkPos::new(cx, cz)).into_iter()
            .find(|s| s.name == name).ok_or("no start for located tower")?;
        println!("world seed {seed}, located {pos:?}: {:?}", start.bounding_box);
        for z in (start.bounding_box.min[2]+12).div_euclid(16)..=(start.bounding_box.max[2]-12).div_euclid(16) {
            for x in (start.bounding_box.min[0]+12).div_euclid(16)..=(start.bounding_box.max[0]-12).div_euclid(16) {
                let mut region = Vec::with_capacity(9);
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        region.push(worker.generate_proto(ChunkPos::new(x + dx, z + dz))?);
                    }
                }
                let mut region: [ProtoChunk; 9] = region.try_into().map_err(|_| "region size")?;
                worker.decorate(&mut region)?;
                let chunk = &region[4];
                for ([bx, by, bz], nbt) in &chunk.block_entities {
                    let (bx, by, bz) = (*bx, *by, *bz);
                    let Some(Tag::String(id)) = nbt.get(&"LootTable".into()) else { continue };
                    if id.to_string_lossy() != table { continue; }
                    let Some(Tag::Long(roll)) = nbt.get(&"LootTableSeed".into()) else { continue };
                    println!("chest {bx} {by} {bz} loot seed {roll} contains core {}", core(*roll));
                    if core(*roll) {
                        for dy in -1..=2 {
                            for dz in -2..=2 {
                                let line: Vec<_> = (-2..=2).map(|dx| chunk.block(bx+dx,by+dy,bz+dz).0).collect();
                                println!("around {dy} {dz} {line:?}");
                            }
                        }
                        return Ok(());
                    }
                }
            }
        }
    }
    Err("tower loot did not produce a deterministic chest in tested worlds".into())
}

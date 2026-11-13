fn dragonfyre_dragon_loot(id: &str, table: &mut Value) {
    // Iron's Spellbooks appends data/irons_spellbooks/loot_table/entities/additional_dragon_loot.json
    // on every Ender Dragon death through a Forge global loot modifier. Horizon does not let a
    // content mod write data/minecraft/, so the same nested table is attached here, after the
    // data packs loaded the empty vanilla table and before anything rolls it.
    if id != "minecraft:entities/ender_dragon" {
        return;
    }
    let pool = serde_json::json!({
        "rolls": 1,
        "bonus_rolls": 0.0,
        "entries": [{
            "type": "minecraft:loot_table",
            "value": "irons_spellbooks:entities/additional_dragon_loot"
        }]
    });
    match table.get_mut("pools").and_then(|pools| pools.as_array_mut()) {
        Some(pools) => pools.push(pool),
        None => table["pools"] = Value::Array(vec![pool]),
    }
}

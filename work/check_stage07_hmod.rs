use horizon_mods::{Content, Package};
use std::{collections::BTreeMap, path::Path};

fn main() {
    let paths: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(paths.len(), 2, "pass both Stage 07 .hmod files");
    let mut items = BTreeMap::new();
    for path in &paths {
        let p = Path::new(path);
        let pkg = Package::read(p).expect("Horizon rejected the mod manifest");
        assert!(pkg.wasm.is_none());
        let content = Content::read(p, &pkg.manifest).expect("Horizon rejected the content registry");
        for item in content.items {
            assert!(items.insert(item.id, item.max_stack_size.unwrap()).is_none());
        }
    }
    assert_eq!(
        items,
        BTreeMap::from([
            ("cisco_mod:champion_coin".into(), 64),
            ("cisco_mod:divine_core".into(), 64),
            ("cisco_mod:radiant_ruby".into(), 64),
            ("farmersdelight:honey_glazed_ham".into(), 16),
        ])
    );
    println!("PASS: Horizon parsed both packages; four original item IDs and their stack limits");
}

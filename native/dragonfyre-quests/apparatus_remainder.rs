fn wear_arcane_apparatus(
    remaining: Vec<Option<ItemStack>>,
    grid: &[Option<ItemStack>],
) -> Vec<Option<ItemStack>> {
    crate::crafting::dragonfyre_apparatus::remainders(remaining, grid)
}

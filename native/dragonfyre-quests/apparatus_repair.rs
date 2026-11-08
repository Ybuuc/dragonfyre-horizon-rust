fn disable_arcane_apparatus_crafting_repair(
    recipe: &Recipe,
    input: &Input<'_>,
) -> std::ops::ControlFlow<Option<Option<ItemStack>>> {
    if matches!(recipe, Recipe::Special(Special::RepairItem))
        && input
            .cells
            .iter()
            .flatten()
            .any(|stack| stack.item.as_ref() == crate::crafting::dragonfyre_apparatus::ID)
    {
        return std::ops::ControlFlow::Break(None);
    }
    std::ops::ControlFlow::Continue(())
}

fn dragonfyre_register(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    use horizon_client::mod_api::NativeCommand;
    commands.push(NativeCommand::op("dragonfyredev status <quest:word>", 3));
    commands.push(NativeCommand::op(
        "dragonfyredev checkmark <quest:word> <task:word>",
        3,
    ));
    for operation in ["claim", "submit", "economy"] {
        commands.push(NativeCommand::op(
            &format!("dragonfyredev {operation} <quest:word>"),
            3,
        ));
        commands.push(NativeCommand::op(
            &format!("dragonfyredev {operation} <quest:word> <cycle:word>"),
            3,
        ));
    }
}

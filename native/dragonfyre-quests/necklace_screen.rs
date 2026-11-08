use super::{Game, HudBatch, NativeScreenView, dragonfyre_necklace as necklace};
use necklace::{Accessory, Action, Error, Slot, View};
use std::sync::{
    Mutex,
    mpsc::{self, Receiver, TryRecvError},
};

const SCREEN: &str = "dragonfyre_quests:necklace";
const WIDTH: f32 = 252.0;
const HEIGHT: f32 = 212.0;

#[derive(Default)]
struct Client {
    view: Option<View>,
    waiting: Option<Receiver<Result<View, Error>>>,
    error: Option<Error>,
}

static CLIENT: Mutex<Client> = Mutex::new(Client {
    view: None,
    waiting: None,
    error: None,
});

impl Client {
    fn request(&mut self, game: &Game<'_>, action: Action) {
        let (tx, rx) = mpsc::channel();
        if game.integrated.as_ref().is_some_and(|server| {
            server.dragonfyre_post(move |game| {
                let _ = tx.send(necklace::access(game, action));
            })
        }) {
            self.waiting = Some(rx);
            self.error = None;
        } else {
            self.error = Some(Error::Unavailable);
        }
    }

    fn update(&mut self) {
        let Some(waiting) = &self.waiting else { return };
        match waiting.try_recv() {
            Ok(Ok(view)) => self.view = Some(view),
            Ok(Err(error)) => self.error = Some(error),
            Err(TryRecvError::Disconnected) => self.error = Some(Error::Unavailable),
            Err(TryRecvError::Empty) => return,
        }
        self.waiting = None;
    }
}

pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || key != "key.keyboard.k" {
        return;
    }
    if game.native_mod_screen() == Some(SCREEN) {
        game.native_mod_screen_close(SCREEN);
    } else if game.native_playing() && game.native_mod_screen_open(SCREEN) {
        let mut client = CLIENT.lock().unwrap();
        *client = Client::default();
        client.request(game, Action::Inspect);
    }
}

fn panel([x, y, w, h]: [f32; 4]) -> [f32; 2] {
    [x + (w - WIDTH) * 0.5, y + (h - HEIGHT) * 0.5]
}

fn button([x, y]: [f32; 2], row: usize) -> [f32; 4] {
    let top = match row {
        3 => 125.0,
        4 => 168.0,
        _ => 32.0 + 25.0 * row as f32,
    };
    [x + 174.0, y + top, 66.0, 20.0]
}

fn rows(view: &View) -> impl Iterator<Item = (usize, Accessory, Action)> + '_ {
    Accessory::ALL
        .into_iter()
        .enumerate()
        .filter_map(|(index, accessory)| {
            let slot = accessory.slot();
            match view.equipped(slot) {
                Some(stack) if stack.item.as_ref() == accessory.id() => {
                    let row = if slot == Slot::Necklace { 0 } else { index };
                    Some((row, accessory, Action::Remove(slot)))
                }
                Some(_) => None,
                None => Some((index, accessory, Action::Equip(accessory))),
            }
        })
}

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if game.native_mod_screen() != Some(SCREEN) {
        return;
    }
    let mut client = CLIENT.lock().unwrap();
    client.update();
    let ru = game.settings.language.starts_with("ru");
    let tr = |en, ru_text| if ru { ru_text } else { en };
    let [x, y] = panel(screen.rect);
    game.native_fill(batch, screen.rect, 0x680a0808);
    game.native_fill(batch, [x, y, WIDTH, HEIGHT], 0xff211b1a);
    game.native_fill(batch, [x, y, WIDTH, 2.0], 0xffbc8547);
    game.native_text(
        batch,
        tr("Necklace slot", "Слот «Ожерелье»"),
        [x + 12.0, y + 12.0],
        0xf1c46c,
        false,
    );
    game.native_text(
        batch,
        tr("Esc", "Esc"),
        [x + 220.0, y + 12.0],
        0xbdb2a4,
        false,
    );
    game.native_text(
        batch,
        tr("Belt slot", "Слот «Пояс»"),
        [x + 12.0, y + 109.0],
        0xf1c46c,
        false,
    );
    game.native_text(
        batch,
        tr("Head accessory slot", "Слот «Голова»"),
        [x + 12.0, y + 152.0],
        0xf1c46c,
        false,
    );
    if let Some(view) = &client.view {
        for (row, accessory, action) in rows(view) {
            let equipped = view.equipped(accessory.slot());
            let bounds = button([x, y], row);
            let line = bounds[1];
            game.native_fill(batch, [x + 12.0, line, 20.0, 20.0], 0xff4b3831);
            let icon = [x + 14.0, line + 2.0];
            if let Some(stack) = equipped {
                game.native_item_stack(batch, stack, icon, 16.0);
            } else {
                game.native_item(batch, accessory.id(), icon, 16.0);
            }
            let (name, effect) = match accessory {
                Accessory::Vitality => (
                    tr("Amulet of Vitality", "Амулет жизненной силы"),
                    tr("+10 max health", "+10 к максимуму здоровья"),
                ),
                Accessory::Luck => (
                    tr("Amulet of Luck", "Амулет удачи"),
                    tr("+5 luck", "+5 к удаче"),
                ),
                Accessory::SwimSpeed => (
                    tr("Amulet of Swim Speed", "Амулет скорости плавания"),
                    tr("+20% swim speed", "+20% к скорости плавания"),
                ),
                Accessory::BracingBelt => (
                    tr("Belt of Bracing", "Пояс стойкости"),
                    tr("+50% knockback resist.", "+50% защиты от толчков"),
                ),
                Accessory::EmperorCrown => (
                    tr("Crown of the Emperor", "Корона императора"),
                    tr("+6 armor", "+6 к броне"),
                ),
            };
            game.native_text(batch, name, [x + 38.0, line], 0xf6eee0, false);
            game.native_text(batch, effect, [x + 38.0, line + 10.0], 0xbdb2a4, false);
            let fill = if client.waiting.is_some() {
                0xff332f2c
            } else {
                0xff6e502b
            };
            game.native_fill(batch, bounds, fill);
            let label = if matches!(action, Action::Remove(_)) {
                tr("Remove", "Снять")
            } else {
                tr("Equip", "Надеть")
            };
            let bx = bounds[0] + (bounds[2] - game.native_text_width(label)) * 0.5;
            game.native_text(batch, label, [bx, line + 6.0], 0xf6eee0, false);
        }
    }
    let status = if client.waiting.is_some() {
        tr("Working…", "Подождите…")
    } else {
        match client.error {
            Some(Error::Unavailable) => tr("Slot unavailable", "Слот недоступен"),
            Some(Error::Missing) => tr(
                "Accessory missing from inventory",
                "Аксессуара нет в инвентаре",
            ),
            Some(Error::Full) => tr(
                "Free a slot in your inventory",
                "Освободите место в инвентаре",
            ),
            Some(Error::Storage) => tr("Could not read or save the slot", "Ошибка данных слота"),
            None => "",
        }
    };
    game.native_text(batch, status, [x + 12.0, y + 196.0], 0xf1c46c, false);
}

pub(super) fn click(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    mouse_button: i32,
    canceled: &mut bool,
) {
    if *canceled || game.native_mod_screen() != Some(SCREEN) {
        return;
    }
    *canceled = true;
    if mouse_button != 0 {
        return;
    }
    let hit = |row| {
        let [x, y, w, h] = button(panel(screen.rect), row);
        let [mx, my] = screen.mouse;
        (x..x + w).contains(&mx) && (y..y + h).contains(&my)
    };
    let mut client = CLIENT.lock().unwrap();
    if client.waiting.is_none()
        && let Some(view) = &client.view
    {
        let action = rows(view)
            .find(|(row, _, _)| hit(*row))
            .map(|(_, _, action)| action);
        if let Some(action) = action {
            client.request(game, action);
        }
    }
}

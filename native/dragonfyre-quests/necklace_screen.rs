use super::{Game, HudBatch, NativeScreenView, dragonfyre_necklace as necklace};
use necklace::{Action, Error, View};
use std::sync::{
    Mutex,
    mpsc::{self, Receiver, TryRecvError},
};

const SCREEN: &str = "dragonfyre_quests:necklace";
const WIDTH: f32 = 252.0;
const HEIGHT: f32 = 105.0;

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

fn button([x, y]: [f32; 2]) -> [f32; 4] {
    [x + 146.0, y + 49.0, 94.0, 20.0]
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
    game.native_fill(batch, [x + 12.0, y + 34.0, 26.0, 26.0], 0xff4b3831);
    let equipped = client.view.as_ref().and_then(|v| v.equipped.as_ref());
    if let Some(stack) = equipped {
        game.native_item_stack(batch, stack, [x + 17.0, y + 39.0], 16.0);
    }
    let name = if equipped.is_some() {
        tr("Amulet of Vitality", "Амулет жизненной силы")
    } else {
        tr("Empty", "Пусто")
    };
    game.native_text(batch, name, [x + 44.0, y + 35.0], 0xf6eee0, false);
    game.native_text(
        batch,
        tr("+10 max health", "+10 к максимуму здоровья"),
        [x + 12.0, y + 72.0],
        0xbdb2a4,
        false,
    );
    if client.view.is_some() {
        let bounds = button([x, y]);
        let fill = if client.waiting.is_some() {
            0xff332f2c
        } else {
            0xff6e502b
        };
        game.native_fill(batch, bounds, fill);
        let label = if equipped.is_some() {
            tr("Remove", "Снять")
        } else {
            tr("Equip", "Надеть")
        };
        let bx = bounds[0] + (bounds[2] - game.native_text_width(label)) * 0.5;
        game.native_text(batch, label, [bx, bounds[1] + 6.0], 0xf6eee0, false);
    }
    let status = if client.waiting.is_some() {
        tr("Working…", "Подождите…")
    } else {
        match client.error {
            Some(Error::Unavailable) => tr("Slot unavailable", "Слот недоступен"),
            Some(Error::Missing) => tr("Amulet missing from inventory", "Амулета нет в инвентаре"),
            Some(Error::Full) => tr(
                "Free a slot in your inventory",
                "Освободите место в инвентаре",
            ),
            Some(Error::Storage) => tr("Could not read or save the slot", "Ошибка данных слота"),
            None => "",
        }
    };
    game.native_text(batch, status, [x + 12.0, y + 89.0], 0xf1c46c, false);
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
    let [x, y, w, h] = button(panel(screen.rect));
    let [mx, my] = screen.mouse;
    if mouse_button != 0 || !(x..x + w).contains(&mx) || !(y..y + h).contains(&my) {
        return;
    }
    let mut client = CLIENT.lock().unwrap();
    if client.waiting.is_none()
        && let Some(view) = &client.view
    {
        let action = if view.equipped.is_some() {
            Action::Remove
        } else {
            Action::Equip
        };
        client.request(game, action);
    }
}

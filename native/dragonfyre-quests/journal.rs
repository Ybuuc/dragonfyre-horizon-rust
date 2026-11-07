use super::{Game, HudBatch, NativeScreenView, dragonfyre_economy as economy};
use economy::{Journal, JournalState};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock, mpsc::{self, Receiver, TryRecvError}},
    time::{Duration, Instant},
};

const MOD: &str = "dragonfyre_quests";
const WIDTH: f32 = 360.0;
const HEIGHT: f32 = 196.0;
const ROWS: [f32; 3] = [37.0, 81.0, 125.0];
const GOLD: u32 = 0xf1c46c;
const LIGHT: u32 = 0xf6eee0;
const MUTED: u32 = 0xbdb2a4;

type Reply<T> = Receiver<Result<T, String>>;
type Locale = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Default)]
struct Client {
    view: Option<Journal>,
    reading: Option<Reply<Journal>>,
    pending: Option<Reply<()>>,
    refresh_at: Option<Instant>,
    notice: Option<&'static str>,
}

static CLIENT: Mutex<Client> = Mutex::new(Client {
    view: None, reading: None, pending: None, refresh_at: None, notice: None,
});

fn text<'a>(game: &Game<'_>, key: &'a str) -> &'a str {
    static LOCALE: OnceLock<Locale> = OnceLock::new();
    let locales = LOCALE.get_or_init(|| {
        serde_json::from_slice(horizon_client::native_data!("dragonfyre_quests", "journal.json"))
            .expect("Dragonfyre journal translation")
    });
    let locale = if game.settings.language.starts_with("ru") { "ru" } else { "en" };
    locales.get(locale).and_then(|v| v.get(key)).map_or(key, String::as_str)
}

fn receive<T>(reply: &mut Option<Reply<T>>) -> Option<Result<T, String>> {
    let result = match reply.as_ref()?.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => return None,
        Err(TryRecvError::Disconnected) => Err("player unavailable".into()),
    };
    *reply = None;
    Some(result)
}

fn error_key(error: &str) -> &'static str {
    match error {
        "FullInventory" => "full",
        "MissingItems" => "missing",
        "player unavailable" | "no world" | "close the container first" => "unavailable",
        "quest not complete" | "previous exchange cycle is not claimed" => "changed",
        _ => "storage",
    }
}

impl Client {
    fn update(&mut self, game: &Game<'_>) {
        let Some(server) = &game.integrated else {
            self.notice = Some("local_only");
            return;
        };
        if let Some(result) = receive(&mut self.pending) {
            self.notice = Some(match result {
                Ok(()) => "saved",
                Err(e) => error_key(&e),
            });
            self.view = None;
            self.refresh_at = None;
        }
        if let Some(result) = receive(&mut self.reading) {
            match result {
                Ok(view) => self.view = Some(view),
                Err(error) => {
                    self.view = None;
                    self.notice = Some(error_key(&error));
                }
            }
        }
        if self.pending.is_none() && self.reading.is_none()
            && self.refresh_at.is_none_or(|deadline| Instant::now() >= deadline)
        {
            self.refresh_at = Some(Instant::now() + Duration::from_millis(850));
            let (tx, rx) = mpsc::channel();
            if server.dragonfyre_post(move |game| {
                let _ = tx.send(economy::journal(game));
            }) {
                self.reading = Some(rx);
            } else {
                self.view = None;
                self.notice = Some("unavailable");
            }
        }
    }
}

pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || key != "key.keyboard.j" {
        return;
    }
    if game.native_mod_screen() == Some(MOD) {
        game.native_mod_screen_close(MOD);
    } else if game.native_playing() && game.native_mod_screen_open(MOD) {
        let mut client = CLIENT.lock().unwrap();
        *client = Client::default();
        client.update(game);
    }
}

struct Layout {
    x: f32,
    y: f32,
    scale: f32,
}

impl Layout {
    fn new(rect: [f32; 4]) -> Self {
        let [x, y, w, h] = rect;
        let scale = (w / (WIDTH + 12.0)).min(h / (HEIGHT + 12.0)).min(1.0);
        Self { x: x + (w - WIDTH * scale) * 0.5, y: y + (h - HEIGHT * scale) * 0.5, scale }
    }

    fn point(&self, mouse: [f32; 2]) -> [f32; 2] {
        [(mouse[0] - self.x) / self.scale, (mouse[1] - self.y) / self.scale]
    }
}

fn hit([x, y]: [f32; 2], [left, top, w, h]: [f32; 4]) -> bool {
    x >= left && x < left + w && y >= top && y < top + h
}

fn button_rect(index: usize, kind: u8) -> [f32; 4] {
    match (index, kind) {
        (2, 2) => [220.0, ROWS[index] + 10.0, 61.0, 19.0],
        (2, _) => [286.0, ROWS[index] + 10.0, 59.0, 19.0],
        _ => [244.0, ROWS[index] + 10.0, 101.0, 19.0],
    }
}

fn button(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], mouse: [f32; 2],
          label: &str, enabled: bool) {
    let [x, y, w, h] = rect;
    let fill = if !enabled { 0xff332f2c } else if hit(mouse, rect) { 0xff8c6530 } else { 0xff6e502b };
    game.native_fill(batch, rect, fill);
    game.native_fill(batch, [x, y, w, 1.0], if enabled { 0xffbd8a44 } else { 0xff504945 });
    game.native_text(batch, label, [x + (w - game.native_text_width(label)) * 0.5, y + (h - 8.0) * 0.5],
                     if enabled { LIGHT } else { MUTED }, false);
}

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if game.native_mod_screen() != Some(MOD) {
        return;
    }
    let layout = Layout::new(screen.rect);
    if layout.scale <= 0.0 {
        return;
    }
    let mouse = layout.point(screen.mouse);
    let mut client = CLIENT.lock().unwrap();
    client.update(game);
    game.native_fill(batch, screen.rect, 0x680a0808);
    let start = batch.vertices.len();
    game.native_fill(batch, [0.0, 0.0, WIDTH, HEIGHT], 0xff211b1a);
    game.native_fill(batch, [0.0, 0.0, WIDTH, 2.0], 0xffbc8547);
    game.native_text(batch, text(game, "title"), [14.0, 12.0], GOLD, false);
    let close = text(game, "close");
    game.native_text(batch, close, [WIDTH - 14.0 - game.native_text_width(close), 12.0], MUTED, false);

    if let Some(view) = &client.view {
        let mut completed = 0;
        for (index, row) in view.rows.iter().enumerate() {
            let y = ROWS[index];
            game.native_fill(batch, [13.0, y, 334.0, 1.0], 0xff50423a);
            let (title, icon, required, reward) = match index {
                0 => ("ruby", "cisco_mod:radiant_ruby", 1, "reward_three"),
                1 => ("core", "cisco_mod:divine_core", 1, "reward_two"),
                _ => ("trade", "farmersdelight:honey_glazed_ham", 2, "reward_ham"),
            };
            game.native_item(batch, icon, [15.0, y + 10.0], 17.0);
            game.native_text(batch, text(game, title), [39.0, y + 5.0], LIGHT, false);
            let count = match row.state {
                JournalState::Claimed | JournalState::Claim => required,
                _ => row.count.min(required),
            };
            let detail = format!("{count}/{required}  ·  {}", text(game, reward));
            game.native_text(batch, &detail, [39.0, y + 16.0], MUTED, false);
            let status = match row.state {
                JournalState::Collect => "collect",
                JournalState::Submit => "ready_submit",
                JournalState::Claim => "ready_claim",
                JournalState::Claimed => {
                    completed += 1;
                    "claimed"
                }
            };
            let status = if index == 2 {
                format!("{} {}  ·  {}", text(game, "cycle"), row.cycle, text(game, status))
            } else {
                text(game, status).to_owned()
            };
            game.native_text(batch, &status, [39.0, y + 27.0], GOLD, false);
            let active = client.pending.is_none();
            if index == 2 {
                button(game, batch, button_rect(index, 2), mouse, text(game, "submit"),
                       active && row.state == JournalState::Submit);
            }
            button(game, batch, button_rect(index, 1), mouse, text(game, "claim"),
                   active && row.state == JournalState::Claim);
        }
        let totals = format!("{completed}/2  ·  {} {}", text(game, "exchanges"), view.exchanges);
        game.native_text(batch, &totals, [14.0, 171.0], MUTED, false);
    }
    let status = if client.pending.is_some() { Some("working") }
        else { client.notice.or(if client.view.is_none() { Some("loading") } else { None }) };
    if let Some(key) = status {
        game.native_text(batch, text(game, key), [14.0, 184.0], LIGHT, false);
    }
    for vertex in &mut batch.vertices[start..] {
        vertex.pos[0] = layout.x + vertex.pos[0] * layout.scale;
        vertex.pos[1] = layout.y + vertex.pos[1] * layout.scale;
    }
}

pub(super) fn click(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if *canceled || game.native_mod_screen() != Some(MOD) {
        return;
    }
    *canceled = true;
    if button != 0 {
        return;
    }
    let layout = Layout::new(screen.rect);
    if layout.scale <= 0.0 {
        return;
    }
    let mouse = layout.point(screen.mouse);
    let mut client = CLIENT.lock().unwrap();
    if client.pending.is_some() {
        return;
    }
    let Some((index, row, kind)) = client.view.as_ref().and_then(|view| {
        view.rows.iter().enumerate().find_map(|(index, row)| {
            let kind = match row.state {
                JournalState::Submit => 2,
                JournalState::Claim => 1,
                _ => return None,
            };
            hit(mouse, button_rect(index, kind)).then_some((index, *row, kind))
        })
    }) else {
        return;
    };
    debug_assert_eq!(economy::JOURNAL_QUESTS[index], row.quest);
    let Some(server) = &game.integrated else {
        return;
    };
    let (tx, rx) = mpsc::channel();
    if server.dragonfyre_post(move |game| economy::journal_action(game, row.quest, kind, row.cycle, tx)) {
        client.pending = Some(rx);
        client.reading = None;
        client.notice = None;
    } else {
        client.notice = Some("unavailable");
    }
}


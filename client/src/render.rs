use crate::mediator::{PacketMessage, WindowMessage};
use gtk::gdk_pixbuf::{Pixbuf, PixbufLoader};
use gtk::prelude::*;
use relm4::component::{AsyncComponent, AsyncComponentParts};
use relm4::drawing::DrawHandler;
use relm4::loading_widgets::LoadingWidgets;
use relm4::*;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const BOARD_ASSET: &[u8] = include_bytes!("assets/board-big.png");
const RED_COIN_ASSET: &[u8] = include_bytes!("assets/red-coin-big.png");
const YELLOW_COIN_ASSET: &[u8] = include_bytes!("assets/yellow-coin-big.png");

pub fn spawn_ui(
    message_sender: UnboundedSender<PacketMessage>,
    message_receiver: UnboundedReceiver<WindowMessage>,
) -> anyhow::Result<()> {
    let connect_4_app = RelmApp::new("rs.scrapyard.Connect4App");
    connect_4_app.run_async::<App>((message_sender, message_receiver));
    Ok(())
}

#[derive(Debug)]
enum AppMessage {
    ForwardRequestUsername,
    LookForGame,
    PlaceColumn(u8),
    Window(WindowMessage),
}

#[derive(Debug)]
enum ViewMode {
    RequestUsername,
    Lobby,
    LookingForGame,
    Game,
}

#[derive(Debug)]
struct App {
    mode: ViewMode,
    packet_message_sender: UnboundedSender<PacketMessage>,
    username_buffer: gtk::EntryBuffer,
    username: Option<String>,
    last_username_failure: Option<String>,
    known_board: Rc<RefCell<[[Option<bool>; 6]; 7]>>,
    my_turn: bool,
    opponent: Option<String>,
    game_draw_handler: DrawHandler,
}

fn pixbuf_from(width: i32, height: i32, bytes: &[u8]) -> Pixbuf {
    let buf = PixbufLoader::with_type("png").unwrap();
    buf.set_size(width, height);
    buf.write(&bytes).unwrap();
    let pixbuf = buf.pixbuf().unwrap();
    buf.close().unwrap();
    pixbuf
}

#[relm4::component(async)]
impl AsyncComponent for App {
    type Init = (
        UnboundedSender<PacketMessage>,
        UnboundedReceiver<WindowMessage>,
    );
    type Input = AppMessage;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::Window {
            gtk::Box {
                set_hexpand: true,
                set_vexpand: true,

                gtk::Box {
                    #[watch]
                    set_visible: matches!(model.mode, ViewMode::RequestUsername),
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 5,
                    set_margin_all: 5,

                    gtk::Label {
                        set_label: &format!("Enter Username:"),
                        set_margin_all: 5,
                    },

                    gtk::Entry {
                        set_buffer: &model.username_buffer,
                        set_tooltip_text: Some("Select a username"),
                        set_margin_all: 5,
                        connect_activate => AppMessage::ForwardRequestUsername,
                    },

                    gtk::Label {
                        #[watch]
                        set_visible: model.last_username_failure.is_some(),
                        #[watch]
                        set_label: &format!("Failed to acquire username `{}`; it's probably already taken.\nMake sure it's alphanumeric.", model.last_username_failure.as_ref().unwrap_or(&String::new())),
                        set_margin_all: 5,
                    }
                },


                gtk::Box {
                    #[watch]
                    set_visible: matches!(model.mode, ViewMode::Lobby),
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 5,
                    set_margin_all: 5,

                    gtk::Button {
                        set_label: "Look for Game",
                        set_margin_all: 5,
                        connect_clicked => AppMessage::LookForGame,
                    }
                },

                gtk::Box {
                    #[watch]
                    set_visible: matches!(model.mode, ViewMode::LookingForGame),
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 5,
                    set_margin_all: 5,

                    gtk::Label {
                        set_label: "Waiting for opponent...",
                        set_margin_all: 5,
                    },
                },

                gtk::Box {
                    #[watch]
                    set_visible: matches!(model.mode, ViewMode::Game),
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 5,
                    set_margin_all: 5,

                    gtk::Label {
                        set_label: "Waiting for opponent...",
                        set_margin_all: 5,
                        #[watch]
                        set_visible: matches!(model.opponent, None),
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &format!("Opponent: {}", model.opponent.as_ref().unwrap_or(&String::new())),
                        #[watch]
                        set_visible: !matches!(model.opponent, None),
                    },

                    gtk::Label {
                        set_label: &format!("Your turn!"),
                        #[watch]
                        set_visible: model.my_turn,
                    },

                    #[local_ref]
                    area -> gtk::DrawingArea {
                        set_size_request: (276, 238),
                        set_draw_func: move |_, ctx, _, _| {
                            let board_pix_buf = pixbuf_from(276, 238, BOARD_ASSET);
                            let red_coin_pix_buf = pixbuf_from(28, 28, RED_COIN_ASSET);
                            let yellow_coin_pix_buf = pixbuf_from(28, 28, YELLOW_COIN_ASSET);

                            for x in 0..7 {
                                for y in 0..6 {
                                    if let Some(is_red) = board.borrow()[x][y] {
                                        let coin_pix_buf = if is_red {
                                            &red_coin_pix_buf
                                        } else {
                                            &yellow_coin_pix_buf
                                        };
                                        ctx.set_source_pixbuf(coin_pix_buf, (10 + (x * 38)) as f64, (10 + ((5 - y) * 38)) as f64);
                                        ctx.paint().expect("Painting coins.");
                                    }
                                }
                            }

                            ctx.set_source_pixbuf(&board_pix_buf, 0f64, 0f64);
                            ctx.paint().expect("Failed to paint");
                        }
                    },

                    gtk::Box {
                        #[watch]
                        set_visible: model.my_turn,
                        set_orientation: gtk::Orientation::Horizontal,

                        gtk::Button {
                            set_label: "1",
                            set_margin_all: 5,
                            connect_clicked => AppMessage::PlaceColumn(0),
                        },
                        gtk::Button {
                            set_label: "2",
                            set_margin_all: 5,
                            connect_clicked => AppMessage::PlaceColumn(1),
                        },
                        gtk::Button {
                            set_label: "3",
                            set_margin_all: 5,
                            connect_clicked => AppMessage::PlaceColumn(2),
                        },
                        gtk::Button {
                            set_label: "4",
                            set_margin_all: 5,
                            connect_clicked => AppMessage::PlaceColumn(3),
                        },
                        gtk::Button {
                            set_label: "5",
                            set_margin_all: 5,
                            connect_clicked => AppMessage::PlaceColumn(4),
                        },
                        gtk::Button {
                            set_label: "6",
                            set_margin_all: 5,
                            connect_clicked => AppMessage::PlaceColumn(5),
                        },
                        gtk::Button {
                            set_label: "7",
                            set_margin_all: 5,
                            connect_clicked => AppMessage::PlaceColumn(6),
                        },
                    }
                }
            }
        }
    }

    fn init_loading_widgets(root: &mut Self::Root) -> Option<LoadingWidgets> {
        relm4::view! {
            #[local_ref]
            root {
                set_title: Some("Connect 4 Supremacy"),
                set_default_size: (300, 100),

                #[name(spinner)]
                gtk::Spinner {
                    start: (),
                    set_halign: gtk::Align::Center,
                }
            }
        }
        Some(LoadingWidgets::new(root, spinner))
    }

    async fn init(
        (message_sender, mut message_receiver): Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = App {
            mode: ViewMode::RequestUsername,
            packet_message_sender: message_sender,
            username_buffer: gtk::EntryBuffer::new(None),
            username: None,
            last_username_failure: None,
            known_board: Rc::new(RefCell::new([[None; 6]; 7])),
            my_turn: false,
            opponent: None,
            game_draw_handler: DrawHandler::new(),
        };

        let mut flip = false;
        let mut board = model.known_board.borrow_mut();
        for x in 0..7 {
            for y in 0..6 {
                board[x][y] = Some(flip);
                flip = !flip;
            }
        }
        drop(board);

        let area = model.game_draw_handler.drawing_area();
        let board = model.known_board.clone();

        let sender_clone = sender.clone();
        tokio::spawn(async move {
            while let Some(message) = message_receiver.recv().await {
                sender_clone.input(AppMessage::Window(message));
            }
        });

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: AppMessage,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            AppMessage::ForwardRequestUsername => {
                self.packet_message_sender
                    .send(PacketMessage::RequestUsername {
                        username: self.username_buffer.text().to_string(),
                    })
                    .unwrap();
            }
            AppMessage::LookForGame => {
                self.mode = ViewMode::LookingForGame;
                self.packet_message_sender
                    .send(PacketMessage::SearchForGame)
                    .unwrap();
            }
            AppMessage::PlaceColumn(column) => {
                self.packet_message_sender
                    .send(PacketMessage::PlacePieceInGame { column })
                    .unwrap();
            }
            AppMessage::Window(window_message) => match window_message {
                WindowMessage::UsernameResult { username, success } => {
                    if success {
                        self.last_username_failure = None;
                        self.username = Some(username);
                        self.mode = ViewMode::Lobby;
                    } else {
                        self.last_username_failure = Some(username);
                    }
                }
                WindowMessage::TransferToGame => {
                    let mut mut_board = self.known_board.borrow_mut();
                    for x in 0..7 {
                        for y in 0..6 {
                            mut_board[x][y] = None;
                        }
                    }
                    self.my_turn = false;
                    self.opponent = None;
                    self.mode = ViewMode::Game;
                }
                WindowMessage::ExitToLobby => {
                    self.mode = ViewMode::Lobby;
                }
                WindowMessage::PlacePieceInGame { me, column } => {
                    let mut_pos = self.known_board.borrow()[column as usize]
                        .iter()
                        .position(|x| x.is_none())
                        .unwrap();
                    self.known_board.borrow_mut()[column as usize][mut_pos] = Some(me);
                    self.my_turn = !self.my_turn;
                    self.game_draw_handler.drawing_area().queue_draw();
                }
                WindowMessage::WinGame => {
                    self.mode = ViewMode::Lobby;
                }
                WindowMessage::LoseGame => {
                    self.mode = ViewMode::Lobby;
                }
                WindowMessage::NotifyOpponentJoin {
                    i_go_first,
                    username,
                } => {
                    self.my_turn = i_go_first;
                    self.opponent = Some(username);
                }
            },
        }
    }
}

use std::{collections::VecDeque, io::{Bytes, Read, Write}, net::{TcpListener, TcpStream}, sync::{Arc, RwLock}};

use chess::{Chess, Color as ChessColor, Move, PieceType, Position, Status, ValidationResult};
use chess_networking::{Ack, GameState, PromotionPiece, Start};
use ggez::{
    conf::WindowMode, event::{self, MouseButton}, glam::*, graphics::{self, Canvas, Color, DrawParam, Drawable, Image, ImageFormat, Mesh, Rect, Text, TextFragment}, input::keyboard::KeyCode, Context, GameResult
};

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 800.0;
const PIECE_TEX_SIZE: f32 = 1024.0;

fn get_board_coordinate(x: f32, y: f32, sc_width: f32, sc_height: f32) -> Option<(u8, u8)> {
    let sq_size = WIDTH / 8.0;
    let x = (WIDTH - sc_width) / 2. + x;
    let y = (HEIGHT - sc_height) / 2. + y;
    if x < 0.0 || y < 0.0 {
        return None;
    }
    let x = (x / sq_size) as u8;
    let y = (y / sq_size) as u8;
    if x >= 8 || y >= 8 {
        return None;
    }
    Some((x, y))
}

#[derive(Debug, Clone)]
enum GameType {
    Local,
    Host(String),
    Client(String),
}

#[derive(Debug, Clone)]
struct Player {
    color: ChessColor,
    name: Option<String>,
    local: bool,
}

struct Players {
    black: Player,
    white: Player,
}

impl Players {
    fn get_player(&self, color: ChessColor) -> &Player {
        if color == ChessColor::White {
            &self.white
        } else {
            &self.black
        }
    }
}

enum NetworkType {
    Host {
        listener: TcpListener,
        stream: TcpStream,
    },
    Client(TcpStream),
}

#[derive(Debug, Clone)]
enum PacketType {
    Start(Start),
    Move(chess_networking::Move),
    Ack(Ack),
}

impl TryFrom<&[u8]> for PacketType {
    type Error = ();
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if let Ok(start) = Start::try_from(data) {
            return Ok(Self::Start(start));
        }
        if let Ok(mv) = chess_networking::Move::try_from(data) {
            return Ok(Self::Move(mv));
        }
        if let Ok(ack) = Ack::try_from(data) {
            return Ok(Self::Ack(ack));
        }
        Err(())
    }
}

impl TryFrom<PacketType> for Vec<u8> {
    type Error = rmp_serde::encode::Error;
    fn try_from(packet: PacketType) -> Result<Self, Self::Error> {
        match packet {
            PacketType::Start(start) => {
                Vec::try_from(start)
            }
            PacketType::Move(mv) => {
                Vec::try_from(mv)
            }
            PacketType::Ack(ack) => {
                Vec::try_from(ack)
            }
        }
    }
}

struct Network {
    ty: NetworkType,
    cache: Arc<RwLock<VecDeque<PacketType>>>,
    thread_handle: std::thread::JoinHandle<()>,
}

impl Network {
    fn new_host(host: &str) -> Self {
        let listener = TcpListener::bind(host).unwrap();
        let (stream, _) = listener.accept().unwrap();
        let cache = Arc::new(RwLock::new(VecDeque::new()));
        let cache_clone = cache.clone();
        let thread_handle = Self::spawn_thread(stream.try_clone().unwrap(), cache_clone);
        Self {
            ty: NetworkType::Host {
                listener,
                stream,
            },
            cache,
            thread_handle,
        }
    }

    fn spawn_thread(stream: TcpStream, cache: Arc<RwLock<VecDeque<PacketType>>>) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut stream = stream;
            loop {
                let mut data = [0u8; 1024];
                if let Ok(size) = stream.read(&mut data) {
                    let packet = PacketType::try_from(&data[..size]).unwrap();
                    let mut cache = cache.write().unwrap();
                    cache.push_back(packet);
                }
            }
        })
    }

    fn new_client(host: &str) -> Self {
        let stream = TcpStream::connect(host).unwrap();
        let cache = Arc::new(RwLock::new(VecDeque::new()));
        let cache_clone = cache.clone();
        let thread_handle = Self::spawn_thread(stream.try_clone().unwrap(), cache_clone);
        Self {
            ty: NetworkType::Client(stream),
            cache,
            thread_handle,
        }
    }

    fn send(&mut self, data: &[u8]) {
        match self.ty {
            NetworkType::Host { ref mut stream, .. } => {
                stream.write(data).unwrap();
            }
            NetworkType::Client(ref mut stream) => {
                stream.write(data).unwrap();
            }
        }
    }

    fn init(&mut self) -> Players {
        match self.ty {
            NetworkType::Host { .. } => {
                let start = if let PacketType::Start(start) = self.get_packet_blocking() {
                    start
                } else {
                    panic!("Failed to receive start packet");
                };
                let start_packet = PacketType::Start(Start {
                    name: None,
                    is_white: true,
                    fen: None,
                    time: None,
                    inc: None,
                });
                self.send_packet(start_packet);
                let main = Player {
                    color: ChessColor::White,
                    name: None,
                    local: true,
                };
                let opp = Player {
                    color: ChessColor::Black,
                    name: start.name,
                    local: false,
                };
                Players {
                    white: main,
                    black: opp,
                }
            }
            NetworkType::Client(_) => {
                let start = Start {
                    name: None,
                    is_white: true,
                    fen: None,
                    time: None,
                    inc: None,
                };
                let start_packet = PacketType::Start(start);
                self.send_packet(start_packet);
                let start_packet = self.get_packet_blocking();
                if let PacketType::Start(start) = start_packet {
                    if start.is_white {
                        let main = Player {
                            color: ChessColor::Black,
                            name: start.name,
                            local: true,
                        };
                        let opp = Player {
                            color: ChessColor::White,
                            name: None,
                            local: false,
                        };
                        return Players {
                            white: opp,
                            black: main,
                        };
                    } else {
                        let main = Player {
                            color: ChessColor::White,
                            name: start.name,
                            local: true,
                        };
                        let opp = Player {
                            color: ChessColor::Black,
                            name: None,
                            local: false,
                        };
                        return Players {
                            white: main,
                            black: opp,
                        };
                    }
                } else {
                    panic!("Failed to receive start packet");
                }
            }
        }
    }

    fn get_packet(&mut self) -> Option<PacketType> {
        let mut cache = self.cache.write().unwrap();
        let packet = cache.pop_front();
        packet
    }

    fn get_packet_blocking(&mut self) -> PacketType {
        loop {
            if let Some(packet) = self.get_packet() {
                return packet;
            }
        }
    }

    fn send_packet(&mut self, packet: PacketType) {
        let data : Vec<u8> = Vec::try_from(packet).unwrap();
        self.send(&data);
    }


    fn close(self) {
        match self.ty {
            NetworkType::Host { listener, stream } => {
                drop(listener);
                drop(stream);
            }
            NetworkType::Client(stream) => {
                drop(stream);
            }
        }
        self.thread_handle.join().unwrap();
    }
}

struct PlayerHandler {
    game_type: GameType,
    players: Players,
    network: Option<Network>,
}

impl PlayerHandler {
    fn new(game_type: GameType) -> Self {
        let mut network = match &game_type {
            GameType::Host(host) => Some(Network::new_host(host)),
            GameType::Client(host) => Some(Network::new_client(host)),
            _ => None,
        };
        let players = match game_type {
            GameType::Local => Players {
                white: Player {
                    color: ChessColor::White,
                    name: None,
                    local: true,
                },
                black: Player {
                    color: ChessColor::Black,
                    name: None,
                    local: true,
                },
            },
            _ => {
                let network = network.as_mut().unwrap();
                network.init()
            }
        };
        Self {
            game_type,
            players,
            network,
        }
    }

    fn can_move(&self, color: ChessColor) -> bool {
        if let GameType::Local = self.game_type {
            return true;
        }
        let player = self.players.get_player(color);
        player.local
    }

    fn both_local(&self) -> bool {
        self.players.black.local && self.players.white.local
    }

    fn one_local(&self) -> Option<ChessColor> {
        if self.players.black.local {
            Some(ChessColor::Black)
        } else if self.players.white.local {
            Some(ChessColor::White)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
enum MoveKind {
    Builtin(Move),
    Network(chess_networking::Move),
}

impl MoveKind {
    fn to(&self) -> Position {
        match self {
            MoveKind::Builtin(mv) => mv.to,
            MoveKind::Network(mv) => Position { x: mv.to.0 as usize, y: mv.to.1 as usize },
        }
    }

    fn from(&self) -> Position {
        match self {
            MoveKind::Builtin(mv) => mv.from,
            MoveKind::Network(mv) => Position { x: mv.from.0 as usize, y: mv.from.1 as usize },
        }
    }

    fn promotion(&self) -> PieceType {
        match self {
            // Always promote to queen, had to change this since networking would
            // require a super dumb hackfix to work.
            // TODO: If time permits, think of something smart to do here
            MoveKind::Builtin(_) => PieceType::Queen,
            MoveKind::Network(mv) => {
                if let Some(promotion) = &mv.promotion {
                    match promotion {
                        PromotionPiece::Queen => PieceType::Queen,
                        PromotionPiece::Rook => PieceType::Rook,
                        PromotionPiece::Bishop => PieceType::Bishop,
                        PromotionPiece::Knight => PieceType::Knight,
                    }
                } else {
                    PieceType::Queen
                }
            }
        }
    }
}

enum Phase {
    Move,
    Validate(MoveKind),
    End(Status)
}

struct MainState {
    board: Chess,
    board_texture: Image,
    piece_textures: [Image; 12],
    move_to_dot: Mesh,
    current_moves: Option<[Vec<Move>; 64]>,
    selected_square: Option<(u8, u8)>,
    text_prompt: Option<Text>,
    player_handler: PlayerHandler,
    phase: Phase,
}

impl MainState {
    fn new(ctx: &mut Context, game_type: GameType) -> GameResult<MainState> {
        let board = Chess::new();
        let format = ctx.gfx.surface_format();
        let mut pixels = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
        let sq_size = WIDTH / 8.0;
        for y in 0..8 {
            for _ in 0..sq_size as usize {
                for x in 0..8 {
                    let color = if (x + y) % 2 == 0 {
                        Color::from_rgb(255, 206, 158)
                    } else {
                        Color::from_rgb(209, 139, 71)
                    };
                    let color_slice = color.to_rgba();
                    let color_slice = [color_slice.0, color_slice.1, color_slice.2, color_slice.3];
                    for _ in 0..sq_size as usize {
                        pixels.extend_from_slice(&color_slice);
                    }
                }
            }
        }
        let board_texture = Image::from_pixels(
            ctx,
            &pixels,
            ImageFormat::Rgba8Unorm,
            WIDTH as u32,
            HEIGHT as u32,
        );
        let piece_textures = [
            Image::from_bytes(ctx, include_bytes!("../assets/k_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/q_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/r_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/b_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/n_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/p_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/k_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/q_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/r_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/b_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/n_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/p_b.png"))?,
        ];

        let move_to_dot = Mesh::new_circle(ctx, graphics::DrawMode::fill(), Vec2::new(0., 0.), 20., 2., Color::from_rgba(255, 255, 255, 128))?;

        Ok(MainState {
            board,
            board_texture,
            move_to_dot,
            piece_textures,
            current_moves: None,
            selected_square: None,
            text_prompt: None,
            player_handler: PlayerHandler::new(game_type),
            phase: Phase::Move,
        })
    }

    fn get_moves(&self) -> Option<&Vec<Move>> {
        let selected_square = self.selected_square?;
        let moves = self.current_moves.as_ref().unwrap();
        Some(&moves[selected_square.0 as usize + selected_square.1 as usize * 8])
    }

    fn draw_pieces(&self, canvas: &mut Canvas) -> GameResult {
        let reverse = self.should_reverse();
        let pieces = &self.board.board;
        for piece in pieces {
            let piece = if let Some(piece) = piece {
                piece
            } else {
                continue;
            };
            let texture_idx = piece.piece_type as usize + if piece.color == ChessColor::White { 0 } else { 6 };
            let texture = &self.piece_textures[texture_idx];
            let x = piece.position.x as f32 * WIDTH / 8.0;
            let y = piece.position.y as f32 * HEIGHT / 8.0;
            let mut dest = Vec2::new(x, y);
            if reverse {
                dest.y = 700. - dest.y;
            }
            const SCALE: f32 = 100.0 / PIECE_TEX_SIZE;
            let draw_params = DrawParam::new()
                .dest(dest)
                .scale(Vec2::new(SCALE, SCALE));
            canvas.draw(texture, draw_params);
        }
        Ok(())
    }
    

    fn draw_selected(&self, canvas: &mut Canvas) -> GameResult {
        let reverse = self.should_reverse();
        let moves = self.get_moves();
        if moves.is_none() {
            return Ok(())
        }
        let moves = moves.unwrap();
        for mv in moves {
            let x = mv.to.x as f32 * WIDTH / 8.0;
            let y = mv.to.y as f32 * HEIGHT / 8.0;
            let mut dest = Vec2::new(50. + x,  y);
            if reverse {
                dest.y = 700. - dest.y;
            }
            dest.y += 50.;
            canvas.draw(&self.move_to_dot, DrawParam::new().dest(dest));
        }
        Ok(())
    }

    fn draw_prompt(&self, ctx: &mut Context, canvas: &mut Canvas) -> GameResult {
        if let Some(text) = &self.text_prompt {
            let dims = text.dimensions(ctx).unwrap();
            let width = dims.w;
            let height = dims.h;
            let x = WIDTH / 2. - width / 2.;
            let y = HEIGHT / 2. - height / 2.;
            let dest = Vec2::new(x, y);
            canvas.draw(text, DrawParam::new().dest(dest));
        }
        Ok(())
    }

    fn client_move(&mut self, ctx: &mut Context) -> GameResult<()> {
        if !self.player_handler.can_move(self.board.turn) {
            return Ok(());
        }
        if !ctx.mouse.button_just_pressed(MouseButton::Left) {
            return Ok(());
        }
        let pos = ctx.mouse.position();
        let (x, y) = (pos.x, pos.y);
        let (sc_width, sc_height) = ctx.gfx.size();
        let reverse = self.should_reverse();
        let board_coords = get_board_coordinate(x, y, sc_width, sc_height);
        let mut clicked = if let Some(coords) = board_coords {
            coords
        } else {
            return Ok(());
        };
        if reverse {
            clicked.1 = 7 - clicked.1;
        }
        if let Some(current) = self.selected_square {
            if current == clicked {
                self.selected_square = None;
                return Ok(());
            }
            let mv = {
                let moves = self.get_moves();
                if moves.is_none() {
                    return Ok(());
                }
                let moves = moves.unwrap();
                moves.iter().find(|mv| (mv.to.x as u8, mv.to.y as u8) == clicked)
            };
            if let Some(mv) = mv {
                let mv = mv.clone();
                self.phase = Phase::Validate(MoveKind::Builtin(mv));
            } else {
                self.selected_square = Some(clicked);
            }
        } else {
            self.selected_square = Some(clicked);
        }
        Ok(())
    }

    fn network_move(&mut self) -> GameResult<()> {
        if let Some(network) = &mut self.player_handler.network {
            if let Some(packet) = network.get_packet() {
                match packet {
                    PacketType::Move(mv) => {
                        self.phase = Phase::Validate(MoveKind::Network(mv));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn client_validate(&mut self, mv: MoveKind) -> GameResult<()> {
        let current_turn = self.board.turn;
        let result = self.board.move_piece(mv.from(), mv.to());
        match result {
            ValidationResult::Valid(mut status) => {
                if self.board.status == Status::AwaitingPromotion {
                    status = self.board.promote_piece(mv.promotion()).unwrap();
                }
                let end_state = match status {
                    Status::Checkmate(_) => Some(GameState::CheckMate),
                    Status::Draw(_) => Some(GameState::Draw),
                    _ => None,
                };
                if end_state.is_some() {
                    self.phase = Phase::End(status);
                } else {
                    self.phase = Phase::Move;
                }
                self.selected_square = None;
                self.current_moves = None;
                let one_local = self.player_handler.one_local();
                if let Some(network) = &mut self.player_handler.network {
                    if one_local == Some(current_turn) {
                        let packet = PacketType::Move(chess_networking::Move {
                            from: (mv.from().x as u8, mv.from().y as u8),
                            to: (mv.to().x as u8, mv.to().y as u8),
                            promotion: Some(chess_networking::PromotionPiece::Queen),
                            forfeit: false,
                            offer_draw: false,
                        });
                        network.send_packet(packet);
                    } else {
                        let ack = Ack {
                            ok: true,
                            end_state,
                        };
                        let packet = PacketType::Ack(ack);
                        network.send_packet(packet);
                    }
                }
            }
            _ => {
                if let Some(network) = &mut self.player_handler.network {
                    let ack = Ack {
                        ok: false,
                        end_state: None,
                    };
                    let packet = PacketType::Ack(ack);
                    network.send_packet(packet);
                }
                self.phase = Phase::Move;
                self.selected_square = None;
            }
        }
        Ok(())
    }

    fn should_reverse(&self) -> bool {
        (self.board.turn == ChessColor::White
            && self.player_handler.both_local())
        || self.player_handler.one_local().is_some_and(|color| color == ChessColor::White)
    }
}

impl event::EventHandler<ggez::GameError> for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        match &self.phase {
            Phase::Move => {
                if self.player_handler.both_local() {
                    self.client_move(ctx)?;
                } else if self.player_handler.one_local() == Some(self.board.turn) {
                    self.client_move(ctx)?;
                } else {
                    self.network_move()?;
                }
            }
            Phase::Validate(mv) => {
                self.client_validate(mv.clone())?;
            }
            Phase::End(status) => {
                match status {
                    Status::Checkmate(_) => {
                        let text = if self.board.turn == ChessColor::White {
                            "Black wins"
                        } else {
                            "White wins"
                        };
                        let text = Text::new(TextFragment::new(text).color(Color::from_rgb(255, 0, 0)).scale(64.));
                        self.text_prompt = Some(text);
                    }
                    Status::Draw(draw_type) => {
                        let text = match draw_type {
                            chess::DrawType::Stalemate => "Stalemate",
                            chess::DrawType::ThreefoldRepetition => "Threefold Repetition",
                            chess::DrawType::FiftyMoveRule => "Fifty Move Rule",
                        };
                        let text = Text::new(TextFragment::new(text).color(Color::from_rgb(255, 0, 0)).scale(64.));
                        self.text_prompt = Some(text);
                    }
                    _ => {}
                }
                if ctx.keyboard.is_key_just_pressed(KeyCode::Space) {
                    self.board = Chess::new();
                    self.current_moves = None;
                    self.text_prompt = None;
                    self.phase = Phase::Move;
                    if let Some(network) = &mut self.player_handler.network {
                        network.init();
                    }
                }
            }
        }
        
        if self.current_moves.is_none() {
            self.current_moves = Some(self.board.generate_valid_moves());
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas =
            graphics::Canvas::from_frame(ctx, graphics::Color::from([0.1, 0.2, 0.3, 1.0]));
        let reverse = self.should_reverse();
        let offset = if reverse { 800. } else { 0. };
        let scale = Vec2::new(1.0, if reverse {-1.0} else {1.0});
        let dest = Vec2::new(0., offset);
        let draw_params = DrawParam::new()
            .scale(scale)
            .dest(dest);
        canvas.draw(&self.board_texture, draw_params);

        self.draw_pieces(&mut canvas)?;
        self.draw_selected(&mut canvas)?;
        self.draw_prompt(ctx, &mut canvas)?;



        canvas.finish(ctx)?;

        Ok(())
    }

}

pub fn main() -> GameResult {
    let cli_flags = std::env::args().collect::<Vec<_>>();
    let game_type = if cli_flags.len() == 1 {
        GameType::Local
    } else if cli_flags.len() == 2 {
        if cli_flags[1] == "--host" {
            GameType::Host("localhost:3000".to_owned())
        } else if cli_flags[1] == "--client" {
            GameType::Client("localhost:3000".to_owned())
        } else {
            panic!("Invalid flag");
        }
    } else if cli_flags.len() == 3 {
        if cli_flags[1] == "--host" {
            GameType::Host(cli_flags[2].to_owned())
        } else if cli_flags[1] == "--client" {
            GameType::Client(cli_flags[2].to_owned())
        } else {
            panic!("Invalid flag");
        }
    } else {
        panic!("Invalid flag");
    };

    let title = match game_type {
        GameType::Local => "Chess",
        GameType::Host(_) => "Chess Host",
        GameType::Client(_) => "Chess Client",
    };

    let cb = ggez::ContextBuilder::new("Chess GUI", "Dexter WS").window_mode(
        WindowMode::default()
            .dimensions(WIDTH, HEIGHT)
            .max_dimensions(WIDTH, HEIGHT)
            .resizable(false)
    ).window_setup(ggez::conf::WindowSetup::default().title(title));
    let (mut ctx, event_loop) = cb.build()?;

    let state = MainState::new(&mut ctx, game_type)?;
    event::run(ctx, event_loop, state)
}

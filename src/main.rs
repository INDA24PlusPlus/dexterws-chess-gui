use chess::{Chess, Color as ChessColor, Move, PieceType, Status, ValidationResult};
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

// Since board is represented from top to down
// it makes sense to reverse when white is playing
// to display their pieces at the bottom
fn should_reverse(s2m: ChessColor) -> bool {
    s2m as usize == 1
}


struct MainState {
    board: Chess,
    board_texture: Image,
    piece_textures: [Image; 12],
    move_to_dot: Mesh,
    current_moves: Option<[Vec<Move>; 64]>,
    selected_square: Option<(u8, u8)>,
    text_prompt: Option<Text>,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let board = Chess::new();
        let format = ctx.gfx.surface_format();
        println!("Surface format: {:?}", format);
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
        })
    }

    fn get_moves(&self) -> Option<&Vec<Move>> {
        let selected_square = self.selected_square?;
        let moves = self.current_moves.as_ref().unwrap();
        Some(&moves[selected_square.0 as usize + selected_square.1 as usize * 8])
    }

    fn draw_pieces(&self, canvas: &mut Canvas) -> GameResult {
        let reverse = should_reverse(self.board.turn);
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
        let reverse = should_reverse(self.board.turn);
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
            println!("ddd");
            let dims = text.dimensions(ctx).unwrap();
            let width = dims.w;
            let height = dims.h;
            println!("{:?}", dims);
            let x = WIDTH / 2. - width / 2.;
            let y = HEIGHT / 2. - height / 2.;
            let dest = Vec2::new(x, y);
            canvas.draw(text, DrawParam::new().dest(dest));
        }
        Ok(())
    }

    fn click_square(&mut self, clicked: (u8, u8)) -> GameResult<Option<Status>> {
        if let Some(current) = self.selected_square {
            if current == clicked {
                self.selected_square = None;
                return Ok(None);
            }
            let moves = self.get_moves();
            if moves.is_none() {
                return Ok(None);
            }
            let moves = moves.unwrap();
            let mv = moves.iter().find(|mv| (mv.to.x as u8, mv.to.y as u8) == clicked);
            if let Some(mv) = mv {
                let res = self.board.move_piece(mv.from, mv.to);
                println!("{:?}", res);
                if let ValidationResult::Valid(status) = res {
                }

                self.current_moves = None;
                self.selected_square = None;
            } else {
                self.selected_square = Some(clicked);
            }

        } else {
            self.selected_square = Some(clicked);
        }
        Ok(None)

    }
}

impl event::EventHandler<ggez::GameError> for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        let status = self.board.status;
        match status {
            Status::Checkmate(color) => {
                let text = if color == ChessColor::White {
                    "Black wins"
                } else {
                    "White wins"
                };
                let text = Text::new(TextFragment::new(text).color(Color::from_rgb(255, 0, 0)).scale(64.));
                self.text_prompt = Some(text);
                if ctx.keyboard.is_key_just_pressed(KeyCode::Space) {
                    self.board = Chess::new();
                    self.current_moves = None;
                    self.text_prompt = None;
                }
            }
            Status::Draw(draw_type) => {
                let text = match draw_type {
                    chess::DrawType::Stalemate => "Stalemate",
                    chess::DrawType::ThreefoldRepetition => "Threefold Repetition",
                    chess::DrawType::FiftyMoveRule => "Fifty Move Rule",
                };
                let text = Text::new(TextFragment::new(text).color(Color::from_rgb(255, 0, 0)).scale(64.));
                self.text_prompt = Some(text);
                if ctx.keyboard.is_key_just_pressed(KeyCode::Space) {
                    self.board = Chess::new();
                    self.current_moves = None;
                    self.text_prompt = None;
                }
            }
            Status::AwaitingPromotion => {
                let text = Text::new(TextFragment::new("Press Q, R, B, N to choose promotion").color(Color::from_rgb(255, 0, 0)).scale(32.));
                self.text_prompt = Some(text);
                let piece = if ctx.keyboard.is_key_just_pressed(KeyCode::Q) {
                    Some(PieceType::Queen)
                } else if ctx.keyboard.is_key_just_pressed(KeyCode::R) {
                    Some(PieceType::Rook)
                } else if ctx.keyboard.is_key_just_pressed(KeyCode::B) {
                    Some(PieceType::Bishop)
                } else if ctx.keyboard.is_key_just_pressed(KeyCode::N) {
                    Some(PieceType::Knight)
                } else {
                    None
                };
                if let Some(piece) = piece {
                    self.board.promote_piece(piece);
                    self.current_moves = None;
                    self.text_prompt = None;
                }
            }
            _ => {}
        }
        
        if self.current_moves.is_none() {
            self.current_moves = Some(self.board.generate_valid_moves());
        }
        if !ctx.mouse.button_just_pressed(MouseButton::Left) {
            return Ok(());
        }
        let pos = ctx.mouse.position();
        let (x, y) = (pos.x, pos.y);
        let (sc_width, sc_height) = ctx.gfx.size();
        let reverse = should_reverse(self.board.turn);
        let board_coords = get_board_coordinate(x, y, sc_width, sc_height);
        let mut board_coords = if let Some(coords) = board_coords {
            coords
        } else {
            return Ok(());
        };
        if reverse {
            board_coords.1 = 7 - board_coords.1;
        }
        self.click_square(board_coords)?;
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas =
            graphics::Canvas::from_frame(ctx, graphics::Color::from([0.1, 0.2, 0.3, 1.0]));
        let reverse = should_reverse(self.board.turn);
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
    let cb = ggez::ContextBuilder::new("Chess GUI", "Dexter WS").window_mode(
        WindowMode::default()
            .dimensions(WIDTH, HEIGHT)
            .max_dimensions(WIDTH, HEIGHT)
            .resizable(false)
    );
    let (mut ctx, event_loop) = cb.build()?;
    let state = MainState::new(&mut ctx)?;
    event::run(ctx, event_loop, state)
}

use chess::{Chess, Move, Piece};
use ggez::{
    conf::WindowMode,
    event,
    glam::*,
    graphics::{self, Color, DrawParam, Drawable, Image, ImageFormat},
    Context, GameResult,
};

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 800.0;

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


struct MainState {
    board: Chess,
    board_texture: Image,
    piece_textures: [Image; 12],
    current_moves: Option<[Vec<Move>; 64]>,
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
            Image::from_bytes(ctx, include_bytes!("../assets/n_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/b_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/p_w.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/k_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/q_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/r_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/n_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/b_b.png"))?,
            Image::from_bytes(ctx, include_bytes!("../assets/p_b.png"))?,
        ];

        Ok(MainState {
            board,
            board_texture,
            piece_textures,
            current_moves: None,
        })
    }
}

impl event::EventHandler<ggez::GameError> for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        let pos = ctx.mouse.position();
        let (x, y) = (pos.x, pos.y);
        let dims = ctx.gfx.size();
        let (sc_width, sc_height) = (dims.0 as f32, dims.1 as f32);
        let board_coords = get_board_coordinate(x, y, sc_width, sc_height);
        if let Some((x, y)) = board_coords {
            println!("Board coords: ({}, {})", x, y);
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas =
            graphics::Canvas::from_frame(ctx, graphics::Color::from([0.1, 0.2, 0.3, 1.0]));
        let dims = ctx.gfx.drawable_size();
        let (sc_width, sc_height) = (dims.0 as f32, dims.1 as f32);
        println!("Screen dimensions: ({}, {})", sc_width, sc_height);
        let dest = Vec2::new((sc_width - WIDTH) / 2., (sc_height - HEIGHT) / 2.);
        let draw_params = DrawParam::new()
            .dest(dest);
        canvas.draw(&self.board_texture, draw_params);

        let pieces = self.board.board;

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

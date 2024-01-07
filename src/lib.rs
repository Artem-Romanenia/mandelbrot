pub use plot_point_mod::PlotPoint;

use serde::Deserialize;
use serde::Serialize;
use web_sys::js_sys::Math;

#[derive(Clone)]
pub struct Complex {
    pub re: i64,
    pub im: i64,
}

impl Complex {
    fn new(re: i64, im: i64) -> Self {
        Self { re, im }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Ctx {
    pub win_height: usize,
    pub win_width: usize,

    pub center_x: i128,
    pub center_y: i128,
    pub horizontal_span: i128,
    pub vertical_span: i128,

    pub x_min: i128,
    pub x_max: i128,
    pub y_min: i128,
    pub y_max: i128,

    pub max_iters: usize,

    pub min_value: i128,
    pub max_value: i128,

    pub chunk_size: usize,
    pub rows_processed: usize,
    pub total_rows_processed: usize,

    pub offset: usize,

    pub brightness: f64,

    pub needs_recalc: bool,
}

impl Default for Ctx {
    fn default() -> Self {
        Self {
            win_height: Default::default(),
            win_width: Default::default(),
            center_x: Default::default(),
            center_y: Default::default(),
            horizontal_span: Default::default(),
            vertical_span: Default::default(),
            x_min: Default::default(),
            x_max: Default::default(),
            y_min: Default::default(),
            y_max: Default::default(),
            max_iters: 500,
            min_value: i128::max_value(),
            max_value: Default::default(),
            chunk_size: 128,
            rows_processed: Default::default(),
            total_rows_processed: Default::default(),
            offset: Default::default(),
            brightness: 0.6,
            needs_recalc: true,
        }
    }
}

impl Ctx {
    pub fn define_bounds_from(&mut self, x: usize, y: usize, w: usize, h: usize) {
        let cx = self.get_x(x + w / 2);
        let cy = self.get_y(y + h / 2);
        let hs = self.horizontal_span * w as i128 / self.win_width as i128;

        self.define_bounds(cx, cy, hs as i64);
    }

    pub fn define_bounds(&mut self, center_x: i64, center_y: i64, horizontal_span: i64) {
        self.center_x = center_x as i128;
        self.center_y = center_y as i128;
        self.horizontal_span = horizontal_span as i128;
        self.vertical_span =
            (self.horizontal_span * self.win_height as i128) / self.win_width as i128;

        self.x_min = self.center_x - self.horizontal_span / 2;
        self.x_max = self.x_min + self.horizontal_span;
        self.y_min = self.center_y - self.vertical_span / 2;
        self.y_max = self.y_min + self.vertical_span;
    }

    pub fn apply_changes(&mut self, other: &Ctx) {
        self.min_value = other.min_value;
        self.max_value = other.max_value;
        self.brightness = other.brightness;
    }

    pub fn reset_min_max(&mut self) {
        self.needs_recalc = true;
        self.min_value = i128::max_value();
        self.max_value = Default::default();
    }

    pub fn get_x(&self, x: usize) -> i64 {
        let x_percent = (x as f64 * 10000.0 / self.win_width as f64) as i128;
        (self.x_min + (self.x_max - self.x_min) * x_percent / 10000) as i64
    }

    pub fn get_y(&self, y: usize) -> i64 {
        let y_percent = (y as f64 * 10000.0 / self.win_height as f64) as i128;
        (self.y_max - (self.y_max - self.y_min) * y_percent / 10000) as i64
    }

    pub fn revert_y(&self, y: i64) -> (usize, Symmetry) {
        let p =
            (self.y_max - y as i128) * (self.win_height as i128 - 1) / (self.y_max - self.y_min);
        let rem = p % 1;
        let p1 = p - rem;
        let third = 1 / 3;

        (
            p1 as usize,
            if p < p1 + third {
                Symmetry::OverTwo
            } else if p > p1 + third && p < p1 + third * 2 {
                Symmetry::OverOne
            } else {
                Symmetry::Exact
            },
        )
    }

    pub fn get_coords(&self, x: usize, y: usize) -> (i64, i64) {
        (self.get_x(x), self.get_y(y))
    }
}

#[derive(Debug)]
pub enum Symmetry {
    Exact,
    OverOne,
    OverTwo,
}

mod plot_point_mod {
    #[derive(Default, Clone, Copy)]
    pub struct PlotPoint {
        val: i128,
        i: usize,

        calc_val: Option<i128>,

        pub filled: bool,
    }

    impl PlotPoint {
        pub fn new_from<T: FnMut(&mut PlotPoint)>(other: &PlotPoint, mut upd: T) -> PlotPoint {
            let mut new = other.clone();
            upd(&mut new);

            new
        }

        pub fn calculate(&mut self, val: i128, i: usize) {
            if self.calc_val.is_some() {
                panic!("Attempt to recalculate plot point.")
            }
            self.val = val;
            self.i = i;
            // self.calc_val = Some((i as f64 - val.log10().log2()).log10())
            self.calc_val = Some(i as i128);
        }

        pub fn processed(&self) -> bool {
            self.calc_val.is_some()
        }

        pub fn stable(&self) -> bool {
            self.calc_val.is_some() && self.val == 0
        }

        pub fn calculated_value(&self) -> i128 {
            self.calc_val
                .expect("Method should not be called before the point is processed.")
        }

        pub fn reset(&mut self) {
            self.calc_val = None;
            self.filled = false;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum TraceDirection {
    Right,
    RightDown,
    Down,
    LeftDown,
    Left,
    LeftUp,
    Up,
    RightUp,
}

impl TraceDirection {
    pub fn get_index(&self) -> usize {
        match self {
            TraceDirection::Right => 0,
            TraceDirection::RightDown => 1,
            TraceDirection::Down => 2,
            TraceDirection::LeftDown => 3,
            TraceDirection::Left => 4,
            TraceDirection::LeftUp => 5,
            TraceDirection::Up => 6,
            TraceDirection::RightUp => 7,
        }
    }
}

pub fn near_border(plot: &[Vec<PlotPoint>], x: i16, y: usize) -> bool {
    if x < 0 {
        return false;
    }

    let point = plot[y][x as usize];

    point.stable()
}

pub fn process_plot_cpu(ctx: &mut Ctx, plot: &mut [Vec<PlotPoint>]) -> usize {
    let mut rows_processed = 0;
    let mut cur_y = ctx.rows_processed;
    while cur_y < (ctx.rows_processed + ctx.chunk_size).min(plot.len()) {
        let mut cur_x = 0;
        while cur_x < ctx.win_width {
            let (cx, cy) = ctx.get_coords(cur_x, cur_y + ctx.offset);

            let plot_point = &mut plot[cur_y][cur_x];

            if calculate_point(ctx, plot_point, cx, cy) {
                cur_x += 1;
                continue;
            }

            if plot_point.stable() {
                if near_border(plot, cur_x as i16 - 1, cur_y) {
                    cur_x = process_fast(ctx, plot, cur_x + 1, cur_y);
                } else {
                    trace_edge(ctx, plot, cur_x, cur_y);
                }
            }

            cur_x += 1;
        }

        cur_y += 1;
        rows_processed += 1;
    }

    rows_processed
}

fn process_fast(ctx: &mut Ctx, plot: &mut [Vec<PlotPoint>], x: usize, y: usize) -> usize {
    let mut cur_x = x;

    let max = Math::min(
        (ctx.win_width - ((ctx.win_width - 1) % 1) - 1) as f64,
        (ctx.win_width - 1) as f64,
    ) as usize;

    loop {
        let plot_point = &mut plot[y][cur_x];

        if plot_point.processed() && (!plot_point.stable() || (plot_point.stable() && cur_x == max))
        {
            return cur_x;
        } else if !plot_point.stable() {
            plot_point.calculate(0, ctx.max_iters);
            plot_point.filled = true;
        }

        cur_x += 1;
    }
}

fn trace_edge(ctx: &mut Ctx, plot: &mut [Vec<PlotPoint>], x: usize, y: usize) {
    let mut cur_x = x;
    let mut cur_y = y;
    let mut cur_dir = TraceDirection::Right;

    loop {
        (cur_x, cur_y, cur_dir) = process_neighbours(ctx, plot, cur_x, cur_y, cur_dir);

        if cur_x == x && cur_y == y {
            return;
        }
    }
}

fn process_neighbours(
    ctx: &mut Ctx,
    plot: &mut [Vec<PlotPoint>],
    x: usize,
    y: usize,
    direction: TraceDirection,
) -> (usize, usize, TraceDirection) {
    static NEIGHBOURS: [(i16, i16, TraceDirection); 8] = [
        (1, 0, TraceDirection::Right),
        (1, 1, TraceDirection::RightDown),
        (0, 1, TraceDirection::Down),
        (-1, 1, TraceDirection::LeftDown),
        (-1, 0, TraceDirection::Left),
        (-1, -1, TraceDirection::LeftUp),
        (0, -1, TraceDirection::Up),
        (1, -1, TraceDirection::RightUp),
    ];

    let mut next: (usize, usize, TraceDirection) = (x, y, TraceDirection::Right);

    for n in NEIGHBOURS
        .iter()
        .rev()
        .cycle()
        .skip(8 - direction.get_index() + 3)
        .take(8)
    {
        let nx = x as i16 + n.0;
        let ny = y as i16 + n.1;

        if nx >= 0 && nx < ctx.win_width as i16 {
            if ny >= 0 && ny < plot.len() as i16 {
                if process_point(ctx, plot, nx as usize, ny as usize) {
                    // web_sys::console::log_1(&"Yes".into());
                    next = (nx as usize, ny as usize, n.2)
                }
            }
        }
    }

    next
}

fn process_point(ctx: &mut Ctx, plot: &mut [Vec<PlotPoint>], x: usize, y: usize) -> bool {
    let plot_point = &mut plot[y][x];

    if plot_point.processed() {
        return plot_point.stable();
    }

    let (cx, cy) = ctx.get_coords(x, y + ctx.offset);
    calculate_point(ctx, plot_point, cx, cy);

    plot_point.stable()
}

fn calculate_point(ctx: &mut Ctx, plot_point: &mut PlotPoint, cx: i64, cy: i64) -> bool {
    if plot_point.processed() {
        return true;
    }

    mandelbrot_val_at_point(Complex::new(cx, cy), ctx.max_iters, plot_point);

    let calc_value = plot_point.calculated_value() as i128;

    if calc_value < ctx.min_value {
        ctx.min_value = calc_value;
    }

    if calc_value > ctx.max_value {
        ctx.max_value = calc_value;
    }

    false
}

fn mandelbrot_val_at_point(c: Complex, max_iters: usize, p: &mut PlotPoint) {
    // 4 * 2 ^ 60
    static THRESHOLD: i128 = 4 << 60;

    let mut z = c.clone();

    for i in 0..=max_iters {
        let re_sq = (z.re as i128).pow(2) >> 60;
        let im_sq = (z.im as i128).pow(2) >> 60;

        let n = re_sq + im_sq;
        if n > THRESHOLD {
            p.calculate(n, i);
            return;
        }
        let re_im: i128 = (z.re as i128 * z.im as i128) >> 59;
        z = Complex {
            re: re_sq as i64 - im_sq as i64 + c.re,
            im: re_im as i64 + c.im,
        };
    }

    p.calculate(0, max_iters);
}

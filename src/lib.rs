pub use plot_point_mod::PlotPoint;

use num::Complex;
use num::Float;
use serde::Deserialize;
use serde::Serialize;
use web_sys::js_sys::Math;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Ctx {
    pub win_height: usize,
    pub win_width: usize,

    pub center_x: f64,
    pub center_y: f64,
    pub horizontal_span: f64,
    pub vertical_span: f64,

    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,

    pub max_iters: usize,

    pub min_value: f64,
    pub max_value: f64,

    pub chunk_size: usize,
    pub rows_processed: usize,
    pub total_rows_processed: usize,

    pub offset: usize,
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
            min_value: f64::max_value(),
            max_value: Default::default(),
            chunk_size: 128,
            rows_processed: Default::default(),
            total_rows_processed: Default::default(),
            offset: Default::default(),
        }
    }
}

impl Ctx {
    pub fn define_bounds(&mut self, center_x: f64, center_y: f64, horizontal_span: f64) {
        self.center_x = center_x;
        self.center_y = center_y;
        self.horizontal_span = horizontal_span;
        self.vertical_span = (horizontal_span * self.win_height as f64) / self.win_width as f64;

        self.x_min = center_x - horizontal_span / 2.0;
        self.x_max = self.x_min + horizontal_span;
        self.y_min = center_y - self.vertical_span / 2.0;
        self.y_max = self.y_min + self.vertical_span;
    }

    pub fn get_x(&self, x: usize) -> f64 {
        let x_percent = x as f64 / self.win_width as f64;
        self.x_min + (self.x_max - self.x_min) * x_percent
    }

    pub fn get_y(&self, y: usize) -> f64 {
        let y_percent = y as f64 / self.win_height as f64;
        self.y_max - (self.y_max - self.y_min) * y_percent
    }

    pub fn revert_y(&self, y: f64) -> (usize, Symmetry) {
        let p = (self.y_max - y) * (self.win_height as f64 - 1.0) / (self.y_max - self.y_min);
        let rem = p % 1.0;
        let p1 = p - rem;
        let third = 1.0 / 3.0;

        (
            p1 as usize,
            if p < p1 + third {
                Symmetry::OverTwo
            } else if p > p1 + third && p < p1 + third * 2.0 {
                Symmetry::OverOne
            } else {
                Symmetry::Exact
            },
        )
    }

    pub fn get_coords(&self, x: usize, y: usize) -> (f64, f64) {
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
        val: f64,
        i: usize,

        calc_val: Option<f64>,

        pub filled: bool,
    }

    impl PlotPoint {
        pub fn new_from<T: FnMut(&mut PlotPoint)>(other: &PlotPoint, mut upd: T) -> PlotPoint {
            let mut new = other.clone();
            upd(&mut new);

            new
        }

        pub fn calculate(&mut self, val: f64, i: usize) {
            if self.calc_val.is_some() {
                panic!("Attempt to recalculate plot point.")
            }
            self.val = val;
            self.i = i;
            self.calc_val = Some((i as f64 - val.log10().log2()).log10())
        }

        pub fn processed(&self) -> bool {
            self.calc_val.is_some()
        }

        pub fn stable(&self) -> bool {
            self.calc_val.is_some() && self.val == 0.0
        }

        pub fn calculated_value(&self) -> f64 {
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
            plot_point.calculate(0.0, ctx.max_iters);
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

fn calculate_point(ctx: &mut Ctx, plot_point: &mut PlotPoint, cx: f64, cy: f64) -> bool {
    if plot_point.processed() {
        return true;
    }

    mandelbrot_val_at_point(Complex::new(cx, cy), ctx.max_iters, plot_point);

    let calc_value = plot_point.calculated_value();

    if calc_value < ctx.min_value {
        ctx.min_value = calc_value;
    }

    if calc_value > ctx.max_value {
        ctx.max_value = calc_value;
    }

    false
}

fn mandelbrot_val_at_point(c: Complex<f64>, max_iters: usize, p: &mut PlotPoint) {
    let mut z = c.clone();

    for i in 0..=max_iters {
        let n = z.re * z.re + z.im * z.im;
        if n > 4.0 {
            p.calculate(n, i);
            return;
        }
        z = z * z + c;
    }

    p.calculate(0.0, max_iters)
}

use mandelbrot_web::{process_plot_cpu, Ctx, PlotPoint, Symmetry};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::RwLock;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::js_sys::{Array, Uint8ClampedArray, WebAssembly};
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

static GRAD: RwLock<Option<colorgrad::Gradient>> = RwLock::new(None);

fn get_grad() -> colorgrad::Gradient {
    colorgrad::CustomGradient::new()
        .colors(&[
            colorgrad::Color::from_rgba8(0, 0, 0, 255),
            colorgrad::Color::from_rgba8(0, 0, 145, 255),
            colorgrad::Color::from_rgba8(145, 0, 255, 255),
            colorgrad::Color::from_rgba8(255, 0, 0, 255),
            colorgrad::Color::from_rgba8(255, 255, 0, 255),
            colorgrad::Color::from_rgba8(255, 255, 255, 255),
        ])
        .mode(colorgrad::BlendMode::Oklab)
        .interpolation(colorgrad::Interpolation::CatmullRom)
        .build()
        .unwrap()
}

fn main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"Plotter worker starting".into());

    let scope = DedicatedWorkerGlobalScope::from(JsValue::from(web_sys::js_sys::global()));

    let context_ref = Rc::new(RefCell::new(None));
    let plot_ref = Rc::new(RefCell::new(None));
    let rgb_data_ref = Rc::new(RefCell::new(None));

    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |msg: MessageEvent| {
        let context_ref = context_ref.clone();

        *GRAD.write().unwrap() = Some(get_grad());

        let data = Array::from(&msg.data());
        let first_param = data.get(0);
        let mut ctx: Ctx = if first_param.is_instance_of::<OffscreenCanvas>() {
            let canvas = first_param.dyn_into::<OffscreenCanvas>().unwrap();
            *context_ref.borrow_mut() = Some(canvas.get_context("2d").unwrap().unwrap());
            serde_wasm_bindgen::from_value(data.get(1)).unwrap()
        } else {
            serde_wasm_bindgen::from_value(first_param).unwrap()
        };

        web_sys::console::log_1(&format!("Plotting initiated. {:?}", ctx).into());

        let plot_initialized = plot_ref.borrow_mut().as_ref().is_some();

        if !plot_initialized {
            web_sys::console::log_1(&"Allocating plot data".into());
            *plot_ref.borrow_mut() = Some(vec![
                vec![PlotPoint::default(); ctx.win_width];
                ctx.win_height
            ]);
        } else if ctx.needs_recalc {
            plot_ref
                .borrow_mut()
                .as_mut()
                .unwrap()
                .iter_mut()
                .for_each(|row| row.iter_mut().for_each(|val| val.reset()))
        }

        if ctx.needs_recalc {
            *rgb_data_ref.borrow_mut() = Some(vec![0 as u8; 4 * ctx.win_width * ctx.win_height]);

            let _ = draw_plot_cpu(
                &mut ctx,
                context_ref.clone(),
                plot_ref.clone(),
                rgb_data_ref.clone(),
            );
        } else {
            draw_context2d(
                &mut ctx,
                context_ref.clone(),
                plot_ref.clone(),
                rgb_data_ref.clone(),
            )
        }
    });

    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    // The worker must send a message to indicate that it's ready to receive messages.
    let _ = scope.post_message(&Array::new().into());
}

fn draw_plot_cpu(
    ctx: &mut Ctx,
    context_ref: Rc<RefCell<Option<OffscreenCanvasRenderingContext2d>>>,
    plot_ref: Rc<RefCell<Option<Vec<Vec<PlotPoint>>>>>,
    data_ref: Rc<RefCell<Option<Vec<u8>>>>,
) {
    {
        let mut plot = plot_ref.borrow_mut();
        let plot: &mut Vec<Vec<PlotPoint>> = plot.as_mut().unwrap();

        let (y0, symmetry) = ctx.revert_y(0);
        let symmetry_shift = match symmetry {
            Symmetry::Exact => 0,
            Symmetry::OverOne => 1,
            Symmetry::OverTwo => 2,
        };
        let y_max = plot.len() - 1;
        let y1 = y0.min(y_max);
        let y2 = y1 + 1;
        let y3 = if y1 > 0 {
            (y1 * 2 + 1 - symmetry_shift).min(y_max)
        } else {
            0
        };
        let y4 = if y3 > 0 { y3 + 1 } else { 0 };

        let mut rows_processed: usize = 0;

        if ctx.rows_processed == 0 {
            rows_processed += y3 - y1;
        }

        if y1 > 0 {
            ctx.offset = 0;
            rows_processed += process_plot_cpu(ctx, &mut plot[0..=y1]);
        }

        if y3 > y2 {
            for (i, y) in (y2..=y3).enumerate() {
                for x in 0..ctx.win_width {
                    plot[y][x] = plot[y1 - i - symmetry_shift][x];
                }
            }
        }

        if y_max >= y4 {
            ctx.offset = y4;
            rows_processed += process_plot_cpu(ctx, &mut plot[y4..=y_max]);
        }

        ctx.rows_processed += ctx.chunk_size;
        ctx.total_rows_processed += rows_processed;
    }

    let mut ctx_cl = ctx.clone();
    draw_context2d(
        &mut ctx_cl,
        context_ref.clone(),
        plot_ref.clone(),
        data_ref.clone(),
    );

    let scope = DedicatedWorkerGlobalScope::from(JsValue::from(web_sys::js_sys::global()));

    if ctx.total_rows_processed < ctx.win_height {
        let mut ctx_cl = ctx.clone();

        let cl = Closure::<dyn FnMut()>::new(move || {
            draw_plot_cpu(
                &mut ctx_cl,
                context_ref.clone(),
                plot_ref.clone(),
                data_ref.clone(),
            );
        });

        scope
            .request_animation_frame(cl.as_ref().unchecked_ref())
            .unwrap();

        cl.forget();
    } else {
        let scope = DedicatedWorkerGlobalScope::from(JsValue::from(web_sys::js_sys::global()));

        let msg = Array::new();
        msg.push(&serde_wasm_bindgen::to_value(&ctx).unwrap());

        scope.post_message(&msg).unwrap();
    }
}

fn draw_context2d(
    ctx: &Ctx,
    context_ref: Rc<RefCell<Option<OffscreenCanvasRenderingContext2d>>>,
    plot_ref: Rc<RefCell<Option<Vec<Vec<PlotPoint>>>>>,
    data_ref: Rc<RefCell<Option<Vec<u8>>>>,
) {
    let mut context = context_ref.borrow_mut();
    let context: &mut OffscreenCanvasRenderingContext2d = context.as_mut().unwrap();
    let mut plot = plot_ref.borrow_mut();
    let plot: &mut Vec<Vec<PlotPoint>> = plot.as_mut().unwrap();
    let mut data = data_ref.borrow_mut();
    let data: &mut Vec<u8> = data.as_mut().unwrap();

    let grad = GRAD.read().unwrap();
    let grad = grad.as_ref().unwrap();

    let coef: f64 = (1.0 - ctx.brightness).powi(10);
    let coef_ln = coef.ln();

    for (cur_y, row) in plot.iter().enumerate() {
        for (cur_x, val) in row.iter().enumerate() {
            if !val.processed() {
                continue;
            }

            let color = if val.stable() {
                grad.at(0.0).to_rgba8()
            } else {
                let rel_val = (val.calculated_value() as i128 - ctx.min_value) as f64
                    / (ctx.max_value - ctx.min_value) as f64;
                let rel_val: f64 = (((rel_val.powi(2) + coef) as f64).ln() - coef_ln) / ((rel_val + coef).ln() - coef_ln);
                grad.at(rel_val).to_rgba8()
            };

            set_pixel(ctx, data, cur_x, cur_y, color[0], color[1], color[2]);
        }
    }

    let base = data.as_ptr() as usize;
    let len = data.len();

    let img = image_data(base, len, ctx.win_width as u32, ctx.win_height as u32);
    context.put_image_data(&img, 0.0, 0.0).unwrap();
}

fn set_pixel(ctx: &Ctx, data: &mut Vec<u8>, x: usize, y: usize, r: u8, g: u8, b: u8) {
    let index = (x + y * ctx.win_width) * 4;

    data[index + 0] = r;
    data[index + 1] = g;
    data[index + 2] = b;
    data[index + 3] = 255;
}

#[wasm_bindgen]
extern "C" {
    pub type OffscreenCanvas;
    pub type OffscreenCanvasRenderingContext2d;
    pub type ImageData;

    #[wasm_bindgen(constructor, catch, js_class = "ImageData")]
    fn new(data: &Uint8ClampedArray, width: f64, height: f64) -> Result<ImageData, JsValue>;

    #[wasm_bindgen(catch , method , structural , js_class = "OffscreenCanvasRenderingContext2d" , js_name = putImageData)]
    pub fn put_image_data(
        this: &OffscreenCanvasRenderingContext2d,
        imagedata: &ImageData,
        dx: f64,
        dy: f64,
    ) -> Result<(), JsValue>;

    #[wasm_bindgen(catch , method , structural , js_class = "OffscreenCanvas" , js_name = getContext)]
    pub fn get_context(
        this: &OffscreenCanvas,
        context_id: &str,
    ) -> Result<Option<OffscreenCanvasRenderingContext2d>, JsValue>;
}

fn image_data(base: usize, len: usize, width: u32, height: u32) -> ImageData {
    let mem = wasm_bindgen::memory().unchecked_into::<WebAssembly::Memory>();
    let mem = Uint8ClampedArray::new(&mem.buffer()).slice(base as u32, (base + len) as u32);
    ImageData::new(&mem, width as f64, height as f64).unwrap()
}

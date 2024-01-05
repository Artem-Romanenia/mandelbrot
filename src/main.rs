use leptos::html::Canvas;
use leptos::*;
use mandelbrot_web::Ctx;
use num::Float;
use wasm_bindgen::closure::Closure;
use web_sys::js_sys::Array;
use web_sys::js_sys::Math;
use web_sys::wasm_bindgen::JsCast;
use web_sys::Blob;
use web_sys::BlobPropertyBag;
use web_sys::MessageEvent;
use web_sys::MouseEvent;
use web_sys::Url;
use web_sys::Worker;

fn worker_new(name: &str) -> Worker {
    let base = leptos::window()
        .location()
        .href()
        .unwrap();

    let script = Array::new();
    script.push(
        &format!(r#"importScripts("{base}/{name}.js");wasm_bindgen("{base}/{name}_bg.wasm");"#)
            .into(),
    );

    let blob = Blob::new_with_str_sequence_and_options(
        &script,
        BlobPropertyBag::new().type_("text/javascript"),
    )
    .unwrap();

    let url = Url::create_object_url_with_blob(&blob).unwrap();

    Worker::new(&url).expect("Spawning worker should succeed.")
}

fn main() {
    _ = console_log::init_with_level(log::Level::Debug).unwrap();
    console_error_panic_hook::set_once();

    let window = leptos::window();

    let window_width = window.inner_width().unwrap().as_f64().unwrap() as u32;
    let window_height = window.inner_height().unwrap().as_f64().unwrap() as u32;

    let canvas_width = window_width - 25 - 10 - 300 - 25;
    let canvas_height = window_height - 25 - 25;

    let mut ctx = Ctx {
        win_width: canvas_width as usize,
        win_height: canvas_height as usize,
        ..Default::default()
    };
    ctx.define_bounds(-0.8, 0.0001, 3.5);

    let worker = worker_new("worker");

    let canvas_node = create_node_ref::<Canvas>();

    let worker_clone = worker.clone();
    create_effect(move |_| {
        let canvas = canvas_node.get_untracked().unwrap();

        canvas.set_width(canvas_width);
        canvas.set_height(canvas_height);

        let canvas = canvas.transfer_control_to_offscreen().unwrap();

        let worker_clone_clone = worker_clone.clone();
        let onmessage =
            Closure::<dyn Fn(MessageEvent)>::wrap(Box::new(move |msg: MessageEvent| {
                let data = Array::from(&msg.data());
                let g = Array::new();
                g.push(&canvas);

                // data.length == 0 means that it is an initial message indicating that worker is ready
                if data.length() == 0 {
                    let msg = Array::new();
                    msg.push(&canvas);
                    msg.push(&serde_wasm_bindgen::to_value(&ctx).unwrap());
                    worker_clone_clone
                        .post_message_with_transfer(&msg.into(), &g)
                        .unwrap();
                }
            }));
        worker_clone.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
    });

    let (hidden, set_hidden) = create_signal(true);

    let (x1, set_x1) = create_signal(0);
    let (y1, set_y1) = create_signal(0);
    let (x2, set_x2) = create_signal(0);
    let (y2, set_y2) = create_signal(0);

    let desc = move || format!("X: {}, Y: {}", x2.get(), y2.get());

    let x = move || Math::min(x1.get() as f64, x2.get() as f64);
    let w = move || Math::abs((x1.get() - x2.get()) as f64);
    let y = move || Math::min(y1.get() as f64, y2.get() as f64);
    let h = move || Math::abs((y1.get() - y2.get()) as f64);

    let container = leptos::document().get_element_by_id("main").unwrap();

    let omd = move |e: MouseEvent| {
        if e.button() != 0 {
            return;
        }

        set_hidden.update(|v| *v = false);
        set_x1.update(|v| *v = e.offset_x());
        set_y1.update(|v| *v = e.offset_y());
    };

    let omm = move |e: MouseEvent| {
        set_x2.update(|v| *v = e.offset_x());
        set_y2.update(|v| *v = e.offset_y());
    };

    let worker_clone = worker.clone();
    let omu = move |e: MouseEvent| {
        if e.button() != 0 {
            return;
        }

        let w = w();
        let h = h();
        let cx = ctx.get_x((x() + w / 2.0) as usize);
        let cy = ctx.get_y((y() + h / 2.0) as usize);
        let hs = ctx.horizontal_span * w / ctx.win_width as f64;

        ctx.define_bounds(cx, cy, hs);
        ctx.min_value = f64::max_value();
        ctx.max_value = 0.0;

        let msg = Array::new();
        msg.push(&serde_wasm_bindgen::to_value(&ctx).unwrap());
        let _ = worker_clone
            .post_message(&msg.into());

        set_hidden.update(|v| *v = true);
    };

    mount_to(container.unchecked_into(), move || {
        view! {
            <div id="canv" on:mousedown=omd on:mousemove=omm on:mouseup=omu>
                <div id="selection" hidden=hidden style:left=x style:top=y style:width=w style:height=h />
                <canvas _ref=canvas_node title=desc></canvas>
            </div>
            <div id="ctrls">
            </div>
        }
    });
}

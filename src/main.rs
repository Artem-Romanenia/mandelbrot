use mandelbrot_web::Ctx;
use leptos::html::Canvas;
use leptos::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::{
    js_sys::Array, wasm_bindgen::JsCast, Blob, BlobPropertyBag, MessageEvent, MouseEvent,
    OffscreenCanvas, TouchEvent, Url, Worker,
};

fn worker_new(name: &str) -> Worker {
    let base = leptos::window().location().href().unwrap();

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

    let canvas_width = window_width - 10 - 300 - 25;
    let canvas_height = window_height;

    let mut ctx = Ctx {
        win_width: canvas_width as usize,
        win_height: canvas_height as usize,
        ..Default::default()
    };
    ctx.define_bounds(-922337203685477580, 10, 4035225266123964416);

    let worker = worker_new("worker");

    let canvas_node = create_node_ref::<Canvas>();

    let (ctx, set_ctx) = create_signal(ctx);
    let (hidden, set_hidden) = create_signal(true);
    let (x1, set_x1) = create_signal(0);
    let (y1, set_y1) = create_signal(0);
    let (x2, set_x2) = create_signal(0);
    let (y2, set_y2) = create_signal(0);

    let brightness = move || ctx.get().brightness;
    let iters = move || ctx.get().max_iters;
    let x = move || x1.get().min(x2.get()) as usize;
    let w = move || (x1.get() as i32 - x2.get()).abs() as usize;
    let y = move || y1.get().min(y2.get()) as usize;
    let h = move || (y1.get() as i32 - y2.get()).abs() as usize;

    let worker_clone = worker.clone();
    create_effect(move |_| {
        let canvas = canvas_node.get_untracked().unwrap();

        canvas.set_width(canvas_width);
        canvas.set_height(canvas_height);

        let canvas = canvas.transfer_control_to_offscreen().unwrap();

        let worker_clone_clone = worker_clone.clone();
        let onmessage =
            Closure::<dyn FnMut(MessageEvent)>::wrap(Box::new(move |msg: MessageEvent| {
                let data = Array::from(&msg.data());

                // data.length == 0 means that it is an initial message indicating that worker is ready
                if data.length() == 0 {
                    let new_data = Array::new();
                    new_data.push(&canvas);

                    let ctx = ctx.get_untracked();

                    worker_clone_clone
                        .post_message_with_transfer(&pack_init_message(&canvas, &ctx), &new_data)
                        .unwrap();
                } else {
                    let new_ctx: Ctx = serde_wasm_bindgen::from_value(data.get(0)).unwrap();
                    set_ctx.update(|v| {
                        v.needs_recalc = false;
                        v.apply_changes(&new_ctx)
                    });
                }
            }));
        worker_clone.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
    });

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

        let mut ctx = ctx.get();

        ctx.define_bounds_from(x(), y(), w(), h());
        ctx.reset_min_max();
        ctx.needs_recalc = true;

        set_ctx.update(|v| *v = ctx);

        let _ = worker_clone.post_message(&pack_message(&ctx));

        set_hidden.update(|v| *v = true);
    };

    let ots = move |e: TouchEvent| {
        set_hidden.update(|v| *v = false);

        let touch = e.touches().get(0).unwrap();
        set_x1.update(|v| *v = touch.page_x());
        set_y1.update(|v| *v = touch.page_y());
        set_x2.update(|v| *v = touch.page_x());
        set_y2.update(|v| *v = touch.page_y());
    };

    let otm = move |e: TouchEvent| {
        let touch = e.changed_touches().get(0).unwrap();
        set_x2.update(|v| *v = touch.page_x());
        set_y2.update(|v| *v = touch.page_y());
    };

    let worker_clone = worker.clone();
    let ote = move |_| {
        let mut ctx = ctx.get();

        ctx.define_bounds_from(x(), y(), w(), h());
        ctx.reset_min_max();
        ctx.needs_recalc = true;

        set_ctx.update(|v| *v = ctx);

        let _ = worker_clone.post_message(&pack_message(&ctx));

        set_hidden.update(|v| *v = true);
    };

    let worker_clone = worker.clone();
    let on_update_click = move |_| {
        let ctx = ctx.get();
        let _ = worker_clone.post_message(&pack_message(&ctx));
    };

    let container = leptos::document().get_element_by_id("main").unwrap();
    mount_to(container.unchecked_into(), move || {
        view! {
            <div id="canv" on:mousedown=omd on:mousemove=omm on:mouseup=omu on:touchstart=ots on:touchmove=otm on:touchend=ote>
                <div id="selection" hidden=hidden style:left=x style:top=y style:width=w style:height=h />
                <canvas _ref=canvas_node></canvas>
            </div>
            <div id="ctrls">
                <div>
                    <label>Brightness</label><input type="number" value=brightness on:input=move |ev| {
                        let parsed_v = event_target_value(&ev).parse();
                        if let Ok(parsed_v) = parsed_v {
                            set_ctx.update(|v| v.brightness = parsed_v)
                        }
                    } />
                </div>
                <div>
                    <label>Iters</label><input type="number" value=iters on:input=move |ev| {
                        let parsed_v = event_target_value(&ev).parse();
                        if let Ok(parsed_v) = parsed_v {
                            set_ctx.update(|v| {
                                v.needs_recalc = true;
                                v.max_iters = parsed_v;
                            })
                        }
                    } />
                </div>
                <button on:click=on_update_click>Update</button>
            </div>
        }
    });
}

fn pack_init_message(canvas: &OffscreenCanvas, ctx: &Ctx) -> JsValue {
    let msg = Array::new();
    msg.push(canvas);
    msg.push(&serde_wasm_bindgen::to_value(ctx).unwrap());

    msg.into()
}

fn pack_message(ctx: &Ctx) -> JsValue {
    let msg = Array::new();
    msg.push(&serde_wasm_bindgen::to_value(ctx).unwrap());

    msg.into()
}

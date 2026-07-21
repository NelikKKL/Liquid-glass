//! Liquid Glass — лёгкий WebGL2-рендерер эффекта "жидкого стекла"
//! (прозрачность, блики, преломление света) поверх любого HTML-элемента.
//!
//! Идея: нативный `backdrop-filter: blur()` даёт настоящее (и очень быстрое,
//! GPU-композитное) размытие фона, а маленький WebGL2-канвас поверх элемента
//! рисует только динамические блики / рефракцию / кромку — это дёшево
//! (один fullscreen-quad, один fragment shader) и не требует захвата пикселей
//! страницы.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, Element, HtmlCanvasElement, HtmlElement, MouseEvent, WebGl2RenderingContext,
    WebGlProgram, WebGlShader, WebGlUniformLocation, Window,
};

const VERT_SRC: &str = r#"#version 300 es
layout(location = 0) in vec2 aPos;
out vec2 vUv;
void main() {
    vUv = aPos * 0.5 + 0.5;
    gl_Position = vec4(aPos, 0.0, 1.0);
}
"#;

const FRAG_SRC: &str = r#"#version 300 es
precision highp float;
in vec2 vUv;
out vec4 fragColor;

uniform vec2  uResolution;
uniform float uRadius;
uniform float uBorder;
uniform float uTime;
uniform vec2  uMouse;
uniform float uIntensity;
uniform vec3  uTint;

float sdRoundRect(vec2 p, vec2 halfSize, float r) {
    vec2 q = abs(p) - halfSize + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    vec2 res = uResolution;
    vec2 p = vUv * res;
    vec2 center = res * 0.5;
    float d = sdRoundRect(p - center, res * 0.5, uRadius);

    // За пределами скруглённого прямоугольника — полностью прозрачно.
    if (d > 1.0) { discard; }

    // Лёгкое "дыхание" поверхности жидкости.
    float wobble = sin(uTime * 0.6 + p.x * 0.02) * cos(uTime * 0.5 + p.y * 0.02);

    // Кромка (rim light) — светлая полоска по краю стекла.
    float rim = 1.0 - clamp(abs(d) / max(uBorder, 1.0), 0.0, 1.0);
    rim = pow(rim, 2.0);

    // Блик, следующий за курсором (или плавающий, если мышь не задана).
    vec2 mousePx = vec2(uMouse.x, 1.0 - uMouse.y) * res;
    float distToMouse = length(p - mousePx);
    float spec = smoothstep(res.x * 0.4, 0.0, distToMouse) * 0.65;

    // Диагональный "блик преломления", как на реальном стекле.
    vec2 sweepDir = normalize(vec2(1.0, -0.6));
    float sweep = dot(p - center, sweepDir) / max(res.x, res.y);
    float sweepLight = smoothstep(0.18, 0.0, abs(sweep - sin(uTime * 0.3) * 0.35)) * 0.22;

    float highlight = (rim * 0.55 + spec + sweepLight + wobble * 0.03) * uIntensity;

    // Хроматическая аберрация у краёв — усиливает ощущение преломления света.
    float ab = rim * 1.6;
    vec3 col = uTint + vec3(highlight);
    col.r += ab * 0.025;
    col.b -= ab * 0.025;

    float alpha = clamp(0.06 + highlight * 0.55, 0.0, 0.9);
    float edgeAA = 1.0 - smoothstep(-1.5, 1.0, d);
    alpha *= edgeAA;

    fragColor = vec4(col * alpha, alpha);
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

fn window() -> Window {
    web_sys::window().expect("no global `window`")
}

fn document() -> Document {
    window().document().expect("no `document`")
}

fn now_ms() -> f64 {
    window().performance().expect("no `performance`").now()
}

fn compile_shader(
    gl: &WebGl2RenderingContext,
    kind: u32,
    src: &str,
) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(kind).ok_or("cannot create shader")?;
    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        let log = gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| "unknown shader error".into());
        Err(log)
    }
}

fn link_program(
    gl: &WebGl2RenderingContext,
    vert: &WebGlShader,
    frag: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("cannot create program")?;
    gl.attach_shader(&program, vert);
    gl.attach_shader(&program, frag);
    gl.link_program(&program);
    if gl
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        let log = gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| "unknown program error".into());
        Err(log)
    }
}

fn opt_f32(options: &JsValue, key: &str, default: f32) -> f32 {
    match Reflect::get(options, &JsValue::from_str(key)) {
        Ok(v) => v
            .as_f64()
            .or_else(|| v.as_string().and_then(|s| s.parse::<f64>().ok()))
            .map(|v| v as f32)
            .unwrap_or(default),
        Err(_) => default,
    }
}

fn opt_str(options: &JsValue, key: &str, default: &str) -> String {
    Reflect::get(options, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| default.to_string())
}

fn parse_hex_rgb(hex: &str) -> (f32, f32, f32) {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(255);
        (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    } else {
        (1.0, 1.0, 1.0)
    }
}

struct Locations {
    resolution: WebGlUniformLocation,
    radius: WebGlUniformLocation,
    border: WebGlUniformLocation,
    time: WebGlUniformLocation,
    mouse: WebGlUniformLocation,
    intensity: WebGlUniformLocation,
    tint: WebGlUniformLocation,
}

struct Inner {
    canvas: HtmlCanvasElement,
    gl: WebGl2RenderingContext,
    loc: Locations,
    target: HtmlElement,
    start_time: f64,
    mouse: Rc<Cell<(f32, f32)>>,
    intensity: Cell<f32>,
    tint: Cell<(f32, f32, f32)>,
    radius: Cell<f32>,
    border: Cell<f32>,
    last_w: Cell<i32>,
    last_h: Cell<i32>,
    raf_id: Cell<Option<i32>>,
    running: Cell<bool>,
    // держим замыкание mousemove живым, чтобы его можно было снять при destroy
    mousemove_cb: RefCell<Option<Closure<dyn FnMut(MouseEvent)>>>,
}

/// Хэндл на активный эффект Liquid Glass. Держите его, пока эффект должен жить;
/// вызовите `destroy()`, чтобы остановить рендер и убрать канвас.
#[wasm_bindgen]
pub struct LiquidGlass {
    inner: Rc<Inner>,
}

#[wasm_bindgen]
impl LiquidGlass {
    /// Создаёт и запускает эффект на первом элементе, подходящем под `selector`.
    ///
    /// options (JS-объект, все поля опциональны):
    /// ```js
    /// {
    ///   radius: 24,       // px, скругление (по умолчанию — border-radius элемента)
    ///   border: 1.5,      // px, толщина кромки/блика
    ///   blur: 16,         // px, backdrop-filter blur применяется автоматически
    ///   intensity: 1.0,   // сила бликов/преломления
    ///   tint: "#ffffff",  // оттенок стекла
    ///   interactive: true // блик следует за курсором
    /// }
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(selector: &str, options: JsValue) -> Result<LiquidGlass, JsValue> {
        #[cfg(feature = "panic_hook")]
        console_error_panic_hook::set_once();

        let doc = document();
        let el: Element = doc
            .query_selector(selector)
            .map_err(|e| e)?
            .ok_or_else(|| JsValue::from_str(&format!("liquid-glass: элемент \"{selector}\" не найден")))?;
        let target: HtmlElement = el.dyn_into().map_err(|_| JsValue::from_str("liquid-glass: элемент должен быть HTMLElement"))?;

        Self::from_element(target, options)
    }

    /// То же самое, но принимает уже готовый `HTMLElement` (удобно вызывать из Rust/JS напрямую).
    #[wasm_bindgen(js_name = fromElement)]
    pub fn from_element(target: HtmlElement, options: JsValue) -> Result<LiquidGlass, JsValue> {
        let doc = document();

        // --- параметры ---
        let intensity = opt_f32(&options, "intensity", 1.0);
        let border = opt_f32(&options, "border", 1.5);
        let blur = opt_f32(&options, "blur", 16.0);
        let tint_hex = opt_str(&options, "tint", "#ffffff");
        let tint = parse_hex_rgb(&tint_hex);
        let interactive = Reflect::get(&options, &JsValue::from_str("interactive"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let style = window()
            .get_computed_style(&target)
            .map_err(|e| e)?
            .ok_or_else(|| JsValue::from_str("liquid-glass: computed style недоступен"))?;
        let computed_radius = style
            .get_property_value("border-radius")
            .ok()
            .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
            .unwrap_or(0.0);
        let radius = opt_f32(&options, "radius", computed_radius.max(0.0));

        // --- готовим сам элемент: настоящее нативное размытие фона ---
        let target_style = target.style();
        let _ = target_style.set_property("position", "relative");
        let _ = target_style.set_property("overflow", "hidden");
        let _ = target_style.set_property(
            "backdrop-filter",
            &format!("blur({blur}px) saturate(160%)"),
        );
        let _ = target_style.set_property(
            "-webkit-backdrop-filter",
            &format!("blur({blur}px) saturate(160%)"),
        );
        if style.get_property_value("background-color").unwrap_or_default() == "rgba(0, 0, 0, 0)" {
            let _ = target_style.set_property(
                "background-color",
                &format!("rgba({}, {}, {}, 0.12)", (tint.0 * 255.0) as u8, (tint.1 * 255.0) as u8, (tint.2 * 255.0) as u8),
            );
        }

        // --- создаём канвас-оверлей ---
        let canvas: HtmlCanvasElement = doc
            .create_element("canvas")
            .map_err(|e| e)?
            .dyn_into()
            .unwrap();
        let cs = canvas.style();
        let _ = cs.set_property("position", "absolute");
        let _ = cs.set_property("inset", "0");
        let _ = cs.set_property("width", "100%");
        let _ = cs.set_property("height", "100%");
        let _ = cs.set_property("pointer-events", "none");
        let _ = cs.set_property("mix-blend-mode", "screen");
        let _ = cs.set_property("z-index", "1");
        target.append_child(&canvas).map_err(|e| e)?;

        let gl_obj = canvas
            .get_context("webgl2")
            .map_err(|e| e)?
            .ok_or_else(|| JsValue::from_str("WebGL2 не поддерживается этим браузером"))?;
        let gl: WebGl2RenderingContext = gl_obj
            .dyn_into()
            .map_err(|_| JsValue::from_str("не удалось получить WebGL2RenderingContext"))?;

        let vert = compile_shader(&gl, WebGl2RenderingContext::VERTEX_SHADER, VERT_SRC)
            .map_err(|e| JsValue::from_str(&e))?;
        let frag = compile_shader(&gl, WebGl2RenderingContext::FRAGMENT_SHADER, FRAG_SRC)
            .map_err(|e| JsValue::from_str(&e))?;
        let program = link_program(&gl, &vert, &frag).map_err(|e| JsValue::from_str(&e))?;
        gl.use_program(Some(&program));

        // fullscreen quad (два треугольника)
        let verts: [f32; 12] = [
            -1.0, -1.0, 1.0, -1.0, -1.0, 1.0,
            -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
        ];
        let buf = gl.create_buffer().ok_or("cannot create buffer")?;
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buf));
        unsafe {
            let view = js_sys::Float32Array::view(&verts);
            gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER,
                &view,
                WebGl2RenderingContext::STATIC_DRAW,
            );
        }
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 2, WebGl2RenderingContext::FLOAT, false, 0, 0);

        gl.enable(WebGl2RenderingContext::BLEND);
        gl.blend_func(WebGl2RenderingContext::SRC_ALPHA, WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA);

        let loc = Locations {
            resolution: gl.get_uniform_location(&program, "uResolution").ok_or("no uResolution")?,
            radius: gl.get_uniform_location(&program, "uRadius").ok_or("no uRadius")?,
            border: gl.get_uniform_location(&program, "uBorder").ok_or("no uBorder")?,
            time: gl.get_uniform_location(&program, "uTime").ok_or("no uTime")?,
            mouse: gl.get_uniform_location(&program, "uMouse").ok_or("no uMouse")?,
            intensity: gl.get_uniform_location(&program, "uIntensity").ok_or("no uIntensity")?,
            tint: gl.get_uniform_location(&program, "uTint").ok_or("no uTint")?,
        };

        let mouse = Rc::new(Cell::new((0.5f32, 0.5f32)));

        let inner = Rc::new(Inner {
            canvas,
            gl,
            loc,
            target: target.clone(),
            start_time: now_ms(),
            mouse: mouse.clone(),
            intensity: Cell::new(intensity),
            tint: Cell::new(tint),
            radius: Cell::new(radius),
            border: Cell::new(border.max(0.5)),
            last_w: Cell::new(0),
            last_h: Cell::new(0),
            raf_id: Cell::new(None),
            running: Cell::new(true),
            mousemove_cb: RefCell::new(None),
        });

        if interactive {
            let mouse_clone = mouse.clone();
            let target_for_mouse = target.clone();
            let cb = Closure::wrap(Box::new(move |ev: MouseEvent| {
                let rect = target_for_mouse.get_bounding_client_rect();
                let w = rect.width().max(1.0) as f32;
                let h = rect.height().max(1.0) as f32;
                let x = (ev.client_x() as f32 - rect.left() as f32) / w;
                let y = (ev.client_y() as f32 - rect.top() as f32) / h;
                mouse_clone.set((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
            }) as Box<dyn FnMut(MouseEvent)>);
            window()
                .add_event_listener_with_callback("mousemove", cb.as_ref().unchecked_ref())
                .map_err(|e| e)?;
            *inner.mousemove_cb.borrow_mut() = Some(cb);
        }

        LiquidGlass::start_loop(inner.clone());

        Ok(LiquidGlass { inner })
    }

    fn start_loop(inner: Rc<Inner>) {
        let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let g = f.clone();
        let inner_for_frame = inner.clone();

        *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            if !inner_for_frame.running.get() {
                return;
            }
            render_frame(&inner_for_frame);
            let id = window()
                .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                .expect("requestAnimationFrame failed");
            inner_for_frame.raf_id.set(Some(id));
        }) as Box<dyn FnMut()>));

        let id = window()
            .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .expect("requestAnimationFrame failed");
        inner.raf_id.set(Some(id));
        // Замыкание живёт, пока жив Rc-цикл внутри requestAnimationFrame callback'ов;
        // `g` намеренно не дропаем раньше времени.
        std::mem::forget(g);
    }

    /// Сила бликов/преломления (0.0–2.0, по умолчанию 1.0).
    #[wasm_bindgen(js_name = setIntensity)]
    pub fn set_intensity(&self, value: f32) {
        self.inner.intensity.set(value);
    }

    /// Оттенок стекла в hex, например "#88ccff".
    #[wasm_bindgen(js_name = setTint)]
    pub fn set_tint(&self, hex: &str) {
        self.inner.tint.set(parse_hex_rgb(hex));
    }

    /// Останавливает рендер и удаляет канвас с элемента.
    pub fn destroy(&self) {
        self.inner.running.set(false);
        if let Some(id) = self.inner.raf_id.get() {
            window().cancel_animation_frame(id).ok();
        }
        if let Some(cb) = self.inner.mousemove_cb.borrow_mut().take() {
            let _ = window().remove_event_listener_with_callback(
                "mousemove",
                cb.as_ref().unchecked_ref(),
            );
        }
        if let Some(parent) = self.inner.canvas.parent_node() {
            let _ = parent.remove_child(&self.inner.canvas);
        }
    }
}

fn render_frame(inner: &Rc<Inner>) {
    let rect = inner.target.get_bounding_client_rect();
    let dpr = window().device_pixel_ratio().max(1.0);
    let w = (rect.width() * dpr).round() as i32;
    let h = (rect.height() * dpr).round() as i32;
    if w <= 0 || h <= 0 {
        return;
    }
    if w != inner.last_w.get() || h != inner.last_h.get() {
        inner.canvas.set_width(w as u32);
        inner.canvas.set_height(h as u32);
        inner.gl.viewport(0, 0, w, h);
        inner.last_w.set(w);
        inner.last_h.set(h);
    }

    let gl = &inner.gl;
    gl.clear_color(0.0, 0.0, 0.0, 0.0);
    gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);

    let t = ((now_ms() - inner.start_time) / 1000.0) as f32;
    let (mx, my) = inner.mouse.get();
    let (tr, tg, tb) = inner.tint.get();

    gl.uniform2f(Some(&inner.loc.resolution), w as f32, h as f32);
    gl.uniform1f(Some(&inner.loc.radius), inner.radius.get() * dpr as f32);
    gl.uniform1f(Some(&inner.loc.border), inner.border.get() * dpr as f32);
    gl.uniform1f(Some(&inner.loc.time), t);
    gl.uniform2f(Some(&inner.loc.mouse), mx, my);
    gl.uniform1f(Some(&inner.loc.intensity), inner.intensity.get());
    gl.uniform3f(Some(&inner.loc.tint), tr, tg, tb);

    gl.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, 6);
}

/// Одна строка из JS: `liquidGlass(".my-button")`.
#[wasm_bindgen(js_name = liquidGlass)]
pub fn liquid_glass(selector: &str, options: JsValue) -> Result<LiquidGlass, JsValue> {
    LiquidGlass::new(selector, options)
}

/// Применяет эффект ко всем элементам, подходящим под селектор, возвращает массив хэндлов.
#[wasm_bindgen(js_name = liquidGlassAll)]
pub fn liquid_glass_all(selector: &str, options: JsValue) -> Result<js_sys::Array, JsValue> {
    let doc = document();
    let list = doc.query_selector_all(selector)?;
    let out = js_sys::Array::new();
    for i in 0..list.length() {
        if let Some(node) = list.item(i) {
            if let Ok(el) = node.dyn_into::<HtmlElement>() {
                let handle = LiquidGlass::from_element(el, options.clone())?;
                out.push(&JsValue::from(handle));
            }
        }
    }
    Ok(out)
}

/// Автоинициализация: находит все элементы с классом `.liquid-glass` и применяет эффект,
/// читая параметры из data-атрибутов (`data-lg-intensity`, `data-lg-tint`, `data-lg-blur`,
/// `data-lg-radius`, `data-lg-border`). Удобно вызвать один раз после загрузки страницы.
#[wasm_bindgen(js_name = autoInit)]
pub fn auto_init() -> Result<js_sys::Array, JsValue> {
    let doc = document();
    let list = doc.query_selector_all(".liquid-glass")?;
    let out = js_sys::Array::new();
    for i in 0..list.length() {
        if let Some(node) = list.item(i) {
            if let Ok(el) = node.dyn_into::<HtmlElement>() {
                let opts = js_sys::Object::new();
                let dataset = el.dataset();
                for (data_key, opt_key) in [
                    ("lgIntensity", "intensity"),
                    ("lgTint", "tint"),
                    ("lgBlur", "blur"),
                    ("lgRadius", "radius"),
                    ("lgBorder", "border"),
                ] {
                    if let Ok(v) = Reflect::get(&dataset, &JsValue::from_str(data_key)) {
                        if v.is_string() {
                            let _ = Reflect::set(&opts, &JsValue::from_str(opt_key), &v);
                        }
                    }
                }
                let handle = LiquidGlass::from_element(el, opts.into())?;
                out.push(&JsValue::from(handle));
            }
        }
    }
    Ok(out)
}

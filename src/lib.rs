use wasm_bindgen::prelude::*;
use web_sys::{WebGl2RenderingContext as GL, WebGlProgram, WebGlShader};

const VERTEX_SHADER: &str = r#"#version 300 es
in vec2 position;
out vec2 v_uv;
void main() {
    v_uv = (position + 1.0) * 0.5;
    gl_Position = vec4(position, 0.0, 1.0);
}
"#;

// Шейдер эффектa "Жидкого стекла"
const FRAGMENT_SHADER: &str = r#"#version 300 es
precision highp float;

in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_resolution;
uniform float u_time;
uniform float u_blur;

// Псевдо-случайный шум для эффекта неоднородности стекла
float hash(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

void main() {
    vec2 st = v_uv;
    
    // Вычисление Френеля для краев (блики по периметру)
    vec2 edge = smoothstep(vec2(0.0), vec2(0.1), st) * smoothstep(vec2(1.0), vec2(0.9), st);
    float fresnel = 1.0 - (edge.x * edge.y);

    // Базовый объём и мягкие внутренние тени/свет
    vec3 baseColor = vec3(1.0, 1.0, 1.0);
    float glassAlpha = mix(0.12, 0.35, fresnel);

    // Блик от источника света (движется от u_time)
    vec2 lightPos = vec2(sin(u_time * 0.5) * 0.5 + 0.5, cos(u_time * 0.5) * 0.5 + 0.5);
    float dist = distance(st, lightPos);
    float highlight = smoothstep(0.8, 0.0, dist) * 0.25;

    // Смешиваем финальный цвет
    vec3 finalColor = baseColor + highlight + vec3(fresnel * 0.4);
    
    fragColor = vec4(finalColor, glassAlpha);
}
"#;

#[wasm_bindgen]
pub struct LiquidGlassRenderer {
    gl: GL,
    program: WebGlProgram,
    time_location: Option<web_sys::WebGlUniformLocation>,
    resolution_location: Option<web_sys::WebGlUniformLocation>,
    start_time: f64,
}

#[wasm_bindgen]
impl LiquidGlassRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: web_sys::HtmlCanvasElement) -> Result<LiquidGlassRenderer, JsValue> {
        let gl = canvas
            .get_context("webgl2")?
            .unwrap()
            .dyn_into::<GL>()?;

        let vert_shader = compile_shader(&gl, GL::VERTEX_SHADER, VERTEX_SHADER)?;
        let frag_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, FRAGMENT_SHADER)?;
        let program = link_program(&gl, &vert_shader, &frag_shader)?;

        gl.use_program(Some(&program));

        // Инициализация буфера геометрии (Quad)
        let vertices: [f32; 8] = [
            -1.0, -1.0,
             1.0, -1.0,
            -1.0,  1.0,
             1.0,  1.0,
        ];

        let buffer = gl.create_buffer().ok_or("Failed to create buffer")?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer));
        unsafe {
            let matrix_array = js_sys::Float32Array::view(&vertices);
            gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &matrix_array, GL::STATIC_DRAW);
        }

        let pos_attrib = gl.get_attrib_location(&program, "position") as u32;
        gl.enable_vertex_attrib_array(pos_attrib);
        gl.vertex_attrib_pointer_with_i32(pos_attrib, 2, GL::FLOAT, false, 0, 0);

        // Настройка блендинга для прозрачности и стекла
        gl.enable(GL::BLEND);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);

        let time_location = gl.get_uniform_location(&program, "u_time");
        let resolution_location = gl.get_uniform_location(&program, "u_resolution");

        let window = web_sys::window().unwrap();
        let start_time = window.performance().unwrap().now();

        Ok(LiquidGlassRenderer {
            gl,
            program,
            time_location,
            resolution_location,
            start_time,
        })
    }

    pub fn render(&self, width: f32, height: f32) {
        let window = web_sys::window().unwrap();
        let current_time = (window.performance().unwrap().now() - self.start_time) / 1000.0;

        self.gl.viewport(0, 0, width as i32, height as i32);
        self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
        self.gl.clear(GL::COLOR_BUFFER_BIT);

        self.gl.use_program(Some(&self.program));

        if let Some(ref loc) = self.time_location {
            self.gl.uniform1f(Some(loc), current_time as f32);
        }
        if let Some(ref loc) = self.resolution_location {
            self.gl.uniform2f(Some(loc), width, height);
        }

        self.gl.draw_arrays(GL::TRIANGLE_STRIP, 0, 4);
    }
}

fn compile_shader(gl: &GL, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("Could not create shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl.get_shader_parameter(&shader, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
        Ok(shader)
    } else {
        Err(gl.get_shader_info_log(&shader).unwrap_or_default())
    }
}

fn link_program(gl: &GL, vert: &WebGlShader, frag: &WebGlShader) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("Could not create program")?;
    gl.attach_shader(&program, vert);
    gl.attach_shader(&program, frag);
    gl.link_program(&program);

    if gl.get_program_parameter(&program, GL::LINK_STATUS).as_bool().unwrap_or(false) {
        Ok(program)
    } else {
        Err(gl.get_program_info_log(&program).unwrap_or_default())
    }
}
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlShader,
    WebGlTexture,
};
fn shader(gl: &Gl, kind: u32, src: &str) -> Result<WebGlShader, JsValue> {
    let s = gl.create_shader(kind).ok_or("shader allocation")?;
    gl.shader_source(&s, src);
    gl.compile_shader(&s);
    if !gl
        .get_shader_parameter(&s, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        return Err(gl.get_shader_info_log(&s).unwrap_or_default().into());
    }
    Ok(s)
}
fn program(gl: &Gl, vs: &str, fs: &str) -> Result<WebGlProgram, JsValue> {
    let p = gl.create_program().ok_or("program allocation")?;
    gl.attach_shader(&p, &shader(gl, Gl::VERTEX_SHADER, vs)?);
    gl.attach_shader(&p, &shader(gl, Gl::FRAGMENT_SHADER, fs)?);
    gl.link_program(&p);
    if !gl
        .get_program_parameter(&p, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        return Err(gl.get_program_info_log(&p).unwrap_or_default().into());
    }
    Ok(p)
}
struct Plot {
    gl: Gl,
    p: WebGlProgram,
    canvas: HtmlCanvasElement,
    buffer: WebGlBuffer,
}
impl Plot {
    fn new(id: &str, vs: &str, fs: &str) -> Result<Self, JsValue> {
        let canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(id)
            .ok_or("canvas missing")?
            .dyn_into::<HtmlCanvasElement>()?;
        let gl = canvas
            .get_context("webgl2")?
            .ok_or("WebGL2 unavailable")?
            .dyn_into::<Gl>()?;
        let p = program(&gl, vs, fs)?;
        let buffer = gl.create_buffer().ok_or("buffer allocation")?;
        Ok(Self {
            gl,
            p,
            canvas,
            buffer,
        })
    }
    fn begin(&self) {
        let ratio = web_sys::window().unwrap().device_pixel_ratio().min(2.);
        let w = (self.canvas.client_width() as f64 * ratio) as u32;
        let h = (self.canvas.client_height() as f64 * ratio) as u32;
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        self.gl.viewport(0, 0, w as i32, h as i32);
        self.gl.use_program(Some(&self.p));
        self.gl.clear_color(0., 0., 0., 0.);
        self.gl.clear(Gl::COLOR_BUFFER_BIT);
    }
    fn vertices(&self, v: &[f32]) {
        self.gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.buffer));
        let data = js_sys::Float32Array::from(v);
        self.gl
            .buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &data, Gl::STREAM_DRAW);
        let loc = self.gl.get_attrib_location(&self.p, "a") as u32;
        self.gl.enable_vertex_attrib_array(loc);
        self.gl
            .vertex_attrib_pointer_with_i32(loc, 2, Gl::FLOAT, false, 0, 0);
    }
    fn line(&self, v: &[f32], color: [f32; 4], mode: u32) {
        self.vertices(v);
        self.gl.uniform4fv_with_f32_array(
            self.gl.get_uniform_location(&self.p, "color").as_ref(),
            &color,
        );
        self.gl.draw_arrays(mode, 0, (v.len() / 2) as i32);
    }
}
const LINE_VS: &str =
    "#version 300 es\nin vec2 a; void main(){gl_Position=vec4(a,0.,1.);gl_PointSize=2.5;}";
const LINE_FS: &str =
    "#version 300 es\nprecision highp float;uniform vec4 color;out vec4 o;void main(){o=color;}";
const WATER_VS: &str =
    "#version 300 es\nin vec2 a;out vec2 uv;void main(){uv=a*0.5+0.5;gl_Position=vec4(a,0.,1.);}";
const WATER_FS:&str="#version 300 es\nprecision highp float;in vec2 uv;uniform sampler2D image;uniform float head;out vec4 o;void main(){float y=fract(head+(1.-uv.y)*0.996);float t=texture(image,vec2(uv.x,y)).r;vec3 c=mix(vec3(0.018,0.03,0.085),vec3(0.07,0.15,0.34),smoothstep(0.04,0.28,t));c=mix(c,vec3(0.08,0.52,0.68),smoothstep(0.25,0.5,t));c=mix(c,vec3(0.35,0.95,0.75),smoothstep(0.48,0.73,t));c=mix(c,vec3(1.,0.78,0.32),smoothstep(0.73,0.91,t));c=mix(c,vec3(1.,0.94,0.86),smoothstep(0.91,1.,t));o=vec4(c,1.);}";
#[wasm_bindgen]
pub struct Dashboard {
    water: Plot,
    spec: Plot,
    iq: Plot,
    scatter: Plot,
    texture: WebGlTexture,
    history: Vec<u8>,
    head: usize,
    width: usize,
}
#[wasm_bindgen]
impl Dashboard {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Dashboard, JsValue> {
        let water = Plot::new("waterfall", WATER_VS, WATER_FS)?;
        let spec = Plot::new("spectrum", LINE_VS, LINE_FS)?;
        let iq = Plot::new("iq", LINE_VS, LINE_FS)?;
        let scatter = Plot::new("scatter", LINE_VS, LINE_FS)?;
        let texture = water.gl.create_texture().ok_or("texture")?;
        water.gl.bind_texture(Gl::TEXTURE_2D, Some(&texture));
        for k in [Gl::TEXTURE_MIN_FILTER, Gl::TEXTURE_MAG_FILTER] {
            water
                .gl
                .tex_parameteri(Gl::TEXTURE_2D, k, Gl::NEAREST as i32);
        }
        for k in [Gl::TEXTURE_WRAP_S, Gl::TEXTURE_WRAP_T] {
            water
                .gl
                .tex_parameteri(Gl::TEXTURE_2D, k, Gl::CLAMP_TO_EDGE as i32);
        }
        Ok(Self {
            water,
            spec,
            iq,
            scatter,
            texture,
            history: vec![0; 2048 * 256 * 4],
            head: 0,
            width: 2048,
        })
    }
    pub fn reset(&mut self) {
        self.history.fill(0);
        self.head = 0;
    }
    pub fn draw(&mut self, spectrum: &[f32], samples: &[f32], floor: f32, ceiling: f32) {
        let range = (ceiling - floor).max(1.);
        if spectrum.len() != self.width {
            return;
        }
        self.head = (self.head + 255) % 256;
        let start = self.head * self.width * 4;
        for (i, p) in spectrum.iter().enumerate() {
            let t = ((p - floor) / range * 255.).clamp(0., 255.) as u8;
            self.history[start + 4 * i] = t;
            self.history[start + 4 * i + 3] = 255;
        }
        self.water.begin();
        let gl = &self.water.gl;
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));
        let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D,
            0,
            Gl::RGBA as i32,
            self.width as i32,
            256,
            0,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            Some(&self.history),
        );
        self.water
            .vertices(&[-1., -1., 1., -1., -1., 1., -1., 1., 1., -1., 1., 1.]);
        gl.uniform1f(
            gl.get_uniform_location(&self.water.p, "head").as_ref(),
            self.head as f32 / 256.,
        );
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
        self.spec.begin();
        let verts: Vec<f32> = spectrum
            .iter()
            .enumerate()
            .flat_map(|(i, v)| {
                [
                    i as f32 / (spectrum.len() - 1) as f32 * 2. - 1.,
                    ((v - floor) / range * 2. - 1.).clamp(-1., 1.),
                ]
            })
            .collect();
        self.spec
            .line(&verts, [0.35, 0.97, 0.80, 1.], Gl::LINE_STRIP);
        self.iq.begin();
        let n = samples.len() / 2;
        for ch in 0..2 {
            let verts: Vec<f32> = (0..n)
                .flat_map(|i| {
                    [
                        i as f32 / (n - 1).max(1) as f32 * 2. - 1.,
                        samples[2 * i + ch] * 0.9,
                    ]
                })
                .collect();
            self.iq.line(
                &verts,
                if ch == 0 {
                    [0.35, 0.97, 0.8, 0.9]
                } else {
                    [0.64, 0.53, 1., 0.8]
                },
                Gl::LINE_STRIP,
            );
        }
        self.scatter.begin();
        self.scatter
            .line(samples, [0.35, 0.9, 0.96, 0.8], Gl::POINTS);
    }
}

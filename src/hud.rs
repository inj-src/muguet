// muguet - hud.rs
// Constantin Clerc (c22dev) - 0.1

// Android has no UIKit views to put the touch controls in: touch.rs describes them here as a
// scene of circles and labels, and the gpu thread draws that scene over each frame it presents,
// in one instanced draw (circles are distance fields, labels a built-in 5x7 font, no textures).

use std::sync::Mutex;
use wgpu::*;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Root,
    Circle { center: (f64, f64), r: f64, white: f64, alpha: f64 },
    Label { r: f64 },
}

struct Node {
    parent: Option<usize>,
    kind: Kind,
    text: String,
    hidden: bool,
}

struct Scene {
    nodes: Vec<Node>,
    size: (f64, f64),
}

static SCENE: Mutex<Scene> = Mutex::new(Scene { nodes: Vec::new(), size: (1.0, 1.0) });

/// The same calls as the UIKit overlay in touch.rs, recorded into the scene.
pub mod view {
    use super::{Kind, Node, SCENE};
    use winit::window::Window;

    pub type View = usize;

    fn edit(v: View, f: impl FnOnce(&mut Node)) {
        if let Some(n) = SCENE.lock().unwrap().nodes.get_mut(v) { f(n); }
    }
    fn add(parent: Option<View>, kind: Kind) -> View {
        let mut s = SCENE.lock().unwrap();
        s.nodes.push(Node { parent, kind, text: String::new(), hidden: false });
        s.nodes.len() - 1
    }

    pub fn root(_window: &Window, width: f64, height: f64) -> Option<View> {
        let mut s = SCENE.lock().unwrap();
        s.nodes.clear();
        s.size = (width.max(1.0), height.max(1.0));
        drop(s);
        Some(add(None, Kind::Root))
    }
    pub fn circle(parent: &View, center: (f64, f64), r: f64, white: f64, alpha: f64) -> View {
        add(Some(*parent), Kind::Circle { center, r, white, alpha })
    }
    pub fn label(parent: &View, r: f64) -> View {
        add(Some(*parent), Kind::Label { r })
    }
    pub fn set_text(v: &View, text: &str) {
        edit(*v, |n| n.text = text.to_string());
    }
    pub fn set_hidden(v: &View, hidden: bool) {
        edit(*v, |n| n.hidden = hidden);
    }
    pub fn set_alpha_bg(v: &View, alpha: f64) {
        edit(*v, |n| if let Kind::Circle { white, alpha: a, .. } = &mut n.kind { *white = 1.0; *a = alpha; });
    }
    pub fn move_to(v: &View, c: (f64, f64)) {
        edit(*v, |n| if let Kind::Circle { center, .. } = &mut n.kind { *center = c; });
    }
}

/// 5x7 glyphs, one byte per row, high bit on the left.
fn glyph(c: char) -> Option<[u8; 7]> {
    let rows = match c {
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        '◀' => [0x02, 0x06, 0x0e, 0x1e, 0x0e, 0x06, 0x02],
        '▶' => [0x08, 0x0c, 0x0e, 0x0f, 0x0e, 0x0c, 0x08],
        '▲' => [0x00, 0x00, 0x04, 0x0e, 0x1f, 0x00, 0x00],
        '▼' => [0x00, 0x00, 0x1f, 0x0e, 0x04, 0x00, 0x00],
        _ => return None,
    };
    Some(rows)
}

/// Bit (row * 5 + column) of the glyph, split in two words.
fn glyph_bits(rows: [u8; 7]) -> (u32, u32) {
    let mut bits = 0u64;
    for (y, row) in rows.iter().enumerate() {
        for x in 0..5 {
            if row & (0x10 >> x) != 0 { bits |= 1 << (y * 5 + x); }
        }
    }
    (bits as u32, (bits >> 32) as u32)
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Quad {
    rect: [f32; 4],
    color: [f32; 2],
    glyph: [u32; 3],
}
const QUAD_SIZE: u64 = std::mem::size_of::<Quad>() as u64;
const MAX_QUADS: usize = 2048;

const SHADER: &str = "
struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) @interpolate(flat) color: vec2f,
    @location(2) @interpolate(flat) glyph: vec3u,
}
@vertex fn vs(@builtin(vertex_index) i: u32, @location(0) rect: vec4f, @location(1) color: vec2f, @location(2) glyph: vec3u) -> VOut {
    let c = vec2f(f32(i & 1u), f32(i >> 1u));
    var o: VOut;
    o.pos = vec4f(mix(rect.xy, rect.zw, c), 0.0, 1.0);
    o.uv = c;
    o.color = color;
    o.glyph = glyph;
    return o;
}
@fragment fn fs(v: VOut) -> @location(0) vec4f {
    let d = length(v.uv - vec2f(0.5)) * 2.0;
    let w = max(fwidth(d), 1e-4);
    var a = v.color.y;
    if (v.glyph.x == 0u) {
        a *= 1.0 - smoothstep(1.0 - w, 1.0, d);
    } else {
        let cell = vec2u(min(v.uv * vec2f(5.0, 7.0), vec2f(4.0, 6.0)));
        let bit = cell.y * 5u + cell.x;
        let word = select(v.glyph.y, v.glyph.z, bit >= 32u);
        if (((word >> (bit & 31u)) & 1u) == 0u) { a = 0.0; }
    }
    return vec4f(vec3f(v.color.x) * a, a);
}";

pub struct Hud {
    pipeline: RenderPipeline,
    quads: Buffer,
    count: u32,
    scratch: Vec<Quad>,
}

impl Hud {
    pub fn new(device: &Device, format: TextureFormat) -> Hud {
        let module = device.create_shader_module(ShaderModuleDescriptor { label: Some("hud"), source: ShaderSource::Wgsl(SHADER.into()) });
        let attributes = vertex_attr_array![0 => Float32x4, 1 => Float32x2, 2 => Uint32x3];
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("hud"),
            layout: None,
            vertex: VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(VertexBufferLayout { array_stride: QUAD_SIZE, step_mode: VertexStepMode::Instance, attributes: &attributes })],
            },
            primitive: PrimitiveState { topology: PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState { format, blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let quads = device.create_buffer(&BufferDescriptor {
            label: Some("hud quads"),
            size: QUAD_SIZE * MAX_QUADS as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Hud { pipeline, quads, count: 0, scratch: Vec::with_capacity(MAX_QUADS) }
    }

    /// Lays the scene out for this frame; the upload lands before the frame's commands run.
    pub fn prepare(&mut self, queue: &Queue) {
        self.scratch.clear();
        {
            let s = SCENE.lock().unwrap();
            let (w, h) = s.size;
            let ndc = |x0: f64, y0: f64, x1: f64, y1: f64| [(x0 / w * 2.0 - 1.0) as f32, (1.0 - y0 / h * 2.0) as f32, (x1 / w * 2.0 - 1.0) as f32, (1.0 - y1 / h * 2.0) as f32];
            let shown = |mut i: usize| loop {
                let n = &s.nodes[i];
                if n.hidden { return false; }
                match n.parent { Some(p) => i = p, None => return true }
            };
            for (i, n) in s.nodes.iter().enumerate() {
                if !shown(i) { continue; }
                match n.kind {
                    Kind::Root => {}
                    Kind::Circle { center: (x, y), r, white, alpha } => {
                        self.scratch.push(Quad { rect: ndc(x - r, y - r, x + r, y + r), color: [white as f32, alpha as f32], glyph: [0; 3] });
                    }
                    Kind::Label { r } => {
                        let Some(Kind::Circle { center, .. }) = n.parent.map(|p| s.nodes[p].kind) else { continue };
                        layout_text(&n.text, center, r, &mut |x0, y0, x1, y1, bits| {
                            self.scratch.push(Quad { rect: ndc(x0, y0, x1, y1), color: [1.0, 0.9], glyph: [1, bits.0, bits.1] });
                        });
                    }
                }
            }
        }
        self.scratch.truncate(MAX_QUADS);
        self.count = self.scratch.len() as u32;
        if self.count > 0 {
            let bytes = unsafe { std::slice::from_raw_parts(self.scratch.as_ptr() as *const u8, self.scratch.len() * QUAD_SIZE as usize) };
            queue.write_buffer(&self.quads, 0, bytes);
        }
    }

    pub fn draw(&self, pass: &mut RenderPass<'_>) {
        if self.count == 0 { return; }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.quads.slice(..));
        pass.draw(0..4, 0..self.count);
    }
}

/// Centers up to two lines in a button of radius r, as the UIKit label does (bold 9-15 pt,
/// shrunk to fit the button's width), calling `out` with each glyph's box and bits.
fn layout_text(text: &str, center: (f64, f64), r: f64, out: &mut dyn FnMut(f64, f64, f64, f64, (u32, u32))) {
    let lines: Vec<&str> = text.lines().take(2).collect();
    if lines.is_empty() { return; }
    let font = (r * 0.42).clamp(9.0, 15.0);
    let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
    // one font pixel: caps ~0.72 of the point size tall; advance 6 pixels, line 10
    let mut px = font * 0.72 / 7.0;
    let avail = 2.0 * r - 8.0;
    if widest * 6.0 * px > avail { px = avail / (widest * 6.0).max(1.0); }
    let height = lines.len() as f64 * 10.0 * px - 3.0 * px;
    let mut y = center.1 - height / 2.0;
    for line in lines {
        let n = line.chars().count() as f64;
        let mut x = center.0 - (n * 6.0 - 1.0) * px / 2.0;
        for c in line.chars() {
            if let Some(rows) = glyph(c.to_ascii_uppercase()) {
                out(x, y, x + 5.0 * px, y + 7.0 * px, glyph_bits(rows));
            }
            x += 6.0 * px;
        }
        y += 10.0 * px;
    }
}

use std::collections::HashMap;
use std::mem::size_of;

use glow::{HasContext, NativeBuffer, NativeProgram, NativeUniformLocation, NativeVertexArray};

use crate::commands::{Command, CommandQueue, RenderContext};
use crate::texture::TextureId;

#[derive(Clone)]
pub struct DrawMonochromeSprite {
    pub texture_id: TextureId,
    pub region:     (f32, f32, f32, f32), // x, y, w, h in texture pixels
    pub origin:     (f32, f32, f32),      // screen x, y, z-depth
    pub size:       (f32, f32),           // destination size on screen in pixels
    pub color:      (f32, f32, f32, f32), // r, g, b, a tint
}

impl Command for DrawMonochromeSprite {
    fn get_queue_from_registry(
        registry: &mut super::CommandQueueRegistry,
    ) -> &mut impl CommandQueue<Self> {
        &mut registry.draw_mono_sprite_queue
    }
}

// GPU-side instance data; excludes texture_id which is CPU-only.
#[repr(C)]
#[derive(Clone, Copy)]
struct SpriteInstance {
    color:  (f32, f32, f32, f32),
    origin: (f32, f32, f32),
    size:   (f32, f32),
    region: (f32, f32, f32, f32),
}
unsafe impl bytemuck::Pod for SpriteInstance {}
unsafe impl bytemuck::Zeroable for SpriteInstance {
    fn zeroed() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

impl From<&DrawMonochromeSprite> for SpriteInstance {
    fn from(s: &DrawMonochromeSprite) -> Self {
        Self {
            color:  s.color,
            origin: s.origin,
            size:   s.size,
            region: s.region,
        }
    }
}

#[derive(Default)]
pub(crate) struct MonoSpriteQueue {
    shader_program:         Option<NativeProgram>,
    vao:                    Option<NativeVertexArray>,
    vbo:                    Option<NativeBuffer>,
    ibo:                    Option<NativeBuffer>,
    u_viewport_inv_res_loc: Option<NativeUniformLocation>,
    u_tex_inv_size_loc:     Option<NativeUniformLocation>,
    u_texture_loc:          Option<NativeUniformLocation>,
    batches:                HashMap<TextureId, Vec<DrawMonochromeSprite>>,
}

impl CommandQueue<DrawMonochromeSprite> for MonoSpriteQueue {
    fn init(&mut self, ctx: &RenderContext) {
        unsafe {
            let gl = ctx.gl;
            let program = gl.create_program().expect("glCreateProgram");

            // UV mapping: aPos is in [-0.5, 0.5]. We map it to [0, 1] in both axes,
            // accounting for the Y-flip between screen space (Y down) and texture space (Y down).
            // uv_frac.x = aPos.x + 0.5  (left→0, right→1)
            // uv_frac.y = 0.5 - aPos.y  (top→0, bottom→1)
            let vs_src = r#"#version 300 es
                layout(location = 0) in vec2 aPos;
                layout(location = 1) in vec4 aColor;
                layout(location = 2) in vec3 aOrigin;
                layout(location = 3) in vec2 aSize;
                layout(location = 4) in vec4 aRegion;

                out vec4 vColor;
                out vec2 vUV;

                uniform vec2 uViewportInvRes;
                uniform vec2 uTexInvSize;

                void main() {
                    vec2 center   = aOrigin.xy + aSize * 0.5;
                    vec2 pixelPos = vec2(aPos.x, -aPos.y) * aSize + center;
                    vec2 ndc      = pixelPos * uViewportInvRes - 1.0;
                    gl_Position   = vec4(ndc, aOrigin.z, 1.0);

                    vColor = aColor;

                    vec2 uv_frac = vec2(aPos.x + 0.5, 0.5 - aPos.y);
                    vUV = (aRegion.xy + uv_frac * aRegion.zw) * uTexInvSize;
                }
            "#;

            let vs = gl
                .create_shader(glow::VERTEX_SHADER)
                .expect("glCreateShader(VERTEX_SHADER)");
            gl.shader_source(vs, vs_src);
            gl.compile_shader(vs);
            if !gl.get_shader_compile_status(vs) {
                panic!("vertex shader compile error: {}", gl.get_shader_info_log(vs));
            }

            // Texture is R8. The red channel acts as an alpha mask multiplied with the tint color's alpha.
            let fs_src = r#"#version 300 es
                precision mediump float;
                in vec4 vColor;
                in vec2 vUV;
                uniform sampler2D uTexture;
                out vec4 fragColor;

                void main() {
                    float mask = texture(uTexture, vUV).r;
                    fragColor  = vec4(vColor.rgb, vColor.a * mask);
                }
            "#;

            let fs = gl
                .create_shader(glow::FRAGMENT_SHADER)
                .expect("glCreateShader(FRAGMENT_SHADER)");
            gl.shader_source(fs, fs_src);
            gl.compile_shader(fs);
            if !gl.get_shader_compile_status(fs) {
                panic!("fragment shader compile error: {}", gl.get_shader_info_log(fs));
            }

            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("shader link error: {}", gl.get_program_info_log(program));
            }
            gl.delete_shader(vs);
            gl.delete_shader(fs);

            self.u_viewport_inv_res_loc = gl.get_uniform_location(program, "uViewportInvRes");
            self.u_tex_inv_size_loc     = gl.get_uniform_location(program, "uTexInvSize");
            self.u_texture_loc          = gl.get_uniform_location(program, "uTexture");
            self.shader_program = Some(program);

            self.vao = Some(gl.create_vertex_array().expect("glCreateVertexArray"));
            gl.bind_vertex_array(self.vao);

            self.vbo = Some(gl.create_buffer().expect("glCreateBuffer"));
            gl.bind_buffer(glow::ARRAY_BUFFER, self.vbo);
            #[rustfmt::skip]
            let vertices: [f32; 8] = [
                -0.5,  0.5,   // TL
                 0.5,  0.5,   // TR
                -0.5, -0.5,   // BL
                 0.5, -0.5,   // BR
            ];
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STATIC_DRAW,
            );
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, (2 * size_of::<f32>()) as i32, 0);

            self.ibo = Some(gl.create_buffer().expect("glCreateBuffer"));
            gl.bind_buffer(glow::ARRAY_BUFFER, self.ibo);
            gl.buffer_data_size(glow::ARRAY_BUFFER, 1024 * 1024, glow::DYNAMIC_DRAW);

            let stride = size_of::<SpriteInstance>() as i32;

            // aColor — offset 0
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, stride, 0);
            gl.vertex_attrib_divisor(1, 1);

            // aOrigin — offset 16
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 3, glow::FLOAT, false, stride, 4 * size_of::<f32>() as i32);
            gl.vertex_attrib_divisor(2, 1);

            // aSize — offset 28
            gl.enable_vertex_attrib_array(3);
            gl.vertex_attrib_pointer_f32(3, 2, glow::FLOAT, false, stride, 7 * size_of::<f32>() as i32);
            gl.vertex_attrib_divisor(3, 1);

            // aRegion — offset 36
            gl.enable_vertex_attrib_array(4);
            gl.vertex_attrib_pointer_f32(4, 4, glow::FLOAT, false, stride, 9 * size_of::<f32>() as i32);
            gl.vertex_attrib_divisor(4, 1);

            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
    }

    fn enqueue(&mut self, command: DrawMonochromeSprite) {
        self.batches.entry(command.texture_id).or_default().push(command);
    }

    fn process(&mut self, ctx: &RenderContext) {
        if self.batches.is_empty() {
            return;
        }

        let Some(program) = self.shader_program else {
            self.batches.clear();
            return;
        };

        let gl = ctx.gl;
        let vp_w = ctx.viewport_width as f32;
        let vp_h = ctx.viewport_height as f32;

        unsafe {
            gl.use_program(Some(program));
            gl.bind_vertex_array(self.vao);
            gl.bind_buffer(glow::ARRAY_BUFFER, self.ibo);

            gl.uniform_2_f32(self.u_viewport_inv_res_loc.as_ref(), 2.0 / vp_w, 2.0 / vp_h);

            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::GEQUAL);
            gl.depth_mask(false);
            gl.enable(glow::BLEND);
            gl.blend_func_separate(
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ZERO,
                glow::ONE,
            );

            gl.active_texture(glow::TEXTURE0);
            gl.uniform_1_i32(self.u_texture_loc.as_ref(), 0);

            for (tex_id, batch) in &mut self.batches {
                if batch.is_empty() {
                    continue;
                }
                let Some(gpu_tex) = ctx.textures.get(tex_id) else {
                    batch.clear();
                    continue;
                };

                batch.sort_unstable_by(|a, b| a.origin.2.total_cmp(&b.origin.2));

                gl.bind_texture(glow::TEXTURE_2D, Some(gpu_tex.handle));
                gl.uniform_2_f32(
                    self.u_tex_inv_size_loc.as_ref(),
                    1.0 / gpu_tex.width as f32,
                    1.0 / gpu_tex.height as f32,
                );

                let instances: Vec<SpriteInstance> = batch.iter().map(SpriteInstance::from).collect();
                gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytemuck::cast_slice(&instances));
                gl.draw_arrays_instanced(glow::TRIANGLE_STRIP, 0, 4, instances.len() as i32);
                batch.clear();
            }
            self.batches.clear();

            gl.depth_mask(true);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::BLEND);
            gl.bind_texture(glow::TEXTURE_2D, None);

            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.use_program(None);
        }
    }
}

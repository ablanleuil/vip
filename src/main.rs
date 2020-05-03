mod keyboard;
mod text;
mod canvas;
mod ui;
mod maths;
mod bitmap2d;

use image::{open, DynamicImage};
use luminance::{
    context::GraphicsContext,
    pipeline::PipelineState,
    shader::program::Program,
    render_state::{RenderState},
    tess::{Mode, TessBuilder},
    texture::{Sampler, Wrap, MinFilter, MagFilter, Texture, Dim2, GenMipmaps},
    pixel::{NormRGB8UI, NormRGBA8UI},
    blending::{Factor, Equation},
};
use luminance_glfw::{Surface, GlfwSurface, WindowDim, WindowOpt, WindowEvent};
use std::{fs, collections::{HashSet, HashMap}};

use ui::{
    selection as sel,
    uistate::{UiState, VisualType},
    Ui,
};
use canvas::Canvas;
use maths::*;
use keyboard::CharKeyMod;
use bitmap2d::*;

fn main() {

    const WIDTH : f32 = 800.0;
    const HEIGHT : f32 = 600.0;

    let dim = WindowDim::Windowed(WIDTH as u32, HEIGHT as u32);
    let opt = WindowOpt::default();
    let mut glfw = GlfwSurface::new(dim, "VIsual Pixels", opt)
        .expect("Couldn't create glfw window");

    
    let pipestate = PipelineState::new()
        .set_clear_color([0.3, 0.3, 0.3, 1.0])
        .enable_clear_color(true);

    let VS = fs::read_to_string("src/canvas/normal.vert").unwrap();
    let FS = fs::read_to_string("src/canvas/normal.frag").unwrap();
    let program : Program<canvas::Semantics, (), canvas::ShaderInterface> =
        Program::from_strings(None, &VS, None, &FS)
        .expect("Couldn't compile OpenGL program")
        .ignore_warnings();

    let TVS = fs::read_to_string("src/text/text.vert").unwrap();
    let TFS = fs::read_to_string("src/text/text.frag").unwrap();
    let text_program : Program<text::Semantics, (), text::ShaderInterface> =
        Program::from_strings(None, &TVS, None, &TFS)
        .expect("Couldn't compile Text shader program")
        .ignore_warnings();

    let SVS = fs::read_to_string("src/selection.vert").unwrap();
    let SFS = fs::read_to_string("src/selection.frag").unwrap();
    let select_program : Program<sel::Semantics, (), sel::ShaderInterface> =
        Program::from_strings(None, &SVS, None, &SFS)
        .expect("Couldn't compile Selection shader program")
        .ignore_warnings();


    let mut framebuffer = glfw.back_buffer().unwrap();

    let mut textb = text::TextRendererBuilder::for_resolution(64);
    let fid = textb.add_font("/usr/share/fonts/TTF/Hack-Regular.ttf").unwrap();

    let text_sampler = Sampler {
        wrap_r : Wrap::ClampToEdge,
        wrap_s : Wrap::ClampToEdge,
        wrap_t : Wrap::ClampToEdge,
        min_filter : MinFilter::LinearMipmapLinear,
        mag_filter : MagFilter::Linear,
        depth_comparison : None,
    };
    let text = textb.build(&mut glfw, text_sampler)
        .expect("Cannot load fonts");


    let render_state = RenderState::default()
        .set_blending(Some((Equation::Additive, Factor::SrcAlpha, Factor::SrcAlphaComplement)))
        .set_depth_test(None);

    let mut text_tess;

    let sampler = Sampler {
        wrap_r : Wrap::ClampToEdge,
        wrap_s : Wrap::ClampToEdge,
        wrap_t : Wrap::ClampToEdge,
        min_filter : MinFilter::Nearest,
        mag_filter : MagFilter::Nearest,
        depth_comparison : None,
    };

    let (width, height) = (64, 64);

    let tex : Texture<Dim2, NormRGB8UI> = Texture::new(&mut glfw, [width, height], 0, sampler)
        .expect("Cannot create texture");

    let pattern = Canvas::new(width as usize, height as usize);

    tex.upload(GenMipmaps::No, &pattern)
        .expect("Cannot upload texture");


    let mut ui : Ui<UiState> = Ui::new(|ui: &mut Ui<UiState>, UiState { selection, canvas, palette, ..}, c| {
        if let Some(color) = palette.get(&c) {
            if selection.is_empty() {
                let (x, y) = ui.cursor();
                canvas.set_pixel_color(x, y, *color);
            } else {
                for &(x, y) in selection.iter() {
                    canvas.set_pixel_color(x, y, *color);
                }
            }
        }
    });

    ui.set_window_event_listener(Some(|UiState { must_resize, scale:(x,y), window_size, ..} : &mut UiState, e| {
        match e {
            WindowEvent::FramebufferSize(bx, by) => {
                *x = 1.0 / (bx as f32);
                *y = 1.0 / (by as f32);
                *must_resize = true;
                *window_size = (bx as f32, by as f32);
            },
            _ => {},
        }
    }));

    "hjkl."
        .chars()
        .zip([(-1,0),(0,1),(0,-1),(1,0),(0,0)].iter())
        .for_each(|(l, (x,y))| {
            ui.add_object(l.to_string().as_ref(), move |ui, UiState { canvas,.. }, positions| {
                positions.insert(ui.cursor());
                let (w, h) = canvas.size();
                ui.wrapping_displace(*x, *y, w, h);
                positions.insert(ui.cursor());
            });
        });

    ui.add_verb("s", true, |_, UiState { canvas,.. }, positions| {
        let positions = positions.unwrap();
        for &(x, y) in positions {
            canvas.set_pixel_color(x, y, (255, 255, 255));
        }
    });

    ui.add_verb("<S-+>", false, |_, UiState { zoom, .. }, _| {
        if *zoom >= 0.1 {
            *zoom += 0.1;
        } else {
            *zoom += 0.01;
        }
    });

    ui.add_verb("-", false, |_, UiState { zoom, .. }, _| {
        if *zoom <= 0.1 {
            *zoom -= 0.01;
        } else {
            *zoom -= 0.1;
        }
    });

    ui.add_verb(":", false, |ui, _, _| {
        ui.set_mode(ui::Mode::Command);
    });

    ui.add_verb("i", false, |ui, UiState { selection, visual_type, .. }, _| {
        if ui.get_mode() == ui::Mode::Visual {
            selection.clear();
            let (a, b) = ui.get_selection();
            visual_type.select_pixels(selection, a, b);
        }
        ui.set_mode(ui::Mode::Insertion);
    });

    ui.add_verb("v", false, |ui, UiState { visual_type, .. }, _| {
        *visual_type = VisualType::Square;
        ui.set_mode(ui::Mode::Visual);
    });

    ui.add_verb("V", false, |ui, UiState { visual_type, .. }, _| {
        *visual_type = VisualType::Circle;
        ui.set_mode(ui::Mode::Visual);
    });

    ui.add_verb("e", false, |_, UiState { exploded, .. }, _| {
        *exploded = !(*exploded);
    });
    ui.add_verb("<C-S-+>", false, |_, UiState { chunk_size, .. }, _| {
        chunk_size.0 += 1;
        chunk_size.1 += 1;
    });
    ui.add_verb("<C-Minus>", false, |_, UiState { chunk_size, .. }, _| {
        chunk_size.0 -= 1;
        chunk_size.1 -= 1;
    });

    ui.add_verb("e", false, |_, UiState { exploded, .. }, _| {
        *exploded = !(*exploded);
    });
    
    ui.add_verb("H", false, |_, UiState { center,.. }:&mut UiState, _| {
        center.0 -= 1.0;
    });
    ui.add_verb("J", false, |_, UiState { center,.. }:&mut UiState, _| {
        center.1 += 1.0;
    });
    ui.add_verb("K", false, |_, UiState { center,.. }:&mut UiState, _| {
        center.1 -= 1.0;
    });
    ui.add_verb("L", false, |_, UiState { center,.. }:&mut UiState, _| {
        center.0 += 1.0;
    });

    ui.add_command("q", |ui, _, _| {
        ui.close()
    });

    ui.add_command("quit", |ui, _, _| {
        ui.close()
    });

    ui.add_command("color", |_, UiState { palette, .. }, args| {
        let key = args[0].into();
        let r = args[1].parse::<u8>().unwrap();
        let g = args[2].parse::<u8>().unwrap();
        let b = args[3].parse::<u8>().unwrap();

        palette.insert(key, (r, g, b));
    });

    ui.add_command("zoom", |_, UiState { zoom, .. }, args| {
        if let Ok(z) = args[0].parse::<f32>() {
            *zoom = z;
        }
    });

    // empty action
    ui.add_verb("_", true, |_,_,_| {});

    ui.bind_key("<Left>", ui::Mode::Insertion, "<Esc>hi");
    ui.bind_key("<Right>", ui::Mode::Insertion, "<Esc>li");
    ui.bind_key("<Down>", ui::Mode::Insertion, "<Esc>ji");
    ui.bind_key("<Up>", ui::Mode::Insertion, "<Esc>ki");

    ui.add_command("imap", |ui, _, args| {
        ui.bind_key(args[0], ui::Mode::Insertion, args[1]);
    });

    ui.add_verb("<Esc>", false, |_, UiState { selection, .. }, _| {
        selection.clear();
    });

    let mut palette = HashMap::new();
    palette.insert(CharKeyMod::from("a"), (255, 0, 0));
    palette.insert(CharKeyMod::from("z"), (0, 255, 0));
    palette.insert(CharKeyMod::from("e"), (0, 0, 255));
    let mut state = UiState {
        must_resize:false,
        scale:(1.0/WIDTH, 1.0/HEIGHT),
        zoom:1.0,
        canvas:pattern,
        center:(-8.0, -8.0),
        visual_type:VisualType::Square,
        palette,
        window_size:(WIDTH, HEIGHT),
        selection:HashSet::new(),
        chunk_size:(4, 4),
        exploded:false,
    };

    let img = open("selecteur.png").unwrap();
    let raw : Vec<(u8, u8, u8, u8)> =
        match img {
            DynamicImage::ImageRgba8(img) => {
                img
                    .into_vec()
                    .chunks(4)
                    .map(|l| {
                        match l {
                            [r,b,g,a] => (*r,*b,*g,*a),
                            _ => panic!("not normal"),
                        }
                    })
                    .collect()
            },
            _ => { unimplemented!("Error while loading selection image") },
        };
    let tex_sel : Texture<Dim2, NormRGBA8UI> = Texture::new(&mut glfw, [256, 256], 0, sampler)
        .expect("Cannot create selection texture");
    tex_sel.upload(GenMipmaps::No, raw.as_ref())
        .expect("Cannot upload selection texture");

    'main_loop: loop {
        if !ui.input(&mut glfw, &mut state) { break 'main_loop }


        if state.must_resize {
            framebuffer = glfw.back_buffer().unwrap();
            state.must_resize = false;
        }


        tex.upload(GenMipmaps::No, state.canvas.as_ref())
            .expect("Cannot upload texture");

        let mut verts = text.render_text(
            format!("{:?}:{}",
            ui.get_mode(),
            ui.get_buffer()),
            (0.0, state.window_size.1 - 64.0),
            fid,
            64.0);

        verts.append(&mut
            text.render_text(
                format!("Exploded: {}, Chunk Size: {:?}"
                        , state.exploded
                        , state.chunk_size),
                (0.0, -state.window_size.1 * 2.9),
                fid,
                64.0));
                

        text_tess = TessBuilder::new(&mut glfw)
            .add_vertices(&verts[..])
            .set_mode(Mode::Triangle)
            .build().ok();

        let set =
            if ui.get_mode() == ui::Mode::Visual {
                let (a, b) = ui.get_selection();
                let mut set = HashSet::new();
                state.visual_type.select_pixels(&mut set, a, b);
                set
            } else if state.selection.is_empty() {
                let (x, y) = ui.cursor();
                //let (w, h) = state.chunk_size;
                //let (x, y) = (x + (x / w), y + (y / h));
                [(x, y)].iter().cloned().collect()
            } else {
                state.selection.clone()
            };

        let select_tess = TessBuilder::new(&mut glfw)
            .add_vertices(&state.render_selection(&set))
            .set_mode(Mode::Triangle)
            .build()
            .unwrap();


        let tri_vert = state.render_canvas();
        let tess = TessBuilder::new(&mut glfw)
                .add_vertices(tri_vert)
                .set_mode(Mode::Triangle)
                .build()
                .unwrap();
        // draw
        glfw.pipeline_builder().pipeline(&framebuffer, &pipestate,
            |pipeline, mut shd_gate| {
                
                let drawing_buffer = pipeline.bind_texture(&tex);
                let font_atlas = pipeline.bind_texture(&text.atlas);
                let select_atlas = pipeline.bind_texture(&tex_sel);

                let text_view =
                    to_raw(
                        scale(state.scale.0 * 0.5, -state.scale.1 * 0.5)
                        *
                        translate(-state.window_size.0 * 2.0, state.window_size.1)
                    );



                let canvas_view =
                    to_raw(
                        scale(state.scale.0 * (width as f32) * state.zoom, -state.scale.1 * (height as f32) * state.zoom)
                        *
                        translate(state.center.0, state.center.1)
                    );

                // render canvas
                shd_gate.shade(&program, |iface, mut rdr_gate| {
                    iface.query().ask("tex").unwrap().update(&drawing_buffer);
                    iface.query().ask("view").unwrap().update(canvas_view);
                    rdr_gate.render(&render_state, |mut tess_gate| {
                        tess_gate.render(&tess)
                    });
                });

                // render selector
                shd_gate.shade(&select_program, |iface, mut rdr_gate| {
                    iface.query().ask("tex").unwrap().update(&select_atlas);
                    iface.query().ask("view").unwrap().update(canvas_view);
                    rdr_gate.render(&render_state, |mut tess_gate| {
                        tess_gate.render(&select_tess);
                    });
                });

                // render ui text
                text_tess.map(|text_tess| {
                    shd_gate.shade(&text_program, |iface, mut rdr_gate| {
                        let uniform = iface.query();
                        uniform.ask("tex").unwrap().update(&font_atlas);
                        uniform.ask("view").unwrap().update(text_view);


                        rdr_gate.render(&render_state, |mut tess_gate| {
                            tess_gate.render(&text_tess);
                        });
                    });
                });
            });

        // display
        glfw.swap_buffers();
    }
}

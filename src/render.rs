use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, ExportMem, Offscreen,
            damage::OutputDamageTracker,
            element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
            pixman::PixmanRenderer,
        },
    },
    utils::{Rectangle, Scale, Transform},
};

use crate::state::State;

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

const BACKGROUND: [f32; 4] = [0.1, 0.1, 0.1, 1.0];

pub fn render(state: &mut State, window: Option<u32>) -> Result<Image, String> {
    let State { toplevels, output, renderer, .. } = state;
    let toplevels: Vec<_> = match window {
        Some(id) => vec![toplevels.iter().find(|t| t.id == id).ok_or(format!("no window {id}"))?],
        None => toplevels.iter().collect(),
    };
    let size = match window {
        Some(_) => toplevels[0].window.geometry().size,
        None => output.current_mode().ok_or("no output mode")?.size.to_logical(1),
    };
    if size.w <= 0 || size.h <= 0 {
        return Err("window has no buffer yet".into());
    }
    let elements: Vec<WaylandSurfaceRenderElement<PixmanRenderer>> = toplevels
        .iter()
        .rev()
        .flat_map(|toplevel| {
            let origin = (-toplevel.window.geometry().loc.x, -toplevel.window.geometry().loc.y);
            toplevel.window.render_elements(renderer, origin.into(), Scale::from(1.0), 1.0)
        })
        .collect();
    let buffer_size = (size.w, size.h).into();
    let mut image: pixman::Image<'static, 'static> =
        renderer.create_buffer(Fourcc::Abgr8888, buffer_size).map_err(|e| e.to_string())?;
    let mut target = renderer.bind(&mut image).map_err(|e| e.to_string())?;
    OutputDamageTracker::new((size.w, size.h), 1.0, Transform::Normal)
        .render_output(renderer, &mut target, 0, &elements, BACKGROUND)
        .map_err(|e| e.to_string())?;
    let mapping = renderer
        .copy_framebuffer(&target, Rectangle::from_size(buffer_size), Fourcc::Abgr8888)
        .map_err(|e| e.to_string())?;
    let rgba = renderer.map_texture(&mapping).map_err(|e| e.to_string())?.to_vec();
    Ok(Image { width: size.w as u32, height: size.h as u32, rgba })
}

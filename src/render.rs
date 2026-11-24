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

pub fn render(state: &mut State, id: u32) -> Result<Image, String> {
    let State { toplevels, renderer, .. } = state;
    let toplevel = toplevels.iter().find(|t| t.id == id).ok_or(format!("no window {id}"))?;
    let geometry = toplevel.window.geometry();
    let size = geometry.size;
    if size.w <= 0 || size.h <= 0 {
        return Err("window has no buffer yet".into());
    }
    let origin = (-geometry.loc.x, -geometry.loc.y);
    let elements: Vec<WaylandSurfaceRenderElement<PixmanRenderer>> =
        toplevel.window.render_elements(renderer, origin.into(), Scale::from(1.0), 1.0);
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

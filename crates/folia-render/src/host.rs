// Adapts iced's public compositor API without modifying its renderer or widget implementations.

use graphics::compositor;
use graphics::core::{self, Color, Font, Pixels, Point, Rectangle, Size, Transformation, renderer, text};
use iced_wgpu::{Renderer as WgpuRenderer, graphics};

pub struct Renderer(pub WgpuRenderer);
pub struct Compositor(iced_wgpu::window::Compositor);

impl compositor::Default for Renderer {
    type Compositor = Compositor;
}

impl graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = wgpu::Surface<'static>;

    async fn with_backend(
        settings: graphics::Settings,
        _display: impl compositor::Display + Clone,
        window: impl compositor::Window + Clone,
        shell: graphics::Shell,
        backend: Option<&str>,
    ) -> Result<Self, graphics::Error> {
        if backend.is_some_and(|value| value != "wgpu") {
            return Err(graphics::Error::GraphicsAdapterNotFound {
                backend: "wgpu",
                reason: graphics::error::Reason::DidNotMatch {
                    preferred_backend: backend.unwrap_or_default().to_owned(),
                },
            });
        }
        crate::backend::remember(&window);
        let mut errors = Vec::new();
        for backends in crate::backend::configured() {
            let mut settings = iced_wgpu::Settings { backends, ..settings.into() };
            if let Some(mode) = iced_wgpu::settings::present_mode_from_env() {
                settings.present_mode = mode;
            }
            match iced_wgpu::window::Compositor::request(settings, Some(window.clone()), shell.clone()).await {
                Ok(compositor) => {
                    tracing::info!(?backends, "graphics backend ready");
                    return Ok(Self(compositor));
                }
                Err(err) => {
                    tracing::error!(
                        ?backends,
                        error = &err as &dyn std::error::Error,
                        "graphics initialization failed"
                    );
                    errors.push(err.to_string());
                }
            }
        }
        Err(graphics::Error::GraphicsAdapterNotFound {
            backend: "wgpu",
            reason: graphics::error::Reason::RequestFailed(errors.join("; ")),
        })
    }

    fn create_renderer(&self) -> Renderer {
        Renderer(self.0.create_renderer())
    }
    fn create_surface<W: compositor::Window + Clone>(&mut self, window: W, width: u32, height: u32) -> Self::Surface {
        self.0.create_surface(window, width, height)
    }
    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        self.0.configure_surface(surface, width, height);
    }
    fn information(&self) -> compositor::Information {
        self.0.information()
    }
    fn present(
        &mut self,
        renderer: &mut Renderer,
        surface: &mut Self::Surface,
        viewport: &graphics::Viewport,
        background: Color,
        before: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        self.0.present(&mut renderer.0, surface, viewport, background, before)
    }
    fn screenshot(&mut self, renderer: &mut Renderer, viewport: &graphics::Viewport, background: Color) -> Vec<u8> {
        self.0.screenshot(&mut renderer.0, viewport, background)
    }
}

impl core::Renderer for Renderer {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.0.start_layer(bounds);
    }
    fn end_layer(&mut self) {
        self.0.end_layer();
    }
    fn start_transformation(&mut self, transform: Transformation) {
        self.0.start_transformation(transform);
    }
    fn end_transformation(&mut self) {
        self.0.end_transformation();
    }
    fn fill_quad(&mut self, quad: renderer::Quad, background: impl Into<core::Background>) {
        self.0.fill_quad(quad, background);
    }
    fn reset(&mut self, bounds: Rectangle) {
        self.0.reset(bounds);
    }
    fn allocate_image(
        &mut self,
        handle: &core::image::Handle,
        callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        self.0.allocate_image(handle, callback);
    }
}

impl text::Renderer for Renderer {
    type Font = Font;
    type Paragraph = <WgpuRenderer as text::Renderer>::Paragraph;
    type Editor = <WgpuRenderer as text::Renderer>::Editor;
    const ICON_FONT: Font = WgpuRenderer::ICON_FONT;
    const CHECKMARK_ICON: char = WgpuRenderer::CHECKMARK_ICON;
    const ARROW_DOWN_ICON: char = WgpuRenderer::ARROW_DOWN_ICON;
    const SCROLL_UP_ICON: char = WgpuRenderer::SCROLL_UP_ICON;
    const SCROLL_DOWN_ICON: char = WgpuRenderer::SCROLL_DOWN_ICON;
    const SCROLL_LEFT_ICON: char = WgpuRenderer::SCROLL_LEFT_ICON;
    const SCROLL_RIGHT_ICON: char = WgpuRenderer::SCROLL_RIGHT_ICON;
    const ICED_LOGO: char = WgpuRenderer::ICED_LOGO;
    fn default_font(&self) -> Font {
        self.0.default_font()
    }
    fn default_size(&self) -> Pixels {
        self.0.default_size()
    }
    fn fill_paragraph(&mut self, paragraph: &Self::Paragraph, position: Point, color: Color, clip: Rectangle) {
        self.0.fill_paragraph(paragraph, position, color, clip);
    }
    fn fill_editor(&mut self, editor: &Self::Editor, position: Point, color: Color, clip: Rectangle) {
        self.0.fill_editor(editor, position, color, clip);
    }
    fn fill_text(&mut self, text: text::Text<String>, position: Point, color: Color, clip: Rectangle) {
        self.0.fill_text(text, position, color, clip);
    }
}

impl renderer::Headless for Renderer {
    async fn new(font: Font, size: Pixels, backend: Option<&str>) -> Option<Self> {
        if backend.is_some_and(|value| value != "wgpu") {
            return None;
        }
        let (adapter, device, queue) = crate::backend::device()
            .await
            .map_err(|error| tracing::error!(%error, "headless graphics initialization failed"))
            .ok()?;
        let engine = iced_wgpu::Engine::new(
            &adapter,
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            Some(graphics::Antialiasing::MSAAx4),
            graphics::Shell::headless(),
        );
        Some(Self(WgpuRenderer::new(engine, font, size)))
    }
    fn name(&self) -> String {
        "folia-wgpu".to_owned()
    }
    fn screenshot(&mut self, size: Size<u32>, scale: f32, color: Color) -> Vec<u8> {
        self.0.screenshot(&graphics::Viewport::with_physical_size(size, scale), color)
    }
}

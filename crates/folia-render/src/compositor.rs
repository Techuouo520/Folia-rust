// Configures alpha-capable window surfaces while reusing iced's renderer and presentation path.
use iced_wgpu::graphics::{self, color, compositor};

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("window surface creation failed")]
    Surface(#[from] wgpu::CreateSurfaceError),
    #[error("no compatible graphics adapter")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("graphics device creation failed")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("window surface has no supported color format")]
    Format,
}

pub(crate) type Surface = Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>;

pub(crate) struct Compositor {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    _quality_device: std::sync::Arc<crate::quality::DeviceRegistration>,
    engine: iced_wgpu::Engine,
    settings: iced_wgpu::Settings,
    format: wgpu::TextureFormat,
    alpha: wgpu::CompositeAlphaMode,
}

impl Compositor {
    pub(crate) async fn request(
        settings: iced_wgpu::Settings,
        window: impl compositor::Window,
        shell: graphics::Shell,
    ) -> Result<Self, Error> {
        let mut descriptor = wgpu::InstanceDescriptor { backends: settings.backends, ..Default::default() };
        descriptor.backend_options.dx12.presentation_system = wgpu::wgt::Dx12SwapchainKind::DxgiFromVisual;
        let instance = wgpu::Instance::new(&descriptor);
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: crate::backend::power_preference(),
                force_fallback_adapter: false,
            })
            .await?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.required_features().is_empty() && format.is_srgb() == color::GAMMA_CORRECTION)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or(Error::Format)?;
        let alpha = [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ]
        .into_iter()
        .find(|mode| capabilities.alpha_modes.contains(mode))
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        tracing::info!(adapter = ?adapter.get_info(), ?format, ?alpha, "window composition selected");
        crate::backend::adapter_ready(adapter.get_info().name);
        let (device, queue) = request_device(&adapter).await?;
        tracing::info!(features = ?device.features(), limits = ?device.limits(), "graphics device capabilities selected");
        let quality_device = crate::quality::register_device(&adapter, &device, shell.clone());
        let engine = iced_wgpu::Engine::new(&adapter, device.clone(), queue, format, settings.antialiasing, shell);
        Ok(Self { instance, adapter, device, _quality_device: quality_device, engine, settings, format, alpha })
    }

    pub(crate) fn create_renderer(&self) -> iced_wgpu::Renderer {
        iced_wgpu::Renderer::new(self.engine.clone(), self.settings.default_font, self.settings.default_text_size)
    }

    pub(crate) fn uses_direct_composition(&self) -> bool {
        cfg!(windows) && self.adapter.get_info().backend == wgpu::Backend::Dx12
    }

    pub(crate) fn create_surface(&self, window: impl compositor::Window, width: u32, height: u32) -> Surface {
        let mut surface = self.instance.create_surface(window).inspect_err(|err| {
            tracing::error!(error = err as &dyn std::error::Error, "window surface creation failed");
        });
        self.configure_surface(&mut surface, width, height);
        surface
    }

    pub(crate) fn configure_surface(&self, surface: &mut Surface, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Ok(surface) = surface {
            surface.configure(
                &self.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: self.format,
                    width,
                    height,
                    present_mode: self.settings.present_mode,
                    alpha_mode: self.alpha,
                    view_formats: Vec::new(),
                    desired_maximum_frame_latency: 1,
                },
            );
        }
    }

    pub(crate) fn information(&self) -> compositor::Information {
        let info = self.adapter.get_info();
        compositor::Information { adapter: info.name, backend: format!("{:?}", info.backend) }
    }
}

async fn request_device(adapter: &wgpu::Adapter) -> Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError> {
    let mut descriptor = wgpu::DeviceDescriptor {
        label: Some("Folia window renderer"),
        required_limits: limits(wgpu::Limits::default()),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        ..Default::default()
    };
    match adapter.request_device(&descriptor).await {
        Ok(device) => Ok(device),
        Err(err) => {
            tracing::warn!(error = &err as &dyn std::error::Error, "retrying graphics device with downlevel limits");
            descriptor.required_limits = limits(wgpu::Limits::downlevel_defaults());
            adapter.request_device(&descriptor).await
        }
    }
}

fn limits(limits: wgpu::Limits) -> wgpu::Limits {
    wgpu::Limits { max_bind_groups: 2, max_non_sampler_bindings: 2048, ..limits }
}

// Backend policy, real device creation in isolated environments and pixel-complete fullscreen layouts.
use folia_render::backend;
use iced::advanced::renderer::Headless;
use iced::{Color, Font, Pixels, Size};

#[test]
fn backend_order_and_environment_override() {
    assert_eq!(backend::candidates(true, None), [wgpu::Backends::DX12, wgpu::Backends::VULKAN]);
    assert_eq!(backend::candidates(true, Some("vulkan")), [wgpu::Backends::VULKAN]);
    assert_eq!(backend::candidates(true, Some("dx12")), [wgpu::Backends::DX12]);
    assert_eq!(backend::candidates(true, Some("gl")), [wgpu::Backends::GL]);
    assert_eq!(backend::candidates(true, Some("")), [wgpu::Backends::empty()]);
    assert_eq!(backend::candidates(true, Some("dx12,vulkan")), [wgpu::Backends::DX12 | wgpu::Backends::VULKAN]);
    assert!(!backend::candidates(false, None)[0].contains(wgpu::Backends::GL));
}

#[test]
#[cfg_attr(feature = "ci", ignore = "requires hardware GPU")]
fn backend_devices() {
    if std::env::var_os("FOLIA_BACKEND_CHILD").is_some() {
        if std::env::var("WGPU_BACKEND").as_deref() == Ok("not-a-backend") {
            let error = pollster::block_on(backend::device()).err().expect("invalid override must fail");
            eprintln!("EXPECTED initialization failure: {error}");
            return;
        }
        let (adapter, device, _) = pollster::block_on(backend::device()).unwrap();
        let info = adapter.get_info();
        let expected = match std::env::var("WGPU_BACKEND").as_deref() {
            Ok("vulkan") => wgpu::Backend::Vulkan,
            _ => wgpu::Backend::Dx12,
        };
        if cfg!(windows) {
            assert_eq!(info.backend, expected);
        }
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        eprintln!("GPU backend={:?} adapter={} driver={} {}", info.backend, info.name, info.driver, info.driver_info);
        return;
    }
    let choices =
        if cfg!(windows) { vec![None, Some("dx12"), Some("vulkan"), Some("not-a-backend")] } else { vec![None] };
    for choice in choices {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "backend::backend_devices", "--nocapture"]).env("FOLIA_BACKEND_CHILD", "1");
        if let Some(value) = choice {
            command.env("WGPU_BACKEND", value);
        } else {
            command.env_remove("WGPU_BACKEND");
        }
        let result = command.output().unwrap();
        eprintln!(
            "WGPU_BACKEND={choice:?}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.status.success());
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "requires hardware GPU")]
fn fullscreen_host_covers_every_pixel_at_fractional_dpi() {
    use iced::widget::{container, space};
    use iced_runtime::user_interface::{Cache, UserInterface};
    let mut renderer =
        pollster::block_on(folia_render::HostRenderer::new(Font::default(), Pixels(16.0), None)).unwrap();
    for (width, height, scale) in
        [(1200, 800, 1.0), (1920, 1080, 1.25), (2560, 1440, 1.5), (1365, 767, 1.75), (801, 603, 2.0)]
    {
        let content: iced::Element<'_, ()> = container(space())
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .style(|_| container::Style { background: Some(Color::from_rgb8(17, 34, 51).into()), ..Default::default() })
            .into();
        let size = Size::new(width as f32 / scale, height as f32 / scale);
        let mut ui = UserInterface::build(folia_render::host(content), size, Cache::new(), &mut renderer);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style { text_color: Color::WHITE },
            iced::mouse::Cursor::Unavailable,
        );
        let pixels = renderer.screenshot(Size::new(width, height), scale, Color::WHITE);
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            assert_eq!(pixel, [17, 34, 51, 255], "uncovered pixel {index} at {width}x{height}/{scale}");
        }
    }
}

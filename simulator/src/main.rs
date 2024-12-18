use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{BufReader, Write},
    path::Path,
};

use clap::Parser;
use simulator::{Simulator, State};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s
        .find(',')
        .ok_or_else(|| format!("no `,` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(
    help_template = "Author: {author-with-newline} {about-section}Version: {version} \n\n {usage-heading} {usage} \n\n {all-args} {tab}"
)]
struct Args {
    /// Windows Size (Optional, if set, overrides settings from file)
    #[arg(short = 'w', long = "window_size", value_name = "Width,Height" , value_parser = parse_key_val::<u32, u32>)]
    window_size: Option<(u32, u32)>,

    /// Port (Optional, if set, overrides settings from file)
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,

    /// Vsync (Optional, if set, overrides settings from file)
    #[arg(short = 'v', long = "vsync")]
    vsync: Option<bool>,

    /// Setting file dir
    #[arg(long = "setting_dir")]
    setting_dir: Option<String>,

    /// Setting file name
    #[arg(short = 's', long = "setting_file", default_value = "settings.json")]
    setting_file: String,

    /// lightweight mode (Optional, if set, overrides settings from file)
    #[arg(long = "lightweight", default_value = "false")]
    lightweight: Option<bool>,

    /// Debug mode
    #[arg(short = 'd', long = "debug", default_value = "false")]
    debug: bool,
}

fn main() -> anyhow::Result<()> {
    let arg = Args::parse();

    let port = arg.port;
    let window_size = arg.window_size;
    let settings_path = if let Some(path) = &arg.setting_dir {
        Path::new(path).join(&arg.setting_file)
    } else {
        Path::new(&arg.setting_file).to_owned()
    };
    let vsync = arg.vsync;
    let lightweight = arg.lightweight;
    let debug = arg.debug;

    let filter = if debug {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .parse("wgpu_core=warn,simulator=debug")?
    } else {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .parse("wgpu_core=off,simulator=info")?
    };
    tracing_subscriber::registry()
        .with(fmt::layer().with_ansi(false))
        .with(filter)
        .init();

    let mut state: State = if settings_path.exists() {
        let file = File::open(&settings_path)?;
        let reader = BufReader::new(file);
        match serde_json::from_reader(reader) {
            Ok(state) => state,
            Err(e) => {
                tracing::error!(
                    "Failed to parse settings file ({}): {}, using default settings.",
                    settings_path.display(),
                    e
                );
                Default::default()
            }
        }
    } else {
        tracing::info!(
            "Settings file ({}) not found, using default settings.",
            settings_path.display()
        );
        Default::default()
    };

    state.debug = debug;
    if let Some(port) = port {
        state.port = port;
    }
    if let Some(window_size) = window_size {
        state.window_size = window_size;
    }
    if let Some(vsync) = vsync {
        state.vsync = vsync;
    }
    if let Some(path) = &arg.setting_dir {
        state.settings_dir = path.clone();
    }
    if let Some(lightweight) = lightweight {
        state.lightweight = lightweight;
    }

    let event_loop = winit::event_loop::EventLoop::with_user_event().build()?;
    let state = Simulator::run(event_loop, state)?;

    {
        let settings_str = serde_json::to_string_pretty(&state)?;
        if settings_path.exists() {
            fs::remove_file(&settings_path)?;
        }
        std::fs::create_dir_all(settings_path.parent().unwrap())?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .append(false)
            .open(&settings_path)?;
        write!(file, "{}", settings_str)?;
    }

    Ok(())
}

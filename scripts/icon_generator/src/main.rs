use std::ffi::OsString;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use icns::{IconFamily, Image as IcnsImage, PixelFormat};
use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::{FilterType, overlay, resize};
use image::{ExtendedColorType, Rgba, RgbaImage};

const DEFAULT_ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const DEFAULT_ICNS_SIZES: [u32; 8] = [16, 32, 48, 64, 128, 256, 512, 1024];
const SUPPORTED_ICNS_SIZES: [u32; 8] = [16, 32, 48, 64, 128, 256, 512, 1024];

#[derive(Debug)]
struct Config {
    input: PathBuf,
    ico_output: Option<PathBuf>,
    icns_output: Option<PathBuf>,
    ico_sizes: Vec<u32>,
    icns_sizes: Vec<u32>,
}

fn usage(bin: &str) -> String {
    format!(
        "\
Usage:
  {bin} <input_png> [output.ico|output.icns]
  {bin} <input_png> [--ico <output.ico>] [--icns <output.icns>] [--ico-sizes <csv>] [--icns-sizes <csv>]

Examples:
  {bin} assets/app.png assets/app.ico
  {bin} assets/app.png --ico assets/app.ico --icns assets/app.icns
  {bin} assets/app.png --ico-sizes 16,32,48,256 --icns-sizes 16,32,128,256,512

Notes:
  - If no output is specified, both files are generated next to input:
    <input_stem>.ico and <input_stem>.icns
  - Supported ICNS sizes: {}
",
        join_sizes(&SUPPORTED_ICNS_SIZES),
    )
}

fn parse_sizes(csv: &str, kind: &str) -> Result<Vec<u32>> {
    let mut sizes = Vec::new();
    for raw in csv.split(',') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        let size = token
            .parse::<u32>()
            .with_context(|| format!("invalid {kind} size: '{token}'"))?;
        sizes.push(size);
    }

    if sizes.is_empty() {
        bail!("{kind} size list is empty");
    }

    sizes.sort_unstable();
    sizes.dedup();
    Ok(sizes)
}

fn join_sizes(sizes: &[u32]) -> String {
    sizes
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_output_by_extension(path: &Path) -> Result<(&'static str, PathBuf)> {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| x.to_ascii_lowercase());

    match ext.as_deref() {
        Some("ico") => Ok(("ico", path.to_path_buf())),
        Some("icns") => Ok(("icns", path.to_path_buf())),
        _ => bail!(
            "output extension must be .ico or .icns, got: {}",
            path.display()
        ),
    }
}

fn parse_args() -> Result<Config> {
    let mut args = std::env::args_os();
    let bin = args
        .next()
        .and_then(|x| x.into_string().ok())
        .unwrap_or_else(|| "icon_generator".to_string());

    let first = args.next();
    if matches!(first.as_deref(), Some(x) if x == "-h" || x == "--help") {
        println!("{}", usage(&bin));
        std::process::exit(0);
    }

    let input = first.context("missing input PNG path")?;

    let mut positional_output: Option<PathBuf> = None;
    let mut ico_output: Option<PathBuf> = None;
    let mut icns_output: Option<PathBuf> = None;
    let mut ico_sizes = DEFAULT_ICO_SIZES.to_vec();
    let mut icns_sizes = DEFAULT_ICNS_SIZES.to_vec();

    let pending = args.collect::<Vec<OsString>>();
    let mut index = 0usize;
    while index < pending.len() {
        let arg = &pending[index];
        let arg_str = arg
            .to_str()
            .with_context(|| format!("invalid non-utf8 argument at position {}", index + 2))?;

        if arg_str == "-h" || arg_str == "--help" {
            println!("{}", usage(&bin));
            std::process::exit(0);
        }

        if arg_str == "--ico" {
            index += 1;
            let value = pending
                .get(index)
                .context("missing value for --ico")?
                .to_str()
                .context("invalid non-utf8 value for --ico")?;
            ico_output = Some(PathBuf::from(value));
            index += 1;
            continue;
        }
        if let Some(value) = arg_str.strip_prefix("--ico=") {
            if value.trim().is_empty() {
                bail!("missing value for --ico");
            }
            ico_output = Some(PathBuf::from(value));
            index += 1;
            continue;
        }

        if arg_str == "--icns" {
            index += 1;
            let value = pending
                .get(index)
                .context("missing value for --icns")?
                .to_str()
                .context("invalid non-utf8 value for --icns")?;
            icns_output = Some(PathBuf::from(value));
            index += 1;
            continue;
        }
        if let Some(value) = arg_str.strip_prefix("--icns=") {
            if value.trim().is_empty() {
                bail!("missing value for --icns");
            }
            icns_output = Some(PathBuf::from(value));
            index += 1;
            continue;
        }

        if arg_str == "--ico-sizes" {
            index += 1;
            let value = pending
                .get(index)
                .context("missing value for --ico-sizes")?
                .to_str()
                .context("invalid non-utf8 value for --ico-sizes")?;
            ico_sizes = parse_sizes(value, "ICO")?;
            index += 1;
            continue;
        }
        if let Some(value) = arg_str.strip_prefix("--ico-sizes=") {
            ico_sizes = parse_sizes(value, "ICO")?;
            index += 1;
            continue;
        }

        if arg_str == "--icns-sizes" {
            index += 1;
            let value = pending
                .get(index)
                .context("missing value for --icns-sizes")?
                .to_str()
                .context("invalid non-utf8 value for --icns-sizes")?;
            icns_sizes = parse_sizes(value, "ICNS")?;
            index += 1;
            continue;
        }
        if let Some(value) = arg_str.strip_prefix("--icns-sizes=") {
            icns_sizes = parse_sizes(value, "ICNS")?;
            index += 1;
            continue;
        }

        if arg_str.starts_with("--") {
            bail!("unknown option: {arg_str}");
        }

        if positional_output.is_some() {
            bail!("too many positional arguments");
        }
        positional_output = Some(PathBuf::from(arg_str));
        index += 1;
    }

    if positional_output.is_some() && (ico_output.is_some() || icns_output.is_some()) {
        bail!("cannot mix positional output with --ico/--icns options");
    }

    if let Some(path) = positional_output {
        let (kind, resolved) = parse_output_by_extension(&path)?;
        if kind == "ico" {
            ico_output = Some(resolved);
        } else {
            icns_output = Some(resolved);
        }
    }

    if ico_output.is_none() && icns_output.is_none() {
        let input_path = PathBuf::from(&input);
        ico_output = Some(input_path.with_extension("ico"));
        icns_output = Some(input_path.with_extension("icns"));
    }

    for size in &ico_sizes {
        if !(1..=256).contains(size) {
            bail!("ICO size out of range (must be 1..=256): {size}");
        }
    }
    for size in &icns_sizes {
        if !SUPPORTED_ICNS_SIZES.contains(size) {
            bail!(
                "unsupported ICNS size: {size} (supported: {})",
                join_sizes(&SUPPORTED_ICNS_SIZES)
            );
        }
    }

    Ok(Config {
        input: PathBuf::from(input),
        ico_output,
        icns_output,
        ico_sizes,
        icns_sizes,
    })
}

fn build_icon_frame(source: &RgbaImage, size: u32) -> RgbaImage {
    let src_w = source.width().max(1);
    let src_h = source.height().max(1);

    let scale = f64::min(size as f64 / src_w as f64, size as f64 / src_h as f64);
    let resized_w = ((src_w as f64 * scale).round() as u32).clamp(1, size);
    let resized_h = ((src_h as f64 * scale).round() as u32).clamp(1, size);

    let resized = resize(source, resized_w, resized_h, FilterType::Lanczos3);
    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));

    let offset_x = i64::from((size - resized_w) / 2);
    let offset_y = i64::from((size - resized_h) / 2);
    overlay(&mut canvas, &resized, offset_x, offset_y);

    canvas
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory: {}", parent.display())
            })?;
        }
    }
    Ok(())
}

fn write_ico(source: &RgbaImage, output_path: &Path, sizes: &[u32]) -> Result<()> {
    ensure_parent_dir(output_path)?;

    let mut frames: Vec<IcoFrame<'static>> = Vec::with_capacity(sizes.len());
    for &size in sizes {
        let frame = build_icon_frame(source, size);
        let ico_frame = IcoFrame::as_png(frame.as_raw(), size, size, ExtendedColorType::Rgba8)
            .with_context(|| format!("failed to encode ICO frame {size}x{size}"))?;
        frames.push(ico_frame);
    }

    let file = File::create(output_path)
        .with_context(|| format!("failed to create output file: {}", output_path.display()))?;
    IcoEncoder::new(file)
        .encode_images(&frames)
        .with_context(|| format!("failed to write ICO file: {}", output_path.display()))?;

    println!(
        "Generated ICO: {} (sizes: {})",
        output_path.display(),
        join_sizes(sizes)
    );
    Ok(())
}

fn write_icns(source: &RgbaImage, output_path: &Path, sizes: &[u32]) -> Result<()> {
    ensure_parent_dir(output_path)?;

    let mut family = IconFamily::new();
    for &size in sizes {
        let frame = build_icon_frame(source, size);
        let image = IcnsImage::from_data(PixelFormat::RGBA, size, size, frame.into_raw())
            .with_context(|| format!("failed to prepare ICNS frame {size}x{size}"))?;
        family
            .add_icon(&image)
            .with_context(|| format!("failed to add ICNS frame {size}x{size}"))?;
    }

    let file = File::create(output_path)
        .with_context(|| format!("failed to create output file: {}", output_path.display()))?;
    let writer = BufWriter::new(file);
    family
        .write(writer)
        .with_context(|| format!("failed to write ICNS file: {}", output_path.display()))?;

    println!(
        "Generated ICNS: {} (sizes: {})",
        output_path.display(),
        join_sizes(sizes)
    );
    Ok(())
}

fn run() -> Result<()> {
    let cfg = parse_args()?;

    let source = image::open(&cfg.input)
        .with_context(|| format!("failed to open input image: {}", cfg.input.display()))?
        .into_rgba8();

    if source.width() == 0 || source.height() == 0 {
        bail!(
            "input image has invalid dimensions: {}x{}",
            source.width(),
            source.height()
        );
    }

    if let Some(ico_output) = cfg.ico_output.as_deref() {
        write_ico(&source, ico_output, &cfg.ico_sizes)?;
    }
    if let Some(icns_output) = cfg.icns_output.as_deref() {
        write_icns(&source, icns_output, &cfg.icns_sizes)?;
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

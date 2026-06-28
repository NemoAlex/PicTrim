use libvips::{ops, VipsApp, VipsImage};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use walkdir::WalkDir;

const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "gif", "jfif",
];

#[derive(Default)]
struct AppState {
    current_job: Mutex<Option<JobHandle>>,
}

struct JobHandle {
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchSettings {
    input_dir: String,
    output_dir: String,
    max_side: i32,
    quality: i32,
    concurrency: usize,
    output_format: OutputFormat,
    copy_non_images: bool,
    skip_existing: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum OutputFormat {
    Jpg,
    Png,
    Webp,
    Keep,
}

#[derive(Debug, Clone)]
struct WorkItem {
    src: PathBuf,
    rel: PathBuf,
    is_image: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchProgress {
    phase: String,
    discovered: usize,
    processed: usize,
    images: usize,
    copied: usize,
    skipped: usize,
    failed: usize,
    total_src_bytes: u64,
    total_dst_bytes: u64,
    current: Option<String>,
    message: Option<String>,
    done: bool,
    cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureEntry {
    rel: String,
    message: String,
}

#[derive(Default)]
struct Counters {
    processed: usize,
    images: usize,
    copied: usize,
    skipped: usize,
    failed: usize,
    total_src_bytes: u64,
    total_dst_bytes: u64,
    failures: Vec<FailureEntry>,
}

#[tauri::command]
fn start_batch(app: AppHandle, state: tauri::State<AppState>, settings: BatchSettings) -> Result<(), String> {
    validate_settings(&settings)?;

    let mut current_job = state
        .current_job
        .lock()
        .map_err(|_| "无法锁定任务状态".to_string())?;

    if current_job.is_some() {
        return Err("已有任务正在运行".to_string());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    *current_job = Some(JobHandle {
        cancel: cancel.clone(),
    });
    drop(current_job);

    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        let cancelled = run_batch(app_for_thread.clone(), settings, cancel);
        if let Some(state) = app_for_thread.try_state::<AppState>() {
            if let Ok(mut current_job) = state.current_job.lock() {
                *current_job = None;
            }
        }
        if cancelled {
            let _ = app_for_thread.emit(
                "batch-status",
                serde_json::json!({ "status": "cancelled" }),
            );
        }
    });

    Ok(())
}

#[tauri::command]
fn cancel_batch(state: tauri::State<AppState>) -> Result<(), String> {
    let current_job = state
        .current_job
        .lock()
        .map_err(|_| "无法锁定任务状态".to_string())?;
    if let Some(job) = current_job.as_ref() {
        job.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

fn validate_settings(settings: &BatchSettings) -> Result<(), String> {
    let input = Path::new(&settings.input_dir);
    if !input.is_dir() {
        return Err("输入目录不存在".to_string());
    }
    if settings.output_dir.trim().is_empty() {
        return Err("请选择输出目录".to_string());
    }
    if settings.max_side < 1 || settings.max_side > 50000 {
        return Err("最长边必须在 1 到 50000 之间".to_string());
    }
    if settings.quality < 1 || settings.quality > 100 {
        return Err("质量必须在 1 到 100 之间".to_string());
    }
    if settings.concurrency < 1 || settings.concurrency > 128 {
        return Err("并发数必须在 1 到 128 之间".to_string());
    }
    Ok(())
}

fn run_batch(app: AppHandle, settings: BatchSettings, cancel: Arc<AtomicBool>) -> bool {
    let input_dir = PathBuf::from(&settings.input_dir);
    let output_dir = PathBuf::from(&settings.output_dir);

    let _ = fs::create_dir_all(&output_dir);

    let vips_app = match VipsApp::new("PicTrim", false) {
        Ok(app) => app,
        Err(err) => {
            emit_error(&app, format!("libvips 初始化失败: {err}"));
            return false;
        }
    };
    vips_app.concurrency_set(1);

    emit_progress(
        &app,
        BatchProgress {
            phase: "scanning".to_string(),
            message: Some("正在扫描文件".to_string()),
            ..BatchProgress::empty()
        },
    );

    let items = match collect_items(&input_dir, &settings, &cancel) {
        Ok(items) => items,
        Err(err) => {
            emit_error(&app, err);
            return false;
        }
    };

    let discovered = items.len();
    emit_progress(
        &app,
        BatchProgress {
            phase: "processing".to_string(),
            discovered,
            message: Some(format!("发现 {discovered} 个待处理文件")),
            ..BatchProgress::empty()
        },
    );

    let counters = Arc::new(Mutex::new(Counters::default()));
    let pool = match ThreadPoolBuilder::new()
        .num_threads(settings.concurrency)
        .build()
    {
        Ok(pool) => pool,
        Err(err) => {
            emit_error(&app, format!("创建处理线程池失败: {err}"));
            return false;
        }
    };

    let cancelled = cancel.clone();
    pool.install(|| {
        items.par_iter().for_each(|item| {
            if cancelled.load(Ordering::Relaxed) {
                return;
            }

            let result = process_item(&input_dir, &output_dir, item, &settings);
            let mut counters = counters.lock().expect("counter mutex poisoned");
            apply_result(&mut counters, item, result);

            let current = item.rel.to_string_lossy().to_string();
            let should_emit = counters.processed % 20 == 0 || counters.processed == discovered;
            if should_emit {
                emit_progress(
                    &app,
                    BatchProgress {
                        phase: "processing".to_string(),
                        discovered,
                        processed: counters.processed,
                        images: counters.images,
                        copied: counters.copied,
                        skipped: counters.skipped,
                        failed: counters.failed,
                        total_src_bytes: counters.total_src_bytes,
                        total_dst_bytes: counters.total_dst_bytes,
                        current: Some(current),
                        message: None,
                        done: false,
                        cancelled: false,
                    },
                );
            }
        });
    });

    let counters = counters.lock().expect("counter mutex poisoned");
    let is_cancelled = cancel.load(Ordering::Relaxed);
    emit_progress(
        &app,
        BatchProgress {
            phase: "done".to_string(),
            discovered,
            processed: counters.processed,
            images: counters.images,
            copied: counters.copied,
            skipped: counters.skipped,
            failed: counters.failed,
            total_src_bytes: counters.total_src_bytes,
            total_dst_bytes: counters.total_dst_bytes,
            current: None,
            message: Some(if is_cancelled {
                "已停止任务".to_string()
            } else {
                "处理完成".to_string()
            }),
            done: true,
            cancelled: is_cancelled,
        },
    );
    let _ = app.emit("batch-failures", counters.failures.clone());

    is_cancelled
}

fn collect_items(
    input_dir: &Path,
    settings: &BatchSettings,
    cancel: &Arc<AtomicBool>,
) -> Result<Vec<WorkItem>, String> {
    let mut items = Vec::new();
    for entry in WalkDir::new(input_dir).into_iter().filter_entry(|entry| {
        if entry.depth() == 0 {
            return true;
        }
        !is_hidden_name(entry.file_name())
    }) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        let entry = entry.map_err(|err| err.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }

        let src = entry.path().to_path_buf();
        let rel = src
            .strip_prefix(input_dir)
            .map_err(|err| err.to_string())?
            .to_path_buf();
        if rel.components().any(|part| is_hidden_name(part.as_os_str())) {
            continue;
        }

        let is_image = is_supported_image(&src);
        if is_image || settings.copy_non_images {
            items.push(WorkItem { src, rel, is_image });
        }
    }
    Ok(items)
}

#[derive(Debug)]
enum ItemResult {
    Image { src_bytes: u64, dst_bytes: u64 },
    Copied { src_bytes: u64, dst_bytes: u64 },
    Skipped,
    Failed(String),
}

fn process_item(
    input_dir: &Path,
    output_dir: &Path,
    item: &WorkItem,
    settings: &BatchSettings,
) -> ItemResult {
    let dst = if item.is_image {
        output_path(input_dir, output_dir, &item.src, &item.rel, settings.output_format)
    } else {
        output_dir.join(&item.rel)
    };

    if settings.skip_existing && dst.exists() {
        return ItemResult::Skipped;
    }

    if let Some(parent) = dst.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return ItemResult::Failed(err.to_string());
        }
    }

    if item.is_image {
        match resize_image(&item.src, &dst, settings) {
            Ok((src_bytes, dst_bytes)) => ItemResult::Image {
                src_bytes,
                dst_bytes,
            },
            Err(err) => ItemResult::Failed(err),
        }
    } else {
        match fs::copy(&item.src, &dst) {
            Ok(dst_bytes) => {
                let src_bytes = file_size(&item.src);
                ItemResult::Copied {
                    src_bytes,
                    dst_bytes,
                }
            }
            Err(err) => ItemResult::Failed(err.to_string()),
        }
    }
}

fn resize_image(src: &Path, dst: &Path, settings: &BatchSettings) -> Result<(u64, u64), String> {
    let src_bytes = file_size(src);
    let mut image = VipsImage::new_from_file(src.to_string_lossy().as_ref())
        .map_err(|err| format!("读取失败: {err}"))?;

    let width = image.get_width();
    let height = image.get_height();
    let largest = width.max(height);
    if largest > settings.max_side {
        let scale = settings.max_side as f64 / largest as f64;
        image = ops::resize(&image, scale).map_err(|err| format!("缩放失败: {err}"))?;
    }

    let format = effective_output_format(src, settings.output_format);
    if format == OutputFormat::Jpg && image_has_alpha(&image) {
        let mut opts = ops::FlattenOptions::default();
        opts.background = vec![255.0, 255.0, 255.0];
        image = ops::flatten_with_opts(&image, &opts).map_err(|err| format!("透明背景处理失败: {err}"))?;
    }

    let save_path = save_path_with_options(dst, format, settings.quality);
    image
        .image_write_to_file(&save_path)
        .map_err(|err| format!("写入失败: {err}"))?;

    Ok((src_bytes, file_size(dst)))
}

fn apply_result(counters: &mut Counters, item: &WorkItem, result: ItemResult) {
    counters.processed += 1;
    match result {
        ItemResult::Image {
            src_bytes,
            dst_bytes,
        } => {
            counters.images += 1;
            counters.total_src_bytes += src_bytes;
            counters.total_dst_bytes += dst_bytes;
        }
        ItemResult::Copied {
            src_bytes,
            dst_bytes,
        } => {
            counters.copied += 1;
            counters.total_src_bytes += src_bytes;
            counters.total_dst_bytes += dst_bytes;
        }
        ItemResult::Skipped => {
            counters.skipped += 1;
        }
        ItemResult::Failed(message) => {
            counters.failed += 1;
            counters.failures.push(FailureEntry {
                rel: item.rel.to_string_lossy().to_string(),
                message,
            });
        }
    }
}

fn output_path(
    _input_dir: &Path,
    output_dir: &Path,
    _src: &Path,
    rel: &Path,
    format: OutputFormat,
) -> PathBuf {
    let mut dst = output_dir.join(rel);
    match format {
        OutputFormat::Jpg => {
            dst.set_extension("jpg");
        }
        OutputFormat::Png => {
            dst.set_extension("png");
        }
        OutputFormat::Webp => {
            dst.set_extension("webp");
        }
        OutputFormat::Keep => {}
    }
    dst
}

fn image_has_alpha(image: &VipsImage) -> bool {
    matches!(image.get_bands(), 2 | 4)
}

fn effective_output_format(src: &Path, requested: OutputFormat) -> OutputFormat {
    if requested != OutputFormat::Keep {
        return requested;
    }
    match src
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => OutputFormat::Png,
        Some("webp") => OutputFormat::Webp,
        _ => OutputFormat::Jpg,
    }
}

fn save_path_with_options(dst: &Path, format: OutputFormat, quality: i32) -> String {
    let path = dst.to_string_lossy();
    match format {
        OutputFormat::Jpg => format!("{path}[Q={quality},strip,interlace]"),
        OutputFormat::Webp => format!("{path}[Q={quality},strip]"),
        OutputFormat::Png => format!("{path}[compression=6,strip]"),
        OutputFormat::Keep => path.to_string(),
    }
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| IMAGE_EXTS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_hidden_name(name: &OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn emit_progress(app: &AppHandle, progress: BatchProgress) {
    let _ = app.emit("batch-progress", progress);
}

fn emit_error(app: &AppHandle, message: String) {
    let _ = app.emit(
        "batch-progress",
        BatchProgress {
            phase: "error".to_string(),
            message: Some(message),
            done: true,
            ..BatchProgress::empty()
        },
    );
}

impl BatchProgress {
    fn empty() -> Self {
        Self {
            phase: String::new(),
            discovered: 0,
            processed: 0,
            images: 0,
            copied: 0,
            skipped: 0,
            failed: 0,
            total_src_bytes: 0,
            total_dst_bytes: 0,
            current: None,
            message: None,
            done: false,
            cancelled: false,
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![start_batch, cancel_batch])
        .run(tauri::generate_context!())
        .expect("error while running PicTrim");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_images_case_insensitively() {
        assert!(is_supported_image(Path::new("a.JPG")));
        assert!(is_supported_image(Path::new("a.webp")));
        assert!(!is_supported_image(Path::new("a.txt")));
    }

    #[test]
    fn maps_output_extension() {
        let output = output_path(
            Path::new("/in"),
            Path::new("/out"),
            Path::new("/in/a/b.png"),
            Path::new("a/b.png"),
            OutputFormat::Webp,
        );
        assert_eq!(output, PathBuf::from("/out/a/b.webp"));
    }

    #[test]
    fn keeps_output_extension_when_requested() {
        let output = output_path(
            Path::new("/in"),
            Path::new("/out"),
            Path::new("/in/a/b.png"),
            Path::new("a/b.png"),
            OutputFormat::Keep,
        );
        assert_eq!(output, PathBuf::from("/out/a/b.png"));
    }

    #[test]
    fn hidden_names_start_with_dot() {
        assert!(is_hidden_name(OsStr::new(".git")));
        assert!(!is_hidden_name(OsStr::new("photos")));
    }
}

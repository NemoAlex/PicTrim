use rs_vips::{bindings, Vips, VipsImage, voption::{call, call_option_string, Setter, VOption}};
use rayon::ThreadPoolBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use walkdir::WalkDir;

const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "gif", "jfif",
];
const PROGRESS_INTERVAL_MS: u64 = 100;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static VIPS_INIT: OnceLock<bool> = OnceLock::new();

fn vips_error_detail() -> String {
    Vips::error_buffer()
        .ok()
        .map(|detail| detail.trim().to_string())
        .filter(|detail| !detail.is_empty())
        .unwrap_or_default()
}

fn ensure_vips() -> Result<(), String> {
    let initialized = VIPS_INIT.get_or_init(|| {
        if Vips::init("PicTrim").is_ok() {
            Vips::concurrency_set(1);
            true
        } else {
            false
        }
    });
    if *initialized {
        Ok(())
    } else {
        Err("libvips 初始化失败".to_string())
    }
}

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
    dst: PathBuf,
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
    processed: AtomicUsize,
    images: AtomicUsize,
    copied: AtomicUsize,
    skipped: AtomicUsize,
    failed: AtomicUsize,
    total_src_bytes: AtomicU64,
    total_dst_bytes: AtomicU64,
    failures: Mutex<Vec<FailureEntry>>,
}

impl Counters {
    fn snapshot(
        &self,
        phase: &str,
        discovered: usize,
        current: Option<String>,
        message: Option<String>,
        done: bool,
        cancelled: bool,
    ) -> BatchProgress {
        BatchProgress {
            phase: phase.to_string(),
            discovered,
            processed: self.processed.load(Ordering::Relaxed),
            images: self.images.load(Ordering::Relaxed),
            copied: self.copied.load(Ordering::Relaxed),
            skipped: self.skipped.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            total_src_bytes: self.total_src_bytes.load(Ordering::Relaxed),
            total_dst_bytes: self.total_dst_bytes.load(Ordering::Relaxed),
            current,
            message,
            done,
            cancelled,
        }
    }
}

#[tauri::command]
fn start_batch(
    app: AppHandle,
    state: tauri::State<AppState>,
    settings: BatchSettings,
) -> Result<(), String> {
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
        run_batch(app_for_thread.clone(), settings, cancel);
        if let Some(state) = app_for_thread.try_state::<AppState>() {
            if let Ok(mut current_job) = state.current_job.lock() {
                *current_job = None;
            }
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
    if output_inside_input(input, Path::new(&settings.output_dir)) {
        return Err("输出目录不能位于输入目录内部，请另选位置".to_string());
    }
    Ok(())
}

fn output_inside_input(input: &Path, output: &Path) -> bool {
    let input = fs::canonicalize(input).unwrap_or_else(|_| input.to_path_buf());
    let output = resolve_existing_ancestor(output);
    output != input && output.starts_with(&input)
}

fn resolve_existing_ancestor(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(resolved) = fs::canonicalize(&current) {
            let mut result = resolved;
            for part in tail.iter().rev() {
                result.push(part);
            }
            return result;
        }
        match (current.file_name(), current.parent()) {
            (Some(name), Some(parent)) => {
                tail.push(name.to_os_string());
                current = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

fn run_batch(app: AppHandle, settings: BatchSettings, cancel: Arc<AtomicBool>) -> bool {
    let input_dir = PathBuf::from(&settings.input_dir);
    let output_dir = PathBuf::from(&settings.output_dir);

    let _ = fs::create_dir_all(&output_dir);

    if let Err(err) = ensure_vips() {
        emit_error(&app, err);
        return false;
    }

    emit_progress(
        &app,
        BatchProgress {
            phase: "processing".to_string(),
            message: Some("正在扫描并处理文件".to_string()),
            ..BatchProgress::empty()
        },
    );

    let counters = Arc::new(Counters::default());
    let discovered = Arc::new(AtomicUsize::new(0));
    let last_emit = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let check_collisions = settings.output_format != OutputFormat::Keep;
    let collision_seen: Arc<Mutex<HashMap<PathBuf, PathBuf>>> =
        Arc::new(Mutex::new(HashMap::new()));

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

    let (tx, rx) = std::sync::mpsc::sync_channel(settings.concurrency * 4);

    let cancel_producer = cancel.clone();
    let input_dir_producer = input_dir.clone();
    let output_dir_producer = output_dir.clone();
    let settings_producer = settings.clone();
    let producer = std::thread::spawn(move || {
        for entry in WalkDir::new(&input_dir_producer)
            .into_iter()
            .filter_entry(|entry| {
                if entry.depth() == 0 {
                    return true;
                }
                !is_hidden_name(entry.file_name())
            })
        {
            if cancel_producer.load(Ordering::Relaxed) {
                break;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() {
                continue;
            }
            if is_temp_output_name(entry.file_name()) {
                continue;
            }
            let src = entry.path().to_path_buf();
            let rel = match src.strip_prefix(&input_dir_producer) {
                Ok(r) => r.to_path_buf(),
                Err(_) => continue,
            };
            if rel
                .components()
                .any(|part| is_hidden_name(part.as_os_str()))
            {
                continue;
            }
            let is_image = is_supported_image(&src);
            if is_image || settings_producer.copy_non_images {
                let dst = if is_image {
                    output_path(&output_dir_producer, &rel, settings_producer.output_format)
                } else {
                    output_dir_producer.join(&rel)
                };
                if tx.send(WorkItem {
                    src,
                    rel,
                    dst,
                    is_image,
                })
                .is_err()
                {
                    break;
                }
            }
        }
    });

    let cancelled = cancel.clone();
    let app_done = app.clone();
    let counters_done = counters.clone();
    let discovered_done = discovered.clone();

    pool.install(move || {
        rayon::scope(move |s| {
            while let Ok(item) = rx.recv() {
                if cancelled.load(Ordering::Relaxed) {
                    break;
                }

                discovered.fetch_add(1, Ordering::Relaxed);

                if check_collisions {
                    let mut seen = collision_seen.lock().expect("collision map poisoned");
                    if let Some(previous) = seen.insert(item.dst.clone(), item.rel.clone()) {
                        emit_error(
                            &app,
                            format!(
                                "输出路径冲突: {} 和 {} 都会写入 {}。请改用保持原格式，或调整输入文件名。",
                                previous.to_string_lossy(),
                                item.rel.to_string_lossy(),
                                item.dst.to_string_lossy()
                            ),
                        );
                        drop(seen);
                        cancelled.store(true, Ordering::Relaxed);
                        break;
                    }
                }

                let counters = counters.clone();
                let discovered = discovered.clone();
                let last_emit = last_emit.clone();
                let cancelled = cancelled.clone();
                let app = app.clone();
                let settings = settings.clone();

                s.spawn(move |_| {
                    if cancelled.load(Ordering::Relaxed) {
                        return;
                    }
                    let result = process_item(&item, &settings);
                    apply_result(&counters, &item, result);

                    let elapsed = start.elapsed().as_millis() as u64;
                    let last = last_emit.load(Ordering::Relaxed);
                    if elapsed.saturating_sub(last) >= PROGRESS_INTERVAL_MS
                        && last_emit
                            .compare_exchange(
                                last,
                                elapsed,
                                Ordering::Relaxed,
                                Ordering::Relaxed,
                            )
                            .is_ok()
                    {
                        emit_progress(
                            &app,
                            counters.snapshot(
                                "processing",
                                discovered.load(Ordering::Relaxed),
                                Some(item.rel.to_string_lossy().to_string()),
                                None,
                                false,
                                false,
                            ),
                        );
                    }
                });
            }
        });
    });

    let _ = producer.join();

    let is_cancelled = cancel.load(Ordering::Relaxed);
    let discovered = discovered_done.load(Ordering::Relaxed);
    emit_progress(
        &app_done,
        counters_done.snapshot(
            "done",
            discovered,
            None,
            Some(if is_cancelled {
                "已停止任务".to_string()
            } else {
                "处理完成".to_string()
            }),
            true,
            is_cancelled,
        ),
    );
    let failures = counters_done
        .failures
        .lock()
        .map(|failures| failures.clone())
        .unwrap_or_default();
    let _ = app_done.emit("batch-failures", failures);

    is_cancelled
}

#[derive(Debug)]
enum ItemResult {
    Image { src_bytes: u64, dst_bytes: u64 },
    Copied { src_bytes: u64, dst_bytes: u64 },
    Skipped,
    Failed(String),
}

fn process_item(item: &WorkItem, settings: &BatchSettings) -> ItemResult {
    let dst = &item.dst;

    if settings.skip_existing && dst.exists() {
        return ItemResult::Skipped;
    }

    if let Some(parent) = dst.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return ItemResult::Failed(err.to_string());
        }
    }

    if item.is_image {
        match resize_image(&item.src, dst, settings) {
            Ok((src_bytes, dst_bytes)) => ItemResult::Image {
                src_bytes,
                dst_bytes,
            },
            Err(err) => ItemResult::Failed(err),
        }
    } else {
        if same_file_path(&item.src, dst) {
            return ItemResult::Skipped;
        }
        match fs::copy(&item.src, dst) {
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
    let temp = temp_output_path(dst);
    {
        let load_source = thumbnail_load_source(src, settings.output_format);
        let mut out = VipsImage::from(null_mut() as *mut bindings::VipsImage);
        let result = call_option_string("thumbnail", "", VOption::new()
            .set("filename", load_source.as_str())
            .set("width", settings.max_side)
            .set("height", settings.max_side)
            .set("size", "down")
            .set("no-rotate", false)
            .set("import-profile", "srgb")
            .set("export-profile", "srgb")
            .set("out", &mut out))
            .map_err(|e| format!("读取失败: {e} {}", vips_error_detail()))?;
        if result < 0 {
            return Err(format!("读取失败: {}", vips_error_detail()));
        }
        let mut image = out;

        let format = effective_output_format(src, settings.output_format);
        if format == OutputFormat::Jpg && image.hasalpha() {
            let mut flattened = VipsImage::from(null_mut() as *mut bindings::VipsImage);
            let bg = [255.0_f64, 255.0, 255.0];
            let result = call("flatten", VOption::new()
                .set("in", &image)
                .set("background", &bg[..])
                .set("out", &mut flattened))
                .map_err(|e| format!("透明背景处理失败: {e} {}", vips_error_detail()))?;
            if result < 0 {
                return Err(format!("透明背景处理失败: {}", vips_error_detail()));
            }
            image = flattened;
        }

        let save_path = save_path_with_options(&temp, format, settings.quality);
        image.write_to_file(&save_path).map_err(|err| {
            let _ = fs::remove_file(&temp);
            format!("写入失败: {err}")
        })?;
    }

    replace_file(&temp, dst).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("替换失败: {err}")
    })?;
    Ok((src_bytes, file_size(dst)))
}

fn apply_result(counters: &Counters, item: &WorkItem, result: ItemResult) {
    counters.processed.fetch_add(1, Ordering::Relaxed);
    match result {
        ItemResult::Image {
            src_bytes,
            dst_bytes,
        } => {
            counters.images.fetch_add(1, Ordering::Relaxed);
            counters.total_src_bytes.fetch_add(src_bytes, Ordering::Relaxed);
            counters.total_dst_bytes.fetch_add(dst_bytes, Ordering::Relaxed);
        }
        ItemResult::Copied {
            src_bytes,
            dst_bytes,
        } => {
            counters.copied.fetch_add(1, Ordering::Relaxed);
            counters.total_src_bytes.fetch_add(src_bytes, Ordering::Relaxed);
            counters.total_dst_bytes.fetch_add(dst_bytes, Ordering::Relaxed);
        }
        ItemResult::Skipped => {
            counters.skipped.fetch_add(1, Ordering::Relaxed);
        }
        ItemResult::Failed(message) => {
            counters.failed.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut failures) = counters.failures.lock() {
                failures.push(FailureEntry {
                    rel: item.rel.to_string_lossy().to_string(),
                    message,
                });
            }
        }
    }
}

fn output_path(output_dir: &Path, rel: &Path, format: OutputFormat) -> PathBuf {
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

fn temp_output_path(dst: &Path) -> PathBuf {
    let parent = dst.parent().unwrap_or_else(|| Path::new("."));
    let stem = dst
        .file_stem()
        .and_then(OsStr::to_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("pictrim");
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = match dst.extension().and_then(OsStr::to_str) {
        Some(ext) if !ext.is_empty() => {
            format!("{stem}.pictrim-tmp-{}-{count}.{ext}", std::process::id())
        }
        _ => format!("{stem}.pictrim-tmp-{}-{count}", std::process::id()),
    };
    parent.join(name)
}

fn replace_file(temp: &Path, dst: &Path) -> std::io::Result<()> {
    match fs::rename(temp, dst) {
        Ok(()) => Ok(()),
        Err(first_err) => {
            if dst.exists() {
                fs::remove_file(dst)?;
                fs::rename(temp, dst)
            } else {
                Err(first_err)
            }
        }
    }
}

fn same_file_path(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
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

fn thumbnail_load_source(src: &Path, requested: OutputFormat) -> String {
    let path = src.to_string_lossy();
    if loads_all_pages(src, requested) {
        format!("{path}[n=-1]")
    } else {
        path.to_string()
    }
}

fn loads_all_pages(src: &Path, requested: OutputFormat) -> bool {
    let ext = src
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase());
    let ext = match ext.as_deref() {
        Some(ext) => ext,
        None => return false,
    };
    let is_animated_container = matches!(ext, "gif" | "webp" | "tif" | "tiff");
    if !is_animated_container {
        return false;
    }
    match requested {
        OutputFormat::Keep => true,
        OutputFormat::Webp => ext == "webp",
        OutputFormat::Jpg | OutputFormat::Png => false,
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

fn is_temp_output_name(name: &OsStr) -> bool {
    name.to_string_lossy().contains(".pictrim-tmp-")
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
        let output = output_path(Path::new("/out"), Path::new("a/b.png"), OutputFormat::Webp);
        assert_eq!(output, PathBuf::from("/out/a/b.webp"));
    }

    #[test]
    fn keeps_output_extension_when_requested() {
        let output = output_path(Path::new("/out"), Path::new("a/b.png"), OutputFormat::Keep);
        assert_eq!(output, PathBuf::from("/out/a/b.png"));
    }

    #[test]
    fn rejects_output_inside_input_but_allows_others() {
        let base = std::env::temp_dir().join(format!(
            "pictrim-io-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let input = base.join("input");
        let nested = input.join("out");
        let sibling = base.join("output");
        fs::create_dir_all(&input).unwrap();

        assert!(output_inside_input(&input, &nested));
        assert!(!output_inside_input(&input, &sibling));
        assert!(!output_inside_input(&input, &input));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn temp_output_path_keeps_extension_for_vips_format_detection() {
        let temp = temp_output_path(Path::new("/out/a/photo.jpg"));
        assert_eq!(temp.extension().and_then(OsStr::to_str), Some("jpg"));
        assert!(temp
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap()
            .contains(".pictrim-tmp-"));
    }

    #[test]
    fn hidden_names_start_with_dot() {
        assert!(is_hidden_name(OsStr::new(".git")));
        assert!(!is_hidden_name(OsStr::new("photos")));
    }

    #[test]
    fn recognizes_pictrim_temp_outputs() {
        assert!(is_temp_output_name(OsStr::new(
            "photo.pictrim-tmp-123-0.jpg"
        )));
        assert!(!is_temp_output_name(OsStr::new("photo.jpg")));
    }

    #[test]
    fn resize_can_replace_source_file_through_temp_output() {
        Vips::init("PicTrimTest").expect("initialize libvips");
        Vips::concurrency_set(1);

        let dir = std::env::temp_dir().join(format!(
            "pictrim-test-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let image_path = dir.join("source.png");
        fs::write(&image_path, ONE_BY_ONE_PNG).unwrap();

        let settings = BatchSettings {
            input_dir: dir.to_string_lossy().to_string(),
            output_dir: dir.to_string_lossy().to_string(),
            max_side: 1,
            quality: 85,
            concurrency: 1,
            output_format: OutputFormat::Keep,
            copy_non_images: false,
            skip_existing: false,
        };

        let result = resize_image(&image_path, &image_path, &settings);
        let _ = fs::remove_dir_all(&dir);
        assert!(result.is_ok(), "{result:?}");
    }

    const ONE_BY_ONE_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4,
        0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 31, 0, 3, 3,
        2, 0, 239, 191, 167, 219, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
}

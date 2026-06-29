use rayon::ThreadPoolBuilder;
use rs_vips::{
    bindings,
    voption::{call, call_option_string, Setter, VOption},
    Vips, VipsImage,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchSettings {
    input_sources: Vec<String>,
    output_dir: String,
    resize_mode: ResizeMode,
    max_side: i32,
    width: i32,
    height: i32,
    allow_upscale: bool,
    crop_horizontal: CropHorizontal,
    crop_vertical: CropVertical,
    rotation: Rotation,
    thumbnail: bool,
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum ResizeMode {
    FitLongestSide,
    FitBox,
    FitWidth,
    FitHeight,
    FixedCrop,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum CropHorizontal {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum CropVertical {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Rotation {
    Auto,
    Rotate0,
    Rotate90,
    Rotate180,
    Rotate270,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum SourceKind {
    File,
    Directory,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceEntry {
    path: String,
    kind: SourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourcePathKind {
    File,
    Directory,
}

#[derive(Debug, Clone)]
struct InputSource {
    path: PathBuf,
    kind: SourcePathKind,
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

#[tauri::command]
fn classify_sources(paths: Vec<String>) -> Vec<SourceEntry> {
    paths
        .into_iter()
        .map(|path| {
            let kind = classify_path(Path::new(&path))
                .map(|kind| match kind {
                    SourcePathKind::File => SourceKind::File,
                    SourcePathKind::Directory => SourceKind::Directory,
                })
                .unwrap_or(SourceKind::Missing);
            SourceEntry { path, kind }
        })
        .collect()
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<Option<BatchSettings>, String> {
    let path = settings_path(&app)?;
    let data = match fs::read_to_string(&path) {
        Ok(data) => data,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("读取设置失败: {err}")),
    };
    let value: serde_json::Value =
        serde_json::from_str(&data).map_err(|err| format!("解析设置失败: {err}"))?;
    let migrated = migrate_settings_value(value);
    serde_json::from_value(migrated)
        .map(Some)
        .map_err(|err| format!("加载设置失败: {err}"))
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: BatchSettings) -> Result<(), String> {
    let path = settings_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建设置目录失败: {err}"))?;
    }
    let data =
        serde_json::to_string_pretty(&settings).map_err(|err| format!("序列化设置失败: {err}"))?;
    fs::write(path, data).map_err(|err| format!("保存设置失败: {err}"))
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("settings.json"))
        .map_err(|err| format!("定位设置目录失败: {err}"))
}

fn migrate_settings_value(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        if !object.contains_key("inputSources") {
            if let Some(input_dir) = object.remove("inputDir") {
                let sources = input_dir
                    .as_str()
                    .filter(|path| !path.is_empty())
                    .map(|path| vec![serde_json::Value::String(path.to_string())])
                    .unwrap_or_default();
                object.insert(
                    "inputSources".to_string(),
                    serde_json::Value::Array(sources),
                );
            }
        }
        object
            .entry("resizeMode")
            .or_insert_with(|| serde_json::Value::String("fitLongestSide".to_string()));
        object
            .entry("width")
            .or_insert_with(|| serde_json::Value::Number(2000.into()));
        object
            .entry("height")
            .or_insert_with(|| serde_json::Value::Number(2000.into()));
        object
            .entry("allowUpscale")
            .or_insert_with(|| serde_json::Value::Bool(false));
        object
            .entry("cropHorizontal")
            .or_insert_with(|| serde_json::Value::String("center".to_string()));
        object
            .entry("cropVertical")
            .or_insert_with(|| serde_json::Value::String("center".to_string()));
        object
            .entry("rotation")
            .or_insert_with(|| serde_json::Value::String("auto".to_string()));
        object
            .entry("thumbnail")
            .or_insert_with(|| serde_json::Value::Bool(false));
        if object
            .get("resizeMode")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value == "fillCrop")
        {
            object.insert(
                "resizeMode".to_string(),
                serde_json::Value::String("fixedCrop".to_string()),
            );
        }
    }
    value
}

fn validate_settings(settings: &BatchSettings) -> Result<(), String> {
    let sources = input_sources(settings)?;
    if sources.is_empty() {
        return Err("请选择输入来源".to_string());
    }
    if settings.output_dir.trim().is_empty() {
        return Err("请选择输出目录".to_string());
    }
    validate_dimensions(settings)?;
    if settings.max_side < 1 || settings.max_side > 50000 {
        return Err("最长边必须在 1 到 50000 之间".to_string());
    }
    if settings.quality < 1 || settings.quality > 100 {
        return Err("质量必须在 1 到 100 之间".to_string());
    }
    if settings.concurrency < 1 || settings.concurrency > 128 {
        return Err("并发数必须在 1 到 128 之间".to_string());
    }
    for source in sources
        .iter()
        .filter(|source| source.kind == SourcePathKind::Directory)
    {
        if output_inside_input(&source.path, Path::new(&settings.output_dir)) {
            return Err("输出目录不能位于输入目录内部，请另选位置".to_string());
        }
    }
    Ok(())
}

fn validate_dimensions(settings: &BatchSettings) -> Result<(), String> {
    match settings.resize_mode {
        ResizeMode::FitLongestSide => {
            if settings.max_side < 1 || settings.max_side > 50000 {
                return Err("最长边必须在 1 到 50000 之间".to_string());
            }
        }
        ResizeMode::FitBox | ResizeMode::FixedCrop => {
            if settings.width < 1 || settings.width > 50000 {
                return Err("宽度必须在 1 到 50000 之间".to_string());
            }
            if settings.height < 1 || settings.height > 50000 {
                return Err("高度必须在 1 到 50000 之间".to_string());
            }
        }
        ResizeMode::FitWidth => {
            if settings.width < 1 || settings.width > 50000 {
                return Err("宽度必须在 1 到 50000 之间".to_string());
            }
        }
        ResizeMode::FitHeight => {
            if settings.height < 1 || settings.height > 50000 {
                return Err("高度必须在 1 到 50000 之间".to_string());
            }
        }
    }
    Ok(())
}

fn input_sources(settings: &BatchSettings) -> Result<Vec<InputSource>, String> {
    let mut sources = Vec::with_capacity(settings.input_sources.len());
    for source in &settings.input_sources {
        let path = PathBuf::from(source);
        let kind = classify_path(&path).ok_or_else(|| format!("输入来源不存在: {source}"))?;
        sources.push(InputSource { path, kind });
    }
    Ok(sources)
}

fn classify_path(path: &Path) -> Option<SourcePathKind> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() => Some(SourcePathKind::Directory),
        Ok(meta) if meta.is_file() => Some(SourcePathKind::File),
        _ => None,
    }
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
    let output_dir = PathBuf::from(&settings.output_dir);
    let sources = match input_sources(&settings) {
        Ok(sources) => sources,
        Err(err) => {
            emit_error(&app, err);
            return false;
        }
    };
    let single_directory_source =
        sources.len() == 1 && sources[0].kind == SourcePathKind::Directory;

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
    let output_dir_producer = output_dir.clone();
    let settings_producer = settings.clone();
    let producer = std::thread::spawn(move || {
        for source in sources {
            if cancel_producer.load(Ordering::Relaxed) {
                break;
            }
            match source.kind {
                SourcePathKind::File => {
                    if send_file_source(
                        &tx,
                        &source.path,
                        file_rel(&source.path),
                        &output_dir_producer,
                        &settings_producer,
                    ) {
                        continue;
                    }
                    break;
                }
                SourcePathKind::Directory => {
                    let prefix = if single_directory_source {
                        None
                    } else {
                        source.path.file_name().map(PathBuf::from)
                    };
                    if !send_directory_source(
                        &tx,
                        &source.path,
                        prefix.as_deref(),
                        &output_dir_producer,
                        &settings_producer,
                        &cancel_producer,
                    ) {
                        break;
                    }
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

                let mut seen = collision_seen.lock().expect("collision map poisoned");
                if let Some(previous) = seen.insert(item.dst.clone(), item.rel.clone()) {
                    emit_error(
                        &app,
                        format!(
                            "输出路径冲突: {} 和 {} 都会写入 {}。请调整输入文件名或输出位置。",
                            previous.to_string_lossy(),
                            item.rel.to_string_lossy(),
                            item.dst.to_string_lossy()
                        ),
                    );
                    drop(seen);
                    cancelled.store(true, Ordering::Relaxed);
                    break;
                }
                drop(seen);

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
                            .compare_exchange(last, elapsed, Ordering::Relaxed, Ordering::Relaxed)
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

fn send_directory_source(
    tx: &std::sync::mpsc::SyncSender<WorkItem>,
    root: &Path,
    prefix: Option<&Path>,
    output_dir: &Path,
    settings: &BatchSettings,
    cancel: &AtomicBool,
) -> bool {
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        if entry.depth() == 0 {
            return true;
        }
        !is_hidden_name(entry.file_name())
    }) {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if is_temp_output_name(entry.file_name()) {
            continue;
        }
        let src = entry.path().to_path_buf();
        let inner_rel = match src.strip_prefix(root) {
            Ok(rel) => rel,
            Err(_) => continue,
        };
        if inner_rel
            .components()
            .any(|part| is_hidden_name(part.as_os_str()))
        {
            continue;
        }
        let rel = match prefix {
            Some(prefix) => prefix.join(inner_rel),
            None => inner_rel.to_path_buf(),
        };
        if !send_file_source(tx, &src, rel, output_dir, settings) {
            return false;
        }
    }
    true
}

fn send_file_source(
    tx: &std::sync::mpsc::SyncSender<WorkItem>,
    src: &Path,
    rel: PathBuf,
    output_dir: &Path,
    settings: &BatchSettings,
) -> bool {
    if is_hidden_name(src.file_name().unwrap_or_else(|| OsStr::new(""))) {
        return true;
    }
    if is_temp_output_name(src.file_name().unwrap_or_else(|| OsStr::new(""))) {
        return true;
    }
    let is_image = is_supported_image(src);
    if !is_image && !settings.copy_non_images {
        return true;
    }
    let dst = if is_image {
        output_path(output_dir, &rel, settings.output_format)
    } else {
        output_dir.join(&rel)
    };
    tx.send(WorkItem {
        src: src.to_path_buf(),
        rel,
        dst,
        is_image,
    })
    .is_ok()
}

fn file_rel(path: &Path) -> PathBuf {
    path.file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("file"))
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
        let mut image = match settings.resize_mode {
            ResizeMode::FitLongestSide | ResizeMode::FitBox => {
                thumbnail_image(&load_source, settings)?
            }
            ResizeMode::FitWidth | ResizeMode::FitHeight | ResizeMode::FixedCrop => {
                manual_resize_image(src, settings)?
            }
        };

        let format = effective_output_format(src, settings.output_format);
        if format == OutputFormat::Jpg && image.hasalpha() {
            let mut flattened = VipsImage::from(null_mut() as *mut bindings::VipsImage);
            let bg = [255.0_f64, 255.0, 255.0];
            let result = call(
                "flatten",
                VOption::new()
                    .set("in", &image)
                    .set("background", &bg[..])
                    .set("out", &mut flattened),
            )
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

#[derive(Debug, Clone, Copy)]
struct ResizeTarget {
    width: i32,
    height: i32,
    size: &'static str,
}

fn resize_target(settings: &BatchSettings) -> ResizeTarget {
    let (width, height) = match settings.resize_mode {
        ResizeMode::FitLongestSide => (settings.max_side, settings.max_side),
        ResizeMode::FitBox
        | ResizeMode::FitWidth
        | ResizeMode::FitHeight
        | ResizeMode::FixedCrop => (settings.width, settings.height),
    };
    ResizeTarget {
        width,
        height,
        size: if settings.allow_upscale {
            "both"
        } else {
            "down"
        },
    }
}

fn thumbnail_image(load_source: &str, settings: &BatchSettings) -> Result<VipsImage, String> {
    let target = resize_target(settings);
    let mut out = VipsImage::from(null_mut() as *mut bindings::VipsImage);
    let options = VOption::new()
        .set("filename", load_source)
        .set("width", target.width)
        .set("height", target.height)
        .set("size", target.size)
        .set("no-rotate", settings.rotation != Rotation::Auto)
        .set("import-profile", "srgb")
        .set("export-profile", "srgb")
        .set("out", &mut out);
    let result = call_option_string("thumbnail", "", options)
        .map_err(|e| format!("读取失败: {e} {}", vips_error_detail()))?;
    if result < 0 {
        return Err(format!("读取失败: {}", vips_error_detail()));
    }
    rotate_image(out, settings.rotation)
}

fn manual_resize_image(src: &Path, settings: &BatchSettings) -> Result<VipsImage, String> {
    let mut image =
        VipsImage::new_from_file(src).map_err(|e| format!("读取失败: {e} {}", vips_error_detail()))?;
    if settings.rotation == Rotation::Auto {
        image = image
            .autorot()
            .map_err(|e| format!("EXIF方向校正失败: {e} {}", vips_error_detail()))?;
    } else {
        image = rotate_image(image, settings.rotation)?;
    }

    let src_width = image.get_width().max(1) as f64;
    let src_height = image.get_height().max(1) as f64;
    let scale = match settings.resize_mode {
        ResizeMode::FitWidth => settings.width as f64 / src_width,
        ResizeMode::FitHeight => settings.height as f64 / src_height,
        ResizeMode::FixedCrop => {
            let scale_x = settings.width as f64 / src_width;
            let scale_y = settings.height as f64 / src_height;
            scale_x.max(scale_y) + 0.000001
        }
        ResizeMode::FitLongestSide | ResizeMode::FitBox => 1.0,
    };
    let scale = if settings.resize_mode == ResizeMode::FixedCrop || settings.allow_upscale {
        scale
    } else {
        scale.min(1.0)
    };
    if (scale - 1.0).abs() > f64::EPSILON {
        image = image
            .resize(scale)
            .map_err(|e| format!("缩放失败: {e} {}", vips_error_detail()))?;
    }

    if settings.resize_mode == ResizeMode::FixedCrop {
        image = crop_to_target(image, settings)?;
    }
    Ok(image)
}

fn crop_to_target(image: VipsImage, settings: &BatchSettings) -> Result<VipsImage, String> {
    let width = image.get_width();
    let height = image.get_height();
    let target_width = settings.width.min(width).max(1);
    let target_height = settings.height.min(height).max(1);
    let left = crop_offset(width - target_width, settings.crop_horizontal);
    let top = crop_offset(height - target_height, settings.crop_vertical);
    image
        .crop(left, top, target_width, target_height)
        .map_err(|e| format!("裁剪失败: {e} {}", vips_error_detail()))
}

fn crop_offset(extra: i32, position: impl CropPosition) -> i32 {
    match position.anchor() {
        CropAnchor::Start => 0,
        CropAnchor::Center => extra / 2,
        CropAnchor::End => extra,
    }
    .max(0)
}

#[derive(Debug, Clone, Copy)]
enum CropAnchor {
    Start,
    Center,
    End,
}

trait CropPosition {
    fn anchor(self) -> CropAnchor;
}

impl CropPosition for CropHorizontal {
    fn anchor(self) -> CropAnchor {
        match self {
            CropHorizontal::Left => CropAnchor::Start,
            CropHorizontal::Center => CropAnchor::Center,
            CropHorizontal::Right => CropAnchor::End,
        }
    }
}

impl CropPosition for CropVertical {
    fn anchor(self) -> CropAnchor {
        match self {
            CropVertical::Top => CropAnchor::Start,
            CropVertical::Center => CropAnchor::Center,
            CropVertical::Bottom => CropAnchor::End,
        }
    }
}

fn rotate_image(image: VipsImage, rotation: Rotation) -> Result<VipsImage, String> {
    let angle = match rotation {
        Rotation::Auto | Rotation::Rotate0 => return Ok(image),
        Rotation::Rotate90 => "d90",
        Rotation::Rotate180 => "d180",
        Rotation::Rotate270 => "d270",
    };
    let mut rotated = VipsImage::from(null_mut() as *mut bindings::VipsImage);
    let result = call(
        "rot",
        VOption::new()
            .set("in", &image)
            .set("angle", angle)
            .set("out", &mut rotated),
    )
    .map_err(|e| format!("旋转失败: {e} {}", vips_error_detail()))?;
    if result < 0 {
        return Err(format!("旋转失败: {}", vips_error_detail()));
    }
    Ok(rotated)
}

fn apply_result(counters: &Counters, item: &WorkItem, result: ItemResult) {
    counters.processed.fetch_add(1, Ordering::Relaxed);
    match result {
        ItemResult::Image {
            src_bytes,
            dst_bytes,
        } => {
            counters.images.fetch_add(1, Ordering::Relaxed);
            counters
                .total_src_bytes
                .fetch_add(src_bytes, Ordering::Relaxed);
            counters
                .total_dst_bytes
                .fetch_add(dst_bytes, Ordering::Relaxed);
        }
        ItemResult::Copied {
            src_bytes,
            dst_bytes,
        } => {
            counters.copied.fetch_add(1, Ordering::Relaxed);
            counters
                .total_src_bytes
                .fetch_add(src_bytes, Ordering::Relaxed);
            counters
                .total_dst_bytes
                .fetch_add(dst_bytes, Ordering::Relaxed);
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
        .invoke_handler(tauri::generate_handler![
            start_batch,
            cancel_batch,
            classify_sources,
            load_settings,
            save_settings
        ])
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
    fn single_directory_source_keeps_legacy_relative_paths() {
        let base = temp_test_dir("single-source");
        let input = base.join("photos");
        let output = base.join("out");
        fs::create_dir_all(input.join("nested")).unwrap();
        fs::write(input.join("nested").join("a.jpg"), b"not-real").unwrap();

        let settings = test_settings(vec![input.to_string_lossy().to_string()], &output);
        let (tx, rx) = std::sync::mpsc::sync_channel(4);
        assert!(send_directory_source(
            &tx,
            &input,
            None,
            &output,
            &settings,
            &AtomicBool::new(false)
        ));
        drop(tx);

        let item = rx.recv().unwrap();
        assert_eq!(item.rel, PathBuf::from("nested/a.jpg"));
        assert_eq!(item.dst, output.join("nested/a.jpg"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn mixed_sources_wrap_directories_and_keep_files_at_root() {
        let base = temp_test_dir("mixed-source");
        let input = base.join("photos");
        let output = base.join("out");
        let loose = base.join("loose.jpg");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("a.jpg"), b"not-real").unwrap();
        fs::write(&loose, b"not-real").unwrap();

        let settings = test_settings(
            vec![
                input.to_string_lossy().to_string(),
                loose.to_string_lossy().to_string(),
            ],
            &output,
        );
        let (tx, rx) = std::sync::mpsc::sync_channel(4);
        assert!(send_directory_source(
            &tx,
            &input,
            Some(Path::new("photos")),
            &output,
            &settings,
            &AtomicBool::new(false)
        ));
        assert!(send_file_source(
            &tx,
            &loose,
            file_rel(&loose),
            &output,
            &settings
        ));
        drop(tx);

        let mut rels: Vec<PathBuf> = rx.iter().map(|item| item.rel).collect();
        rels.sort();
        assert_eq!(
            rels,
            vec![PathBuf::from("loose.jpg"), PathBuf::from("photos/a.jpg")]
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migrates_legacy_input_dir_setting() {
        let value = serde_json::json!({
            "inputDir": "/tmp/photos",
            "outputDir": "/tmp/out",
            "maxSide": 2000,
            "quality": 85,
            "concurrency": 4,
            "outputFormat": "keep",
            "copyNonImages": false,
            "skipExisting": true
        });

        let migrated = migrate_settings_value(value);
        let settings: BatchSettings = serde_json::from_value(migrated).unwrap();
        assert_eq!(settings.input_sources, vec!["/tmp/photos"]);
        assert_eq!(settings.resize_mode, ResizeMode::FitLongestSide);
        assert_eq!(settings.rotation, Rotation::Auto);
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
            input_sources: vec![dir.to_string_lossy().to_string()],
            output_dir: dir.to_string_lossy().to_string(),
            resize_mode: ResizeMode::FitLongestSide,
            max_side: 1,
            width: 1,
            height: 1,
            allow_upscale: false,
            crop_horizontal: CropHorizontal::Center,
            crop_vertical: CropVertical::Center,
            rotation: Rotation::Auto,
            thumbnail: false,
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

    fn temp_test_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pictrim-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn test_settings(input_sources: Vec<String>, output_dir: &Path) -> BatchSettings {
        BatchSettings {
            input_sources,
            output_dir: output_dir.to_string_lossy().to_string(),
            resize_mode: ResizeMode::FitLongestSide,
            max_side: 2000,
            width: 2000,
            height: 2000,
            allow_upscale: false,
            crop_horizontal: CropHorizontal::Center,
            crop_vertical: CropVertical::Center,
            rotation: Rotation::Auto,
            thumbnail: false,
            quality: 85,
            concurrency: 1,
            output_format: OutputFormat::Keep,
            copy_non_images: false,
            skip_existing: true,
        }
    }
}

mod pdf;

use base64::{engine::general_purpose, Engine as _};
use rayon::ThreadPoolBuilder;
use rs_vips::{
    bindings,
    enums::{BandFormat, Interpretation},
    voption::{call, call_option_string, Setter, VOption},
    Vips, VipsImage,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
const PREVIEW_MAX_SIDE: i32 = 1600;
const PDF_IMAGE_MEMORY_LIMIT: usize = 512 * 1024 * 1024;
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
    kind: WorkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkKind {
    RasterImage,
    Pdf,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewTree {
    items: Vec<PreviewItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewItem {
    path: String,
    rel: String,
    name: String,
    segments: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewDirectoryPage {
    dir_path: Option<String>,
    entries: Vec<PreviewDirectoryEntry>,
    next_offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewDirectoryEntry {
    kind: String,
    path: String,
    rel: String,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewPair {
    rel: String,
    before: PreviewImage,
    after: PreviewImage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewImage {
    data: String,
    mime: String,
    width: i32,
    height: i32,
    bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchProgress {
    phase: String,
    discovered: usize,
    processed: usize,
    images: usize,
    pdfs: usize,
    embedded_images: usize,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WarningEntry {
    rel: String,
    message: String,
}

#[derive(Default)]
struct Counters {
    processed: AtomicUsize,
    images: AtomicUsize,
    pdfs: AtomicUsize,
    embedded_images: AtomicUsize,
    copied: AtomicUsize,
    skipped: AtomicUsize,
    failed: AtomicUsize,
    total_src_bytes: AtomicU64,
    total_dst_bytes: AtomicU64,
    failures: Mutex<Vec<FailureEntry>>,
    warnings: Mutex<Vec<WarningEntry>>,
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
            pdfs: self.pdfs.load(Ordering::Relaxed),
            embedded_images: self.embedded_images.load(Ordering::Relaxed),
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
async fn load_preview_tree(settings: BatchSettings) -> Result<PreviewTree, String> {
    tauri::async_runtime::spawn_blocking(move || load_preview_tree_blocking(settings))
        .await
        .map_err(|err| format!("加载预览列表失败: {err}"))?
}

#[tauri::command]
async fn load_preview_directory(
    settings: BatchSettings,
    dir_path: Option<String>,
    offset: usize,
    limit: usize,
) -> Result<PreviewDirectoryPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        load_preview_directory_blocking(settings, dir_path, offset, limit)
    })
    .await
    .map_err(|err| format!("加载预览目录失败: {err}"))?
}

fn load_preview_directory_blocking(
    settings: BatchSettings,
    dir_path: Option<String>,
    offset: usize,
    limit: usize,
) -> Result<PreviewDirectoryPage, String> {
    validate_preview_settings(&settings)?;
    let limit = limit.clamp(1, 500);

    match dir_path {
        Some(dir_path) => {
            load_preview_directory_children(&settings, PathBuf::from(dir_path), offset, limit)
        }
        None => load_preview_root_directory(&settings, offset, limit),
    }
}

fn load_preview_root_directory(
    settings: &BatchSettings,
    offset: usize,
    limit: usize,
) -> Result<PreviewDirectoryPage, String> {
    let sources = input_sources(settings)?;
    let single_directory_source =
        sources.len() == 1 && sources[0].kind == SourcePathKind::Directory;
    if single_directory_source {
        return load_preview_directory_children(settings, sources[0].path.clone(), offset, limit);
    }

    let mut entries = Vec::new();
    let mut accepted = 0usize;
    let mut has_more = false;
    for source in sources {
        if accepted < offset {
            accepted += 1;
            continue;
        }
        if entries.len() >= limit {
            has_more = true;
            break;
        }
        match source.kind {
            SourcePathKind::Directory => {
                let name = source
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| source.path.to_string_lossy().to_string());
                entries.push(PreviewDirectoryEntry {
                    kind: "directory".to_string(),
                    path: source.path.to_string_lossy().to_string(),
                    rel: name.clone(),
                    name,
                });
            }
            SourcePathKind::File => {
                if let Some(item) = preview_work_item_for_path(settings, &source.path)? {
                    entries.push(preview_entry_from_work_item(item));
                }
            }
        }
        accepted += 1;
    }

    Ok(PreviewDirectoryPage {
        dir_path: None,
        entries,
        next_offset: if has_more { Some(offset + limit) } else { None },
    })
}

fn load_preview_directory_children(
    settings: &BatchSettings,
    dir: PathBuf,
    offset: usize,
    limit: usize,
) -> Result<PreviewDirectoryPage, String> {
    if !dir.is_dir() {
        return Err("预览目录不存在".to_string());
    }
    if !directory_in_preview_sources(settings, &dir)? {
        return Err("预览目录不在输入来源中".to_string());
    }

    let mut entries = Vec::new();
    let mut accepted = 0usize;
    let mut has_more = false;
    for entry in fs::read_dir(&dir).map_err(|err| format!("读取目录失败: {err}"))? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        let name_os = entry.file_name();
        if is_hidden_name(&name_os) || is_temp_output_name(&name_os) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        let next_entry = if file_type.is_dir() {
            let name = name_os.to_string_lossy().to_string();
            let rel = preview_rel_for_path(settings, &path)?;
            Some(PreviewDirectoryEntry {
                kind: "directory".to_string(),
                path: path.to_string_lossy().to_string(),
                rel: rel.to_string_lossy().to_string(),
                name,
            })
        } else if file_type.is_file() && is_supported_image(&path) {
            preview_work_item_for_path(settings, &path)?.map(preview_entry_from_work_item)
        } else {
            None
        };

        let Some(next_entry) = next_entry else {
            continue;
        };
        if accepted < offset {
            accepted += 1;
            continue;
        }
        if entries.len() >= limit {
            has_more = true;
            break;
        }
        entries.push(next_entry);
        accepted += 1;
    }

    Ok(PreviewDirectoryPage {
        dir_path: Some(dir.to_string_lossy().to_string()),
        entries,
        next_offset: if has_more { Some(offset + limit) } else { None },
    })
}

fn load_preview_tree_blocking(settings: BatchSettings) -> Result<PreviewTree, String> {
    validate_preview_settings(&settings)?;
    let items = collect_preview_work_items(&settings)?
        .into_iter()
        .map(|item| {
            let segments = item
                .rel
                .components()
                .map(|part| part.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>();
            let name = item
                .rel
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| item.rel.to_string_lossy().to_string());
            PreviewItem {
                path: item.src.to_string_lossy().to_string(),
                rel: item.rel.to_string_lossy().to_string(),
                name,
                segments,
            }
        })
        .collect();
    Ok(PreviewTree { items })
}

#[tauri::command]
async fn render_preview(settings: BatchSettings, src_path: String) -> Result<PreviewPair, String> {
    tauri::async_runtime::spawn_blocking(move || render_preview_blocking(settings, src_path))
        .await
        .map_err(|err| format!("生成预览任务失败: {err}"))?
}

fn render_preview_blocking(
    settings: BatchSettings,
    src_path: String,
) -> Result<PreviewPair, String> {
    validate_preview_settings(&settings)?;
    ensure_vips()?;

    let src = PathBuf::from(&src_path);
    if !src.is_file() {
        return Err("预览文件不存在".to_string());
    }
    if !is_supported_image(&src) {
        return Err("不支持的图片格式".to_string());
    }
    let item = preview_work_item_for_path(&settings, &src)?
        .ok_or_else(|| "预览文件不在输入来源中".to_string())?;

    let before = preview_original_image(&src)?;
    let after = preview_processed_image(&src, &settings)?;
    Ok(PreviewPair {
        rel: item.rel.to_string_lossy().to_string(),
        before,
        after,
    })
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

fn validate_preview_settings(settings: &BatchSettings) -> Result<(), String> {
    if settings.output_dir.trim().is_empty() {
        return Err("请选择输出目录".to_string());
    }
    let sources = input_sources(settings)?;
    if sources.is_empty() {
        return Err("请选择输入来源".to_string());
    }
    validate_dimensions(settings)?;
    if settings.max_side < 1 || settings.max_side > 50000 {
        return Err("最长边必须在 1 到 50000 之间".to_string());
    }
    if settings.quality < 1 || settings.quality > 100 {
        return Err("质量必须在 1 到 100 之间".to_string());
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
                    let result = process_item(&item, &settings, &cancelled);
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
    let warnings = counters_done
        .warnings
        .lock()
        .map(|warnings| warnings.clone())
        .unwrap_or_default();
    let _ = app_done.emit("batch-warnings", warnings);

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
    let kind = if is_supported_image(src) {
        WorkKind::RasterImage
    } else if is_pdf(src) {
        WorkKind::Pdf
    } else {
        WorkKind::Other
    };
    if kind == WorkKind::Other && !settings.copy_non_images {
        return true;
    }
    let dst = match kind {
        WorkKind::RasterImage => output_path(output_dir, &rel, settings.output_format),
        WorkKind::Pdf if settings.output_format == OutputFormat::Keep => output_dir.join(&rel),
        WorkKind::Pdf => pdf_output_dir(output_dir, &rel),
        WorkKind::Other => output_dir.join(&rel),
    };
    tx.send(WorkItem {
        src: src.to_path_buf(),
        rel,
        dst,
        kind,
    })
    .is_ok()
}

fn file_rel(path: &Path) -> PathBuf {
    path.file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("file"))
}

fn collect_preview_work_items(settings: &BatchSettings) -> Result<Vec<WorkItem>, String> {
    let output_dir = PathBuf::from(&settings.output_dir);
    let sources = input_sources(settings)?;
    let single_directory_source =
        sources.len() == 1 && sources[0].kind == SourcePathKind::Directory;
    let mut items = Vec::new();

    for source in sources {
        match source.kind {
            SourcePathKind::File => {
                push_preview_file_source(
                    &mut items,
                    &source.path,
                    file_rel(&source.path),
                    &output_dir,
                    settings,
                );
            }
            SourcePathKind::Directory => {
                let prefix = if single_directory_source {
                    None
                } else {
                    source.path.file_name().map(PathBuf::from)
                };
                collect_preview_directory_source(
                    &mut items,
                    &source.path,
                    prefix.as_deref(),
                    &output_dir,
                    settings,
                );
            }
        }
    }

    items.sort_by(|left, right| left.rel.cmp(&right.rel));
    Ok(items)
}

fn collect_preview_directory_source(
    items: &mut Vec<WorkItem>,
    root: &Path,
    prefix: Option<&Path>,
    output_dir: &Path,
    settings: &BatchSettings,
) {
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        if entry.depth() == 0 {
            return true;
        }
        !is_hidden_name(entry.file_name())
    }) {
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
        push_preview_file_source(items, &src, rel, output_dir, settings);
    }
}

fn push_preview_file_source(
    items: &mut Vec<WorkItem>,
    src: &Path,
    rel: PathBuf,
    output_dir: &Path,
    settings: &BatchSettings,
) {
    if is_hidden_name(src.file_name().unwrap_or_else(|| OsStr::new(""))) {
        return;
    }
    if is_temp_output_name(src.file_name().unwrap_or_else(|| OsStr::new(""))) {
        return;
    }
    if !is_supported_image(src) {
        return;
    }
    items.push(WorkItem {
        dst: output_path(output_dir, &rel, settings.output_format),
        src: src.to_path_buf(),
        rel,
        kind: WorkKind::RasterImage,
    });
}

fn preview_work_item_for_path(
    settings: &BatchSettings,
    src: &Path,
) -> Result<Option<WorkItem>, String> {
    if !src.is_file() || !is_supported_image(src) {
        return Ok(None);
    }
    if is_hidden_name(src.file_name().unwrap_or_else(|| OsStr::new("")))
        || is_temp_output_name(src.file_name().unwrap_or_else(|| OsStr::new("")))
    {
        return Ok(None);
    }
    if src
        .components()
        .any(|part| is_hidden_name(part.as_os_str()))
    {
        return Ok(None);
    }

    let rel = match preview_rel_for_path(settings, src) {
        Ok(rel) => rel,
        Err(_) => return Ok(None),
    };
    let output_dir = PathBuf::from(&settings.output_dir);
    Ok(Some(WorkItem {
        dst: output_path(&output_dir, &rel, settings.output_format),
        src: src.to_path_buf(),
        rel,
        kind: WorkKind::RasterImage,
    }))
}

fn preview_rel_for_path(settings: &BatchSettings, path: &Path) -> Result<PathBuf, String> {
    let sources = input_sources(settings)?;
    let single_directory_source =
        sources.len() == 1 && sources[0].kind == SourcePathKind::Directory;

    for source in sources {
        match source.kind {
            SourcePathKind::File => {
                if same_file_path(&source.path, path) {
                    return Ok(file_rel(&source.path));
                }
            }
            SourcePathKind::Directory => {
                if path == source.path || path.starts_with(&source.path) {
                    let inner_rel = path
                        .strip_prefix(&source.path)
                        .map_err(|_| "无法计算预览相对路径".to_string())?;
                    if inner_rel
                        .components()
                        .any(|part| is_hidden_name(part.as_os_str()))
                    {
                        return Err("隐藏文件不参与预览".to_string());
                    }
                    return Ok(if single_directory_source {
                        inner_rel.to_path_buf()
                    } else {
                        source
                            .path
                            .file_name()
                            .map(PathBuf::from)
                            .unwrap_or_else(|| PathBuf::from("source"))
                            .join(inner_rel)
                    });
                }
            }
        }
    }
    Err("预览路径不在输入来源中".to_string())
}

fn directory_in_preview_sources(settings: &BatchSettings, dir: &Path) -> Result<bool, String> {
    Ok(input_sources(settings)?.into_iter().any(|source| {
        source.kind == SourcePathKind::Directory
            && (dir == source.path || dir.starts_with(&source.path))
    }))
}

fn preview_entry_from_work_item(item: WorkItem) -> PreviewDirectoryEntry {
    let name = item
        .rel
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| item.rel.to_string_lossy().to_string());
    PreviewDirectoryEntry {
        kind: "file".to_string(),
        path: item.src.to_string_lossy().to_string(),
        rel: item.rel.to_string_lossy().to_string(),
        name,
    }
}

#[derive(Debug)]
enum ItemResult {
    Image {
        src_bytes: u64,
        dst_bytes: u64,
    },
    Pdf {
        src_bytes: u64,
        dst_bytes: u64,
        embedded_images: usize,
        warnings: Vec<String>,
    },
    Copied {
        src_bytes: u64,
        dst_bytes: u64,
    },
    Skipped,
    Cancelled,
    Failed(String),
}

fn process_item(item: &WorkItem, settings: &BatchSettings, cancel: &AtomicBool) -> ItemResult {
    let dst = &item.dst;

    if settings.skip_existing && dst.exists() {
        return ItemResult::Skipped;
    }

    if let Some(parent) = dst.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return ItemResult::Failed(err.to_string());
        }
    }

    match item.kind {
        WorkKind::RasterImage => match resize_image(&item.src, dst, settings) {
            Ok((src_bytes, dst_bytes)) => ItemResult::Image {
                src_bytes,
                dst_bytes,
            },
            Err(err) => ItemResult::Failed(err),
        },
        WorkKind::Pdf => match process_pdf(&item.src, dst, settings, cancel) {
            Ok(result) => ItemResult::Pdf {
                src_bytes: result.src_bytes,
                dst_bytes: result.dst_bytes,
                embedded_images: result.embedded_images,
                warnings: result.warnings,
            },
            Err(_) if cancel.load(Ordering::Relaxed) => ItemResult::Cancelled,
            Err(err) => ItemResult::Failed(err),
        },
        WorkKind::Other => {
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
}

struct PdfProcessResult {
    src_bytes: u64,
    dst_bytes: u64,
    embedded_images: usize,
    warnings: Vec<String>,
}

fn process_pdf(
    src: &Path,
    dst: &Path,
    settings: &BatchSettings,
    cancel: &AtomicBool,
) -> Result<PdfProcessResult, String> {
    let src_bytes = file_size(src);
    let mut document = pdf::Document::open(src).map_err(|err| format!("打开 PDF 失败: {err}"))?;
    let _input_was_encrypted = document.is_encrypted();
    let infos = document.images()?;
    let mut warnings = Vec::new();
    if document.has_signatures() {
        warnings.push("PDF 包含数字签名；处理后签名字段会保留，但原签名将失效".to_string());
    }

    if settings.output_format == OutputFormat::Keep {
        let mut processed_masks = HashSet::new();
        for info in &infos {
            if cancel.load(Ordering::Relaxed) {
                return Err("任务已取消".to_string());
            }
            replace_pdf_image(&mut document, info, settings, &mut processed_masks)?;
        }
        let temp = temp_output_path(dst);
        if let Err(err) = document.save(&temp) {
            let _ = fs::remove_file(&temp);
            return Err(format!("保存 PDF 失败: {err}"));
        }
        if let Err(err) = pdf::check(&temp) {
            let _ = fs::remove_file(&temp);
            return Err(format!("PDF 结构校验失败: {err}"));
        }
        replace_file(&temp, dst).map_err(|err| {
            let _ = fs::remove_file(&temp);
            format!("发布 PDF 失败: {err}")
        })?;
        Ok(PdfProcessResult {
            src_bytes,
            dst_bytes: file_size(dst),
            embedded_images: infos.len(),
            warnings,
        })
    } else {
        let temp_dir = temp_output_path(dst);
        fs::create_dir_all(&temp_dir).map_err(|err| format!("创建 PDF 暂存目录失败: {err}"))?;
        let result = extract_pdf_images(&mut document, &infos, &temp_dir, settings, cancel);
        let dst_bytes = match result {
            Ok(dst_bytes) => {
                if let Err(err) = replace_directory(&temp_dir, dst) {
                    let _ = fs::remove_dir_all(&temp_dir);
                    return Err(format!("发布 PDF 图片目录失败: {err}"));
                }
                dst_bytes
            }
            Err(err) => {
                let _ = fs::remove_dir_all(&temp_dir);
                return Err(err);
            }
        };
        Ok(PdfProcessResult {
            src_bytes,
            dst_bytes,
            embedded_images: infos.len(),
            warnings,
        })
    }
}

fn replace_pdf_image(
    document: &mut pdf::Document,
    info: &pdf::ImageInfo,
    settings: &BatchSettings,
    processed_masks: &mut HashSet<(i32, i32)>,
) -> Result<(), String> {
    let image = process_pdf_vips_image(load_pdf_vips_image(document, info)?, settings)?;
    let width = image.get_width() as u32;
    let height = image.get_height() as u32;
    let components = image.get_bands() as u32;
    let color_space = match components {
        1 => pdf::COLOR_GRAY,
        3 => pdf::COLOR_RGB,
        4 => pdf::COLOR_CMYK,
        _ => return Err(format!("不支持的 PDF 输出通道数: {components}")),
    };
    let encoded = image
        .write_to_buffer(&format!(".jpg[Q={},strip,interlace]", settings.quality))
        .map_err(|err| format!("编码 PDF 内嵌图片失败: {err} {}", vips_error_detail()))?;
    document.replace_image(
        info,
        pdf::Replacement {
            data: &encoded,
            width,
            height,
            components,
            color_space,
            filter: pdf::FILTER_DCT,
        },
    )?;

    if info.smask_object_id > 0
        && processed_masks.insert((info.smask_object_id, info.smask_generation))
    {
        replace_pdf_mask(
            document,
            info.smask_object_id,
            info.smask_generation,
            settings,
        )?;
    }
    if info.mask_object_id > 0
        && processed_masks.insert((info.mask_object_id, info.mask_generation))
    {
        replace_pdf_mask(
            document,
            info.mask_object_id,
            info.mask_generation,
            settings,
        )?;
    }
    Ok(())
}

fn replace_pdf_mask(
    document: &mut pdf::Document,
    object_id: i32,
    generation: i32,
    settings: &BatchSettings,
) -> Result<(), String> {
    let info = document.object_info(object_id, generation)?;
    let image = process_pdf_vips_image(load_pdf_vips_image(document, &info)?, settings)?;
    if image.get_bands() != 1 {
        return Err("PDF 透明蒙版不是单通道图片".to_string());
    }
    let width = image.get_width() as u32;
    let height = image.get_height() as u32;
    let raw = image.write_to_memory();
    checked_pdf_image_size(width, height, 1, 8)?;
    document.replace_image(
        &info,
        pdf::Replacement {
            data: &raw,
            width,
            height,
            components: 1,
            color_space: pdf::COLOR_GRAY,
            filter: pdf::FILTER_FLATE,
        },
    )
}

fn extract_pdf_images(
    document: &mut pdf::Document,
    infos: &[pdf::ImageInfo],
    temp_dir: &Path,
    settings: &BatchSettings,
    cancel: &AtomicBool,
) -> Result<u64, String> {
    let mut page_counts = HashMap::<u32, usize>::new();
    let mut total_bytes = 0u64;
    for info in infos {
        if cancel.load(Ordering::Relaxed) {
            return Err("任务已取消".to_string());
        }
        let mut image = process_pdf_vips_image(load_pdf_vips_image(document, info)?, settings)?;
        let mask_ref = if info.smask_object_id > 0 {
            Some((info.smask_object_id, info.smask_generation))
        } else if info.mask_object_id > 0 {
            Some((info.mask_object_id, info.mask_generation))
        } else {
            None
        };
        if let Some((object_id, generation)) = mask_ref {
            let mask_info = document.object_info(object_id, generation)?;
            let mask =
                process_pdf_vips_image(load_pdf_vips_image(document, &mask_info)?, settings)?;
            if mask.get_width() != image.get_width() || mask.get_height() != image.get_height() {
                return Err("PDF 主图与透明蒙版处理后的尺寸不一致".to_string());
            }
            image = VipsImage::bandjoin(&[image, mask])
                .map_err(|err| format!("合并 PDF 透明通道失败: {err} {}", vips_error_detail()))?;
        }
        let page_index = page_counts.entry(info.first_page).or_default();
        *page_index += 1;
        let ext = output_format_extension(settings.output_format);
        let filename = format!("page-{:04}-image-{:04}.{ext}", info.first_page, *page_index);
        let path = temp_dir.join(filename);
        let image = prepare_for_output(image, settings.output_format)?;
        let save_path = save_path_with_options(&path, settings.output_format, settings.quality);
        image
            .write_to_file(&save_path)
            .map_err(|err| format!("写入 PDF 内嵌图片失败: {err} {}", vips_error_detail()))?;
        total_bytes = total_bytes.saturating_add(file_size(&path));
    }
    Ok(total_bytes)
}

fn load_pdf_vips_image(
    document: &mut pdf::Document,
    info: &pdf::ImageInfo,
) -> Result<VipsImage, String> {
    validate_pdf_image_info(info)?;
    if matches!(info.filter, pdf::FILTER_DCT | pdf::FILTER_JPX) {
        if info.bits_per_component != 8 {
            return Err(format!("暂不支持 {}-bit PDF 图片", info.bits_per_component));
        }
        let memory_components = if info.color_space == pdf::COLOR_INDEXED {
            4
        } else {
            info.components.max(1)
        };
        checked_pdf_image_size(info.width, info.height, memory_components, 8)?;
        let encoded = document.read_raw(info)?;
        if encoded.len() > PDF_IMAGE_MEMORY_LIMIT {
            return Err("PDF 内嵌图片编码数据超过 512 MiB".to_string());
        }
        let image = VipsImage::new_from_buffer(&encoded, "")
            .map_err(|err| format!("解码 PDF 内嵌图片失败: {err} {}", vips_error_detail()))?;
        checked_pdf_image_size(
            image.get_width() as u32,
            image.get_height() as u32,
            image.get_bands() as u32,
            8,
        )?;
        if image.get_width() as u32 != info.width || image.get_height() as u32 != info.height {
            return Err("PDF 图片字典尺寸与编码数据不一致".to_string());
        }
        let image = set_pdf_interpretation(image, info)?;
        let image = apply_pdf_decode(image, info)?;
        return apply_pdf_icc(document, info, image);
    }

    let decoded = document.read_decoded(info)?;
    let (pixels, components, interpretation) = if info.color_space == pdf::COLOR_INDEXED {
        if info.bits_per_component != 8 {
            return Err("暂不支持非 8-bit Indexed PDF 图片".to_string());
        }
        let base_components = match info.indexed_base_color_space {
            pdf::COLOR_GRAY => 1,
            pdf::COLOR_RGB => 3,
            pdf::COLOR_CMYK => 4,
            _ => return Err("不支持的 Indexed PDF 基础色彩空间".to_string()),
        };
        checked_pdf_image_size(info.width, info.height, 1, 8)?;
        let palette = document.read_palette(info)?;
        let required_palette = (info.indexed_high_value as usize + 1)
            .checked_mul(base_components)
            .ok_or_else(|| "PDF Indexed 调色板尺寸溢出".to_string())?;
        if palette.len() < required_palette {
            return Err("PDF Indexed 调色板数据不足".to_string());
        }
        let pixel_count = (info.width as usize)
            .checked_mul(info.height as usize)
            .ok_or_else(|| "PDF 图片尺寸溢出".to_string())?;
        let decoded = normalize_pdf_decoded_length(decoded, pixel_count)?;
        if decoded.len() != pixel_count {
            return Err("PDF Indexed 解码数据长度不匹配".to_string());
        }
        let output_len = pixel_count
            .checked_mul(base_components)
            .ok_or_else(|| "PDF 图片尺寸溢出".to_string())?;
        if output_len > PDF_IMAGE_MEMORY_LIMIT {
            return Err("PDF 图片解码内存超过 512 MiB".to_string());
        }
        let mut pixels = Vec::with_capacity(output_len);
        for index in decoded {
            let index = if info.decode_mode == 1 {
                info.indexed_high_value.saturating_sub(index as u32) as usize
            } else {
                index as usize
            };
            if index > info.indexed_high_value as usize {
                return Err("PDF Indexed 图片包含越界索引".to_string());
            }
            let offset = index * base_components;
            pixels.extend_from_slice(&palette[offset..offset + base_components]);
        }
        (
            pixels,
            base_components as i32,
            interpretation_for_components(base_components as u32),
        )
    } else {
        let components = info.components;
        let expected =
            checked_pdf_image_size(info.width, info.height, components, info.bits_per_component)?;
        let mut pixels = if info.bits_per_component == 8 {
            normalize_pdf_decoded_length(decoded, expected)?
        } else if info.image_mask != 0 && matches!(info.bits_per_component, 1 | 2 | 4) {
            unpack_pdf_samples(&decoded, info.width, info.height, info.bits_per_component)?
        } else {
            return Err(format!("暂不支持 {}-bit PDF 图片", info.bits_per_component));
        };
        if info.decode_mode == 1 {
            for sample in &mut pixels {
                *sample = 255 - *sample;
            }
        }
        (
            pixels,
            components as i32,
            interpretation_for_components(components),
        )
    };
    let image = VipsImage::new_from_memory_copy(
        &pixels,
        info.width as i32,
        info.height as i32,
        components,
        BandFormat::Uchar,
    )
    .map_err(|err| format!("载入 PDF 像素数据失败: {err} {}", vips_error_detail()))?;
    let image = image
        .copy_with_opts(VOption::new().set("interpretation", interpretation as i32))
        .map_err(|err| format!("设置 PDF 色彩空间失败: {err} {}", vips_error_detail()))?;
    apply_pdf_icc(document, info, image)
}

fn normalize_pdf_decoded_length(mut data: Vec<u8>, expected: usize) -> Result<Vec<u8>, String> {
    if data.len() == expected {
        return Ok(data);
    }
    if data.len() > expected
        && data.len() - expected <= 2
        && data[expected..].iter().all(u8::is_ascii_whitespace)
    {
        data.truncate(expected);
        return Ok(data);
    }
    Err(format!(
        "PDF 图片解码数据长度不匹配: 期望 {expected}，实际 {}",
        data.len()
    ))
}

fn apply_pdf_icc(
    document: &mut pdf::Document,
    info: &pdf::ImageInfo,
    mut image: VipsImage,
) -> Result<VipsImage, String> {
    if info.color_space != pdf::COLOR_ICC {
        return Ok(image);
    }
    let profile = document.read_icc_profile(info)?;
    image
        .set_blob_copy("icc-profile-data", &profile)
        .map_err(|err| format!("附加 PDF ICC 配置失败: {err}"))?;
    image
        .icc_transform_with_opts("srgb", VOption::new().set("embedded", true))
        .map_err(|err| format!("转换 PDF ICC 色彩失败: {err} {}", vips_error_detail()))
}

fn validate_pdf_image_info(info: &pdf::ImageInfo) -> Result<(), String> {
    if info.width == 0 || info.height == 0 {
        return Err("PDF 图片尺寸无效".to_string());
    }
    if !matches!(
        info.filter,
        pdf::FILTER_DCT | pdf::FILTER_JPX | pdf::FILTER_FLATE | pdf::FILTER_LZW
    ) {
        return Err("不支持的 PDF 图片编码（仅支持 JPEG/JPX/Flate/LZW）".to_string());
    }
    if !matches!(
        info.color_space,
        pdf::COLOR_GRAY | pdf::COLOR_RGB | pdf::COLOR_CMYK | pdf::COLOR_INDEXED | pdf::COLOR_ICC
    ) && info.image_mask == 0
    {
        return Err("不支持的 PDF 图片色彩空间".to_string());
    }
    if info.has_color_key_mask != 0 {
        return Err("暂不支持 PDF 颜色键 Mask".to_string());
    }
    if info.decode_mode == 2 {
        return Err("暂不支持 PDF 图片的非标准 Decode 数组".to_string());
    }
    Ok(())
}

fn apply_pdf_decode(image: VipsImage, info: &pdf::ImageInfo) -> Result<VipsImage, String> {
    if info.decode_mode != 1 {
        return Ok(image);
    }
    image
        .linear(&[-1.0], &[255.0])
        .map_err(|err| format!("应用 PDF Decode 数组失败: {err} {}", vips_error_detail()))
}

fn checked_pdf_image_size(
    width: u32,
    height: u32,
    components: u32,
    bits_per_component: u32,
) -> Result<usize, String> {
    if width == 0 || height == 0 || components == 0 || bits_per_component == 0 {
        return Err("PDF 图片参数无效".to_string());
    }
    let bits = (width as usize)
        .checked_mul(height as usize)
        .and_then(|value| value.checked_mul(components as usize))
        .and_then(|value| value.checked_mul(bits_per_component as usize))
        .ok_or_else(|| "PDF 图片尺寸乘法溢出".to_string())?;
    let bytes = bits
        .checked_add(7)
        .ok_or_else(|| "PDF 图片尺寸乘法溢出".to_string())?
        / 8;
    let decoded_bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|value| value.checked_mul(components as usize))
        .ok_or_else(|| "PDF 图片尺寸乘法溢出".to_string())?;
    if bytes > PDF_IMAGE_MEMORY_LIMIT || decoded_bytes > PDF_IMAGE_MEMORY_LIMIT {
        return Err("PDF 图片解码内存超过 512 MiB".to_string());
    }
    Ok(if bits_per_component == 8 {
        decoded_bytes
    } else {
        bytes
    })
}

fn unpack_pdf_samples(
    input: &[u8],
    width: u32,
    height: u32,
    bits_per_component: u32,
) -> Result<Vec<u8>, String> {
    let packed_row = (width as usize * bits_per_component as usize).div_ceil(8);
    let expected = packed_row
        .checked_mul(height as usize)
        .ok_or_else(|| "PDF 蒙版尺寸溢出".to_string())?;
    if input.len() != expected {
        return Err("PDF 蒙版解码数据长度不匹配".to_string());
    }
    let max_value = (1u16 << bits_per_component) - 1;
    let mut output = Vec::with_capacity(width as usize * height as usize);
    for row in input.chunks_exact(packed_row) {
        let mut bit_offset = 0usize;
        for _ in 0..width {
            let byte = row[bit_offset / 8];
            let shift = 8 - bits_per_component as usize - (bit_offset % 8);
            let value = ((byte >> shift) & max_value as u8) as u16;
            output.push(((value * 255) / max_value) as u8);
            bit_offset += bits_per_component as usize;
        }
    }
    Ok(output)
}

fn set_pdf_interpretation(image: VipsImage, info: &pdf::ImageInfo) -> Result<VipsImage, String> {
    let interpretation = if info.color_space == pdf::COLOR_ICC {
        interpretation_for_components(info.components)
    } else {
        match info.color_space {
            pdf::COLOR_GRAY => Interpretation::BW,
            pdf::COLOR_RGB | pdf::COLOR_INDEXED => Interpretation::Srgb,
            pdf::COLOR_CMYK => Interpretation::Cmyk,
            _ => return Err("不支持的 PDF 图片色彩空间".to_string()),
        }
    };
    image
        .copy_with_opts(VOption::new().set("interpretation", interpretation as i32))
        .map_err(|err| format!("设置 PDF 色彩空间失败: {err} {}", vips_error_detail()))
}

fn interpretation_for_components(components: u32) -> Interpretation {
    match components {
        1 => Interpretation::BW,
        4 => Interpretation::Cmyk,
        _ => Interpretation::Srgb,
    }
}

fn process_pdf_vips_image(
    mut image: VipsImage,
    settings: &BatchSettings,
) -> Result<VipsImage, String> {
    image = rotate_image(image, settings.rotation)?;
    let src_width = image.get_width().max(1) as f64;
    let src_height = image.get_height().max(1) as f64;
    let scale = match settings.resize_mode {
        ResizeMode::FitLongestSide => settings.max_side as f64 / src_width.max(src_height),
        ResizeMode::FitBox => {
            (settings.width as f64 / src_width).min(settings.height as f64 / src_height)
        }
        ResizeMode::FitWidth => settings.width as f64 / src_width,
        ResizeMode::FitHeight => settings.height as f64 / src_height,
        ResizeMode::FixedCrop => {
            (settings.width as f64 / src_width).max(settings.height as f64 / src_height) + 0.000001
        }
    };
    let scale = if settings.resize_mode == ResizeMode::FixedCrop || settings.allow_upscale {
        scale
    } else {
        scale.min(1.0)
    };
    let projected_width = (src_width * scale).ceil().max(1.0) as u32;
    let projected_height = (src_height * scale).ceil().max(1.0) as u32;
    checked_pdf_image_size(
        projected_width,
        projected_height,
        image.get_bands() as u32,
        8,
    )?;
    if (scale - 1.0).abs() > f64::EPSILON {
        image = image
            .resize(scale)
            .map_err(|err| format!("缩放 PDF 图片失败: {err} {}", vips_error_detail()))?;
    }
    if settings.resize_mode == ResizeMode::FixedCrop {
        image = crop_to_target(image, settings)?;
    }
    Ok(image)
}

fn output_format_extension(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Jpg => "jpg",
        OutputFormat::Png => "png",
        OutputFormat::Webp => "webp",
        OutputFormat::Keep => "pdf",
    }
}

fn pdf_output_dir(output_dir: &Path, rel: &Path) -> PathBuf {
    let parent = rel.parent().unwrap_or_else(|| Path::new(""));
    let stem = rel
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| OsStr::new("pdf"));
    output_dir.join(parent).join(stem)
}

fn replace_directory(temp: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        return fs::rename(temp, dst);
    }
    let backup = temp_output_path(&dst.with_extension("pictrim-backup"));
    fs::rename(dst, &backup)?;
    match fs::rename(temp, dst) {
        Ok(()) => {
            let _ = fs::remove_dir_all(backup);
            Ok(())
        }
        Err(err) => {
            let _ = fs::rename(&backup, dst);
            Err(err)
        }
    }
}

fn resize_image(src: &Path, dst: &Path, settings: &BatchSettings) -> Result<(u64, u64), String> {
    let src_bytes = file_size(src);
    let temp = temp_output_path(dst);
    {
        let image = processed_image(src, settings)?;
        let format = effective_output_format(src, settings.output_format);
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

fn processed_image(src: &Path, settings: &BatchSettings) -> Result<VipsImage, String> {
    let load_source = thumbnail_load_source(src, settings.output_format);
    let image = match settings.resize_mode {
        ResizeMode::FitLongestSide | ResizeMode::FitBox => thumbnail_image(&load_source, settings)?,
        ResizeMode::FitWidth | ResizeMode::FitHeight | ResizeMode::FixedCrop => {
            manual_resize_image(src, settings)?
        }
    };
    prepare_for_output(image, effective_output_format(src, settings.output_format))
}

fn prepare_for_output(mut image: VipsImage, format: OutputFormat) -> Result<VipsImage, String> {
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
    Ok(image)
}

fn preview_original_image(src: &Path) -> Result<PreviewImage, String> {
    let image = VipsImage::new_from_file(src)
        .map_err(|e| format!("读取失败: {e} {}", vips_error_detail()))?;
    let width = image.get_width();
    let height = image.get_height();
    let data = encode_preview_png(image)?;
    Ok(PreviewImage {
        data: general_purpose::STANDARD.encode(data),
        mime: "image/png".to_string(),
        width,
        height,
        bytes: file_size(src),
    })
}

fn preview_processed_image(src: &Path, settings: &BatchSettings) -> Result<PreviewImage, String> {
    let image = processed_image(src, settings)?;
    let width = image.get_width();
    let height = image.get_height();
    let format = effective_output_format(src, settings.output_format);
    let data = encode_preview_output(image, format, settings.quality)?;
    let bytes = data.len() as u64;
    Ok(PreviewImage {
        width,
        height,
        bytes,
        mime: mime_for_output_format(format).to_string(),
        data: general_purpose::STANDARD.encode(data),
    })
}

fn encode_preview_png(image: VipsImage) -> Result<Vec<u8>, String> {
    let image = downscale_for_preview(image)?;
    image
        .write_to_buffer(".png[compression=6,strip]")
        .map_err(|err| format!("生成预览失败: {err} {}", vips_error_detail()))
}

fn encode_preview_output(
    image: VipsImage,
    format: OutputFormat,
    quality: i32,
) -> Result<Vec<u8>, String> {
    let image = downscale_for_preview(image)?;
    let suffix = buffer_suffix_with_options(format, quality);
    image
        .write_to_buffer(&suffix)
        .map_err(|err| format!("生成预览失败: {err} {}", vips_error_detail()))
}

fn downscale_for_preview(image: VipsImage) -> Result<VipsImage, String> {
    let width = image.get_width().max(1);
    let height = image.get_height().max(1);
    let longest = width.max(height);
    if longest <= PREVIEW_MAX_SIDE {
        return Ok(image);
    }
    let scale = PREVIEW_MAX_SIDE as f64 / longest as f64;
    image
        .resize(scale)
        .map_err(|e| format!("生成预览缩略图失败: {e} {}", vips_error_detail()))
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
    let mut image = VipsImage::new_from_file(src)
        .map_err(|e| format!("读取失败: {e} {}", vips_error_detail()))?;
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
        ItemResult::Pdf {
            src_bytes,
            dst_bytes,
            embedded_images,
            warnings,
        } => {
            counters.pdfs.fetch_add(1, Ordering::Relaxed);
            counters
                .embedded_images
                .fetch_add(embedded_images, Ordering::Relaxed);
            counters
                .total_src_bytes
                .fetch_add(src_bytes, Ordering::Relaxed);
            counters
                .total_dst_bytes
                .fetch_add(dst_bytes, Ordering::Relaxed);
            if let Ok(mut entries) = counters.warnings.lock() {
                for message in warnings {
                    entries.push(WarningEntry {
                        rel: item.rel.to_string_lossy().to_string(),
                        message,
                    });
                }
            }
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
        ItemResult::Cancelled => {}
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
                let backup = temp_output_path(&dst.with_extension("pictrim-backup"));
                fs::rename(dst, &backup)?;
                match fs::rename(temp, dst) {
                    Ok(()) => {
                        let _ = fs::remove_file(backup);
                        Ok(())
                    }
                    Err(err) => {
                        let _ = fs::rename(&backup, dst);
                        Err(err)
                    }
                }
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

fn buffer_suffix_with_options(format: OutputFormat, quality: i32) -> String {
    match format {
        OutputFormat::Jpg => format!(".jpg[Q={quality},strip,interlace]"),
        OutputFormat::Webp => format!(".webp[Q={quality},strip]"),
        OutputFormat::Png => ".png[compression=6,strip]".to_string(),
        OutputFormat::Keep => ".jpg".to_string(),
    }
}

fn mime_for_output_format(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Jpg | OutputFormat::Keep => "image/jpeg",
        OutputFormat::Png => "image/png",
        OutputFormat::Webp => "image/webp",
    }
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| IMAGE_EXTS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
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
            pdfs: 0,
            embedded_images: 0,
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
            load_preview_directory,
            load_preview_tree,
            render_preview,
            load_settings,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running PicTrim");
}

#[cfg(test)]
mod tests {
    use super::*;

    static VIPS_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn recognizes_supported_images_case_insensitively() {
        assert!(is_supported_image(Path::new("a.JPG")));
        assert!(is_supported_image(Path::new("a.webp")));
        assert!(!is_supported_image(Path::new("a.txt")));
        assert!(is_pdf(Path::new("a.PDF")));
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
    fn preview_tree_uses_batch_relative_paths() {
        let base = temp_test_dir("preview-paths");
        let input = base.join("photos");
        let output = base.join("out");
        fs::create_dir_all(input.join("nested")).unwrap();
        fs::write(input.join("nested").join("a.jpg"), b"not-real").unwrap();

        let settings = test_settings(vec![input.to_string_lossy().to_string()], &output);
        let items = collect_preview_work_items(&settings).unwrap();
        assert_eq!(items[0].rel, PathBuf::from("nested/a.jpg"));

        let loose = base.join("loose.png");
        fs::write(&loose, b"not-real").unwrap();
        let settings = test_settings(
            vec![
                input.to_string_lossy().to_string(),
                loose.to_string_lossy().to_string(),
            ],
            &output,
        );
        let rels: Vec<PathBuf> = collect_preview_work_items(&settings)
            .unwrap()
            .into_iter()
            .map(|item| item.rel)
            .collect();
        assert_eq!(
            rels,
            vec![
                PathBuf::from("loose.png"),
                PathBuf::from("photos/nested/a.jpg")
            ]
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_tree_filters_hidden_temp_and_non_images() {
        let base = temp_test_dir("preview-filters");
        let input = base.join("photos");
        let output = base.join("out");
        fs::create_dir_all(input.join(".hidden")).unwrap();
        fs::write(input.join("a.jpg"), b"not-real").unwrap();
        fs::write(input.join("notes.txt"), b"not-real").unwrap();
        fs::write(input.join("a.pictrim-tmp-123-0.jpg"), b"not-real").unwrap();
        fs::write(input.join(".hidden").join("b.jpg"), b"not-real").unwrap();

        let settings = test_settings(vec![input.to_string_lossy().to_string()], &output);
        let rels: Vec<PathBuf> = collect_preview_work_items(&settings)
            .unwrap()
            .into_iter()
            .map(|item| item.rel)
            .collect();
        assert_eq!(rels, vec![PathBuf::from("a.jpg")]);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_directory_pages_one_level_without_recursive_scan() {
        let base = temp_test_dir("preview-directory-page");
        let input = base.join("photos");
        let output = base.join("out");
        fs::create_dir_all(input.join("nested")).unwrap();
        fs::write(input.join("a.jpg"), b"not-real").unwrap();
        fs::write(input.join("nested").join("b.jpg"), b"not-real").unwrap();

        let settings = test_settings(vec![input.to_string_lossy().to_string()], &output);
        let first = load_preview_directory_blocking(settings.clone(), None, 0, 1).unwrap();
        assert_eq!(first.entries.len(), 1);
        assert!(first.next_offset.is_some());
        assert!(first
            .entries
            .iter()
            .all(|entry| entry.rel == "a.jpg" || entry.rel == "nested"));

        let full = load_preview_directory_blocking(settings, None, 0, 10).unwrap();
        let rels: Vec<String> = full.entries.into_iter().map(|entry| entry.rel).collect();
        assert!(rels.contains(&"a.jpg".to_string()));
        assert!(rels.contains(&"nested".to_string()));
        assert!(!rels.contains(&"nested/b.jpg".to_string()));

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
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
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

    #[test]
    fn render_preview_does_not_create_output_file() {
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let _ = ensure_vips();

        let dir = temp_test_dir("render-preview");
        let input = dir.join("input");
        let output = dir.join("out");
        fs::create_dir_all(&input).unwrap();
        let image_path = input.join("source.png");
        fs::write(&image_path, ONE_BY_ONE_PNG).unwrap();

        let settings = test_settings(vec![input.to_string_lossy().to_string()], &output);
        let result =
            render_preview_blocking(settings, image_path.to_string_lossy().to_string()).unwrap();

        assert_eq!(result.before.width, 1);
        assert_eq!(result.before.height, 1);
        assert_eq!(result.after.width, 1);
        assert_eq!(result.after.height, 1);
        assert!(!output.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn preview_output_format_matches_effective_output() {
        assert_eq!(
            buffer_suffix_with_options(OutputFormat::Jpg, 82),
            ".jpg[Q=82,strip,interlace]"
        );
        assert_eq!(
            buffer_suffix_with_options(OutputFormat::Png, 82),
            ".png[compression=6,strip]"
        );
        assert_eq!(
            buffer_suffix_with_options(OutputFormat::Webp, 82),
            ".webp[Q=82,strip]"
        );
        assert_eq!(
            mime_for_output_format(effective_output_format(
                Path::new("source.png"),
                OutputFormat::Keep
            )),
            "image/png"
        );
    }

    #[test]
    fn qpdf_binding_enumerates_unique_page_image() {
        let dir = temp_test_dir("pdf-enumerate");
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("source.pdf");
        write_test_pdf(&src, "/FlateDecode", true);

        let mut document = pdf::Document::open(&src).unwrap();
        let images = document.images().unwrap();
        assert_eq!(images.len(), 1, "the same object is referenced twice");
        assert_eq!(images[0].first_page, 1);
        assert_eq!((images[0].width, images[0].height), (2, 2));
        assert_eq!(images[0].filter, pdf::FILTER_FLATE);
        assert!(!document.is_encrypted());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pdf_keep_rewrites_images_and_preserves_container() {
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let _ = ensure_vips();
        let dir = temp_test_dir("pdf-keep");
        let output = dir.join("out");
        fs::create_dir_all(&output).unwrap();
        let src = dir.join("source.pdf");
        let dst = output.join("source.pdf");
        write_test_pdf(&src, "/FlateDecode", false);
        let settings = test_settings(vec![src.to_string_lossy().to_string()], &output);

        let result = process_pdf(&src, &dst, &settings, &AtomicBool::new(false)).unwrap();
        assert_eq!(result.embedded_images, 1);
        assert!(dst.is_file());
        pdf::check(&dst).unwrap();
        let mut output_document = pdf::Document::open(&dst).unwrap();
        let images = output_document.images().unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].filter, pdf::FILTER_DCT);
        let old_stream = [
            120, 156, 251, 207, 192, 192, 240, 31, 132, 255, 255, 103, 0, 0, 28, 239, 4, 252,
        ];
        assert!(!fs::read(&dst)
            .unwrap()
            .windows(old_stream.len())
            .any(|window| window == old_stream));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pdf_extract_uses_page_and_image_names() {
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let _ = ensure_vips();
        let dir = temp_test_dir("pdf-extract");
        let output = dir.join("out");
        fs::create_dir_all(&output).unwrap();
        let src = dir.join("source.pdf");
        let dst = output.join("source");
        write_test_pdf(&src, "/FlateDecode", true);
        let mut settings = test_settings(vec![src.to_string_lossy().to_string()], &output);
        settings.output_format = OutputFormat::Png;

        let result = process_pdf(&src, &dst, &settings, &AtomicBool::new(false)).unwrap();
        assert_eq!(result.embedded_images, 1);
        assert!(dst.join("page-0001-image-0001.png").is_file());
        assert_eq!(fs::read_dir(&dst).unwrap().count(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsupported_pdf_image_leaves_no_formal_output() {
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let _ = ensure_vips();
        let dir = temp_test_dir("pdf-unsupported");
        let output = dir.join("out");
        fs::create_dir_all(&output).unwrap();
        let src = dir.join("source.pdf");
        let dst = output.join("source.pdf");
        write_test_pdf(&src, "/CCITTFaxDecode", false);
        let settings = test_settings(vec![src.to_string_lossy().to_string()], &output);

        let result = process_pdf(&src, &dst, &settings, &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(!dst.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fixed_pdf_fixtures_cover_traversal_security_and_common_images() {
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let _ = ensure_vips();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

        for (name, expected) in [
            ("repeated-reference.pdf", 1),
            ("nested-form.pdf", 1),
            ("inline-image.pdf", 1),
            ("shared-smask.pdf", 2),
        ] {
            let mut document = pdf::Document::open(&fixtures.join(name)).unwrap();
            assert_eq!(document.images().unwrap().len(), expected, "{name}");
        }

        let signed = pdf::Document::open(&fixtures.join("signed-field.pdf")).unwrap();
        assert!(signed.has_signatures());
        let encrypted =
            pdf::Document::open(&fixtures.join("encrypted-empty-password.pdf")).unwrap();
        assert!(encrypted.is_encrypted());
        assert!(pdf::Document::open(&fixtures.join("encrypted-password-required.pdf")).is_err());

        let dir = temp_test_dir("pdf-common-fixtures");
        fs::create_dir_all(&dir).unwrap();
        let poppler_available = std::process::Command::new("pdftoppm")
            .arg("-h")
            .output()
            .is_ok();
        for name in [
            "rgb-jpeg.pdf",
            "flate-gray.pdf",
            "flate-rgb.pdf",
            "cmyk.pdf",
            "indexed.pdf",
            "icc-based.pdf",
            "smask.pdf",
            "shared-smask.pdf",
            "repeated-reference.pdf",
            "nested-form.pdf",
            "inline-image.pdf",
        ] {
            let src = fixtures.join(name);
            let dst = dir.join(name);
            let settings = test_settings(vec![src.to_string_lossy().to_string()], &dir);
            process_pdf(&src, &dst, &settings, &AtomicBool::new(false))
                .unwrap_or_else(|err| panic!("{name}: {err}"));
            pdf::check(&dst).unwrap_or_else(|err| panic!("{name}: {err}"));
            if poppler_available {
                let stem = name.trim_end_matches(".pdf");
                let before_prefix = dir.join(format!("{stem}-before"));
                let after_prefix = dir.join(format!("{stem}-after"));
                for (pdf_path, prefix) in [(&src, &before_prefix), (&dst, &after_prefix)] {
                    assert!(std::process::Command::new("pdftoppm")
                        .env("XDG_CACHE_HOME", dir.join("font-cache"))
                        .args(["-png", "-r", "72", "-singlefile"])
                        .arg(pdf_path)
                        .arg(prefix)
                        .status()
                        .unwrap()
                        .success());
                }
                let before = VipsImage::new_from_file(before_prefix.with_extension("png")).unwrap();
                let after = VipsImage::new_from_file(after_prefix.with_extension("png")).unwrap();
                assert_eq!(
                    (before.get_width(), before.get_height()),
                    (after.get_width(), after.get_height()),
                    "{name}"
                );
            }
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn poppler_preserves_page_geometry_and_text_positions_when_available() {
        if std::process::Command::new("pdftoppm")
            .arg("-h")
            .output()
            .is_err()
        {
            return;
        }
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let _ = ensure_vips();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let src = fixtures.join("flate-rgb.pdf");
        let dir = temp_test_dir("pdf-poppler");
        fs::create_dir_all(&dir).unwrap();
        let dst = dir.join("output.pdf");
        let settings = test_settings(vec![src.to_string_lossy().to_string()], &dir);
        process_pdf(&src, &dst, &settings, &AtomicBool::new(false)).unwrap();

        let before_bbox = dir.join("before.html");
        let after_bbox = dir.join("after.html");
        assert!(std::process::Command::new("pdftotext")
            .env("XDG_CACHE_HOME", dir.join("font-cache"))
            .args(["-bbox"])
            .arg(&src)
            .arg(&before_bbox)
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new("pdftotext")
            .env("XDG_CACHE_HOME", dir.join("font-cache"))
            .args(["-bbox"])
            .arg(&dst)
            .arg(&after_bbox)
            .status()
            .unwrap()
            .success());
        assert_eq!(
            fs::read(&before_bbox).unwrap(),
            fs::read(&after_bbox).unwrap()
        );

        let before_info = std::process::Command::new("pdfinfo")
            .arg(&src)
            .output()
            .unwrap();
        let after_info = std::process::Command::new("pdfinfo")
            .arg(&dst)
            .output()
            .unwrap();
        assert!(before_info.status.success() && after_info.status.success());
        let geometry = |bytes: &[u8]| {
            String::from_utf8_lossy(bytes)
                .lines()
                .filter(|line| line.starts_with("Pages:") || line.starts_with("Page size:"))
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        assert_eq!(geometry(&before_info.stdout), geometry(&after_info.stdout));

        for (pdf_path, prefix) in [(&src, "before"), (&dst, "after")] {
            assert!(std::process::Command::new("pdftoppm")
                .env("XDG_CACHE_HOME", dir.join("font-cache"))
                .args(["-png", "-r", "72", "-singlefile"])
                .arg(pdf_path)
                .arg(dir.join(prefix))
                .status()
                .unwrap()
                .success());
        }
        let before = VipsImage::new_from_file(dir.join("before.png")).unwrap();
        let after = VipsImage::new_from_file(dir.join("after.png")).unwrap();
        assert_eq!(
            (before.get_width(), before.get_height()),
            (after.get_width(), after.get_height())
        );

        if let Ok(visual_dir) = std::env::var("PICTRIM_PDF_VISUAL_OUTPUT") {
            let visual_dir = PathBuf::from(visual_dir);
            fs::create_dir_all(&visual_dir).unwrap();
            fs::copy(&src, visual_dir.join("before.pdf")).unwrap();
            fs::copy(&dst, visual_dir.join("after.pdf")).unwrap();
            fs::copy(dir.join("before.png"), visual_dir.join("before.png")).unwrap();
            fs::copy(dir.join("after.png"), visual_dir.join("after.png")).unwrap();
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pdf_failure_cancel_skip_and_overwrite_are_atomic() {
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let _ = ensure_vips();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let dir = temp_test_dir("pdf-atomic");
        fs::create_dir_all(&dir).unwrap();

        for name in ["unsupported-jbig2.pdf", "corrupt-stream.pdf"] {
            let src = fixtures.join(name);
            let dst = dir.join(format!("{name}.out.pdf"));
            fs::write(&dst, b"existing-output").unwrap();
            let settings = test_settings(vec![src.to_string_lossy().to_string()], &dir);
            assert!(process_pdf(&src, &dst, &settings, &AtomicBool::new(false)).is_err());
            assert_eq!(fs::read(&dst).unwrap(), b"existing-output");
        }

        let src = fixtures.join("flate-rgb.pdf");
        let dst = dir.join("cancelled.pdf");
        assert!(process_pdf(
            &src,
            &dst,
            &test_settings(vec![src.to_string_lossy().to_string()], &dir),
            &AtomicBool::new(true)
        )
        .is_err());
        assert!(!dst.exists());

        let existing_dir = dir.join("extract-existing");
        fs::create_dir_all(&existing_dir).unwrap();
        let item = WorkItem {
            src: src.clone(),
            rel: PathBuf::from("flate-rgb.pdf"),
            dst: existing_dir,
            kind: WorkKind::Pdf,
        };
        let mut settings = test_settings(vec![src.to_string_lossy().to_string()], &dir);
        settings.output_format = OutputFormat::Png;
        assert!(matches!(
            process_item(&item, &settings, &AtomicBool::new(false)),
            ItemResult::Skipped
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn signed_and_empty_password_pdf_keep_expected_security_state() {
        let _guard = VIPS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let _ = ensure_vips();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let dir = temp_test_dir("pdf-security");
        fs::create_dir_all(&dir).unwrap();

        let signed_src = fixtures.join("signed-field.pdf");
        let signed_dst = dir.join("signed.pdf");
        let signed_result = process_pdf(
            &signed_src,
            &signed_dst,
            &test_settings(vec![signed_src.to_string_lossy().to_string()], &dir),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(signed_result.warnings.len(), 1);
        assert!(pdf::Document::open(&signed_dst).unwrap().has_signatures());

        let encrypted_src = fixtures.join("encrypted-empty-password.pdf");
        let encrypted_dst = dir.join("encrypted.pdf");
        process_pdf(
            &encrypted_src,
            &encrypted_dst,
            &test_settings(vec![encrypted_src.to_string_lossy().to_string()], &dir),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(pdf::Document::open(&encrypted_dst).unwrap().is_encrypted());

        let locked_src = fixtures.join("encrypted-password-required.pdf");
        let locked_dst = dir.join("locked.pdf");
        assert!(process_pdf(
            &locked_src,
            &locked_dst,
            &test_settings(vec![locked_src.to_string_lossy().to_string()], &dir),
            &AtomicBool::new(false),
        )
        .is_err());
        assert!(!locked_dst.exists());

        let _ = fs::remove_dir_all(&dir);
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

    fn write_test_pdf(path: &Path, filter: &str, duplicate_draw: bool) {
        let image_data = [
            120, 156, 251, 207, 192, 192, 240, 31, 132, 255, 255, 103, 0, 0, 28, 239, 4, 252,
        ];
        let draws = if duplicate_draw {
            "q 40 0 0 40 20 20 cm /Im1 Do Q q 20 0 0 20 70 20 cm /Im1 Do Q"
        } else {
            "q 40 0 0 40 20 20 cm /Im1 Do Q"
        };
        let content = format!("BT /F1 12 Tf 20 90 Td (PicTrim PDF text) Tj ET {draws}");
        let mut objects = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 140 120] /Resources << /XObject << /Im1 5 0 R >> /Font << /F1 6 0 R >> >> /Contents 4 0 R >>".to_vec(),
            stream_object(b"", content.as_bytes()),
            stream_object(
                format!("/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter {filter}").as_bytes(),
                &image_data,
            ),
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        ];
        let mut pdf = b"%PDF-1.7\n%\xD0\xD4\xC5\xD8\n".to_vec();
        let mut offsets = Vec::new();
        for (index, object) in objects.drain(..).enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            pdf.extend_from_slice(&object);
            pdf.extend_from_slice(b"\nendobj\n");
        }
        let xref = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n", offsets.len() + 1).as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        fs::write(path, pdf).unwrap();
    }

    fn stream_object(dict: &[u8], data: &[u8]) -> Vec<u8> {
        let mut result = format!(
            "<< {} /Length {} >>\nstream\n",
            String::from_utf8_lossy(dict),
            data.len()
        )
        .into_bytes();
        result.extend_from_slice(data);
        result.extend_from_slice(b"\nendstream");
        result
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

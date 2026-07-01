import type { BatchProgress, OutputFormat, ResizeMode, SourceKind } from "./types";

export type Language = "en" | "zh";

const STORAGE_KEY = "pictrim-language";

const dictionaries = {
  en: {
    languageName: "English",
    switchLanguage: "中文",
    waitTitle: "Ready",
    waitMessage: "Ready to start.",
    readSettingsFailed: (error: string) => `Failed to load settings: ${error}`,
    saveSettingsFailed: (error: string) => `Failed to save settings: ${error}`,
    failureListUpdated: (count: number) => `${count} error${count === 1 ? "" : "s"} found. Failure list updated.`,
    processingImages: "Processing images.",
    processingStopped: "Processing stopped.",
    processingDone: "Processing complete.",
    scanningInput: "Scanning input folders.",
    sourceMissingTitle: "No source selected",
    sourceMissingMessage: "Choose input sources and an output folder first.",
    stoppingMessage: "Stopping. Images already in progress will finish writing first.",
    startLog: (sources: number, quality: number, concurrency: number, format: string) =>
      `Starting: ${sources} source${sources === 1 ? "" : "s"}, quality ${quality}%, concurrency ${concurrency}, format ${format}.`,
    startFailedTitle: "Could not start",
    startFailedLog: (error: string) => `Could not start: ${error}`,
    startButton: "Start",
    startingButton: "Starting...",
    backButton: "Back",
    stopButton: "Stop",
    sourceTaskTitle: "Image sources",
    sourceTaskReady: (count: number) => `${count} source${count === 1 ? "" : "s"} selected`,
    sourceTaskHint: "Choose files, folders, or drop them into the window.",
    inputSourceCount: (count: number) => `${count} input source${count === 1 ? "" : "s"}`,
    dropFilesOrFolders: "Drop files or folders",
    clear: "Clear",
    removeSource: "Remove source",
    moreSources: (count: number) => `${count} more source${count === 1 ? "" : "s"}`,
    collapseSources: "Collapse source list",
    dropHere: "Drop files and folders here",
    addFiles: "Add files",
    addFolders: "Add folders",
    outputTaskTitle: "Output folder",
    outputTaskHint: "Processed images are saved here.",
    outputPlaceholder: "Choose a folder for the results",
    choose: "Choose",
    selectFolderTitle: "Choose folder",
    selectFilesTitle: "Choose files",
    ruleTaskTitle: "Processing rules",
    processingMode: "Mode",
    previewProcessingMode: "Preview processing mode",
    widthMeasure: "W",
    heightMeasure: "H",
    exampleCase: "Example",
    landscapeImage: "Landscape",
    portraitImage: "Portrait",
    sourceTooWide: "Source too wide",
    sourceTooTall: "Source too tall",
    maxWidth: "Max width (px)",
    maxHeight: "Max height (px)",
    targetWidth: "Target width (px)",
    targetHeight: "Target height (px)",
    longestSide: "Longest side (px)",
    quality: "Quality",
    outputFormat: "Output format",
    advancedSettings: "More settings",
    widthShortfall: "When width is smaller",
    heightShortfall: "When height is smaller",
    longestSideShortfall: "When longest side is smaller",
    sizeShortfall: "When size is smaller",
    noUpscale: "Do not upscale",
    upscaleToWidth: "Upscale to target width",
    upscaleToHeight: "Upscale to target height",
    upscaleToLongestSide: "Upscale to target size",
    upscaleToFit: "Upscale until one side fits",
    horizontalOverflow: "Horizontal overflow",
    verticalOverflow: "Vertical overflow",
    cropCenter: "Crop center",
    cropLeft: "Crop left",
    cropRight: "Crop right",
    cropTop: "Crop top",
    cropBottom: "Crop bottom",
    rotation: "Rotation",
    autoRotation: "Auto-correct EXIF",
    noRotation: "Do not rotate",
    rotate90: "90° clockwise",
    rotate180: "180°",
    rotate270: "90° counterclockwise",
    concurrency: "Concurrency",
    nonImageFiles: "Non-image files",
    ignoreNonImages: "Ignore",
    copyNonImages: "Copy to output folder",
    existingFiles: "Existing files",
    skipExisting: "Skip and keep existing",
    overwriteExisting: "Overwrite",
    sourceKindDirectory: "Folder",
    sourceKindFile: "File",
    sourceKindMissing: "Missing",
    flexible: "Flexible",
    outputKeep: "Keep original",
    generatedImages: "Images",
    copied: "Copied",
    skipped: "Skipped",
    failed: "Failed",
    saved: "Saved",
    increased: "Increased",
    logsTitle: "Output log",
    emptyLogs: "No logs yet.",
    failuresTitle: "Failure list",
    failuresSubtitle: (count: number) => `${count} file${count === 1 ? "" : "s"} failed`,
    summarySources: (count: number) => `${count} source${count === 1 ? "" : "s"}`,
    summaryLongestSide: (value: number) => `Longest side ${value}px`,
    summaryWidth: (value: number) => `Width ${value}px`,
    summaryHeight: (value: number) => `Height ${value}px`,
    manualRotation: "Manual rotation",
    allowUpscale: "Allow upscale",
    disallowUpscale: "No upscale",
    qualitySummary: (value: number) => `Quality ${value}`,
    concurrencySummary: (value: number) => `Concurrency ${value}`,
    copyNonImagesSummary: "Copy non-images",
    ignoreNonImagesSummary: "Ignore non-images",
    skipExistingSummary: "Skip existing",
    overwriteExistingSummary: "Overwrite existing",
    phaseScanning: "Scanning",
    phaseProcessing: "Processing",
    phaseError: "Processing error",
    phaseCancelled: "Stopped",
    phaseDone: "Complete",
  },
  zh: {
    languageName: "中文",
    switchLanguage: "English",
    waitTitle: "等待开始",
    waitMessage: "等待开始。",
    readSettingsFailed: (error: string) => `读取设置失败: ${error}`,
    saveSettingsFailed: (error: string) => `保存设置失败: ${error}`,
    failureListUpdated: (count: number) => `发现 ${count} 个错误，已更新错误列表。`,
    processingImages: "正在处理图片。",
    processingStopped: "处理已停止。",
    processingDone: "处理完成。",
    scanningInput: "正在扫描输入目录。",
    sourceMissingTitle: "来源未选择",
    sourceMissingMessage: "请先选择输入来源和输出目录。",
    stoppingMessage: "正在停止，已开始的图片会先完成写入。",
    startLog: (sources: number, quality: number, concurrency: number, format: string) =>
      `开始处理: 来源 ${sources} 个, 质量 ${quality}%, 并发 ${concurrency}, 格式 ${format}。`,
    startFailedTitle: "无法开始",
    startFailedLog: (error: string) => `无法开始: ${error}`,
    startButton: "开始处理",
    startingButton: "正在开始...",
    backButton: "返回",
    stopButton: "停止",
    sourceTaskTitle: "图片来源",
    sourceTaskReady: (count: number) => `已选择 ${count} 个来源`,
    sourceTaskHint: "选择文件、文件夹，或直接拖入窗口。",
    inputSourceCount: (count: number) => `${count} 个输入来源`,
    dropFilesOrFolders: "拖入文件或目录",
    clear: "清空",
    removeSource: "移除来源",
    moreSources: (count: number) => `还有 ${count} 个来源`,
    collapseSources: "收起来源列表",
    dropHere: "把文件、文件夹拖到这里",
    addFiles: "添加文件",
    addFolders: "添加目录",
    outputTaskTitle: "输出位置",
    outputTaskHint: "处理后的图片会保存到这里。",
    outputPlaceholder: "选择保存结果的文件夹",
    choose: "选择",
    selectFolderTitle: "选择目录",
    selectFilesTitle: "选择文件",
    ruleTaskTitle: "处理规则",
    processingMode: "处理方式",
    previewProcessingMode: "查看处理方式预览",
    widthMeasure: "宽",
    heightMeasure: "高",
    exampleCase: "示例情形",
    landscapeImage: "横向图片",
    portraitImage: "纵向图片",
    sourceTooWide: "原图过宽",
    sourceTooTall: "原图过高",
    maxWidth: "最大宽度 (px)",
    maxHeight: "最大高度 (px)",
    targetWidth: "目标宽度 (px)",
    targetHeight: "目标高度 (px)",
    longestSide: "最长边 (px)",
    quality: "质量",
    outputFormat: "输出格式",
    advancedSettings: "更多设置",
    widthShortfall: "宽度不足时",
    heightShortfall: "高度不足时",
    longestSideShortfall: "长边不足时",
    sizeShortfall: "尺寸不足时",
    noUpscale: "不放大",
    upscaleToWidth: "放大到目标宽度",
    upscaleToHeight: "放大到目标高度",
    upscaleToLongestSide: "放大到目标尺寸",
    upscaleToFit: "放大到一边满足尺寸",
    horizontalOverflow: "横向超出时",
    verticalOverflow: "纵向超出时",
    cropCenter: "截取中间",
    cropLeft: "截左边",
    cropRight: "截右边",
    cropTop: "截上面",
    cropBottom: "截下面",
    rotation: "旋转",
    autoRotation: "EXIF自动校正",
    noRotation: "不旋转",
    rotate90: "顺时针 90°",
    rotate180: "180°",
    rotate270: "逆时针 90°",
    concurrency: "并发数",
    nonImageFiles: "非图片文件",
    ignoreNonImages: "忽略，不处理",
    copyNonImages: "复制到目标目录",
    existingFiles: "已存在文件",
    skipExisting: "跳过，保留已有",
    overwriteExisting: "覆盖，重新生成",
    sourceKindDirectory: "目录",
    sourceKindFile: "文件",
    sourceKindMissing: "缺失",
    flexible: "弹性",
    outputKeep: "保持原格式",
    generatedImages: "生成图片",
    copied: "复制",
    skipped: "跳过",
    failed: "失败",
    saved: "已节省",
    increased: "增大",
    logsTitle: "输出日志",
    emptyLogs: "暂无日志。",
    failuresTitle: "错误列表",
    failuresSubtitle: (count: number) => `${count} 个文件处理失败`,
    summarySources: (count: number) => `${count} 个来源`,
    summaryLongestSide: (value: number) => `最长边 ${value}px`,
    summaryWidth: (value: number) => `宽 ${value}px`,
    summaryHeight: (value: number) => `高 ${value}px`,
    manualRotation: "手动旋转",
    allowUpscale: "允许放大",
    disallowUpscale: "不放大",
    qualitySummary: (value: number) => `质量 ${value}`,
    concurrencySummary: (value: number) => `并发 ${value}`,
    copyNonImagesSummary: "复制非图片",
    ignoreNonImagesSummary: "忽略非图片",
    skipExistingSummary: "跳过已存在",
    overwriteExistingSummary: "覆盖已存在",
    phaseScanning: "正在扫描",
    phaseProcessing: "正在处理",
    phaseError: "处理出错",
    phaseCancelled: "已停止",
    phaseDone: "处理完成",
  },
};

export type Copy = typeof dictionaries.en;

function getSystemLanguage(): Language {
  const languages = typeof navigator === "undefined" ? [] : navigator.languages.length > 0 ? navigator.languages : [navigator.language];
  return languages.some((language) => language.toLowerCase().startsWith("zh")) ? "zh" : "en";
}

function getInitialLanguage(): Language {
  if (typeof localStorage !== "undefined") {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "en" || saved === "zh") return saved;
  }
  return getSystemLanguage();
}

export const locale = $state({
  language: getInitialLanguage(),
});

export function getCopy(): Copy {
  return dictionaries[locale.language];
}

export function setLanguage(language: Language) {
  locale.language = language;
  localStorage.setItem(STORAGE_KEY, language);
  document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
}

export function toggleLanguage() {
  setLanguage(locale.language === "zh" ? "en" : "zh");
}

export function syncDocumentLanguage() {
  document.documentElement.lang = locale.language === "zh" ? "zh-CN" : "en";
}

export function outputFormatLabel(value: OutputFormat, copy = getCopy()): string {
  if (value === "jpg") return "JPG";
  if (value === "png") return "PNG";
  if (value === "webp") return "WebP";
  return copy.outputKeep;
}

export function resizeModeCopy(mode: ResizeMode, copy = getCopy()) {
  if (mode === "fitBox") {
    return {
      title: copy.languageName === "中文" ? "等比例缩放，限制宽高" : "Scale proportionally, limit width and height",
      detail: copy.languageName === "中文"
        ? "按最大宽度和最大高度限制图片，保持完整画面，不裁剪。"
        : "Constrain both width and height while keeping the full image without cropping.",
    };
  }
  if (mode === "fitWidth") {
    return {
      title: copy.languageName === "中文" ? "缩放到指定宽度" : "Scale to target width",
      detail: copy.languageName === "中文" ? "宽度缩放到目标值，高度按比例自动计算。" : "Set the width and calculate height proportionally.",
    };
  }
  if (mode === "fitHeight") {
    return {
      title: copy.languageName === "中文" ? "缩放到指定高度" : "Scale to target height",
      detail: copy.languageName === "中文" ? "高度缩放到目标值，宽度按比例自动计算。" : "Set the height and calculate width proportionally.",
    };
  }
  if (mode === "fixedCrop") {
    return {
      title: copy.languageName === "中文" ? "缩放到固定宽高并裁剪" : "Scale and crop to a fixed size",
      detail: copy.languageName === "中文" ? "先等比铺满目标尺寸，再按定位方式裁掉多余部分。" : "Fill the target size proportionally, then crop the overflow by position.",
    };
  }
  return {
    title: copy.languageName === "中文" ? "等比例缩放，限制长边" : "Scale proportionally, limit longest side",
    detail: copy.languageName === "中文" ? "只限制图片最长的一边，另一边按比例自动计算。" : "Constrain only the longest side and calculate the other side proportionally.",
  };
}

export function sourceKindLabel(kind: SourceKind, copy = getCopy()): string {
  if (kind === "directory") return copy.sourceKindDirectory;
  if (kind === "file") return copy.sourceKindFile;
  return copy.sourceKindMissing;
}

export function phaseTitle(progress: BatchProgress, copy = getCopy()): string {
  if (progress.phase === "scanning") return copy.phaseScanning;
  if (progress.phase === "processing") return copy.phaseProcessing;
  if (progress.phase === "error") return copy.phaseError;
  if (progress.cancelled) return copy.phaseCancelled;
  if (progress.done) return copy.phaseDone;
  return copy.waitTitle;
}

export function localizeBackendMessage(message: string, copy = getCopy()): string {
  if (copy.languageName === "中文") return message;

  const exact: Record<string, string> = {
    "libvips 初始化失败": "Failed to initialize libvips",
    "无法锁定任务状态": "Could not lock task state",
    "已有任务正在运行": "A task is already running",
    "请选择输入来源": "Choose input sources",
    "请选择输出目录": "Choose an output folder",
    "最长边必须在 1 到 50000 之间": "Longest side must be between 1 and 50000",
    "宽度必须在 1 到 50000 之间": "Width must be between 1 and 50000",
    "高度必须在 1 到 50000 之间": "Height must be between 1 and 50000",
    "质量必须在 1 到 100 之间": "Quality must be between 1 and 100",
    "并发数必须在 1 到 128 之间": "Concurrency must be between 1 and 128",
    "输出目录不能位于输入目录内部，请另选位置": "The output folder cannot be inside an input folder. Choose another location.",
    "正在扫描并处理文件": "Scanning and processing files",
    "已停止任务": "Task stopped",
    "处理完成": "Processing complete",
  };

  if (exact[message]) return exact[message];

  const prefixMap: Array<[string, string]> = [
    ["读取设置失败", "Failed to read settings"],
    ["解析设置失败", "Failed to parse settings"],
    ["加载设置失败", "Failed to load settings"],
    ["创建设置目录失败", "Failed to create settings folder"],
    ["序列化设置失败", "Failed to serialize settings"],
    ["保存设置失败", "Failed to save settings"],
    ["定位设置目录失败", "Failed to locate settings folder"],
    ["输入来源不存在", "Input source does not exist"],
    ["创建处理线程池失败", "Failed to create processing thread pool"],
    ["输出路径冲突", "Output path conflict"],
    ["透明背景处理失败", "Failed to process transparent background"],
    ["写入失败", "Failed to write"],
    ["替换失败", "Failed to replace"],
    ["读取失败", "Failed to read"],
    ["EXIF方向校正失败", "Failed to correct EXIF orientation"],
    ["缩放失败", "Failed to resize"],
    ["裁剪失败", "Failed to crop"],
    ["旋转失败", "Failed to rotate"],
  ];

  for (const [source, target] of prefixMap) {
    if (message.startsWith(`${source}:`)) {
      return message.replace(source, target);
    }
  }

  return message;
}

import { existsSync, renameSync, rmSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import { arch, platform } from "node:process";
import { join } from "node:path";

const projectRoot = process.cwd();
const applicationPath = "/Applications/AncientMedicalTTS.app";
const stamp = new Date().toISOString().replace(/[-:TZ.]/g, "").slice(0, 14);
const args = new Set(process.argv.slice(2).filter((value) => value !== "--"));

if (args.has("--help")) {
  console.log(`用法：
  pnpm publish:macos                 构建 Debug App 并安装到 /Applications
  pnpm publish:macos -- --release    构建 Release App 并安装
  pnpm publish:macos -- --skip-build 直接安装已有 Bundle
  pnpm publish:macos -- --launch     安装完成后启动应用
`);
  process.exit(0);
}

if (platform !== "darwin") {
  throw new Error("publish:macos 只能在 macOS 上运行");
}

const unknownArgs = [...args].filter((value) => !["--release", "--skip-build", "--launch"].includes(value));
if (unknownArgs.length > 0) {
  throw new Error(`不支持的参数：${unknownArgs.join("、")}`);
}

const profile = args.has("--release") ? "release" : "debug";
const builtApp = join(projectRoot, "src-tauri", "target", profile, "bundle", "macos", "AncientMedicalTTS.app");
const worker = join(builtApp, "Contents", "Resources", "worker-runtime", "ancient-tts-worker");
const ffmpeg = join(builtApp, "Contents", "MacOS", "ffmpeg");
const temporaryApplicationPath = `/Applications/.AncientMedicalTTS.app.installing-${stamp}`;
const backupPath = `/Applications/AncientMedicalTTS.app.backup-${stamp}`;

const environment = {
  ...process.env,
  ANCIENT_MEDICAL_TTS_ADHOC_SIGN: "1",
};

if (!args.has("--skip-build")) {
  const vendorFfmpeg = join(projectRoot, "vendor", "ffmpeg", `ffmpeg-${darwinTriple()}`);
  if (!environment.ANCIENT_MEDICAL_TTS_FFMPEG && !existsSync(vendorFfmpeg)) {
    const systemFfmpeg = resolveCommand("ffmpeg");
    if (systemFfmpeg) {
      environment.ANCIENT_MEDICAL_TTS_ALLOW_SYSTEM_FFMPEG = "1";
      console.log(`使用本机 FFmpeg 构建并将其依赖打入 App Bundle：${systemFfmpeg}`);
    }
  }

  const buildArgs = ["tauri:build", profile === "release" ? "--release" : "--debug", "--bundles", "app"];
  console.log(`开始构建 macOS ${profile === "release" ? "Release" : "Debug"} App…`);
  execFileSync("pnpm", buildArgs, { cwd: projectRoot, env: environment, stdio: "inherit" });
}

validateBundle();
installBundle();

if (args.has("--launch")) {
  execFileSync("/usr/bin/open", ["-a", applicationPath], { stdio: "inherit" });
}

function darwinTriple() {
  if (arch === "arm64") return "aarch64-apple-darwin";
  if (arch === "x64") return "x86_64-apple-darwin";
  throw new Error(`不支持的 macOS 架构：${arch}`);
}

function resolveCommand(command) {
  const result = spawnSync("/usr/bin/which", [command], { encoding: "utf8" });
  if (result.status !== 0) return null;
  return result.stdout.trim() || null;
}

function validateBundle() {
  if (!existsSync(builtApp)) throw new Error(`找不到 App Bundle：${builtApp}`);
  if (!existsSync(worker)) throw new Error(`Bundle 缺少 Python Worker：${worker}`);
  if (!existsSync(ffmpeg)) throw new Error(`Bundle 缺少 FFmpeg：${ffmpeg}`);

  const ping = execFileSync(worker, [], {
    input: '{"id":"publish-check","method":"system.ping","params":{}}\n',
    encoding: "utf8",
  }).split(/\r?\n/).map((line) => line.trim()).find(Boolean);
  const parsed = JSON.parse(ping ?? "{}");
  if (parsed.id !== "publish-check" || parsed.ok !== true || typeof parsed.result?.version !== "string") {
    throw new Error(`Bundle 内 Worker 检查失败：${ping ?? "无响应"}`);
  }

  const ffmpegVersion = execFileSync(ffmpeg, ["-version"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).split(/\r?\n/, 1)[0];
  const encoders = execFileSync(ffmpeg, ["-encoders"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (!ffmpegVersion.includes("ffmpeg version") || !encoders.includes("libmp3lame")) {
    throw new Error("Bundle 内 FFmpeg 检查失败，或缺少 libmp3lame");
  }

  execFileSync("/usr/bin/codesign", ["--verify", "--deep", "--strict", builtApp], { stdio: "ignore" });
  console.log(`Bundle 检查通过：Worker ${parsed.result.version}，${ffmpegVersion}`);
}

function installBundle() {
  if (existsSync(temporaryApplicationPath)) rmSync(temporaryApplicationPath, { recursive: true, force: true });

  try {
    execFileSync("/usr/bin/ditto", ["--rsrc", "--noqtn", builtApp, temporaryApplicationPath], { stdio: "inherit" });
    if (existsSync(applicationPath)) renameSync(applicationPath, backupPath);
    renameSync(temporaryApplicationPath, applicationPath);
  } catch (error) {
    if (existsSync(temporaryApplicationPath)) rmSync(temporaryApplicationPath, { recursive: true, force: true });
    if (!existsSync(applicationPath) && existsSync(backupPath)) renameSync(backupPath, applicationPath);
    throw error;
  }

  console.log(`已安装：${applicationPath}`);
  if (existsSync(backupPath)) console.log(`旧版本备份：${backupPath}`);
}

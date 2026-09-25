import { chmodSync, copyFileSync, existsSync, mkdirSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { arch, platform } from "node:process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const targets = {
  "darwin-arm64": { triple: "aarch64-apple-darwin", extension: "" },
  "darwin-x64": { triple: "x86_64-apple-darwin", extension: "" },
  "win32-x64": { triple: "x86_64-pc-windows-msvc", extension: ".exe" },
};

const target = targets[`${platform}-${arch}`];
if (!target) {
  throw new Error(`当前平台暂不支持 FFmpeg sidecar: ${platform}-${arch}`);
}

const root = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(root, "..");
const outputDir = join(projectRoot, "src-tauri", "binaries");
const runtimeDir = join(projectRoot, "src-tauri", "ffmpeg-runtime");
const output = join(outputDir, `ffmpeg-${target.triple}${target.extension}`);
const vendor = join(projectRoot, "vendor", "ffmpeg", `ffmpeg-${target.triple}${target.extension}`);

let source = process.env.ANCIENT_MEDICAL_TTS_FFMPEG;
if (!source && existsSync(vendor)) source = vendor;
if (!source && process.env.ANCIENT_MEDICAL_TTS_ALLOW_SYSTEM_FFMPEG === "1") {
  const command = platform === "win32" ? "where" : "which";
  try {
    source = execFileSync(command, [platform === "win32" ? "ffmpeg.exe" : "ffmpeg"], { encoding: "utf8" })
      .split(/\r?\n/)
      .find((line) => line.trim().length > 0)
      ?.trim();
  } catch {
    source = undefined;
  }
}

if (!source || !existsSync(source)) {
  throw new Error(
    `缺少 ${target.triple} 的 FFmpeg binary。请放入 ${vendor}，或仅在本地调试时设置 ANCIENT_MEDICAL_TTS_ALLOW_SYSTEM_FFMPEG=1。`,
  );
}

try {
  const version = execFileSync(source, ["-version"], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  const encoders = execFileSync(source, ["-encoders"], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  if (!version.includes("ffmpeg version")) throw new Error("无法读取 FFmpeg 版本");
  if (!encoders.includes("libmp3lame")) throw new Error("FFmpeg 未包含 libmp3lame");
} catch (error) {
  throw new Error(`FFmpeg binary 校验失败: ${error instanceof Error ? error.message : String(error)}`);
}

mkdirSync(outputDir, { recursive: true });
copyFileSync(source, output);
if (platform !== "win32") chmodSync(output, 0o755);

if (platform === "darwin") {
  bundleMacDependencies(output, runtimeDir);
  signMacRuntime(output, runtimeDir);
  chmodSync(output, 0o755);
}
console.log(`Prepared FFmpeg sidecar: ${output}`);

function bundleMacDependencies(binary, destination) {
  try {
    execFileSync("otool", ["-L", binary], { stdio: "ignore" });
  } catch {
    return;
  }
  rmSync(destination, { recursive: true, force: true });
  mkdirSync(destination, { recursive: true });
  writeFileSync(join(destination, ".gitkeep"), "");
  const copied = new Map();
  visit(binary, true);

  function visit(file, isBinary) {
    const dependencies = macDependencies(file);
    for (const dependency of dependencies) {
      if (!dependency.startsWith("/opt/homebrew/") || !existsSync(dependency)) continue;
      const sourcePath = realpathSync(dependency);
      const fileName = sourcePath.split("/").pop();
      if (!fileName) continue;
      const targetPath = join(destination, fileName);
      if (!copied.has(sourcePath)) {
        copyFileSync(sourcePath, targetPath);
        copied.set(sourcePath, targetPath);
        chmodSync(targetPath, 0o755);
        visit(targetPath, false);
      }
      const replacement = isBinary
        ? `@loader_path/../Resources/ffmpeg-runtime/${fileName}`
        : `@loader_path/${fileName}`;
      execFileSync("install_name_tool", ["-change", dependency, replacement, file], { stdio: "ignore" });
      if (!isBinary && file.endsWith(".dylib")) {
        execFileSync("install_name_tool", ["-id", `@loader_path/${fileName}`, file], { stdio: "ignore" });
      }
    }
  }

  function macDependencies(file) {
    const outputText = execFileSync("otool", ["-L", file], { encoding: "utf8" });
    return outputText
      .split(/\r?\n/)
      .slice(1)
      .map((line) => line.trim().split(" (")[0])
      .filter((dependency) => dependency.length > 0);
  }
}

function signMacRuntime(binary, destination) {
  try {
    execFileSync("codesign", ["--version"], { stdio: "ignore" });
  } catch {
    return;
  }
  for (const fileName of readdirSync(destination).filter((name) => name.endsWith(".dylib"))) {
    const file = join(destination, fileName);
    execFileSync("codesign", ["--force", "--sign", "-", file], { stdio: "ignore" });
  }
  execFileSync("codesign", ["--force", "--sign", "-", binary], { stdio: "ignore" });
}

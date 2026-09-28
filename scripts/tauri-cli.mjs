import { existsSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import { basename, dirname, join } from "node:path";

const args = process.argv.slice(2);
const bundleVersion = process.env.ANCIENT_MEDICAL_TTS_BUNDLE_VERSION?.trim();
if (!process.env.TAURI_SIGNING_PRIVATE_KEY?.trim() && process.env.TAURI_SIGNING_PRIVATE_KEY_PATH?.trim()) {
  process.env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(process.env.TAURI_SIGNING_PRIVATE_KEY_PATH, "utf8").trim();
  delete process.env.TAURI_SIGNING_PRIVATE_KEY_PATH;
}
const shouldCreateUpdaterArtifacts = Boolean(
  process.env.TAURI_SIGNING_PRIVATE_KEY?.trim() ||
  process.env.TAURI_SIGNING_PRIVATE_KEY_PATH?.trim(),
);
let bundleConfigPath;
if ((bundleVersion || shouldCreateUpdaterArtifacts) && args[0] === "build") {
  bundleConfigPath = join(process.cwd(), "src-tauri", ".tauri-ci-bundle.conf.json");
  const configOverride = {};
  if (bundleVersion) configOverride.version = bundleVersion;
  if (shouldCreateUpdaterArtifacts) {
    configOverride.bundle = { createUpdaterArtifacts: true };
  }
  writeFileSync(bundleConfigPath, `${JSON.stringify(configOverride)}\n`, "utf8");
  args.push("--config", bundleConfigPath);
}
const tauriCommand = process.platform === "win32" ? "tauri.cmd" : "tauri";
let result;
try {
  result = spawnSync(tauriCommand, args, {
    stdio: "inherit",
    shell: process.platform === "win32",
  });
} finally {
  if (bundleConfigPath) rmSync(bundleConfigPath, { force: true });
}
if (result.error) throw result.error;
if (result.status !== 0 || process.platform !== "darwin" || args[0] !== "build" || process.env.ANCIENT_MEDICAL_TTS_ADHOC_SIGN !== "1") {
  process.exit(result.status ?? 1);
}

const profile = args.includes("--debug") ? "debug" : "release";
const app = join(process.cwd(), "src-tauri", "target", profile, "bundle", "macos", "AncientMedicalTTS.app");
if (!existsSync(app)) throw new Error(`找不到待签名的 macOS app: ${app}`);

const runtime = join(app, "Contents", "Resources", "ffmpeg-runtime");
const sidecar = join(app, "Contents", "MacOS", "ffmpeg");
const worker = join(app, "Contents", "Resources", "worker-runtime", "ancient-tts-worker");
const mainBinary = join(app, "Contents", "MacOS", "ancient-medical-tts");
if (existsSync(runtime)) {
  for (const fileName of readdirSync(runtime).filter((name) => name.endsWith(".dylib"))) {
    execFileSync("codesign", ["--force", "--sign", "-", join(runtime, fileName)], { stdio: "ignore" });
  }
}
if (existsSync(sidecar)) execFileSync("codesign", ["--force", "--sign", "-", sidecar], { stdio: "ignore" });
if (existsSync(worker)) execFileSync("codesign", ["--force", "--sign", "-", worker], { stdio: "ignore" });
if (existsSync(mainBinary)) execFileSync("codesign", ["--force", "--sign", "-", mainBinary], { stdio: "ignore" });
execFileSync("codesign", ["--force", "--sign", "-", app], { stdio: "inherit" });
rebuildSignedMacUpdaterArtifact(app);
console.log(`Applied local ad-hoc signature to ${app}`);

function rebuildSignedMacUpdaterArtifact(signedApp) {
  if (!shouldCreateUpdaterArtifacts) return;
  const bundleDirectory = dirname(signedApp);
  const updaterArchive = readdirSync(bundleDirectory)
    .filter((fileName) => fileName.endsWith(".tar.gz"))
    .map((fileName) => join(bundleDirectory, fileName))
    .at(0);
  if (!updaterArchive) return;

  rmSync(updaterArchive, { force: true });
  rmSync(`${updaterArchive}.sig`, { force: true });
  execFileSync("tar", ["-czf", updaterArchive, "-C", bundleDirectory, basename(signedApp)], { stdio: "inherit" });
  const appVersion = bundleVersion ?? JSON.parse(readFileSync(join(process.cwd(), "package.json"), "utf8")).version;
  execFileSync(tauriCommand, ["signer", "sign", "--app-version", appVersion, "--password", process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "", updaterArchive], { stdio: "inherit" });
  console.log(`Rebuilt signed macOS updater artifact: ${updaterArchive}`);
}

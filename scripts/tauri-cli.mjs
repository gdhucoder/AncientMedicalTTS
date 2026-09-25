import { existsSync, readdirSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import { join } from "node:path";

const args = process.argv.slice(2);
const result = spawnSync("tauri", args, { stdio: "inherit" });
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
console.log(`Applied local ad-hoc signature to ${app}`);

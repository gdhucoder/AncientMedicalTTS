import { chmodSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { arch, platform } from "node:process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(root, "..");
const workerRoot = join(projectRoot, "worker");
const outputDir = join(projectRoot, "src-tauri", "worker-runtime");
const workDir = join(projectRoot, ".worker-build");
const extension = platform === "win32" ? ".exe" : "";
const output = join(outputDir, `ancient-tts-worker${extension}`);

if (platform !== "darwin" && platform !== "win32") {
  throw new Error(`当前平台暂不支持 Worker 打包: ${platform}-${arch}`);
}

const pythonVersion = execFileSync(
  "uv",
  ["run", "--project", workerRoot, "python", "--version"],
  { encoding: "utf8" },
).trim();
if (!/^Python 3\.12\./.test(pythonVersion)) {
  throw new Error(`Worker 必须使用 Python 3.12.x，当前为: ${pythonVersion}`);
}

execFileSync(
  "uv",
  ["run", "--project", workerRoot, "python", "-m", "PyInstaller", "--version"],
  { stdio: "inherit" },
);

rmSync(outputDir, { recursive: true, force: true });
rmSync(workDir, { recursive: true, force: true });
mkdirSync(outputDir, { recursive: true });
mkdirSync(workDir, { recursive: true });

execFileSync(
  "uv",
  [
    "run",
    "--project",
    workerRoot,
    "python",
    "-m",
    "PyInstaller",
    "--clean",
    "--noconfirm",
    "--distpath",
    outputDir,
    "--workpath",
    workDir,
    join(workerRoot, "ancient-tts-worker.spec"),
  ],
  { stdio: "inherit" },
);

if (!existsSync(output)) {
  throw new Error(`PyInstaller 未生成 Worker: ${output}`);
}
if (platform !== "win32") chmodSync(output, 0o755);

const pingResponse = execFileSync(output, [], {
  input: '{"id":"build-check","method":"system.ping","params":{}}\n',
  encoding: "utf8",
  maxBuffer: 1024 * 1024,
});
const response = pingResponse
  .split(/\r?\n/)
  .map((line) => line.trim())
  .find((line) => line.length > 0);
if (!response) throw new Error("bundled Worker 没有返回 system.ping 响应");
const parsed = JSON.parse(response);
if (parsed.id !== "build-check" || parsed.ok !== true || typeof parsed.result?.version !== "string") {
  throw new Error(`bundled Worker system.ping 响应无效: ${response}`);
}

rmSync(workDir, { recursive: true, force: true });
console.log(`Prepared Python ${pythonVersion} Worker: ${output}`);

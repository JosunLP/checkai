import { fileURLToPath } from 'node:url';

const wasmFileUrl = new URL('../pkg/checkai_bg.wasm', import.meta.url);
const wasmUrl = wasmFileUrl.href;
const wasmPath = fileURLToPath(wasmFileUrl);

export function getWasmFileUrl() {
  return new URL(wasmFileUrl);
}

export function getWasmPath() {
  return wasmPath;
}

export { wasmFileUrl, wasmPath, wasmUrl };

export default wasmPath;

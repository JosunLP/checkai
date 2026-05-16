#!/usr/bin/env node

import { Buffer } from 'node:buffer';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { deflateSync } from 'node:zlib';

const SIZE = 1024;
const GREEN = [16, 185, 129, 255];
const WHITE = [255, 255, 255, 255];

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_DIR = path.resolve(SCRIPT_DIR, '..');
const OUTPUT_FILE = path.join(PROJECT_DIR, 'build', 'icon.png');

function setPixel(pixels, x, y, color) {
  const offset = (y * SIZE + x) * 4;
  pixels[offset] = color[0];
  pixels[offset + 1] = color[1];
  pixels[offset + 2] = color[2];
  pixels[offset + 3] = color[3];
}

function isInsideRoundedRect(x, y, left, top, right, bottom, radius) {
  const innerLeft = left + radius;
  const innerRight = right - radius;
  const innerTop = top + radius;
  const innerBottom = bottom - radius;
  const nearestX = Math.max(innerLeft, Math.min(x, innerRight));
  const nearestY = Math.max(innerTop, Math.min(y, innerBottom));
  const dx = x - nearestX;
  const dy = y - nearestY;

  return dx * dx + dy * dy <= radius * radius;
}

function fillRoundedRect(pixels, left, top, right, bottom, radius, color) {
  const minX = Math.max(0, Math.floor(left));
  const maxX = Math.min(SIZE, Math.ceil(right));
  const minY = Math.max(0, Math.floor(top));
  const maxY = Math.min(SIZE, Math.ceil(bottom));

  for (let y = minY; y < maxY; y += 1) {
    for (let x = minX; x < maxX; x += 1) {
      if (
        isInsideRoundedRect(x + 0.5, y + 0.5, left, top, right, bottom, radius)
      ) {
        setPixel(pixels, x, y, color);
      }
    }
  }
}

function fillCircle(pixels, centerX, centerY, radius, color) {
  const minX = Math.max(0, Math.floor(centerX - radius));
  const maxX = Math.min(SIZE, Math.ceil(centerX + radius));
  const minY = Math.max(0, Math.floor(centerY - radius));
  const maxY = Math.min(SIZE, Math.ceil(centerY + radius));
  const radiusSquared = radius * radius;

  for (let y = minY; y < maxY; y += 1) {
    for (let x = minX; x < maxX; x += 1) {
      const dx = x + 0.5 - centerX;
      const dy = y + 0.5 - centerY;

      if (dx * dx + dy * dy <= radiusSquared) {
        setPixel(pixels, x, y, color);
      }
    }
  }
}

function isInsidePolygon(x, y, points) {
  let inside = false;

  for (let i = 0, j = points.length - 1; i < points.length; j = i, i += 1) {
    const [xi, yi] = points[i];
    const [xj, yj] = points[j];
    const intersects =
      yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi;

    if (intersects) {
      inside = !inside;
    }
  }

  return inside;
}

function fillPolygon(pixels, points, color) {
  const xs = points.map(([x]) => x);
  const ys = points.map(([, y]) => y);
  const minX = Math.max(0, Math.floor(Math.min(...xs)));
  const maxX = Math.min(SIZE, Math.ceil(Math.max(...xs)));
  const minY = Math.max(0, Math.floor(Math.min(...ys)));
  const maxY = Math.min(SIZE, Math.ceil(Math.max(...ys)));

  for (let y = minY; y < maxY; y += 1) {
    for (let x = minX; x < maxX; x += 1) {
      if (isInsidePolygon(x + 0.5, y + 0.5, points)) {
        setPixel(pixels, x, y, color);
      }
    }
  }
}

function createCrc32Table() {
  const table = new Uint32Array(256);

  for (let n = 0; n < table.length; n += 1) {
    let c = n;

    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }

    table[n] = c >>> 0;
  }

  return table;
}

const CRC32_TABLE = createCrc32Table();

function crc32(buffer) {
  let crc = 0xffffffff;

  for (const byte of buffer) {
    crc = CRC32_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }

  return (crc ^ 0xffffffff) >>> 0;
}

function createChunk(type, data = Buffer.alloc(0)) {
  const typeBuffer = Buffer.from(type, 'ascii');
  const lengthBuffer = Buffer.alloc(4);
  const crcBuffer = Buffer.alloc(4);

  lengthBuffer.writeUInt32BE(data.length, 0);
  crcBuffer.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);

  return Buffer.concat([lengthBuffer, typeBuffer, data, crcBuffer]);
}

function createPng(pixels) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(SIZE, 0);
  ihdr.writeUInt32BE(SIZE, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;

  const rowLength = SIZE * 4 + 1;
  const raw = Buffer.alloc(rowLength * SIZE);

  for (let y = 0; y < SIZE; y += 1) {
    const sourceStart = y * SIZE * 4;
    const targetStart = y * rowLength + 1;
    pixels.copy(raw, targetStart, sourceStart, sourceStart + SIZE * 4);
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    createChunk('IHDR', ihdr),
    createChunk('IDAT', deflateSync(raw, { level: 9 })),
    createChunk('IEND'),
  ]);
}

async function writeIfChanged(filePath, content) {
  try {
    const existing = await readFile(filePath);

    if (existing.equals(content)) {
      return false;
    }
  } catch (error) {
    if (error.code !== 'ENOENT') {
      throw error;
    }
  }

  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, content);

  return true;
}

export async function generateIcons() {
  const pixels = Buffer.alloc(SIZE * SIZE * 4);

  fillRoundedRect(pixels, 0, 0, SIZE, SIZE, 128, GREEN);

  fillRoundedRect(pixels, 477, 150, 547, 338, 18, WHITE);
  fillRoundedRect(pixels, 396, 212, 628, 282, 18, WHITE);
  fillCircle(pixels, 512, 386, 76, WHITE);
  fillPolygon(
    pixels,
    [
      [512, 370],
      [362, 520],
      [418, 735],
      [606, 735],
      [662, 520],
    ],
    WHITE
  );
  fillRoundedRect(pixels, 340, 695, 684, 788, 42, WHITE);
  fillRoundedRect(pixels, 270, 810, 754, 900, 42, WHITE);
  fillRoundedRect(pixels, 225, 885, 799, 940, 25, WHITE);

  const icon = createPng(pixels);
  const changed = await writeIfChanged(OUTPUT_FILE, icon);
  const relativeOutputFile =
    path.relative(process.cwd(), OUTPUT_FILE) || OUTPUT_FILE;

  console.log(
    `${changed ? 'Generated' : 'Icon already up to date'}: ${relativeOutputFile}`
  );
}

export default generateIcons;

const isCli =
  process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;

if (isCli) {
  generateIcons().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}

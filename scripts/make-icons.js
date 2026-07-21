'use strict';
/*
 * Genera los iconos de la app (PWA de Android y pantalla de inicio del iPhone).
 *
 * Sin dependencias, como el resto del proyecto: la € se dibuja con campos de
 * distancia (arco + dos barras) y el PNG se escribe a mano sobre node:zlib.
 *
 *   node scripts/make-icons.js
 *
 * Los iconos van a sangre, sin esquinas redondeadas ni transparencia: iOS y
 * Android aplican su propia máscara, y una esquina ya redondeada por nosotros
 * saldría recortada dos veces. Por eso el glifo se queda dentro del 80% central
 * (la "zona segura" de los iconos maskable).
 */

const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');

// ---------------------------------------------------------------- PNG mínimo

const CRC_TABLE = new Int32Array(256);
for (let n = 0; n < 256; n++) {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  CRC_TABLE[n] = c;
}

function crc32(buf) {
  let c = ~0;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (~c) >>> 0;
}

function chunk(type, data) {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'ascii');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
  return Buffer.concat([head, data, crc]);
}

// rgb = Buffer con width*height*3 bytes. Truecolor de 8 bits, sin canal alfa.
function encodePng(width, height, rgb) {
  const raw = Buffer.alloc(height * (1 + width * 3));
  for (let y = 0; y < height; y++) {
    raw[y * (1 + width * 3)] = 0; // filtro "None"
    rgb.copy(raw, y * (1 + width * 3) + 1, y * width * 3, (y + 1) * width * 3);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bits por canal
  ihdr[9] = 2; // color type: truecolor
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

// ------------------------------------------------------------------ dibujo

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const mix = (a, b, t) => a + (b - a) * t;
const smooth = (t) => t * t * (3 - 2 * t);

/*
 * Arco de radio `ra` y grosor `2*rb`, simétrico respecto al eje +Y, abierto en
 * -Y con una apertura de 2*(180-θa). sc = [sin θa, cos θa].
 * (campo de distancia de Inigo Quilez)
 */
function sdArc(px, py, scx, scy, ra, rb) {
  const x = Math.abs(px);
  const d = scy * x > scx * py
    ? Math.hypot(x - scx * ra, py - scy * ra) // fuera de la apertura: al extremo
    : Math.abs(Math.hypot(x, py) - ra);
  return d - rb;
}

// Rectángulo de esquinas redondeadas centrado en el origen.
function sdRoundBox(px, py, hx, hy, r) {
  const qx = Math.abs(px) - hx + r;
  const qy = Math.abs(py) - hy + r;
  return Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - r;
}

// Proporciones del glifo, en unidades del radio medio del arco.
const OPEN = (38 * Math.PI) / 180;       // media apertura de la "C", hacia la derecha
const THETA = Math.PI - OPEN;
const SC_X = Math.sin(THETA);
const SC_Y = Math.cos(THETA);
const STROKE = 0.155;                     // medio grosor del trazo del arco
const BAR_Y = 0.26;                       // separación de las dos barras
const BAR_H = 0.113;                      // medio grosor de las barras
const BAR_X0 = -1.40;                     // asoman por la izquierda del arco
const BAR_X1 = 0.45;
const SHIFT = 0.12;                       // recentra la caja (las barras tiran a la izquierda)

// Distancia con signo a la € completa, en unidades de radio.
function sdEuro(x, y) {
  const px = x - SHIFT;
  // El arco se define abierto en -Y, así que giramos: nuestro +X → -Y.
  const arc = sdArc(y, -px, SC_X, SC_Y, 1, STROKE);
  const bx = (BAR_X0 + BAR_X1) / 2;
  const bhx = (BAR_X1 - BAR_X0) / 2;
  const bar = Math.min(
    sdRoundBox(px - bx, y - BAR_Y, bhx, BAR_H, BAR_H * 0.5),
    sdRoundBox(px - bx, y + BAR_Y, bhx, BAR_H, BAR_H * 0.5),
  );
  return Math.min(arc, bar);
}

// Fondo: degradado en diagonal + un brillo suave arriba a la izquierda.
const TOP = [0x55, 0xa2, 0xff];
const BOTTOM = [0x14, 0x51, 0xb4];

function render(size) {
  const rgb = Buffer.alloc(size * size * 3);
  const unit = size * 0.2165;              // radio del arco, en píxeles
  const cx = size / 2;
  const cy = size / 2;
  const aa = 1;                            // suavizado: ~1 píxel de transición
  const shadowDy = size * 0.020;
  const shadowBlur = size * 0.055;

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const sx = x + 0.5;
      const sy = y + 0.5;

      // --- fondo
      const t = clamp01((sx / size + sy / size) / 2);
      let r = mix(TOP[0], BOTTOM[0], smooth(t));
      let g = mix(TOP[1], BOTTOM[1], smooth(t));
      let b = mix(TOP[2], BOTTOM[2], smooth(t));

      const gloss = 1 - clamp01(Math.hypot(sx - size * 0.26, sy - size * 0.16) / (size * 0.8));
      const lift = smooth(gloss) * 26;
      r += lift; g += lift; b += lift;

      // --- glifo (coordenadas en unidades de radio, centradas)
      const gx = (sx - cx) / unit;
      const gy = (sy - cy) / unit;
      const d = sdEuro(gx, gy) * unit;     // de vuelta a píxeles

      // sombra proyectada: el mismo campo, desplazado y muy difuminado
      const ds = sdEuro(gx, (sy - shadowDy - cy) / unit) * unit;
      const shadow = clamp01(0.5 - ds / shadowBlur) * 0.28;
      r = mix(r, 0x0a, shadow);
      g = mix(g, 0x1c, shadow);
      b = mix(b, 0x3a, shadow);

      const ink = clamp01(0.5 - d / aa);
      r = mix(r, 255, ink);
      g = mix(g, 255, ink);
      b = mix(b, 255, ink);

      const i = (y * size + x) * 3;
      rgb[i] = Math.round(clamp01(r / 255) * 255);
      rgb[i + 1] = Math.round(clamp01(g / 255) * 255);
      rgb[i + 2] = Math.round(clamp01(b / 255) * 255);
    }
  }
  return encodePng(size, size, rgb);
}

const outDir = path.join(__dirname, '..', 'public', 'icons');
for (const size of [180, 192, 512]) {
  const file = path.join(outDir, `icon-${size}.png`);
  fs.writeFileSync(file, render(size));
  console.log(`icon-${size}.png  (${fs.statSync(file).size} bytes)`);
}

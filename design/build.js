'use strict';
// Genera design/dist/*.html autocontenidos a partir de design/src/*.html
// (fragmentos <template>) + tokens.css. Cada dist muestra el componente en
// tema claro y oscuro. La primera línea del fragmento es el marcador @dsCard,
// que da nombre y descripción a la tarjeta.

const fs = require('fs');
const path = require('path');

const ROOT = __dirname;
const SRC = path.join(ROOT, 'src');
const DIST = path.join(ROOT, 'dist');
const tokens = fs.readFileSync(path.join(ROOT, 'tokens.css'), 'utf8');

fs.mkdirSync(DIST, { recursive: true });

for (const file of fs.readdirSync(SRC).filter((f) => f.endsWith('.html'))) {
  const raw = fs.readFileSync(path.join(SRC, file), 'utf8');
  const nl = raw.indexOf('\n');
  const marker = raw.slice(0, nl).trim();
  if (!marker.startsWith('<!-- @dsCard')) {
    throw new Error(`${file}: la primera línea debe ser el marcador @dsCard`);
  }
  const body = raw.slice(nl + 1);
  const out = `${marker}
<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
${tokens}
</style>
</head>
<body>
<template id="demo">
${body}
</template>
<script>
for (const theme of ['mg-light', 'mg-dark']) {
  const wrap = document.createElement('div');
  wrap.className = 'mg ' + theme;
  wrap.appendChild(document.getElementById('demo').content.cloneNode(true));
  document.body.appendChild(wrap);
}
</script>
</body>
</html>
`;
  fs.writeFileSync(path.join(DIST, file), out);
  console.log('build', file);
}

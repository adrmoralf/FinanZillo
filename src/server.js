'use strict';

const path = require('path');
const os = require('os');
const express = require('express');
const config = require('./config');
const db = require('./db');
const apiRouter = require('./routes/api');
const { applyRecurring } = require('./recurring');

applyRecurring();

const app = express();
app.disable('x-powered-by');
app.use(express.json({ limit: '5mb' }));
// La cola offline del atajo manda su archivo como texto plano (un JSON por línea).
app.use(express.text({ limit: '2mb' }));

app.use((req, res, next) => {
  res.setHeader(
    'Content-Security-Policy',
    "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'"
  );
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('Referrer-Policy', 'no-referrer');
  next();
});

// Política de origen cruzado: denegar todo. La API es solo para el propio
// origen (la web) y para clientes sin navegador (atajo, curl), que no mandan
// cabecera Origin. Si una página de OTRO origen intenta una petición de
// escritura con la cookie de sesión, se corta aquí (defensa CSRF adicional;
// no se emite ninguna cabecera Access-Control-Allow-*, así que el navegador
// tampoco dejaría leer las respuestas).
app.use((req, res, next) => {
  const origin = req.headers.origin;
  if (origin && !['GET', 'HEAD', 'OPTIONS'].includes(req.method)) {
    let originHost = null;
    try {
      originHost = new URL(origin).host;
    } catch (_) {
      /* Origin ilegible → se trata como cruzado */
    }
    if (originHost !== req.headers.host) {
      return res.status(403).json({ error: 'Origen no permitido' });
    }
  }
  next();
});

// La app Atajos de iOS, cuando se arrastra una variable de tipo Diccionario a
// una fila del cuerpo JSON, a veces la serializa como cadena de texto bajo la
// clave literal de esa fila en vez de fusionarla como objeto anidado — p. ej.
// {"Clave":"{\"tipo\":\"gasto\",...}"} en lugar de {"tipo":"gasto",...}.
// Si el cuerpo tiene una única clave cuyo valor es una cadena JSON de objeto,
// se desenvuelve automáticamente para que el atajo funcione sin tocarlo.
app.use((req, res, next) => {
  if (req.body && typeof req.body === 'object' && !Array.isArray(req.body)) {
    const keys = Object.keys(req.body);
    if (keys.length === 1 && typeof req.body[keys[0]] === 'string') {
      try {
        const parsed = JSON.parse(req.body[keys[0]]);
        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
          req.body = parsed;
        }
      } catch (_) {
        // No era JSON anidado: se deja el cuerpo tal cual.
      }
    }
  }
  next();
});

app.use('/api', apiRouter);
app.use(express.static(path.join(config.ROOT, 'public')));

app.use((err, req, res, next) => {
  if (err.type === 'entity.parse.failed') {
    return res.status(400).json({ error: 'JSON no válido' });
  }
  console.error(err);
  res.status(500).json({ error: 'Error interno' });
});

// Backup diario también si el proceso vive varios días (revisa cada 6 h).
setInterval(() => db.runDailyBackup(), 6 * 60 * 60 * 1000).unref();

app.listen(config.PORT, '0.0.0.0', () => {
  console.log(`FinanZillo escuchando en:`);
  console.log(`  Local:   http://localhost:${config.PORT}`);
  for (const iface of Object.values(os.networkInterfaces())) {
    for (const addr of iface || []) {
      if (addr.family === 'IPv4' && !addr.internal) {
        console.log(`  Red:     http://${addr.address}:${config.PORT}`);
      }
    }
  }
});

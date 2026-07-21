/* Service worker de FinanZillo — permite abrir la app aunque el servidor no esté.
   Estrategias:
   - Navegación (abrir la app): caché primero, para que arranque al instante y
     sin red; se refresca por detrás. NUNCA puede quedarse sin respuesta.
   - Resto del shell (CSS/JS/iconos): igual, caché primero con refresco.
   - GET de /api: red primero con copia en caché, así offline se ve lo último
     conocido en vez de una pantalla vacía.
   - Escrituras (POST/PUT/DELETE): no se tocan; la cola de app.js las guarda y
     las manda cuando vuelve la conexión.
   Al cambiar assets, subir VERSION para invalidar las cachés viejas. */
'use strict';

const VERSION = 'fz-v7';
const SHELL_CACHE = VERSION + '-shell';
const DATA_CACHE = VERSION + '-data';

const SHELL_ASSETS = [
  '/',
  '/index.html',
  '/css/style.css',
  '/js/app.js',
  '/vendor/chart.umd.min.js',
  '/manifest.webmanifest',
  '/icons/icon-180.png',
  '/icons/icon-192.png',
  '/icons/icon-512.png',
];

// Último recurso: si no hay NADA en caché, una página honesta en vez de un
// negro sin explicación (que es lo que se veía al abrir la app sin servidor).
const PAGINA_SIN_COPIA = `<!doctype html><html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>FinanZillo — sin conexión</title>
<style>body{margin:0;min-height:100dvh;display:grid;place-items:center;text-align:center;
padding:2rem;font-family:system-ui,-apple-system,sans-serif;background:#0e0e13;color:#e9e8f0}
h1{font-size:1.25rem;margin:.5rem 0}p{color:#a9a7b8;line-height:1.5;max-width:22rem}
.e{font-size:2.5rem}</style></head><body><div><div class="e">💶</div>
<h1>FinanZillo sin conexión</h1>
<p>Todavía no hay una copia guardada en este dispositivo. Abre la app una vez con
el servidor encendido y a partir de entonces funcionará sin conexión.</p></div></body></html>`;

const respuestaSinCopia = () =>
  new Response(PAGINA_SIN_COPIA, { status: 200, headers: { 'Content-Type': 'text/html; charset=utf-8' } });

self.addEventListener('install', (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(SHELL_CACHE);
      // Uno a uno, NO con addAll(): addAll es atómico y un solo fallo dejaba la
      // caché entera vacía (y la app en negro al abrirla sin servidor).
      await Promise.allSettled(
        SHELL_ASSETS.map(async (u) => {
          try {
            const res = await fetch(new Request(u, { cache: 'reload' }));
            if (res && res.ok) await cache.put(u, res);
          } catch (_) {
            /* ese asset se cacheará al primer uso */
          }
        })
      );
      await self.skipWaiting();
    })()
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(keys.filter((k) => !k.startsWith(VERSION)).map((k) => caches.delete(k)));
      await self.clients.claim();
    })()
  );
});

// Guarda una copia sin romper la respuesta que se devuelve.
function guardar(cacheName, req, res) {
  const copia = res.clone();
  caches.open(cacheName).then((c) => c.put(req, copia)).catch(() => {});
}

/* Abrir la app. Caché primero: arranque instantáneo y, sobre todo, garantizado.
   Siempre devuelve una Response — si respondWith() recibe undefined, el
   navegador lo trata como error de red y sale la pantalla en negro. */
async function abrirApp(req) {
  const cache = await caches.open(SHELL_CACHE);
  const guardada = (await cache.match('/index.html')) || (await cache.match('/'));

  if (guardada) {
    // Refresca por detrás para la próxima vez, sin bloquear el arranque.
    fetch(req)
      .then((res) => { if (res && res.ok) cache.put('/index.html', res.clone()); })
      .catch(() => {});
    return guardada;
  }
  try {
    const res = await fetch(req);
    if (res && res.ok) guardar(SHELL_CACHE, '/index.html', res);
    return res;
  } catch (_) {
    return respuestaSinCopia();
  }
}

async function delShell(req) {
  const cache = await caches.open(SHELL_CACHE);
  const hit = await cache.match(req, { ignoreSearch: true });
  if (hit) {
    fetch(req).then((res) => { if (res && res.ok) cache.put(req, res.clone()); }).catch(() => {});
    return hit;
  }
  try {
    const res = await fetch(req);
    if (res && res.ok) guardar(SHELL_CACHE, req, res);
    return res;
  } catch (_) {
    return new Response('', { status: 504, statusText: 'Sin conexión' });
  }
}

async function delApi(req) {
  try {
    const res = await fetch(req);
    if (res && res.ok) guardar(DATA_CACHE, req, res);
    return res;
  } catch (_) {
    const hit = await caches.match(req);
    if (hit) return hit;
    return new Response(JSON.stringify({ error: 'Sin conexión' }), {
      status: 503,
      headers: { 'Content-Type': 'application/json' },
    });
  }
}

self.addEventListener('fetch', (event) => {
  const req = event.request;
  let url;
  try {
    url = new URL(req.url);
  } catch (_) {
    return;
  }

  if (url.origin !== self.location.origin) return;
  // Las escrituras van siempre a la red, tal cual: si fallan, app.js las encola.
  if (req.method !== 'GET') return;

  // Abrir la app (o recargarla): lo más importante que hay que garantizar.
  if (req.mode === 'navigate') {
    event.respondWith(abrirApp(req));
    return;
  }

  if (url.pathname.startsWith('/api/')) {
    // Sesión y latido no se cachean NUNCA: tienen que fallar de verdad cuando
    // no hay servidor, o la app creería que sigue vivo.
    if (['/api/me', '/api/login', '/api/ping'].includes(url.pathname)) return;
    event.respondWith(delApi(req));
    return;
  }

  event.respondWith(delShell(req));
});

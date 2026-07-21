'use strict';

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const ROOT = path.join(__dirname, '..');
const ENV_FILE = path.join(ROOT, '.env');

// Si no existe .env, se genera uno con credenciales aleatorias en el primer arranque.
if (!fs.existsSync(ENV_FILE)) {
  const password = crypto.randomBytes(4).toString('hex');
  const token = crypto.randomBytes(24).toString('hex');
  fs.writeFileSync(
    ENV_FILE,
    [
      '# Configuración de FinanZillo. Cambia la contraseña por una tuya.',
      `PASSWORD=${password}`,
      `API_TOKEN=${token}`,
      'PORT=3000',
      '',
    ].join('\n'),
    'utf8'
  );
  console.log(`[FinanZillo] Se ha creado .env con credenciales nuevas. Contraseña inicial: ${password}`);
}

process.loadEnvFile(ENV_FILE);

const DATA_DIR = process.env.DATA_DIR || path.join(ROOT, 'data');
fs.mkdirSync(DATA_DIR, { recursive: true });

// Secreto para firmar la cookie de sesión: se genera una vez y se persiste
// para que los inicios de sesión sobrevivan a reinicios del servidor.
const SECRET_FILE = path.join(DATA_DIR, '.session-secret');
if (!fs.existsSync(SECRET_FILE)) {
  fs.writeFileSync(SECRET_FILE, crypto.randomBytes(32).toString('hex'), 'utf8');
}

module.exports = {
  ROOT,
  DATA_DIR,
  // DB_FILE permite apuntar a otra base dentro de DATA_DIR (p. ej. `demo.db`
  // para los datos de demostración) sin tocar la real.
  DB_FILE: path.join(DATA_DIR, process.env.DB_FILE || 'finanzillo.db'),
  PORT: parseInt(process.env.PORT, 10) || 3000,
  PASSWORD: process.env.PASSWORD || '',
  API_TOKEN: process.env.API_TOKEN || '',
  SESSION_SECRET: fs.readFileSync(SECRET_FILE, 'utf8').trim(),
  SESSION_DAYS: 90,
  // Carpeta extra para copias de seguridad (p. ej. dentro de OneDrive); vacía = desactivado.
  EXTERNAL_BACKUP_DIR: (process.env.EXTERNAL_BACKUP_DIR || '').trim(),
};
